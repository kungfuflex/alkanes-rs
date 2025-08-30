use std::time::{Duration, Instant as StdInstant};

#[cfg(test)]
use parking_lot::RwLock;

// WASM-specific imports
#[cfg(target_arch = "wasm32")]
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

// This is `moka`'s `Instant` struct.
use super::Instant;

#[derive(Default, Clone)]
pub(crate) struct Clock {
    ty: ClockType,
}

#[derive(Clone)]
enum ClockType {
    /// A clock that uses `std::time::Instant` as the source of time.
    #[cfg(not(target_arch = "wasm32"))]
    Standard { origin: StdInstant },
    #[cfg(feature = "quanta")]
    /// A clock that uses both `std::time::Instant` and `quanta::Instant` as the
    /// sources of time.
    Hybrid {
        std_origin: StdInstant,
        quanta_origin: quanta::Instant,
    },
    #[cfg(target_arch = "wasm32")]
    /// A clock that works in WASM environments using atomic counters instead of std::time::Instant.
    /// This provides ordering guarantees without relying on system time.
    WasmCompatible {
        start_counter: Arc<AtomicU64>,
        counter: Arc<AtomicU64>,
    },
    #[cfg(test)]
    /// A clock that uses a mocked source of time.
    Mocked { mock: Arc<Mock> },
}

impl Default for ClockType {
    /// Create a new `ClockType` with the current time as the origin.
    ///
    /// For WASM targets, `WasmCompatible` will be used with atomic counters.
    /// For non-WASM targets, if the `quanta` feature is enabled, `Hybrid` will be used.
    /// Otherwise, `Standard` will be used.
    fn default() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            return ClockType::WasmCompatible {
                start_counter: Arc::new(AtomicU64::new(0)),
                counter: Arc::new(AtomicU64::new(0)),
            };
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            #[cfg(feature = "quanta")]
            {
                return ClockType::Hybrid {
                    std_origin: StdInstant::now(),
                    quanta_origin: quanta::Instant::now(),
                };
            }

            #[allow(unreachable_code)]
            ClockType::Standard {
                origin: StdInstant::now(),
            }
        }
    }
}

impl Clock {
    #[cfg(test)]
    /// Creates a new `Clock` with a mocked source of time.
    pub(crate) fn mock() -> (Clock, Arc<Mock>) {
        let mock = Arc::new(Mock::default());
        let clock = Clock {
            ty: ClockType::Mocked {
                mock: Arc::clone(&mock),
            },
        };
        (clock, mock)
    }

    /// Returns the current time using a reliable source of time.
    ///
    /// When the the type is `Standard` or `Hybrid`, the time is based on
    /// `std::time::Instant`. When the type is `Mocked`, the time is based on the
    /// mocked source of time. When the type is `WasmCompatible`, the time is based
    /// on an atomic counter that increments on each call.
    pub(crate) fn now(&self) -> Instant {
        match &self.ty {
            #[cfg(not(target_arch = "wasm32"))]
            ClockType::Standard { origin } => {
                Instant::from_duration_since_clock_start(origin.elapsed())
            }
            #[cfg(feature = "quanta")]
            ClockType::Hybrid { std_origin, .. } => {
                Instant::from_duration_since_clock_start(std_origin.elapsed())
            }
            #[cfg(target_arch = "wasm32")]
            ClockType::WasmCompatible { counter, .. } => {
                // Increment counter and use it as nanoseconds
                // This provides ordering guarantees without real time
                let count = counter.fetch_add(1000, Ordering::SeqCst); // Increment by 1000ns (1μs) per call
                Instant::from_duration_since_clock_start(Duration::from_nanos(count))
            }
            #[cfg(test)]
            ClockType::Mocked { mock } => Instant::from_duration_since_clock_start(mock.elapsed()),
        }
    }

    /// Returns the current time _maybe_ using a fast but less reliable source of
    /// time. The time may drift from the time returned by `now`, or not be
    /// monotonically increasing.
    ///
    /// This is useful for performance critical code that does not require the same
    /// level of precision as `now`. (e.g. measuring the time between two events for
    /// metrics)
    ///
    /// When the type is `Standard`, `Mocked`, or `WasmCompatible`, `now` is internally called.
    /// So there is no performance benefit.
    ///
    /// When the type is `Hybrid`, the time is based on `quanta::Instant`, which can
    /// be faster than `std::time::Instant`, depending on the CPU architecture.
    pub(crate) fn fast_now(&self) -> Instant {
        match &self.ty {
            #[cfg(feature = "quanta")]
            ClockType::Hybrid { quanta_origin, .. } => {
                Instant::from_duration_since_clock_start(quanta_origin.elapsed())
            }
            #[cfg(not(target_arch = "wasm32"))]
            ClockType::Standard { .. } => self.now(),
            #[cfg(target_arch = "wasm32")]
            ClockType::WasmCompatible { .. } => self.now(),
            #[cfg(test)]
            ClockType::Mocked { .. } => self.now(),
        }
    }

    /// Converts an `Instant` to a `std::time::Instant`.
    ///
    /// **IMPORTANT**: The caller must ensure that the `Instant` was created by this
    /// `Clock`, otherwise the resulting `std::time::Instant` will be incorrect.
    ///
    /// **Note**: For WASM targets, this will panic since `std::time::Instant` is not available.
    /// This method should not be called in WASM environments.
    pub(crate) fn to_std_instant(&self, instant: Instant) -> StdInstant {
        match &self.ty {
            #[cfg(not(target_arch = "wasm32"))]
            ClockType::Standard { origin } => {
                let duration = Duration::from_nanos(instant.as_nanos());
                *origin + duration
            }
            #[cfg(feature = "quanta")]
            ClockType::Hybrid { std_origin, .. } => {
                let duration = Duration::from_nanos(instant.as_nanos());
                *std_origin + duration
            }
            #[cfg(target_arch = "wasm32")]
            ClockType::WasmCompatible { start_counter, .. } => {
                // In WASM environments, we create a synthetic StdInstant based on our tick system
                // We use a fixed origin point and add the duration from our atomic counter
                let nanos = instant.as_nanos();
                let duration = Duration::from_nanos(nanos);
                
                // Create a synthetic origin by using the start_counter value
                // This ensures all StdInstants are relative to the same base point
                let origin_nanos = start_counter.load(Ordering::SeqCst);
                let origin_duration = Duration::from_nanos(origin_nanos);
                
                // Create a synthetic StdInstant by using a fixed base time and adding our durations
                // We'll use a deterministic base time that's far enough in the past to avoid issues
                let base_time = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000); // Year ~2001
                let synthetic_system_time = base_time + origin_duration + duration;
                
                // Convert SystemTime to Instant using the current time as reference
                // This is a bit of a hack, but it creates a valid Instant for Moka's use
                let now_system = std::time::SystemTime::now();
                let now_instant = std::time::Instant::now();
                
                // Calculate the offset between our synthetic time and current system time
                match synthetic_system_time.duration_since(now_system) {
                    Ok(future_duration) => now_instant + future_duration,
                    Err(past_error) => {
                        let past_duration = past_error.duration();
                        now_instant.checked_sub(past_duration).unwrap_or(now_instant)
                    }
                }
            }
            #[cfg(test)]
            ClockType::Mocked { mock } => {
                let duration = Duration::from_nanos(instant.as_nanos());
                // https://github.com/moka-rs/moka/issues/487
                //
                // This `dbg!` will workaround an incorrect compilation by Rust
                // 1.84.0 for the armv7-unknown-linux-musleabihf target in the
                // release build of the tests.
                dbg!(mock.origin + duration)
            }
        }
    }
}

#[cfg(test)]
pub(crate) struct Mock {
    origin: StdInstant,
    now: RwLock<StdInstant>,
}

#[cfg(test)]
impl Default for Mock {
    fn default() -> Self {
        let origin = StdInstant::now();
        Self {
            origin,
            now: RwLock::new(origin),
        }
    }
}

#[cfg(test)]
impl Mock {
    pub(crate) fn increment(&self, amount: Duration) {
        *self.now.write() += amount;
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.now.read().duration_since(self.origin)
    }
}
