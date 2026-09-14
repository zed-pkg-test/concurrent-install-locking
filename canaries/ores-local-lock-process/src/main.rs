use ores_locks_and_leases::{LocalFileLock, local_file_lock_exists};
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    io::Error::other(message.into()).into()
}

fn attempt(path: &Path, barrier: &Path, owner: &str) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !barrier.exists() {
        if Instant::now() >= deadline {
            return Err(failure("timed out waiting for process start barrier"));
        }
        thread::sleep(Duration::from_millis(2));
    }

    match LocalFileLock::try_acquire(path, owner.to_owned())? {
        Some(mut lock) => {
            println!("WIN");
            thread::sleep(Duration::from_secs(2));
            lock.release()?;
        }
        None => println!("CONTENDED"),
    }
    Ok(())
}

fn run_round(round: usize) -> Result<(), Box<dyn Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = env::temp_dir().join(format!(
        "ores-local-lock-process-canary-{}-{nonce}-{round}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;
    let lock_path = root.join("install.lock");
    let barrier = root.join("start");
    let executable = env::current_exe()?;

    let mut children = Vec::new();
    for index in 0..12 {
        children.push(
            Command::new(&executable)
                .arg("attempt")
                .arg(&lock_path)
                .arg(&barrier)
                .arg(format!("process-{round}-{index}"))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        );
    }

    // Give every child time to enter the barrier wait before releasing them.
    thread::sleep(Duration::from_millis(300));
    fs::write(&barrier, b"go")?;

    let mut winners = 0usize;
    let mut contended = 0usize;
    for child in children {
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(failure(format!(
                "child failed: status={} stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        match String::from_utf8(output.stdout)?.trim() {
            "WIN" => winners += 1,
            "CONTENDED" => contended += 1,
            other => return Err(failure(format!("unexpected child result: {other:?}"))),
        }
    }

    if winners != 1 || contended != 11 {
        return Err(failure(format!(
            "expected one process winner and eleven contenders; winners={winners} contended={contended}"
        )));
    }
    if local_file_lock_exists(&lock_path)? {
        return Err(failure("winner released but lock still exists"));
    }

    fs::remove_file(&barrier)?;
    fs::remove_dir(&root)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("attempt") {
        let path = args.get(2).ok_or_else(|| failure("missing lock path"))?;
        let barrier = args.get(3).ok_or_else(|| failure("missing barrier path"))?;
        let owner = args.get(4).ok_or_else(|| failure("missing owner"))?;
        return attempt(Path::new(path), Path::new(barrier), owner);
    }

    for round in 0..5 {
        run_round(round)?;
    }
    println!("5 rounds passed: exactly one winner per 12-process contention wave");
    Ok(())
}
