use ores_locks_and_leases::LocalFileLock;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    io::Error::other(message.into()).into()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).ok_or_else(|| failure("missing mode"))?;
    let path = Path::new(args.get(2).ok_or_else(|| failure("missing path"))?);
    let owner = args.get(3).ok_or_else(|| failure("missing owner"))?;

    match mode.as_str() {
        "hold" => {
            let release = Path::new(args.get(4).ok_or_else(|| failure("missing release sentinel"))?);
            let mut lock = LocalFileLock::try_acquire(path, owner.clone())?
                .ok_or_else(|| failure("holder unexpectedly contended"))?;
            println!("HELD");
            io::stdout().flush()?;
            let deadline = Instant::now() + Duration::from_secs(20);
            while !release.exists() {
                if Instant::now() >= deadline {
                    return Err(failure("timed out waiting for release sentinel"));
                }
                thread::sleep(Duration::from_millis(5));
            }
            lock.release()?;
            println!("RELEASED");
        }
        "contend" => {
            match LocalFileLock::try_acquire(path, owner.clone())? {
                Some(mut lock) => {
                    lock.release()?;
                    return Err(failure("contender unexpectedly acquired live cross-runtime lock"));
                }
                None => println!("CONTENDED"),
            }
        }
        "acquire" => {
            let mut lock = LocalFileLock::try_acquire(path, owner.clone())?
                .ok_or_else(|| failure("post-release acquire unexpectedly contended"))?;
            println!("ACQUIRED");
            lock.release()?;
        }
        other => return Err(failure(format!("unknown mode {other:?}"))),
    }

    let _ = fs::metadata(path.parent().unwrap_or(Path::new(".")));
    Ok(())
}
