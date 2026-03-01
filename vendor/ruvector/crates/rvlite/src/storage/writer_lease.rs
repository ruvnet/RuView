//! File-based writer lease for single-writer concurrency in rvlite.
//!
//! Provides a cooperative lock mechanism using a lock file with PID and
//! timestamp. Only one writer may hold the lease at a time. The lease
//! includes a heartbeat timestamp that is checked for staleness so that
//! crashed processes do not permanently block new writers.
//!
//! Lock file location: `{store_path}.lock`
//! Lock file contents: JSON with `pid`, `timestamp_secs`, `hostname`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Default staleness threshold -- if the heartbeat is older than this
/// duration, the lease is considered abandoned and may be force-acquired.
const DEFAULT_STALE_THRESHOLD: Duration = Duration::from_secs(30);

/// Contents written to the lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeaseMeta {
    /// Process ID of the lock holder.
    pid: u32,
    /// Unix timestamp in seconds when the lease was last refreshed.
    timestamp_secs: u64,
    /// Hostname of the lock holder.
    hostname: String,
}

/// A writer lease backed by a lock file on disk.
///
/// While this struct is alive, the lease is held. Dropping it releases
/// the lock file automatically via the `Drop` implementation.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use std::time::Duration;
/// # // This is a doc-test stub; actual usage requires the rvf-backend feature.
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // let lease = WriterLease::acquire(Path::new("/data/store.rvf"), Duration::from_secs(5))?;
/// // ... perform writes ...
/// // lease.release()?; // or just let it drop
/// # Ok(())
/// # }
/// ```
pub struct WriterLease {
    /// Path to the lock file.
    lock_path: PathBuf,
    /// Our PID, used to verify ownership on release.
    pid: u32,
    /// Whether the lease has been explicitly released.
    released: bool,
}

impl WriterLease {
    /// Attempt to acquire the writer lease for the given store path.
    ///
    /// The lock file is created at `{path}.lock`. If another process holds
    /// the lease, this function will retry until `timeout` elapses. If the
    /// existing lease is stale (heartbeat older than 30 seconds and the
    /// holder PID is not alive), the stale lock is broken and acquisition
    /// proceeds.
    ///
    /// # Errors
    ///
    /// Returns `io::Error` with `WouldBlock` if the timeout expires without
    /// acquiring the lease, or propagates any underlying I/O errors.
    pub fn acquire(path: &Path, timeout: Duration) -> io::Result<Self> {
        let lock_path = lock_path_for(path);
        let pid = std::process::id();
        let deadline = Instant::now() + timeout;

        loop {
            // Try to create the lock file exclusively.
            match try_create_lock(&lock_path, pid) {
                Ok(()) => {
                    return Ok(WriterLease {
                        lock_path,
                        pid,
                        released: false,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    // Lock file exists -- check if it is stale.
                    if Self::is_stale(&lock_path, DEFAULT_STALE_THRESHOLD) {
                        // Force-remove the stale lock and retry.
                        let _ = fs::remove_file(&lock_path);
                        continue;
                    }

                    // Lock is active. Check timeout.
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            format!(
                                "writer lease acquisition timed out after {:?} for {:?}",
                                timeout, lock_path
                            ),
                        ));
                    }

                    // Brief sleep before retrying.
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Explicitly release the writer lease.
    ///
    /// Verifies that the lock file still belongs to this process before
    /// removing it to avoid deleting a lock acquired by another process
    /// after a stale break.
    pub fn release(&mut self) -> io::Result<()> {
        if self.released {
            return Ok(());
        }
        self.do_release();
        self.released = true;
        Ok(())
    }

    /// Refresh the heartbeat timestamp in the lock file.
    ///
    /// Writers performing long operations should call this periodically
    /// (e.g. every 10 seconds) to prevent the lease from appearing stale.
    pub fn refresh_heartbeat(&self) -> io::Result<()> {
        if self.released {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "cannot refresh a released lease",
            ));
        }
        // Verify we still own the lock.
        if !self.owns_lock() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "lease was taken over by another process",
            ));
        }
        write_lock_file(&self.lock_path, self.pid)
    }

    /// Check whether the lock file at the given path is stale.
    ///
    /// A lock is stale if:
    /// - The lock file does not exist (vacuously stale).
    /// - The lock file cannot be parsed.
    /// - The heartbeat timestamp is older than `threshold`.
    /// - The PID in the lock file is not alive on the current host.
    pub fn is_stale(path: &Path, threshold: Duration) -> bool {
        let lock_path = if path.extension().map_or(false, |e| e == "lock") {
            path.to_path_buf()
        } else {
            lock_path_for(path)
        };

        let content = match fs::read_to_string(&lock_path) {
            Ok(c) => c,
            Err(_) => return true, // Missing or unreadable = stale.
        };

        let meta: LeaseMeta = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(_) => return true, // Corrupt = stale.
        };

        // Check age.
        let now_secs = current_unix_secs();
        let age_secs = now_secs.saturating_sub(meta.timestamp_secs);
        if age_secs > threshold.as_secs() {
            return true;
        }

        // Check if PID is alive (only meaningful on same host).
        let our_hostname = get_hostname();
        if meta.hostname == our_hostname && !is_pid_alive(meta.pid) {
            return true;
        }

        false
    }

    /// Return the path to the lock file.
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// Check whether this lease still owns the lock file.
    fn owns_lock(&self) -> bool {
        let content = match fs::read_to_string(&self.lock_path) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let meta: LeaseMeta = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(_) => return false,
        };
        meta.pid == self.pid
    }

    /// Internal release logic.
    fn do_release(&self) {
        if self.owns_lock() {
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

impl Drop for WriterLease {
    fn drop(&mut self) {
        if !self.released {
            self.do_release();
            self.released = true;
        }
    }
}

impl std::fmt::Debug for WriterLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriterLease")
            .field("lock_path", &self.lock_path)
            .field("pid", &self.pid)
            .field("released", &self.released)
            .finish()
    }
}

// ---- Helper functions ----

/// Compute the lock file path for a store path.
fn lock_path_for(store_path: &Path) -> PathBuf {
    let mut p = store_path.as_os_str().to_os_string();
    p.push(".lock");
    PathBuf::from(p)
}

/// Try to atomically create the lock file. Fails with `AlreadyExists` if
/// another process holds the lock.
fn try_create_lock(lock_path: &Path, pid: u32) -> io::Result<()> {
    // Ensure parent directory exists.
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Use create_new for O_CREAT | O_EXCL semantics.
    let meta = LeaseMeta {
        pid,
        timestamp_secs: current_unix_secs(),
        hostname: get_hostname(),
    };
    let content = serde_json::to_string(&meta)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("serialize lease meta: {e}")))?;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Overwrite an existing lock file with a fresh timestamp.
fn write_lock_file(lock_path: &Path, pid: u32) -> io::Result<()> {
    let meta = LeaseMeta {
        pid,
        timestamp_secs: current_unix_secs(),
        hostname: get_hostname(),
    };
    let content = serde_json::to_string(&meta)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("serialize lease meta: {e}")))?;
    fs::write(lock_path, content.as_bytes())
}

/// Get the current Unix timestamp in seconds.
fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Best-effort hostname retrieval.
fn get_hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| {
        fs::read_to_string("/etc/hostname")
            .unwrap_or_else(|_| "unknown".into())
            .trim()
            .to_string()
    })
}

/// Check whether a process with the given PID is alive.
fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // kill(pid, 0) checks existence without sending a signal.
        let ret = unsafe { libc_kill(pid as i32, 0) };
        if ret == 0 {
            return true;
        }
        // EPERM means the process exists but belongs to another user.
        let errno = unsafe { *errno_location() };
        errno == 1 // EPERM
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true // Conservatively assume alive on non-Unix.
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

#[cfg(unix)]
unsafe fn libc_kill(pid: i32, sig: i32) -> i32 {
    unsafe { kill(pid, sig) }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn errno_location() -> *mut i32 {
    unsafe { __errno_location() }
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
unsafe fn errno_location() -> *mut i32 {
    unsafe { __error() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

    /// Counter to generate unique directory names for each test, avoiding
    /// cross-test interference when running in parallel.
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_dir(name: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "rvlite_lease_{}_{}_{}",
            std::process::id(),
            id,
            name
        ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn lock_path_computation() {
        let p = Path::new("/tmp/store.rvf");
        assert_eq!(lock_path_for(p), PathBuf::from("/tmp/store.rvf.lock"));
    }

    #[test]
    fn acquire_and_release() {
        let dir = unique_dir("acquire_release");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");

        let mut lease = WriterLease::acquire(&store_path, Duration::from_secs(1)).unwrap();
        assert!(lease.lock_path().exists());

        lease.release().unwrap();
        assert!(!lease.lock_path().exists());

        cleanup(&dir);
    }

    #[test]
    fn double_acquire_fails_within_timeout() {
        let dir = unique_dir("double_acquire");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");

        let _lease = WriterLease::acquire(&store_path, Duration::from_secs(1)).unwrap();

        // Second acquire should time out quickly. The lock is held by our own
        // PID and is fresh, so it cannot be broken as stale.
        let result = WriterLease::acquire(&store_path, Duration::from_millis(150));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);

        cleanup(&dir);
    }

    #[test]
    fn drop_releases_lease() {
        let dir = unique_dir("drop_release");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");

        let lock_file = lock_path_for(&store_path);

        {
            let _lease = WriterLease::acquire(&store_path, Duration::from_secs(1)).unwrap();
            assert!(lock_file.exists());
        }
        // After drop, lock file should be gone.
        assert!(!lock_file.exists());

        cleanup(&dir);
    }

    #[test]
    fn stale_lease_is_detected() {
        let dir = unique_dir("stale_detect");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");
        let lock_path = lock_path_for(&store_path);

        // Write a lock file with a very old timestamp and dead PID.
        let meta = LeaseMeta {
            pid: 999_999_999, // Almost certainly not alive.
            timestamp_secs: current_unix_secs().saturating_sub(120),
            hostname: get_hostname(),
        };
        let content = serde_json::to_string(&meta).unwrap();
        fs::write(&lock_path, content).unwrap();

        assert!(WriterLease::is_stale(&store_path, DEFAULT_STALE_THRESHOLD));

        cleanup(&dir);
    }

    #[test]
    fn fresh_lease_is_not_stale() {
        let dir = unique_dir("fresh_lease");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");

        let _lease = WriterLease::acquire(&store_path, Duration::from_secs(1)).unwrap();

        assert!(!WriterLease::is_stale(&store_path, DEFAULT_STALE_THRESHOLD));

        cleanup(&dir);
    }

    #[test]
    fn missing_lock_file_is_stale() {
        let path = Path::new("/tmp/nonexistent_rvlite_test_12345.rvf");
        assert!(WriterLease::is_stale(path, DEFAULT_STALE_THRESHOLD));
    }

    #[test]
    fn corrupt_lock_file_is_stale() {
        let dir = unique_dir("corrupt");
        let store_path = dir.join("test.rvf");
        let lock_path = lock_path_for(&store_path);

        let _ = fs::create_dir_all(&dir);
        fs::write(&lock_path, b"not json").unwrap();
        assert!(WriterLease::is_stale(&store_path, DEFAULT_STALE_THRESHOLD));

        cleanup(&dir);
    }

    #[test]
    fn refresh_heartbeat_updates_timestamp() {
        let dir = unique_dir("heartbeat");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");

        let lease = WriterLease::acquire(&store_path, Duration::from_secs(1)).unwrap();

        // refresh_heartbeat overwrites the lock file with a new timestamp.
        lease.refresh_heartbeat().unwrap();

        // Read back and verify timestamp is recent.
        let content = fs::read_to_string(lease.lock_path()).unwrap();
        let meta: LeaseMeta = serde_json::from_str(&content).unwrap();
        let age = current_unix_secs().saturating_sub(meta.timestamp_secs);
        assert!(age < 5, "heartbeat should be very recent, got age={age}s");

        cleanup(&dir);
    }

    #[test]
    fn stale_lease_force_acquire() {
        let dir = unique_dir("force_acquire");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");
        let lock_path = lock_path_for(&store_path);

        // Simulate a stale lock from a dead process.
        let meta = LeaseMeta {
            pid: 999_999_999,
            timestamp_secs: current_unix_secs().saturating_sub(60),
            hostname: get_hostname(),
        };
        fs::write(&lock_path, serde_json::to_string(&meta).unwrap()).unwrap();

        // Should succeed because the existing lock is stale.
        let mut lease = WriterLease::acquire(&store_path, Duration::from_secs(1)).unwrap();
        assert_eq!(lease.pid, std::process::id());

        lease.release().unwrap();
        cleanup(&dir);
    }

    #[test]
    fn release_is_idempotent() {
        let dir = unique_dir("idempotent");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");

        let mut lease = WriterLease::acquire(&store_path, Duration::from_secs(1)).unwrap();
        lease.release().unwrap();
        // Second release should be a no-op.
        lease.release().unwrap();

        cleanup(&dir);
    }

    #[test]
    fn debug_format() {
        let dir = unique_dir("debug_fmt");
        let store_path = dir.join("test.rvf");
        let _ = fs::write(&store_path, b"");

        let lease = WriterLease::acquire(&store_path, Duration::from_secs(1)).unwrap();
        let debug = format!("{:?}", lease);
        assert!(debug.contains("WriterLease"));
        assert!(debug.contains("lock_path"));

        cleanup(&dir);
    }
}
