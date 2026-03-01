//! Writer lock management for single-writer / multi-reader concurrency.
//!
//! Implements the advisory lock file protocol from spec 09:
//! - Lock file at `{path}.lock` with PID, hostname, timestamp, UUID
//! - Stale lock detection via PID liveness and age threshold
//! - Atomic creation via O_CREAT | O_EXCL

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The lock file magic: "RVLF" in ASCII (big-endian).
const LOCK_MAGIC: u32 = 0x52564C46;

/// Lock protocol version.
const LOCK_VERSION: u32 = 1;

/// Lock file total size in bytes.
const LOCK_FILE_SIZE: usize = 104;

/// Stale lock age threshold for same-host (30 seconds in nanoseconds).
const STALE_AGE_NS: u64 = 30_000_000_000;

/// Represents an acquired writer lock.
pub(crate) struct WriterLock {
    lock_path: PathBuf,
    writer_id: [u8; 16],
}

impl WriterLock {
    /// Attempt to acquire the writer lock for the given RVF file path.
    ///
    /// Returns `Ok(WriterLock)` on success, or an `io::Error` if the lock
    /// is held by another active writer.
    pub(crate) fn acquire(rvf_path: &Path) -> io::Result<Self> {
        let lock_path = lock_path_for(rvf_path);
        let pid = std::process::id();
        let hostname = get_hostname();
        let timestamp_ns = now_ns();
        let writer_id = random_uuid();

        // Build lock file content.
        let content = build_lock_content(pid, &hostname, timestamp_ns, &writer_id);

        // Attempt atomic creation.
        match atomic_create_file(&lock_path, &content) {
            Ok(()) => Ok(WriterLock {
                lock_path,
                writer_id,
            }),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                // Check for stale lock.
                if try_break_stale_lock(&lock_path)? {
                    // Retry after breaking stale lock.
                    atomic_create_file(&lock_path, &content)?;
                    Ok(WriterLock {
                        lock_path,
                        writer_id,
                    })
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "another writer holds the lock",
                    ))
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Release the writer lock.
    ///
    /// Verifies that the lock file still contains our writer_id before
    /// removing it, preventing deletion of a lock legitimately taken over.
    pub(crate) fn release(self) -> io::Result<()> {
        // Verify our writer_id is still in the lock.
        if let Ok(content) = fs::read(&self.lock_path) {
            if content.len() >= LOCK_FILE_SIZE {
                let stored_id = &content[0x50..0x60];
                if stored_id == self.writer_id {
                    let _ = fs::remove_file(&self.lock_path);
                }
            }
        }
        Ok(())
    }

    /// Check if the lock is still held by us.
    #[allow(dead_code)]
    pub(crate) fn is_valid(&self) -> bool {
        if let Ok(content) = fs::read(&self.lock_path) {
            if content.len() >= LOCK_FILE_SIZE {
                let stored_id = &content[0x50..0x60];
                return stored_id == self.writer_id;
            }
        }
        false
    }
}

impl Drop for WriterLock {
    fn drop(&mut self) {
        // Best-effort release on drop.
        if let Ok(content) = fs::read(&self.lock_path) {
            if content.len() >= LOCK_FILE_SIZE {
                let stored_id = &content[0x50..0x60];
                if stored_id == self.writer_id {
                    let _ = fs::remove_file(&self.lock_path);
                }
            }
        }
    }
}

/// Compute the lock file path for a given RVF file.
pub(crate) fn lock_path_for(rvf_path: &Path) -> PathBuf {
    let mut p = rvf_path.as_os_str().to_os_string();
    p.push(".lock");
    PathBuf::from(p)
}

/// Try to break a stale lock. Returns `true` if the lock was broken.
fn try_break_stale_lock(lock_path: &Path) -> io::Result<bool> {
    let content = match fs::read(lock_path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(e),
    };

    if content.len() < LOCK_FILE_SIZE {
        // Invalid lock file — delete it.
        let _ = fs::remove_file(lock_path);
        return Ok(true);
    }

    // Validate magic.
    let magic = u32::from_le_bytes([content[0], content[1], content[2], content[3]]);
    if magic != LOCK_MAGIC {
        let _ = fs::remove_file(lock_path);
        return Ok(true);
    }

    // Read PID and timestamp.
    let lock_pid = u32::from_le_bytes([content[4], content[5], content[6], content[7]]);
    let lock_timestamp = u64::from_le_bytes([
        content[0x48],
        content[0x49],
        content[0x4A],
        content[0x4B],
        content[0x4C],
        content[0x4D],
        content[0x4E],
        content[0x4F],
    ]);

    let current_time = now_ns();
    let age = current_time.saturating_sub(lock_timestamp);

    // Read hostname.
    let lock_hostname = read_hostname_from_lock(&content[0x08..0x48]);
    let current_hostname = get_hostname();
    let same_host = lock_hostname == current_hostname;

    // Check if PID is alive (same host only).
    let pid_alive = if same_host {
        is_pid_alive(lock_pid)
    } else {
        // Cannot check remote PID; rely on age only.
        true
    };

    // Stale conditions:
    // - PID is dead AND age > threshold (same host)
    // - Age > extended threshold (cross-host)
    let threshold = if same_host {
        STALE_AGE_NS
    } else {
        300_000_000_000
    };

    if !pid_alive && age > threshold {
        let _ = fs::remove_file(lock_path);
        return Ok(true);
    }

    if !same_host && age > threshold {
        let _ = fs::remove_file(lock_path);
        return Ok(true);
    }

    Ok(false)
}

fn build_lock_content(
    pid: u32,
    hostname: &str,
    timestamp_ns: u64,
    writer_id: &[u8; 16],
) -> Vec<u8> {
    let mut buf = vec![0u8; LOCK_FILE_SIZE];

    // Magic (0x00).
    buf[0..4].copy_from_slice(&LOCK_MAGIC.to_le_bytes());
    // PID (0x04).
    buf[4..8].copy_from_slice(&pid.to_le_bytes());
    // Hostname (0x08, max 64 bytes, null-terminated).
    let host_bytes = hostname.as_bytes();
    let copy_len = host_bytes.len().min(62); // Reserve byte for null terminator
    buf[0x08..0x08 + copy_len].copy_from_slice(&host_bytes[..copy_len]);
    buf[0x08 + copy_len] = 0; // Explicit null terminator
                              // Timestamp (0x48).
    buf[0x48..0x50].copy_from_slice(&timestamp_ns.to_le_bytes());
    // Writer ID (0x50).
    buf[0x50..0x60].copy_from_slice(writer_id);
    // Lock version (0x60).
    buf[0x60..0x64].copy_from_slice(&LOCK_VERSION.to_le_bytes());
    // CRC32 (0x64) — simplified: we use a basic checksum.
    let crc = simple_crc32(&buf[0..0x64]);
    buf[0x64..0x68].copy_from_slice(&crc.to_le_bytes());

    buf
}

fn atomic_create_file(path: &Path, content: &[u8]) -> io::Result<()> {
    // Use O_CREAT | O_EXCL semantics via OpenOptions.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(content)?;
    file.sync_all()?;
    Ok(())
}

fn read_hostname_from_lock(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

fn get_hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| {
        fs::read_to_string("/etc/hostname")
            .unwrap_or_else(|_| "unknown".into())
            .trim()
            .to_string()
    })
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn random_uuid() -> [u8; 16] {
    // Simple random UUID generation using /dev/urandom or time-based fallback.
    let mut buf = [0u8; 16];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    } else {
        // Fallback: use timestamp + PID.
        let ts = now_ns();
        buf[0..8].copy_from_slice(&ts.to_le_bytes());
        buf[8..12].copy_from_slice(&std::process::id().to_le_bytes());
    }
    buf
}

fn is_pid_alive(pid: u32) -> bool {
    // On Unix, kill(pid, 0) checks process existence without sending a signal.
    // A return of 0 means the process exists and we have permission to signal it.
    // EPERM (errno = 1) means the process exists but belongs to a different user
    // -- still alive. Any other error (ESRCH = no such process) means dead.
    #[cfg(unix)]
    {
        let ret = libc_kill(pid as i32, 0);
        if ret == 0 {
            return true;
        }
        // Check errno for EPERM -- process exists but we lack permission
        let err = unsafe { *libc_errno() };
        err == EPERM
    }
    #[cfg(not(unix))]
    {
        // On non-Unix platforms, we cannot determine PID liveness.
        // Conservatively assume alive to avoid breaking stale locks
        // that might still be held. The age-based fallback in
        // try_break_stale_lock will handle truly stale locks.
        let _ = pid;
        true
    }
}

#[cfg(unix)]
extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
extern "C" {
    fn __errno_location() -> *mut i32;
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
extern "C" {
    fn __error() -> *mut i32;
}

/// Permission denied errno -- process exists but belongs to another user.
#[cfg(unix)]
const EPERM: i32 = 1;

#[cfg(unix)]
fn libc_kill(pid: i32, sig: i32) -> i32 {
    unsafe { kill(pid, sig) }
}

/// Get a pointer to the thread-local errno value.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn libc_errno() -> *mut i32 {
    unsafe { __errno_location() }
}

/// Get a pointer to the thread-local errno value (macOS/BSD).
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
fn libc_errno() -> *mut i32 {
    unsafe { __error() }
}

/// Simple CRC32 (not CRC32C) for lock file checksumming.
fn simple_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn lock_path_computation() {
        let p = Path::new("/tmp/data.rvf");
        assert_eq!(lock_path_for(p), PathBuf::from("/tmp/data.rvf.lock"));
    }

    #[test]
    fn acquire_and_release() {
        let dir = TempDir::new().unwrap();
        let rvf_path = dir.path().join("test.rvf");
        fs::write(&rvf_path, b"").unwrap();

        let lock = WriterLock::acquire(&rvf_path).unwrap();
        assert!(lock.is_valid());

        // Second acquisition should fail.
        let result = WriterLock::acquire(&rvf_path);
        assert!(result.is_err());

        lock.release().unwrap();

        // Now acquisition should succeed again.
        let lock2 = WriterLock::acquire(&rvf_path).unwrap();
        assert!(lock2.is_valid());
    }

    #[test]
    fn stale_lock_detection() {
        let dir = TempDir::new().unwrap();
        let rvf_path = dir.path().join("test2.rvf");
        fs::write(&rvf_path, b"").unwrap();
        let lock_path = lock_path_for(&rvf_path);

        // Write a lock with PID 999999999 (almost certainly dead) and old timestamp.
        let fake_pid = 999999999u32;
        let old_ts = now_ns().saturating_sub(60_000_000_000); // 60s ago
        let fake_id = [0xABu8; 16];
        let content = build_lock_content(fake_pid, &get_hostname(), old_ts, &fake_id);
        fs::write(&lock_path, &content).unwrap();

        // Should be able to acquire despite existing lock (stale).
        let lock = WriterLock::acquire(&rvf_path).unwrap();
        assert!(lock.is_valid());
    }

    #[test]
    fn simple_crc32_works() {
        let data = b"hello";
        let crc = simple_crc32(data);
        assert_ne!(crc, 0);
        // Same input produces same output.
        assert_eq!(crc, simple_crc32(data));
    }
}
