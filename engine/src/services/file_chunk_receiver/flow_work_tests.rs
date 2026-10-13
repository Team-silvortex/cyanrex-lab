use super::*;

#[test]
fn cancelled_running_stage_retains_shared_download_flow_until_original_inode_completion() {
    let root = Root::new();
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let barrier = Barrier::new();
    let _release = ReleaseOnDrop(barrier.clone());
    let (entered, started) = mpsc::channel();
    runtime.block_on(async {
        let original = manifest(1);
        let receiver = receiver(&root, &original);
        let downloads = Arc::new(Semaphore::new(1));
        let parent = Arc::new(downloads.clone().try_acquire_owned().unwrap());
        let callers = Arc::new(Semaphore::new(1));
        let caller = callers.clone().try_acquire_owned().unwrap();
        let mut reader = receiver
            .receive_checked(&original, 0, response(&original))
            .await
            .unwrap();
        // Exactly the attachment used by receive_and_stage_with_flow; no public raw buffer exists.
        reader.retain_caller(Some(caller));
        reader.retain_flow(Some(parent.clone()));
        let held = barrier.clone();
        reader.observe_first_read(move || {
            entered.send(()).unwrap();
            held.wait();
        });
        let target = receiver.clone();
        let input = original.clone();
        let running = runtime.spawn(async move { target.stage_checked(&input, 0, reader).await });
        started.recv_timeout(Duration::from_secs(10)).unwrap();
        let files = fs::read_dir(&root.0)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(files.len(), 1);
        let created = files[0].metadata().unwrap();
        assert_eq!(created.len(), 0);
        assert_eq!(created.mode() & 0o7777, 0o600);
        assert_eq!(created.nlink(), 1);
        let inode = (created.dev(), created.ino());
        full(&receiver, &original).await;
        drop(parent);
        running.abort();
        assert!(running
            .await
            .err()
            .is_some_and(|error| error.is_cancelled()));
        assert_eq!(downloads.available_permits(), 0);
        assert!(downloads.clone().try_acquire_owned().is_err());
        assert_eq!(callers.available_permits(), 0);
        full(&receiver, &original).await;
        barrier.release();
        let slot = timeout(Duration::from_secs(10), downloads.clone().acquire_owned())
            .await
            .unwrap()
            .unwrap();
        drop(slot);
        restored(&receiver, &root, &original, Some(inode)).await;
        assert_eq!(downloads.available_permits(), 1);
        assert_eq!(callers.available_permits(), 1);
    });
}
