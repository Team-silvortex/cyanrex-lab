//! Explicit private-input loopback host; no teaching state, installer or inferred proxy policy.
mod handshake;
mod http;
mod notify;
mod request;
#[cfg(test)]
#[path = "tls/tests/socket_ownership.rs"]
mod socket_ownership;
mod tls;

use super::config::{self, StorageExpectation};
use cyanrex_engine::{
    platform_host::{
        inspect_platform_artifact_root, open_platform_storage_expected, PlatformProcessStartup,
    },
    platform_http::{build_platform_workspace_router, PlatformWorkspaceHttpState},
};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::Path,
    time::Duration,
};

pub(super) fn parse_bind(value: &str) -> Result<SocketAddr, ()> {
    let bind: SocketAddr = value.parse().map_err(|_| ())?;
    if bind.port() < 1024
        || !matches!(bind.ip(), IpAddr::V4(ip) if ip == Ipv4Addr::LOCALHOST)
            && !matches!(bind.ip(), IpAddr::V6(ip) if ip == Ipv6Addr::LOCALHOST)
        || matches!(bind, SocketAddr::V6(address) if address.scope_id() != 0)
        || bind.to_string() != value
    {
        return Err(());
    }
    Ok(bind)
}

pub(super) fn run(
    path: &Path,
    bind: SocketAddr,
    process: &PlatformProcessStartup,
    notify_systemd: bool,
) -> Result<(), ()> {
    run_with_transport(path, bind, process, None, notify_systemd)
}

pub(super) fn run_tls(
    path: &Path,
    bind: SocketAddr,
    cert: &Path,
    key: &Path,
    process: &PlatformProcessStartup,
    notify_systemd: bool,
) -> Result<(), ()> {
    run_with_transport(path, bind, process, Some((cert, key)), notify_systemd)
}

fn run_with_transport(
    path: &Path,
    bind: SocketAddr,
    process: &PlatformProcessStartup,
    tls_paths: Option<(&Path, &Path)>,
    notify_systemd: bool,
) -> Result<(), ()> {
    install_panic_hook();
    // Explicit opt-in is checked after the process gate, before any private input is read.
    process.verify_before_runtime().map_err(|_| ())?;
    let prepared = notify::Notifier::from_environment(notify_systemd);
    let checked = process.verify_before_runtime().map_err(|_| ());
    let notifier = prepared?;
    checked?;
    let loaded = config::load(path);
    // Recheck even on failed input access, before propagating either failure.
    let checked = process.verify_before_runtime().map_err(|_| ());
    let input = loaded?;
    checked?;
    let StorageExpectation::Markers(expected) = input.storage_expectation else {
        return Err(());
    };
    // This deliberately narrow mode neither resolves localhost nor infers TLS/proxy policy.
    let scheme = if tls_paths.is_some() { "https" } else { "http" };
    if input.config.origin() != format!("{scheme}://{bind}") {
        return Err(());
    }
    let tls_config = if let Some((cert, key)) = tls_paths {
        let identity = tls::load_tls_identity(cert, key, bind.ip());
        let checked = process.verify_before_runtime().map_err(|_| ());
        let identity = identity?;
        checked?;
        Some(identity)
    } else {
        None
    };
    // Existing-root metadata is synchronous, before runtime/dial; no installer or repair.
    let original =
        inspect_platform_artifact_root(&input.config, &input.artifact_root).map_err(|_| ())?;
    process.verify_before_runtime().map_err(|_| ())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|_| ())?;
    let result = runtime.block_on(async {
        let opened = open_platform_storage_expected(
            input.config,
            input.endpoint,
            &input.password,
            expected,
            &input.artifact_root,
            original,
        )
        .await;
        // Compute the current-thread check even if opening/observing storage failed.
        let checked = process.verify_current_thread().map_err(|_| ());
        let storage = opened.map_err(|_| ())?;
        checked?;
        let state = PlatformWorkspaceHttpState::new(storage).map_err(|_| ())?;
        let router = build_platform_workspace_router(state);
        // Register both signals before the final bind. No background signal task or runtime.
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate = signal(SignalKind::terminate()).map_err(|_| ())?;
        let mut interrupt = signal(SignalKind::interrupt()).map_err(|_| ())?;
        let listener = tokio::net::TcpListener::bind(bind).await.map_err(|_| ())?;
        process.verify_current_thread().map_err(|_| ())?;
        if let Some(notifier) = &notifier {
            notifier.ready()?;
        }
        println!("platform loopback host ready");
        let stop = async move {
            tokio::select! {
                _ = terminate.recv() => {},
                _ = interrupt.recv() => {},
            }
        };
        http::serve_notified(
            listener,
            router,
            bind.to_string(),
            tls_config,
            notifier.as_ref(),
            stop,
        )
        .await
    });
    // Bounds waiting for runtime workers, not synchronous filesystem work or remote effects.
    runtime.shutdown_timeout(Duration::from_secs(1));
    result
}

fn install_panic_hook() {
    // This command owns the process. Do not expose panic payloads, file paths or backtraces;
    // keep the global hook unchanged for every legacy diagnostic command and library caller.
    std::panic::set_hook(Box::new(|_| eprintln!("platform loopback host failed")));
}
