use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use zed_lock::{LockClass, LockManager, LockRequest};

const ROLE: &str = "ZED_LOCK_ORG_TEST_ROLE";
const LOCK_PATH: &str = "ZED_LOCK_ORG_TEST_LOCK_PATH";
const ATTEMPTING: &str = "ZED_LOCK_ORG_TEST_ATTEMPTING";
const ACQUIRED: &str = "ZED_LOCK_ORG_TEST_ACQUIRED";
const HOLD_MS: &str = "ZED_LOCK_ORG_TEST_HOLD_MS";
const COUNTER_PATH: &str = "ZED_LOCK_ORG_TEST_COUNTER_PATH";
const ITERATIONS: &str = "ZED_LOCK_ORG_TEST_ITERATIONS";
const CRITICAL_DIR: &str = "ZED_LOCK_ORG_TEST_CRITICAL_DIR";
const OVERLAP_MARKER: &str = "ZED_LOCK_ORG_TEST_OVERLAP_MARKER";
const HELPER_TEST: &str = "process_helper";
const DEADLINE: Duration = Duration::from_secs(20);

struct ManagedChild {
    child: Option<Child>,
    label: String,
}

impl ManagedChild {
    #[allow(clippy::too_many_arguments)]
    fn spawn(
        role: &str,
        lock_path: &Path,
        attempting: &Path,
        acquired: &Path,
        hold: Duration,
        counter_path: Option<&Path>,
        iterations: Option<usize>,
        critical_dir: Option<&Path>,
        overlap_marker: Option<&Path>,
        label: impl Into<String>,
    ) -> Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg(HELPER_TEST)
            .arg("--exact")
            .arg("--nocapture")
            .env(ROLE, role)
            .env(LOCK_PATH, lock_path)
            .env(ATTEMPTING, attempting)
            .env(ACQUIRED, acquired)
            .env(HOLD_MS, hold.as_millis().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        if let Some(counter_path) = counter_path {
            command.env(COUNTER_PATH, counter_path);
        }
        if let Some(iterations) = iterations {
            command.env(ITERATIONS, iterations.to_string());
        }
        if let Some(critical_dir) = critical_dir {
            command.env(CRITICAL_DIR, critical_dir);
        }
        if let Some(overlap_marker) = overlap_marker {
            command.env(OVERLAP_MARKER, overlap_marker);
        }

        let label = label.into();
        let child = command
            .spawn()
            .with_context(|| format!("spawning {label}"))?;
        Ok(Self {
            child: Some(child),
            label,
        })
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("managed child still present")
    }

    fn wait_for_marker(&mut self, marker: &Path) -> Result<()> {
        let deadline = Instant::now() + DEADLINE;
        while !marker.is_file() {
            if let Some(status) = self.child_mut().try_wait()? {
                return Err(anyhow!(
                    "{} exited before writing {}: {status}",
                    self.label,
                    marker.display()
                ));
            }
            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "{} did not write {} before the deadline",
                    self.label,
                    marker.display()
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn assert_running(&mut self) -> Result<()> {
        let status = self.child_mut().try_wait()?;
        anyhow::ensure!(
            status.is_none(),
            "{} exited unexpectedly: {status:?}",
            self.label
        );
        Ok(())
    }

    fn wait_success(&mut self) -> Result<()> {
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Some(status) = self.child_mut().try_wait()? {
                self.child.take();
                anyhow::ensure!(status.success(), "{} failed: {status}", self.label);
                return Ok(());
            }
            if Instant::now() >= deadline {
                let mut child = self.child.take().expect("managed child still present");
                let _ = child.kill();
                let status = child.wait()?;
                return Err(anyhow!("{} timed out; final status: {status}", self.label));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn kill_and_wait(&mut self) -> Result<ExitStatus> {
        let mut child = self.child.take().expect("managed child still present");
        let kill_result = child.kill();
        let wait_result = child.wait();
        kill_result?;
        Ok(wait_result?)
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn request(path: &Path, operation: impl Into<String>) -> LockRequest {
    LockRequest::exclusive(path)
        .operation(operation)
        .class(LockClass::ProjectMutation)
}

#[test]
fn process_helper() -> Result<()> {
    let Some(role) = std::env::var_os(ROLE) else {
        return Ok(());
    };
    let role = role.to_string_lossy().into_owned();
    let lock_path = PathBuf::from(std::env::var_os(LOCK_PATH).context("helper lock path")?);
    let attempting = PathBuf::from(std::env::var_os(ATTEMPTING).context("attempting marker")?);
    let acquired = PathBuf::from(std::env::var_os(ACQUIRED).context("acquired marker")?);
    let hold_ms = std::env::var(HOLD_MS)?.parse::<u64>()?;
    fs::write(&attempting, b"attempting")?;

    let manager = LockManager::default();
    match role.as_str() {
        "hold" | "critical" => {
            let guard = manager.acquire_blocking(request(&lock_path, format!("helper {role}")))?;
            if let Some(critical_dir) = std::env::var_os(CRITICAL_DIR).map(PathBuf::from) {
                if let Err(error) = fs::create_dir(&critical_dir) {
                    if let Some(overlap_marker) =
                        std::env::var_os(OVERLAP_MARKER).map(PathBuf::from)
                    {
                        let _ = fs::write(
                            overlap_marker,
                            format!("critical section overlap: {error}\n"),
                        );
                    }
                    return Err(error).context("entering lock-protected critical directory");
                }
            }
            fs::write(&acquired, b"acquired")?;
            thread::sleep(Duration::from_millis(hold_ms));
            if let Some(critical_dir) = std::env::var_os(CRITICAL_DIR).map(PathBuf::from) {
                fs::remove_dir(critical_dir)?;
            }
            drop(guard);
        }
        "increment" => {
            let counter_path = PathBuf::from(
                std::env::var_os(COUNTER_PATH).context("counter path for increment helper")?,
            );
            let iterations = std::env::var(ITERATIONS)?.parse::<usize>()?;
            fs::write(&acquired, b"started")?;
            for _ in 0..iterations {
                let guard = manager.acquire_blocking(request(&lock_path, "counter increment"))?;
                let current = fs::read_to_string(&counter_path)?
                    .trim()
                    .parse::<usize>()?;
                thread::yield_now();
                fs::write(&counter_path, (current + 1).to_string())?;
                drop(guard);
            }
        }
        other => return Err(anyhow!("unknown helper role: {other}")),
    }
    Ok(())
}

#[test]
fn six_processes_receive_exclusive_handoffs_without_overlap() -> Result<()> {
    const PROCESS_COUNT: usize = 6;

    let temp = tempfile::tempdir()?;
    let lock_path = temp.path().join("queue.lock");
    let critical_dir = temp.path().join("critical-section");
    let overlap_marker = temp.path().join("overlap-detected");
    let owner = LockManager::default().acquire_blocking(request(&lock_path, "parent owner"))?;

    let markers = (0..PROCESS_COUNT)
        .map(|index| {
            (
                temp.path().join(format!("waiter-{index}-attempting")),
                temp.path().join(format!("waiter-{index}-acquired")),
            )
        })
        .collect::<Vec<_>>();
    let mut waiters = markers
        .iter()
        .enumerate()
        .map(|(index, (attempting, acquired))| {
            ManagedChild::spawn(
                "critical",
                &lock_path,
                attempting,
                acquired,
                Duration::from_millis(50),
                None,
                None,
                Some(&critical_dir),
                Some(&overlap_marker),
                format!("queue waiter {index}"),
            )
        })
        .collect::<Result<Vec<_>>>()?;

    for (waiter, (attempting, acquired)) in waiters.iter_mut().zip(&markers) {
        waiter.wait_for_marker(attempting)?;
        anyhow::ensure!(
            !acquired.exists(),
            "a process acquired while the parent still owned the lock"
        );
    }
    drop(owner);

    for waiter in &mut waiters {
        waiter.wait_success()?;
    }
    anyhow::ensure!(
        markers.iter().all(|(_, acquired)| acquired.is_file()),
        "not every queued process acquired the lock"
    );
    anyhow::ensure!(
        !overlap_marker.exists(),
        "two processes entered the exclusive critical section together"
    );
    anyhow::ensure!(
        !critical_dir.exists(),
        "the final process left the critical-section sentinel behind"
    );
    Ok(())
}

#[test]
fn killed_owner_releases_the_lock_and_preserves_the_rendezvous_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let lock_path = temp.path().join("owner-death.lock");
    let owner_attempting = temp.path().join("owner-attempting");
    let owner_acquired = temp.path().join("owner-acquired");
    let mut owner = ManagedChild::spawn(
        "hold",
        &lock_path,
        &owner_attempting,
        &owner_acquired,
        Duration::from_secs(60),
        None,
        None,
        None,
        None,
        "killed owner",
    )?;
    owner.wait_for_marker(&owner_acquired)?;
    let status = owner.kill_and_wait()?;
    anyhow::ensure!(!status.success(), "the killed owner unexpectedly succeeded");
    anyhow::ensure!(
        lock_path.is_file(),
        "the stable lock rendezvous file was deleted on owner death"
    );

    let waiter_attempting = temp.path().join("successor-attempting");
    let waiter_acquired = temp.path().join("successor-acquired");
    let mut successor = ManagedChild::spawn(
        "hold",
        &lock_path,
        &waiter_attempting,
        &waiter_acquired,
        Duration::ZERO,
        None,
        None,
        None,
        None,
        "post-kill successor",
    )?;
    successor.wait_success()?;
    anyhow::ensure!(waiter_acquired.is_file());
    anyhow::ensure!(lock_path.is_file());
    Ok(())
}

#[test]
fn unrelated_process_lock_completes_while_another_identity_is_held() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let held_path = temp.path().join("held.lock");
    let independent_path = temp.path().join("independent.lock");
    let owner_attempting = temp.path().join("held-attempting");
    let owner_acquired = temp.path().join("held-acquired");
    let mut owner = ManagedChild::spawn(
        "hold",
        &held_path,
        &owner_attempting,
        &owner_acquired,
        Duration::from_secs(3),
        None,
        None,
        None,
        None,
        "long unrelated owner",
    )?;
    owner.wait_for_marker(&owner_acquired)?;

    let attempting = temp.path().join("independent-attempting");
    let acquired = temp.path().join("independent-acquired");
    let mut independent = ManagedChild::spawn(
        "hold",
        &independent_path,
        &attempting,
        &acquired,
        Duration::ZERO,
        None,
        None,
        None,
        None,
        "independent process",
    )?;
    independent.wait_success()?;
    anyhow::ensure!(acquired.is_file());
    owner.assert_running()?;
    let _ = owner.kill_and_wait()?;
    Ok(())
}

#[test]
fn many_processes_preserve_one_shared_counter() -> Result<()> {
    let stress = std::env::var_os("ZED_LOCK_E2E_STRESS").is_some();
    let process_count = if stress { 20 } else { 10 };
    let iterations = if stress { 400 } else { 80 };

    let temp = tempfile::tempdir()?;
    let lock_path = temp.path().join("counter.lock");
    let counter_path = temp.path().join("counter.txt");
    fs::write(&counter_path, b"0")?;

    let mut workers = Vec::with_capacity(process_count);
    for index in 0..process_count {
        let attempting = temp.path().join(format!("counter-{index}-attempting"));
        let acquired = temp.path().join(format!("counter-{index}-started"));
        workers.push(ManagedChild::spawn(
            "increment",
            &lock_path,
            &attempting,
            &acquired,
            Duration::ZERO,
            Some(&counter_path),
            Some(iterations),
            None,
            None,
            format!("counter worker {index}"),
        )?);
    }
    for worker in &mut workers {
        worker.wait_success()?;
    }

    let actual = fs::read_to_string(&counter_path)?.trim().parse::<usize>()?;
    assert_eq!(actual, process_count * iterations);
    Ok(())
}
