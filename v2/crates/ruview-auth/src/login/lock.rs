//! The cross-process lock around the refresh critical section.
//!
//! Unix: an advisory `flock` on `credentials.lock`, released by the kernel if
//! the holder dies. Elsewhere: a `credentials.lock.d` directory, created
//! atomically; one older than [`LOCK_STALE`] was left by a process that died
//! holding it. Either way a waiter that cannot get the lock within its wait
//! does NOT proceed: it reports busy and the caller tries again later
//! (console#857 revokes the whole family when a refresh token is presented
//! twice, so proceeding without the lock is the outcome the lock exists to stop).

use std::path::Path;
#[cfg(not(unix))]
use std::path::PathBuf;
use std::time::Duration;

/// A lock directory older than this has no live holder. The holder's work is
/// bounded by the refresh request timeout, far below this. Non-Unix only.
#[cfg_attr(unix, allow(dead_code))]
pub const LOCK_STALE: Duration = Duration::from_secs(120);

pub struct FileLock {
    #[cfg(unix)]
    file: std::fs::File,
    #[cfg(not(unix))]
    dir: PathBuf,
}

impl FileLock {
    /// `None` if another process holds it. Never blocks.
    #[cfg(unix)]
    pub fn try_acquire(credentials_path: &Path) -> Option<Self> {
        use std::os::unix::io::AsRawFd;
        let path = credentials_path.with_extension("lock");
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .ok()?;
        // LOCK_EX | LOCK_NB
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            Some(Self { file })
        } else {
            None
        }
    }

    /// `None` if another process holds it. Never blocks.
    #[cfg(not(unix))]
    pub fn try_acquire(credentials_path: &Path) -> Option<Self> {
        let dir: PathBuf = credentials_path.with_extension("lock.d");
        if let Some(parent) = dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::create_dir(&dir) {
            Ok(()) => Some(Self { dir }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = std::fs::metadata(&dir)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| std::time::SystemTime::now().duration_since(t).ok())
                    .is_some_and(|age| age > LOCK_STALE);
                if stale && std::fs::remove_dir(&dir).is_ok() && std::fs::create_dir(&dir).is_ok() {
                    return Some(Self { dir });
                }
                None
            }
            // Cannot tell whether anyone holds it: fail closed.
            Err(_) => None,
        }
    }

    /// Waits up to `wait` for the lock, polling without blocking the executor.
    pub async fn acquire(credentials_path: &Path, wait: Duration) -> Option<Self> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            if let Some(lock) = Self::try_acquire(credentials_path) {
                return Some(lock);
            }
            if tokio::time::Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            // Released on close anyway; explicit so the intent is legible.
            unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
        #[cfg(not(unix))]
        {
            let _ = std::fs::remove_dir(&self.dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_credential_lock_is_exclusive_and_non_blocking() {
        // Guards the cross-process race: two CLI invocations inside the refresh
        // window used to be able to present the same rotating refresh token,
        // which identity treats as replay and answers by revoking the session.
        let dir = std::env::temp_dir().join(format!("ruview-auth-lock-{}", std::process::id()));
        let path = dir.join("credentials.json");
        std::fs::create_dir_all(&dir).unwrap();

        let first = FileLock::try_acquire(&path).expect("first acquire succeeds");
        assert!(
            FileLock::try_acquire(&path).is_none(),
            "a second holder must be refused, and refused WITHOUT blocking"
        );
        drop(first);
        assert!(
            FileLock::try_acquire(&path).is_some(),
            "the lock must be released on drop"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_waiter_gives_up_rather_than_proceeding_while_the_lock_is_held() {
        let dir = std::env::temp_dir().join(format!("ruview-auth-wait-{}", std::process::id()));
        let path = dir.join("credentials.json");
        std::fs::create_dir_all(&dir).unwrap();
        let held = FileLock::try_acquire(&path).unwrap();
        assert!(FileLock::acquire(&path, Duration::from_millis(200))
            .await
            .is_none());
        drop(held);
        assert!(FileLock::acquire(&path, Duration::from_millis(200))
            .await
            .is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
