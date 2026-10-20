//! One TLS-only deadline, before any HTTP handling or business operation can be dispatched.
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpStream, sync::watch, time::Instant};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

pub(super) async fn accept(
    stream: TcpStream,
    config: Arc<rustls::ServerConfig>,
    shutdown: &mut watch::Receiver<bool>,
) -> Option<TlsStream<TcpStream>> {
    if *shutdown.borrow() {
        return None;
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let acceptor = TlsAcceptor::from(config);
    tokio::select! {
        biased;
        _ = shutdown.changed() => None,
        result = tokio::time::timeout_at(deadline, acceptor.accept(stream)) => {
            let stream = result.ok()?.ok()?;
            let session = &stream.get_ref().1;
            // IP Origins have no DNS SNI. ALPN omission means HTTP/1.1, never HTTP/2.
            if session.server_name().is_some()
                || !matches!(session.alpn_protocol(), None | Some(b"http/1.1"))
            {
                return None;
            }
            Some(stream)
        }
    }
}
