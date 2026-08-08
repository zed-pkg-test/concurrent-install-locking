use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use zed_lock::{LockEventKind, LockManager, LockRequest};

fn unique_lock_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("zed-lock-try-acquire-{}-{nonce}", std::process::id()))
        .join("operation.lock")
}

#[test]
fn immediate_contention_is_none_then_retry_acquires() {
    let path = unique_lock_path();
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let manager = LockManager::builder()
        .event_sink(move |event| {
            observed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event.kind);
        })
        .build();
    let request = || {
        LockRequest::exclusive(&path)
            .operation("test-org immediate contention probe")
            .queue_same_process()
    };

    let owner = manager
        .acquire_blocking(request())
        .expect("the first independent descriptor must acquire the lock");
    let contended = manager
        .try_acquire(request())
        .expect("native lock contention must not be returned as a fatal I/O error");
    assert!(
        contended.is_none(),
        "try_acquire must return Ok(None) while another descriptor owns the lock"
    );
    assert!(
        events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(&LockEventKind::Contended),
        "ordinary native contention must emit LockEventKind::Contended"
    );

    drop(owner);
    let retry = manager
        .try_acquire(request())
        .expect("retry after owner release must not fail")
        .expect("retry after owner release must acquire the lock");
    drop(retry);

    let _ = std::fs::remove_file(&path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}
