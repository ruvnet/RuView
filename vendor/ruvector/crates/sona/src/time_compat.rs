//! Cross-platform time abstraction for native and WASM targets.
//!
//! Uses `std::time::Instant` on native platforms and `performance.now()` on WASM.
//! Uses `std::time::SystemTime` on native platforms and `Date.now()` on WASM.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::fmt;
    use std::time::{Duration, Instant as StdInstant, SystemTime as StdSystemTime, UNIX_EPOCH};

    #[derive(Clone, Copy)]
    pub struct Instant(StdInstant);

    impl Instant {
        pub fn now() -> Self {
            Instant(StdInstant::now())
        }

        pub fn elapsed(&self) -> Duration {
            self.0.elapsed()
        }
    }

    impl Default for Instant {
        fn default() -> Self {
            Self::now()
        }
    }

    impl fmt::Debug for Instant {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }

    #[derive(Clone, Copy)]
    pub struct SystemTime(StdSystemTime);

    impl SystemTime {
        pub fn now() -> Self {
            SystemTime(StdSystemTime::now())
        }

        pub fn duration_since_epoch(&self) -> Duration {
            self.0.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO)
        }
    }

    impl fmt::Debug for SystemTime {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::fmt;
    use std::time::Duration;

    fn performance_now() -> f64 {
        #[cfg(feature = "wasm")]
        {
            use wasm_bindgen::JsCast;
            js_sys::Reflect::get(&js_sys::global(), &"performance".into())
                .ok()
                .and_then(|p| p.dyn_into::<web_sys::Performance>().ok())
                .map(|p| p.now())
                .unwrap_or(0.0)
        }
        #[cfg(not(feature = "wasm"))]
        {
            0.0
        }
    }

    fn date_now() -> f64 {
        #[cfg(feature = "wasm")]
        {
            js_sys::Date::now()
        }
        #[cfg(not(feature = "wasm"))]
        {
            0.0
        }
    }

    #[derive(Clone, Copy)]
    pub struct Instant(f64);

    impl Instant {
        pub fn now() -> Self {
            Instant(performance_now())
        }

        pub fn elapsed(&self) -> Duration {
            let now = performance_now();
            let elapsed_ms = (now - self.0).max(0.0);
            Duration::from_secs_f64(elapsed_ms / 1000.0)
        }
    }

    impl Default for Instant {
        fn default() -> Self {
            Self::now()
        }
    }

    impl fmt::Debug for Instant {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Instant({}ms)", self.0)
        }
    }

    #[derive(Clone, Copy)]
    pub struct SystemTime(f64);

    impl SystemTime {
        pub fn now() -> Self {
            SystemTime(date_now())
        }

        pub fn duration_since_epoch(&self) -> Duration {
            Duration::from_secs_f64(self.0 / 1000.0)
        }
    }

    impl fmt::Debug for SystemTime {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "SystemTime({}ms)", self.0)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{Instant, SystemTime};

#[cfg(target_arch = "wasm32")]
pub use wasm::{Instant, SystemTime};
