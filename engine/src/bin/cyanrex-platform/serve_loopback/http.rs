//! Bounded HTTP/1 admission, not a timeout around admitted Session mutations.
use axum::{middleware, Router};
use hyper::server::conn::http1;
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{watch, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
};

const CONNECTIONS: usize = 16;
const HEADER_TIMEOUT: Duration = Duration::from_secs(2);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) async fn serve_notified(
    listener: TcpListener,
    router: Router,
    authority: String,
    tls: Option<Arc<rustls::ServerConfig>>,
    notifier: Option<&super::notify::Notifier>,
    stop: impl Future<Output = ()>,
) -> Result<(), ()> {
    serve_transport(listener, router, authority, tls, notifier, stop).await
}

#[cfg(test)]
pub(super) async fn serve_tls(
    listener: TcpListener,
    router: Router,
    authority: String,
    config: Arc<rustls::ServerConfig>,
    stop: impl Future<Output = ()>,
) -> Result<(), ()> {
    serve_notified(listener, router, authority, Some(config), None, stop).await
}

#[cfg(test)]
pub(super) async fn serve(
    listener: TcpListener,
    router: Router,
    authority: String,
    stop: impl Future<Output = ()>,
) -> Result<(), ()> {
    serve_notified(listener, router, authority, None, None, stop).await
}

async fn serve_transport(
    listener: TcpListener,
    router: Router,
    authority: String,
    tls: Option<Arc<rustls::ServerConfig>>,
    notifier: Option<&super::notify::Notifier>,
    stop: impl Future<Output = ()>,
) -> Result<(), ()> {
    let router = router.layer(middleware::from_fn_with_state(
        authority,
        super::request::gate,
    ));
    let admission = Arc::new(Semaphore::new(CONNECTIONS));
    let (shutdown, _) = watch::channel(false);
    let mut connections = JoinSet::new();
    let mut failed = false;
    tokio::pin!(stop);
    loop {
        // Completed tasks must not accumulate in the JoinSet under a continuous accept flood.
        while let Some(result) = connections.try_join_next() {
            failed |= result.is_err();
        }
        if failed {
            break;
        }
        tokio::select! {
            biased;
            _ = &mut stop => break,
            result = connections.join_next(), if !connections.is_empty() => {
                failed |= result.is_some_and(|result| result.is_err());
            }
            accepted = listener.accept() => {
                let Ok((stream, peer)) = accepted else {
                    failed = true;
                    break;
                };
                // A connection gets no queued task or body work without a fail-fast permit.
                if peer.ip().is_loopback() {
                    if let Ok(permit) = Arc::clone(&admission).try_acquire_owned() {
                        connections.spawn(connection(
                            stream, router.clone(), tls.clone(), shutdown.subscribe(), permit,
                        ));
                    }
                }
                // The current-thread runtime must also poll timers and accepted connections.
                tokio::task::yield_now().await;
            }
        }
    }
    drop(listener);
    if notifier.is_some_and(|notifier| notifier.stopping().is_err()) {
        // Notification failure is a host failure, not permission to skip or cancel the drain.
        failed = true;
    }
    let _ = shutdown.send(true);
    let drained = tokio::time::timeout(DRAIN_TIMEOUT, async {
        while let Some(result) = connections.join_next().await {
            failed |= result.is_err();
        }
    })
    .await;
    if drained.is_err() {
        // Abort only local async futures. A remote commit or blocking job is not rolled back.
        connections.abort_all();
        while connections.join_next().await.is_some() {}
        return Err(());
    }
    if failed {
        Err(())
    } else {
        Ok(())
    }
}

async fn connection(
    stream: TcpStream,
    router: Router,
    tls: Option<Arc<rustls::ServerConfig>>,
    mut shutdown: watch::Receiver<bool>,
    permit: OwnedSemaphorePermit,
) {
    if let Some(config) = tls {
        if let Some(stream) = super::handshake::accept(stream, config, &mut shutdown).await {
            drive_http(stream, router, shutdown, permit).await;
        }
    } else {
        drive_http(stream, router, shutdown, permit).await;
    }
}

async fn drive_http<I>(
    stream: I,
    router: Router,
    mut shutdown: watch::Receiver<bool>,
    _permit: OwnedSemaphorePermit,
) where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .max_headers(32)
        .max_buf_size(16 * 1024)
        .header_read_timeout(HEADER_TIMEOUT)
        .keep_alive(false);
    // No upgrades, HTTP/2, proxy interpretation or whole-connection/handler timeout.
    let connection =
        builder.serve_connection(TokioIo::new(stream), TowerToHyperService::new(router));
    tokio::pin!(connection);
    if *shutdown.borrow() {
        connection.as_mut().graceful_shutdown();
    }
    tokio::select! {
        biased;
        _ = &mut connection => {},
        _ = shutdown.changed() => {
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
    }
    // Malformed peer traffic closes only its connection; never print parser/request contents.
}

#[cfg(test)]
#[path = "http/tests.rs"]
mod tests;
