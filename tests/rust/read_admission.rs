use super::*;
fn service() -> Service {
    Service {
        input_proofs: Default::default(),
        state: Default::default(),
        send: mpsc::channel(1).0,
        readers: Arc::new(tokio::sync::Semaphore::new(4)),
        heavy_readers: Arc::new(tokio::sync::Semaphore::new(2)),
        pending_reads: Arc::new(tokio::sync::Semaphore::new(32)),
        stopping: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    }
}
#[test]
fn read_bursts_wait_for_returned_slots_without_rejecting_ordinary_navigation() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let service = service();
        let held = service.readers.clone().acquire_many_owned(4).await.unwrap();
        let next = service.clone();
        let task = tokio::spawn(async move { next.read_permits(false).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!task.is_finished());
        drop(held);
        assert!(task.await.unwrap().is_ok());
        assert_eq!(service.readers.available_permits(), 4);
        assert_eq!(service.pending_reads.available_permits(), 32);
    });
}
#[test]
fn heavy_read_waiters_leave_capacity_for_status_and_inspector_and_cancel_cleanly() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let service = service();
        let held = service
            .heavy_readers
            .clone()
            .acquire_many_owned(2)
            .await
            .unwrap();
        let next = service.clone();
        let task = tokio::spawn(async move { next.read_permits(true).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!task.is_finished());
        assert_eq!(service.readers.available_permits(), 4);
        let light = service.read_permits(false).await.unwrap();
        drop(light);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        drop(held);
        assert_eq!(service.pending_reads.available_permits(), 32);
        assert_eq!(service.heavy_readers.available_permits(), 2);
    });
}
#[test]
fn read_waiters_have_a_count_limit_and_a_shared_finite_deadline() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let service = service();
        let held = service
            .pending_reads
            .clone()
            .acquire_many_owned(32)
            .await
            .unwrap();
        assert_eq!(service.read_permits(false).await.unwrap_err().code, "busy");
        drop(held);
        let held = service.readers.clone().acquire_many_owned(4).await.unwrap();
        let started = std::time::Instant::now();
        assert_eq!(service.read_permits(false).await.unwrap_err().code, "busy");
        assert!(started.elapsed() < std::time::Duration::from_secs(4));
        drop(held);
        assert_eq!(service.pending_reads.available_permits(), 32);
    });
}
