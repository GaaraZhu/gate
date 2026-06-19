//! Shared append-only file writer for JSONL event logs ([`crate::stats`] and
//! [`crate::event_log`]). Single-syscall `O_APPEND` writes so concurrent
//! producers cannot interleave bytes on POSIX or Windows.

use anyhow::Result;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Append `bytes` to the file at `path`, creating the parent directory and
/// the file itself if needed.
pub fn append(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    write_with_retry(path, bytes)
}

#[cfg(unix)]
fn write_with_retry(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(windows)]
fn write_with_retry(path: &Path, bytes: &[u8]) -> Result<()> {
    use anyhow::anyhow;
    // Windows antivirus / search-indexer software can briefly hold an exclusive
    // lock on a file it's scanning, causing `OpenOptions::open` to return
    // ERROR_SHARING_VIOLATION (raw OS error 32). Retry a few times with tiny
    // backoff; if we still can't open, give up silently.
    const MAX_TRIES: u32 = 3;
    const BACKOFF_MS: u64 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;

    for attempt in 0..MAX_TRIES {
        match OpenOptions::new().create(true).append(true).open(path) {
            Ok(mut file) => {
                file.write_all(bytes)?;
                return Ok(());
            }
            Err(e) => {
                if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION) && attempt + 1 < MAX_TRIES {
                    std::thread::sleep(std::time::Duration::from_millis(BACKOFF_MS));
                    continue;
                }
                return Err(e.into());
            }
        }
    }
    Err(anyhow!("write_with_retry: exhausted retries"))
}

#[cfg(not(any(unix, windows)))]
fn write_with_retry(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(bytes)?;
    Ok(())
}
