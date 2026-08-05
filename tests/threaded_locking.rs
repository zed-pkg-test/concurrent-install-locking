use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use zed_lock::{LockClass, LockEventKind, LockManager, LockRequest};

const SHORT_WAIT: Duration = Duration::from_millis(40);
const DEADLINE: Duration = Duration::from_secs(10);

fn queued_request(path: impl Into<PathBuf>, operation: impl Into<String>) -> LockRequest {
    LockRequest::exclusive(path)
        .operation(operation)
        .class(LockClass::Artifact)
        .queue_same_process()
}

fn wait_until(predicate: impl Fn() -> bool, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    predicate()
}

#[test]
fn cold_key_progresses_while_many_threads_wait_on_one_hot_key() -> Result<()> {
    const HOT_WAITERS: usize = 12;

    let temp = tempfile::tempdir()?;
    let manager = LockManager::builder().max_waiters(24).build();
    let hot_path = temp.path().join("hot.lock");
    let cold_path = temp.path().join("cold.lock");
    let hot_owner = manager.acquire_blocking(queued_request(&hot_path, "hot owner"))?;

    let mut hot_waiters = (0..HOT_WAITERS)
        .map(|index| manager.acquire(queued_request(&hot_path, format!("hot waiter {index}"))))
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(
        manager.active_waiters(),
        HOT_WAITERS,
        "every hot-key waiter should occupy one bounded native waiter slot"
    );

    let mut cold_waiter = manager.acquire(queued_request(&cold_path, "cold waiter"))?;
    let cold_guard = cold_waiter
        .wait_timeout(DEADLINE)?
        .context("an unrelated cold key was head-of-line blocked behind hot-key waiters")?;

    for waiter in &mut hot_waiters {
        assert!(
            waiter.wait_timeout(SHORT_WAIT)?.is_none(),
            "a hot-key waiter acquired before the hot owner released"
        );
    }
    drop(cold_guard);
    drop(hot_owner);

    for waiter in &mut hot_waiters {
        let guard = waiter
            .wait_timeout(DEADLINE)?
            .context("a queued hot-key waiter did not make eventual progress")?;
        drop(guard);
    }
    assert!(
        wait_until(|| manager.active_waiters() == 0, DEADLINE),
        "native waiter permits were not returned after all handoffs"
    );
    Ok(())
}

#[test]
fn sharded_thread_contention_preserves_every_protected_counter() -> Result<()> {
    let stress = std::env::var_os("ZED_LOCK_E2E_STRESS").is_some();
    let thread_count = if stress { 48 } else { 16 };
    let iterations = if stress { 500 } else { 80 };
    let shard_count = if stress { 12 } else { 6 };

    let temp = tempfile::tempdir()?;
    let manager = Arc::new(LockManager::builder().max_waiters(thread_count * 2).build());
    let barrier = Arc::new(Barrier::new(thread_count + 1));
    let expected = Arc::new(
        (0..shard_count)
            .map(|_| AtomicUsize::new(0))
            .collect::<Vec<_>>(),
    );

    for shard in 0..shard_count {
        fs::write(temp.path().join(format!("counter-{shard}.txt")), b"0")?;
    }

    let mut workers = Vec::with_capacity(thread_count);
    for worker_id in 0..thread_count {
        let root = temp.path().to_path_buf();
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        let expected = Arc::clone(&expected);
        workers.push(thread::spawn(move || -> Result<()> {
            barrier.wait();
            for iteration in 0..iterations {
                let shard = (worker_id * 17 + iteration * 7) % shard_count;
                expected[shard].fetch_add(1, Ordering::Relaxed);
                let lock_path = root.join(format!("counter-{shard}.lock"));
                let counter_path = root.join(format!("counter-{shard}.txt"));
                let guard = manager.acquire_blocking(queued_request(
                    &lock_path,
                    format!("worker {worker_id} shard {shard}"),
                ))?;
                let current = fs::read_to_string(&counter_path)?.trim().parse::<usize>()?;
                thread::yield_now();
                fs::write(&counter_path, (current + 1).to_string())?;
                drop(guard);
            }
            Ok(())
        }));
    }

    barrier.wait();
    for worker in workers {
        worker
            .join()
            .map_err(|_| anyhow!("sharded contention worker panicked"))??;
    }

    for shard in 0..shard_count {
        let actual = fs::read_to_string(temp.path().join(format!("counter-{shard}.txt")))?
            .trim()
            .parse::<usize>()?;
        assert_eq!(
            actual,
            expected[shard].load(Ordering::Relaxed),
            "lost protected writes on shard {shard}"
        );
    }
    Ok(())
}

#[test]
fn repeated_timeouts_observe_one_native_wait_request() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let events = Arc::new(Mutex::new(Vec::<(LockEventKind, String)>::new()));
    let event_log = Arc::clone(&events);
    let manager = LockManager::builder()
        .max_waiters(4)
        .event_sink(move |event| {
            event_log
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push((event.kind, event.operation.clone()));
        })
        .build();
    let path = temp.path().join("timeout.lock");
    let owner = manager.acquire_blocking(queued_request(&path, "timeout owner"))?;
    let mut waiter = manager.acquire(queued_request(&path, "timeout waiter"))?;

    for _ in 0..4 {
        assert!(
            waiter.wait_timeout(SHORT_WAIT)?.is_none(),
            "the waiter acquired while the owner still held the lock"
        );
    }
    {
        let events = events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let waiting = events
            .iter()
            .filter(|(kind, operation)| {
                *kind == LockEventKind::Waiting && operation == "timeout waiter"
            })
            .count();
        assert_eq!(
            waiting, 1,
            "caller timeouts must not create repeated native acquisition attempts"
        );
    }

    drop(owner);
    let guard = waiter
        .wait_timeout(DEADLINE)?
        .context("the original native wait request did not complete after release")?;
    drop(guard);

    let events = events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let acquired = events
        .iter()
        .filter(|(kind, operation)| {
            *kind == LockEventKind::Acquired && operation == "timeout waiter"
        })
        .count();
    let released = events
        .iter()
        .filter(|(kind, operation)| {
            *kind == LockEventKind::Released && operation == "timeout waiter"
        })
        .count();
    assert_eq!(acquired, 1);
    assert_eq!(released, 1);
    Ok(())
}

#[test]
fn cancelled_waiter_drops_any_late_guard_before_another_owner_enters() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let (event_sender, event_receiver) = mpsc::channel::<(LockEventKind, String)>();
    let manager = LockManager::builder()
        .max_waiters(4)
        .event_sink(move |event| {
            let _ = event_sender.send((event.kind, event.operation.clone()));
        })
        .build();
    let path = temp.path().join("cancel.lock");
    let owner = manager.acquire_blocking(queued_request(&path, "cancel owner"))?;
    let waiter = manager.acquire(queued_request(&path, "cancelled waiter"))?;
    drop(waiter);
    drop(owner);

    let deadline = Instant::now() + DEADLINE;
    let mut saw_cancelled = false;
    let mut saw_late_acquire = false;
    let mut saw_late_release = false;
    while Instant::now() < deadline && !(saw_cancelled && saw_late_acquire && saw_late_release) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let (kind, operation) = event_receiver
            .recv_timeout(remaining)
            .context("timed out waiting for cancelled-waiter lifecycle events")?;
        if operation != "cancelled waiter" {
            continue;
        }
        match kind {
            LockEventKind::Cancelled => saw_cancelled = true,
            LockEventKind::Acquired => saw_late_acquire = true,
            LockEventKind::Released => saw_late_release = true,
            _ => {}
        }
    }
    assert!(
        saw_cancelled,
        "dropping the waiter emitted no cancellation event"
    );
    assert!(
        saw_late_acquire && saw_late_release,
        "a detached native request did not immediately release its eventual guard"
    );

    let successor = manager
        .try_acquire(queued_request(&path, "successor"))?
        .context("the cancelled waiter retained ownership after late delivery failed")?;
    drop(successor);
    Ok(())
}

#[test]
fn lock_sets_are_ordered_and_duplicate_canonical_identities_are_rejected() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let events = Arc::new(Mutex::new(Vec::<(LockClass, PathBuf)>::new()));
    let event_log = Arc::clone(&events);
    let manager = LockManager::builder()
        .event_sink(move |event| {
            if event.kind == LockEventKind::Acquired {
                event_log
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push((event.class, event.path.clone()));
            }
        })
        .build();

    let guards = manager.acquire_many_blocking([
        LockRequest::exclusive(temp.path().join("z-build.lock"))
            .operation("build z")
            .class(LockClass::Build),
        LockRequest::exclusive(temp.path().join("project.lock"))
            .operation("project")
            .class(LockClass::ProjectMutation),
        LockRequest::exclusive(temp.path().join("artifact.lock"))
            .operation("artifact")
            .class(LockClass::Artifact),
        LockRequest::exclusive(temp.path().join("a-build.lock"))
            .operation("build a")
            .class(LockClass::Build),
    ])?;
    let observed = events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(observed[0].0, LockClass::ProjectMutation);
    assert_eq!(observed[1].0, LockClass::Artifact);
    assert_eq!(observed[2].0, LockClass::Build);
    assert_eq!(observed[3].0, LockClass::Build);
    assert!(observed[2].1 < observed[3].1);
    drop(observed);
    drop(guards);

    let canonical = temp.path().join("duplicate.lock");
    let alias = temp.path().join(".").join("duplicate.lock");
    let error = match manager.acquire_many_blocking([
        LockRequest::exclusive(&canonical).operation("canonical"),
        LockRequest::exclusive(&alias).operation("alias"),
    ]) {
        Ok(_) => return Err(anyhow!("duplicate canonical lock identity was accepted")),
        Err(error) => error,
    };
    assert!(
        format!("{error:#}").contains("duplicate canonical lock identity"),
        "unexpected duplicate-identity error: {error:#}"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_aliases_share_one_lock_domain() -> Result<()> {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir()?;
    let real = temp.path().join("real");
    let alias = temp.path().join("alias");
    fs::create_dir_all(&real)?;
    symlink(&real, &alias)?;
    let manager = LockManager::default();
    let owner = manager.acquire_blocking(queued_request(real.join("same.lock"), "real owner"))?;
    let mut waiter = manager.acquire(queued_request(alias.join("same.lock"), "alias waiter"))?;
    assert!(
        waiter.wait_timeout(SHORT_WAIT)?.is_none(),
        "a symlink alias split one logical lock domain"
    );
    drop(owner);
    let guard = waiter
        .wait_timeout(DEADLINE)?
        .context("symlink-alias waiter did not wake after owner release")?;
    drop(guard);
    Ok(())
}
