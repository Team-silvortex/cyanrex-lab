use std::{future::Future, time::Duration};

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use tokio::sync::broadcast::{error::RecvError, Receiver};

use crate::models::event::Event;

const SEND_TIMEOUT: Duration = Duration::from_secs(5);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

#[cfg(test)]
mod pressure_tests;

pub(super) async fn handle_ws(
    mut socket: WebSocket,
    mut receiver: Receiver<Event>,
    username: String,
) {
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {},
            },
            message = next_message(&mut receiver, &username) => {
                let closing = matches!(message, Message::Close(_));
                let deadline = if closing { CLOSE_TIMEOUT } else { SEND_TIMEOUT };
                // Drop the transport on timeout: a cancelled send may have written a partial frame.
                // Trying another frame (even Close) on that socket would not be safe.
                if !send_with_deadline(socket.send(message), deadline).await || closing {
                    break;
                }
            }
        }
    }
}

async fn next_message(receiver: &mut Receiver<Event>, username: &str) -> Message {
    loop {
        match receiver.recv().await {
            Ok(event) if event.username == username => {
                return match serde_json::to_string(&event) {
                    Ok(text) => Message::Text(text.into()),
                    Err(_) => close(1011, "event serialization failed; reload /events"),
                };
            }
            Ok(_) => continue,
            // This channel is global, so its skipped count is not an owner-specific loss count.
            // Never expose other users' traffic counts or silently resume after a possible gap.
            Err(RecvError::Lagged(_)) => {
                return close(1013, "event stream lagged; reload /events");
            }
            Err(RecvError::Closed) => return close(1001, "event stream closed"),
        }
    }
}

fn close(code: u16, reason: &'static str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}

async fn send_with_deadline<E>(
    send: impl Future<Output = Result<(), E>>,
    deadline: Duration,
) -> bool {
    matches!(tokio::time::timeout(deadline, send).await, Ok(Ok(())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stalled_send_is_bounded_and_cancelled() {
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            send_with_deadline(
                std::future::pending::<Result<(), ()>>(),
                Duration::from_millis(10),
            ),
        )
        .await
        .expect("the send deadline must release the subscriber");
        assert!(!result);
        assert!(send_with_deadline(async { Ok::<_, ()>(()) }, SEND_TIMEOUT).await);
        assert!(!send_with_deadline(async { Err::<(), _>(()) }, SEND_TIMEOUT).await);
    }

    #[tokio::test]
    async fn closed_channel_produces_a_normal_shutdown_close() {
        let (sender, mut receiver) = tokio::sync::broadcast::channel::<Event>(1);
        drop(sender);
        let Message::Close(Some(frame)) = next_message(&mut receiver, "alice").await else {
            panic!("expected a close frame");
        };
        assert_eq!(frame.code, 1001);
    }
}
