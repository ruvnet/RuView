//! Opt-in lifetime boundary for an app-spawned helper, never port/PID discovery.
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

/// Keep this guard until server shutdown and final persistence have completed.
pub struct ParentLifetimeGuard(Arc<AtomicBool>);

impl Drop for ParentLifetimeGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[cfg(unix)]
fn current_parent() -> u32 {
    extern "C" {
        fn getppid() -> std::os::raw::c_int;
    }
    // getppid has no arguments, allocation, or borrowed memory.
    unsafe { getppid() as u32 }
}

/// Refuse a wrong/already-detached parent before opening listeners or records.
/// A native thread still observes parent death if the async runtime is blocked.
pub fn arm(
    expected_parent: Option<u32>,
) -> io::Result<(ParentLifetimeGuard, Option<oneshot::Receiver<()>>)> {
    let stopped = Arc::new(AtomicBool::new(false));
    let guard = ParentLifetimeGuard(stopped.clone());
    let Some(expected_parent) = expected_parent else {
        return Ok((guard, None));
    };
    #[cfg(not(unix))]
    {
        let _ = expected_parent;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "managed parent lifetime is unsupported on this platform",
        ))
    }
    #[cfg(unix)]
    {
        if expected_parent <= 1 || current_parent() != expected_parent {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "managed parent does not match the spawning process",
            ));
        }
        let (sender, receiver) = oneshot::channel();
        std::thread::Builder::new()
            .name("managed-parent".into())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    // Reparenting changes getppid even if the old PID is reused.
                    if current_parent() != expected_parent {
                        let _ = sender.send(());
                        let deadline = Instant::now() + Duration::from_secs(2);
                        while !stopped.load(Ordering::Acquire) && Instant::now() < deadline {
                            std::thread::sleep(Duration::from_millis(25));
                        }
                        if !stopped.load(Ordering::Acquire) {
                            // Long-lived WebSockets must not keep former account
                            // authority listening after its owning app disappeared.
                        eprintln!("Managed parent ended; helper shutdown deadline reached.");
                            std::process::exit(0);
                        }
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            })?;
        Ok((guard, Some(receiver)))
    }
}

pub async fn wait_for_exit(receiver: Option<oneshot::Receiver<()>>) {
    if let Some(receiver) = receiver {
        let _ = receiver.await;
    } else {
        std::future::pending::<()>().await;
    }
}
