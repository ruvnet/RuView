//! The cross-process lock around the refresh critical section.
//!
//! `File::lock` (`flock` on Unix, `LockFileEx` on Windows) on
//! `credentials.lock`, a file that is never deleted, so every process locks the
//! same inode, and the OS releases the lock when its holder exits or dies.
//! There is no staleness guess and no takeover: a lock that removes a "stale"
//! path can let two takers both hold it (review-393 reproduced that on the
//! console CLI's directory lock).
//!
//! A waiter that cannot get the lock within its wait does NOT proceed: it
//! reports busy and the caller tries again later (cognitum-one/console#857
//! revokes the whole family when a refresh token is presented twice, so
//! proceeding without the lock is the outcome the lock exists to stop).

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::Duration;

pub struct FileLock {
    // Closing the file releases the lock.
    _file: File,
}

fn open(credentials_path: &Path) -> Option<File> {
    let path = credentials_path.with_extension("lock");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&path).ok()
}

impl FileLock {
    /// `None` if another process (or another handle in this one) holds it, or
    /// if the lock cannot be taken at all (fail closed). Never blocks.
    pub fn try_acquire(credentials_path: &Path) -> Option<Self> {
        let file = open(credentials_path)?;
        match file.try_lock() {
            Ok(()) => Some(Self { _file: file }),
            Err(TryLockError::WouldBlock) | Err(TryLockError::Error(_)) => None,
        }
    }

    /// Waits up to `wait` for the lock, polling without blocking the executor.
    pub async fn acquire(credentials_path: &Path, wait: Duration) -> Option<Self> {
        let file = open(credentials_path)?;
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            match file.try_lock() {
                Ok(()) => return Some(Self { _file: file }),
                Err(TryLockError::WouldBlock) => {}
                Err(TryLockError::Error(_)) => return None,
            }
            if tokio::time::Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ruview-auth-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("credentials.json")
    }

    #[test]
    fn the_credential_lock_is_exclusive_and_non_blocking() {
        // Guards the cross-process race: two CLI invocations inside the refresh
        // window used to be able to present the same rotating refresh token,
        // which identity treats as replay and answers by revoking the session.
        let path = scratch("lock");
        let first = FileLock::try_acquire(&path).expect("first acquire succeeds");
        assert!(
            FileLock::try_acquire(&path).is_none(),
            "a second holder must be refused, and refused WITHOUT blocking"
        );
        drop(first);
        // A short wait, not try_acquire: a child another test is spawning right now
        // holds a forked copy of every descriptor until it execs, which delays (but
        // never duplicates) the release.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        assert!(
            rt.block_on(FileLock::acquire(&path, Duration::from_secs(5)))
                .is_some(),
            "the lock must be released on drop"
        );
        assert!(
            path.with_extension("lock").exists(),
            "the lock file is never unlinked"
        );
    }

    #[tokio::test]
    async fn a_waiter_gives_up_rather_than_proceeding_while_the_lock_is_held() {
        let path = scratch("wait");
        let held = FileLock::try_acquire(&path).unwrap();
        assert!(FileLock::acquire(&path, Duration::from_millis(200))
            .await
            .is_none());
        drop(held);
        assert!(FileLock::acquire(&path, Duration::from_millis(200))
            .await
            .is_some());
    }

    #[test]
    fn many_threads_never_hold_the_lock_at_once() {
        let path = scratch("threads");
        let (inside, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let (inside, most, path) = (inside.clone(), most.clone(), path.clone());
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build()
                        .unwrap();
                    for _ in 0..25 {
                        let lock = rt
                            .block_on(FileLock::acquire(&path, Duration::from_secs(30)))
                            .expect("acquired");
                        let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                        most.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(1));
                        inside.fetch_sub(1, Ordering::SeqCst);
                        drop(lock);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(most.load(Ordering::SeqCst), 1, "never more than one holder");
    }
}
