//! Time utilities for Qubitcoin.
//!
//! Maps to: `src/util/time.h` and `src/util/time.cpp` in Bitcoin Core.
//!
//! Provides functions for getting the current time at various granularities,
//! formatting timestamps, and a `MockClock` for deterministic testing.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Get the current Unix timestamp in seconds.
///
/// Equivalent to Bitcoin Core's `GetTime()`.
pub fn get_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs()
}

/// Get the current time in milliseconds since the Unix epoch.
///
/// Equivalent to Bitcoin Core's `GetTimeMillis()`.
pub fn get_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis() as u64
}

/// Get the current time in microseconds since the Unix epoch.
///
/// Equivalent to Bitcoin Core's `GetTimeMicros()`.
pub fn get_time_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_micros() as u64
}

/// Format a Unix timestamp (seconds) as an ISO 8601 string.
///
/// Returns a string in the format `YYYY-MM-DDTHH:MM:SSZ`.
/// This is a simplified formatter that does not depend on the `chrono` crate.
pub fn format_iso8601(timestamp: u64) -> String {
    // Constants for date computation
    const SECONDS_PER_MINUTE: u64 = 60;
    const SECONDS_PER_HOUR: u64 = 3600;
    const SECONDS_PER_DAY: u64 = 86400;

    let mut remaining = timestamp;

    let secs = (remaining % SECONDS_PER_MINUTE) as u32;
    remaining /= SECONDS_PER_MINUTE;
    let mins = (remaining % 60) as u32;
    remaining /= 60;
    let hours = (remaining % 24) as u32;
    let mut days = remaining / 24;

    // Compute year, month, day from days since epoch (1970-01-01)
    // Using a civil calendar algorithm
    let mut year: i64 = 1970;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let days_in_months: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month = 0u32;
    for (i, &dim) in days_in_months.iter().enumerate() {
        if days < dim {
            month = i as u32 + 1;
            break;
        }
        days -= dim;
    }

    let day = days as u32 + 1;

    // Suppress unused variable warnings
    let _ = SECONDS_PER_HOUR;
    let _ = SECONDS_PER_DAY;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, mins, secs
    )
}

/// Check if a year is a leap year.
fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// A mockable clock for deterministic testing.
///
/// Uses an atomic counter so it can be shared across threads safely.
/// The clock is initialized with a given time and can be advanced or set
/// to a specific value.
pub struct MockClock {
    time: AtomicU64,
}

impl MockClock {
    /// Create a new mock clock initialized to the given time (seconds).
    pub fn new(initial: u64) -> Self {
        MockClock {
            time: AtomicU64::new(initial),
        }
    }

    /// Get the current mock time (seconds).
    pub fn now(&self) -> u64 {
        self.time.load(Ordering::SeqCst)
    }

    /// Advance the mock clock by the given number of seconds.
    pub fn advance(&self, secs: u64) {
        self.time.fetch_add(secs, Ordering::SeqCst);
    }

    /// Set the mock clock to a specific time (seconds).
    pub fn set(&self, time: u64) {
        self.time.store(time, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_time_returns_reasonable_value() {
        let now = get_time();
        // Should be after 2020-01-01 (1577836800) and before 2100-01-01 (4102444800)
        assert!(now > 1_577_836_800, "Time should be after 2020");
        assert!(now < 4_102_444_800, "Time should be before 2100");
    }

    #[test]
    fn test_get_time_millis_greater_than_seconds() {
        let secs = get_time();
        let millis = get_time_millis();
        // millis should be roughly secs * 1000
        assert!(millis >= secs * 1000);
        assert!(millis < (secs + 2) * 1000); // within 2 seconds tolerance
    }

    #[test]
    fn test_get_time_micros_greater_than_millis() {
        let millis = get_time_millis();
        let micros = get_time_micros();
        // micros should be roughly millis * 1000
        assert!(micros >= millis * 1000);
        assert!(micros < (millis + 2000) * 1000); // within 2 seconds tolerance
    }

    #[test]
    fn test_format_iso8601_epoch() {
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_format_iso8601_known_date() {
        // 2009-01-03T18:15:05Z = Bitcoin genesis block timestamp = 1231006505
        assert_eq!(format_iso8601(1231006505), "2009-01-03T18:15:05Z");
    }

    #[test]
    fn test_format_iso8601_2024() {
        // 2024-01-01T00:00:00Z = 1704067200
        assert_eq!(format_iso8601(1704067200), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_format_iso8601_leap_year() {
        // 2000-02-29T00:00:00Z = 951782400
        assert_eq!(format_iso8601(951782400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn test_format_iso8601_end_of_year() {
        // 2023-12-31T23:59:59Z = 1704067199
        assert_eq!(format_iso8601(1704067199), "2023-12-31T23:59:59Z");
    }

    // --- MockClock tests ---

    #[test]
    fn test_mock_clock_new() {
        let clock = MockClock::new(1000);
        assert_eq!(clock.now(), 1000);
    }

    #[test]
    fn test_mock_clock_advance() {
        let clock = MockClock::new(1000);
        clock.advance(500);
        assert_eq!(clock.now(), 1500);
        clock.advance(100);
        assert_eq!(clock.now(), 1600);
    }

    #[test]
    fn test_mock_clock_set() {
        let clock = MockClock::new(1000);
        clock.set(5000);
        assert_eq!(clock.now(), 5000);
    }

    #[test]
    fn test_mock_clock_advance_zero() {
        let clock = MockClock::new(42);
        clock.advance(0);
        assert_eq!(clock.now(), 42);
    }

    #[test]
    fn test_mock_clock_thread_safe() {
        use std::sync::Arc;
        use std::thread;

        let clock = Arc::new(MockClock::new(0));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let clock = Arc::clone(&clock);
                thread::spawn(move || {
                    for _ in 0..100 {
                        clock.advance(1);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(clock.now(), 1000);
    }

    // --- is_leap_year tests ---

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap_year(2000)); // divisible by 400
        assert!(!is_leap_year(1900)); // divisible by 100 but not 400
        assert!(is_leap_year(2024)); // divisible by 4 but not 100
        assert!(!is_leap_year(2023)); // not divisible by 4
        assert!(is_leap_year(1972));
    }
}
