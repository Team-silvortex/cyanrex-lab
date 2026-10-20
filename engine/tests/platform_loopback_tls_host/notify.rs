//! Explicit native notifications over owned local datagrams, not a systemd installation test.
use super::{
    child::failed_after_ready, fixture::*, http, plain_fixture, plain_http, receiver::Receiver,
    wait_helpers, waits,
};
use std::{
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_systemd_ready_then_login_stopping_and_clean_exit() {
    let (
        ready,
        output,
        issued,
        current,
        cleared,
        signalled,
        stopping,
        closed,
        silent,
        oracle,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let receiver = Receiver::new();
            let prepared = plain_fixture::Prepared::new(f).await;
            let mut child = prepared.spawn_notify(None, receiver.path().as_os_str());
            let ready = receiver
                .wait_from(
                    b"READY=1",
                    child.pid(),
                    prepared.identity.uid[0],
                    prepared.identity.gid[0],
                )
                .await;
            let output = child.ready().await;
            let mut issued = false;
            let mut current = false;
            let mut cleared = false;
            if ready && output {
                let client = plain_http::client();
                if let Some(token) =
                    plain_http::token(&plain_http::login(&client, &prepared.origin).await)
                {
                    issued = stored_digest(f, &token).await;
                    current = plain_http::current(
                        &plain_http::get(&client, &prepared.origin, &token).await,
                    );
                    cleared = plain_http::cleared(
                        &plain_http::logout(&client, &prepared.origin, &token).await,
                    ) && session_count(f).await == 0;
                }
            }
            let signalled = child.signal(libc::SIGTERM);
            let stopping = receiver
                .wait_from(
                    b"STOPPING=1",
                    child.pid(),
                    prepared.identity.uid[0],
                    prepared.identity.gid[0],
                )
                .await;
            let closed = stopped(&child.finish(None, Duration::from_secs(35)).await);
            let silent = receiver.take().is_none();
            let oracle = receiver.control();
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish() && receiver.finish();
            (
                ready, output, issued, current, cleared, signalled, stopping, closed, silent,
                oracle, no_backend, cleaned,
            )
        })
    })
    .await;
    assert!(
        ready
            && output
            && issued
            && current
            && cleared
            && signalled
            && closed
            && silent
            && oracle
            && no_backend
            && cleaned
    );
    assert!(
        stopping,
        "owned kernel-credentialed STOPPING datagram was missing or invalid"
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL, OpenSSL and non-root capless child support"]
async fn postgres_platform_loopback_tls_host_systemd_ready_then_https_login_stopping_and_clean_exit(
) {
    let (
        ready,
        output,
        issued,
        current,
        cleared,
        signalled,
        stopping,
        closed,
        silent,
        oracle,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let receiver = Receiver::new();
            let prepared = Prepared::new(f, IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
            let identity = prepared.identity();
            let mut child = prepared.spawn_notify(None, receiver.path().as_os_str());
            let ready = receiver
                .wait_from(b"READY=1", child.pid(), identity.uid[0], identity.gid[0])
                .await;
            let output = child.ready().await;
            let mut issued = false;
            let mut current = false;
            let mut cleared = false;
            if ready && output {
                let client = http::client(Some(&prepared.tls.ca));
                if let Some(token) = http::token(&http::login(&client, &prepared.origin).await) {
                    issued = stored_digest(f, &token).await;
                    current = http::current(&http::get(&client, &prepared.origin, &token).await);
                    cleared = http::cleared(&http::logout(&client, &prepared.origin, &token).await)
                        && session_count(f).await == 0;
                }
            }
            let signalled = child.signal(libc::SIGINT);
            let stopping = receiver
                .wait_from(b"STOPPING=1", child.pid(), identity.uid[0], identity.gid[0])
                .await;
            let closed = stopped(&child.finish(None, Duration::from_secs(35)).await);
            let silent = receiver.take().is_none();
            let oracle = receiver.control();
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish() && receiver.finish();
            (
                ready, output, issued, current, cleared, signalled, stopping, closed, silent,
                oracle, no_backend, cleaned,
            )
        })
    })
    .await;
    assert!(
        ready
            && output
            && issued
            && current
            && cleared
            && signalled
            && stopping
            && closed
            && silent
            && oracle
            && no_backend
            && cleaned
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL, OpenSSL and non-root capless child support"]
async fn postgres_platform_loopback_tls_host_notify_startup_failure_silent_and_shutdown_send_failure_still_drains(
) {
    let (
        startup,
        silent,
        oracle,
        unchanged,
        ready_send_failure,
        ready,
        issued,
        pending,
        signalled,
        alive,
        listener_closed,
        released,
        confirmed,
        committed,
        closed,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let mut receiver = Receiver::new();
            let prepared = Prepared::new(f, IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
            let before = snapshot(f).await;
            let mut document = prepared.document.clone();
            document["database"]["password"] =
                "synthetic-notify-deliberately-wrong-scram-password".into();
            let bad = prepared.variant("notify-wrong-password.json", &document);
            let startup = failed(
                &prepared
                    .spawn_notify(Some(&bad), receiver.path().as_os_str())
                    .finish(None, Duration::from_secs(15))
                    .await,
            );
            let silent = receiver.take().is_none();
            let oracle = receiver.control();
            let unchanged = before == snapshot(f).await && session_count(f).await == 0;
            // Observe a real expected-opener SQL wait; elapsed time alone is not a startup oracle.
            let (holder, oid, blocker) = wait_helpers::lock_marker(f).await;
            let mut blocked = prepared.spawn_notify(None, receiver.path().as_os_str());
            let opening = wait_helpers::pending_marker_lock(f, oid, blocker).await;
            let quiet = receiver.take().is_none() && blocked.quiet_before_ready();
            // Connection to the original receiver succeeded before the SQL wait. Losing it now
            // must make the final READY send fail, never turn a bound host into reported Ready.
            receiver.close();
            let released_startup = wait_helpers::release(holder).await;
            let startup_failed = failed(&blocked.finish(None, Duration::from_secs(15)).await);
            let no_bound_host = no_listener(prepared.bind).await;
            let stable = before == snapshot(f).await && session_count(f).await == 0;
            let receiver_cleaned = receiver.finish();
            let ready_send_failure = opening
                && quiet
                && released_startup
                && startup_failed
                && no_bound_host
                && stable
                && receiver_cleaned;
            let mut receiver = Receiver::new();
            let mut child = prepared.spawn_notify(None, receiver.path().as_os_str());
            let identity = prepared.identity();
            let ready = receiver
                .wait_from(b"READY=1", child.pid(), identity.uid[0], identity.gid[0])
                .await
                && child.ready().await;
            let mut issued = false;
            let mut pending = false;
            let mut signalled = false;
            let mut alive = false;
            let mut listener_closed = false;
            let mut released = false;
            let mut confirmed = false;
            let mut committed = false;
            if ready {
                let client = http::client(Some(&prepared.tls.ca));
                let token = http::token(&http::login(&client, &prepared.origin).await);
                issued = token.is_some();
                if let Some(token) = token {
                    let counter = watermark(f).await;
                    let (holder, blocker) = waits::source_holder(f).await;
                    let request = http::logout(&client, &prepared.origin, &token);
                    let control = async {
                        let pending = waits::waiting(f, blocker).await;
                        // The already-connected notifier loses its receiver only after real SQL admission.
                        receiver.close();
                        let signalled = child.signal(libc::SIGTERM);
                        let listener_closed = tokio::time::timeout(Duration::from_secs(1), async {
                            loop {
                                if no_listener(prepared.bind).await {
                                    break true;
                                }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                        })
                        .await
                        .unwrap_or(false);
                        let alive = !child.poll();
                        let released = wait_helpers::release(holder).await;
                        (pending, signalled, alive, listener_closed, released)
                    };
                    let (reply, control) = tokio::join!(request, control);
                    (pending, signalled, alive, listener_closed, released) = control;
                    confirmed = http::cleared(&reply);
                    committed = session_count(f).await == 0 && watermark(f).await == counter;
                }
            }
            if !signalled {
                child.signal(libc::SIGTERM);
            }
            let closed = failed_after_ready(&child.finish(None, Duration::from_secs(35)).await);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish() && receiver.finish();
            (
                startup,
                silent,
                oracle,
                unchanged,
                ready_send_failure,
                ready,
                issued,
                pending,
                signalled,
                alive,
                listener_closed,
                released,
                confirmed,
                committed,
                closed,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(
        startup
            && silent
            && oracle
            && unchanged
            && ready_send_failure
            && ready
            && issued
            && pending
            && signalled
            && alive
            && listener_closed
            && released
            && confirmed
            && committed
            && closed
            && no_backend
            && cleaned
    );
}
