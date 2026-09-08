//! Manual loopback transport pressure checks. No AppState, database, auth secrets, or kernel work.
use super::*;
use axum::{
    extract::{Query, WebSocketUpgrade},
    routing::get,
    Router,
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{net::SocketAddr, time::Instant};
use tokio::{
    net::{TcpListener, TcpSocket},
    sync::{broadcast, mpsc},
    task::JoinSet,
};
use tokio_tungstenite::{client_async, connect_async, tungstenite::Message as ClientMessage};

use crate::models::event::{EventCategory, EventColor, EventSeverity};

#[derive(Deserialize)]
struct Owner {
    username: String,
}

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn server(
    sender: broadcast::Sender<Event>,
) -> (Server, SocketAddr, mpsc::UnboundedReceiver<f64>) {
    let (finished, receiver) = mpsc::unbounded_channel();
    // This unauthenticated router exists only in the private cfg(test) harness. Production route
    // authentication and Origin enforcement are covered separately by routes_tdd.
    let app = Router::new().route(
        "/ws",
        get(move |ws: WebSocketUpgrade, Query(owner): Query<Owner>| {
            let events = sender.subscribe();
            let finished = finished.clone();
            async move {
                ws.on_upgrade(move |socket| async move {
                    let start = Instant::now();
                    handle_ws(socket, events, owner.username).await;
                    let _ = finished.send(start.elapsed().as_secs_f64());
                })
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (Server(task), address, receiver)
}

fn event(user: usize, index: usize, payload: &str, done: bool) -> Event {
    Event {
        username: format!("bench-{user}"),
        timestamp: chrono::Utc::now(),
        source: "pressure-test".into(),
        event_type: if done { "bench.end" } else { "bench.event" }.into(),
        category: EventCategory::Kernel,
        severity: EventSeverity::Success,
        color: EventColor::Green,
        payload: json!({"index": index, "data": payload}),
    }
}

async fn fanout(burst: bool) -> Value {
    let capacity = if burst { 8 } else { 1024 };
    let count = if burst { 60000 } else { 2000 };
    let (sender, _) = broadcast::channel(capacity);
    let (_server, address, _finished) = server(sender.clone()).await;
    let mut clients = JoinSet::new();
    for subscriber in 0..32 {
        let username = format!("bench-{}", subscriber % 8);
        let (mut socket, _) = connect_async(format!("ws://{address}/ws?username={username}"))
            .await
            .unwrap();
        clients.spawn(async move {
            let mut received = 0;
            while let Some(message) = socket.next().await {
                match message.unwrap() {
                    ClientMessage::Text(text) => {
                        let event: Event = serde_json::from_str(&text).unwrap();
                        assert_eq!(event.username, username, "cross-owner event leak");
                        if event.event_type == "bench.end" {
                            socket.close(None).await.unwrap();
                            return (received, false);
                        }
                        received += 1;
                    }
                    ClientMessage::Close(Some(frame)) => {
                        assert_eq!(u16::from(frame.code), 1013, "overload must be signalled");
                        return (received, true);
                    }
                    _ => panic!("unexpected transport frame"),
                }
            }
            panic!("connection ended without end marker or resync close");
        });
    }
    assert_eq!(sender.receiver_count(), 32);
    let start = Instant::now();
    let payload = "x".repeat(128);
    let mut pace = tokio::time::interval(Duration::from_millis(2));
    for index in 0..count {
        if !burst && index % 8 == 0 {
            pace.tick().await;
        }
        let _ = sender.send(event(index % 8, index, &payload, false));
    }
    for user in 0..8 {
        let _ = sender.send(event(user, 0, "", true));
    }
    let mut delivered = 0;
    let mut resync_closes = 0;
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(result) = clients.join_next().await {
            let (received, closed) = result.unwrap();
            delivered += received;
            resync_closes += usize::from(closed);
        }
    })
    .await
    .expect("subscribers must finish within the pressure check deadline");
    if burst {
        assert!(resync_closes > 0);
    } else {
        assert_eq!(resync_closes, 0);
        assert_eq!(delivered, count * 4);
    }
    json!({"scenario": if burst { "burst" } else { "paced" }, "subscribers": 32,
        "users": 8, "published": count, "expected_matching_copies": count * 4,
        "delivered_copies": delivered, "resync_closes": resync_closes,
        "broadcast_capacity": capacity, "elapsed_seconds": start.elapsed().as_secs_f64()})
}

async fn stalled_reader() -> Value {
    let (sender, _) = broadcast::channel(128);
    let (_server, address, mut finished) = server(sender.clone()).await;
    let socket = TcpSocket::new_v4().unwrap();
    socket.set_recv_buffer_size(4096).unwrap();
    let stream = socket.connect(address).await.unwrap();
    let (client, _) = client_async(format!("ws://{address}/ws?username=bench-0"), stream)
        .await
        .unwrap();
    // Keep the connection open but never read. The burst is below channel capacity, so a lag
    // close cannot satisfy this test; the real TCP send must time out and release its receiver.
    let payload = "x".repeat(256 * 1024);
    for index in 0..64 {
        sender.send(event(0, index, &payload, false)).unwrap();
    }
    let seconds = tokio::time::timeout(Duration::from_secs(8), finished.recv())
        .await
        .expect("a non-reading peer must not retain its handler indefinitely")
        .unwrap();
    assert!(
        (4.9..8.0).contains(&seconds),
        "unexpected handler lifetime: {seconds}"
    );
    assert_eq!(
        sender.receiver_count(),
        0,
        "the timed-out subscriber must be dropped"
    );
    drop(client);
    json!({"scenario": "stalled", "published": 64, "payload_bytes": 256 * 1024,
        "requested_receive_buffer_bytes": 4096, "broadcast_capacity": 128,
        "handler_seconds": seconds, "remaining_subscribers": sender.receiver_count()})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
#[ignore = "manual loopback pressure check; use scripts/bench-event-stream.mjs"]
async fn measure() {
    let scenario = std::env::var("CYANREX_WS_BENCH_KIND").expect("scenario is required");
    let result = match scenario.as_str() {
        "paced" => fanout(false).await,
        "burst" => fanout(true).await,
        "stalled" => stalled_reader().await,
        _ => panic!("unknown scenario"),
    };
    println!("CYANREX_WS_BENCH={result}");
}
