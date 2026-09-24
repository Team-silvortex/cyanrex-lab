use super::*;

pub(super) async fn join_writer(mut worker: tokio::task::JoinHandle<()>) {
    let result = tokio::time::timeout(StdDuration::from_secs(2), &mut worker).await;
    if result.is_err() {
        // A red regression must not leak its own background task into fixture cleanup.
        worker.abort();
        let _ = worker.await;
    }
    result
        .expect("the writer must exit after the last producer leaves")
        .unwrap();
}

#[tokio::test]
async fn idle_persistence_worker_does_not_keep_its_own_producer_alive() {
    let mut bus = memory_bus();
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
        .unwrap();
    let (sender, receiver) = mpsc::channel(4);
    bus.persist_sender = Some(sender);
    let state = Arc::downgrade(&bus.history);
    let worker = tokio::spawn(event_bus_db::run_persist_loop(bus.clone(), pool, receiver));
    let last_producer = bus.clone();
    drop(bus);
    tokio::task::yield_now().await;
    assert!(
        !worker.is_finished(),
        "a remaining producer keeps its writer available"
    );
    drop(last_producer);
    join_writer(worker).await;
    assert!(
        state.upgrade().is_none(),
        "writer-owned history must be released"
    );
}

#[tokio::test]
async fn disabled_persistence_worker_consumes_queued_barriers_then_exits() {
    let mut bus = memory_bus();
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
        .unwrap();
    let (sender, receiver) = mpsc::channel(4);
    bus.persist_sender = Some(sender);
    bus.persist_sender
        .as_ref()
        .unwrap()
        .try_send(event_bus_db::PersistMessage::Event(
            event_bus_db::new_persist_request(event("alice", 0), 50, EventOverflowPolicy::DropNew),
        ))
        .unwrap();
    let (reply, barrier) = tokio::sync::oneshot::channel();
    bus.persist_sender
        .as_ref()
        .unwrap()
        .try_send(event_bus_db::PersistMessage::Barrier(reply))
        .unwrap();
    let worker = tokio::spawn(event_bus_db::run_persist_loop(bus.clone(), pool, receiver));
    drop(bus);
    assert!(
        !barrier.await.unwrap(),
        "fallback must never acknowledge a durable barrier"
    );
    join_writer(worker).await;
}
