//! Retry/backoff helper for transient RPC failures.
//!
//! Modeled on `subfrost-mobile/crates/subfrost-mobile-api/src/upstream.rs::retry_transient`.
//! Three attempts total, 200ms → 500ms base sleeps with jitter. We retry on:
//!
//! * subfrost rate-limit (`IP_RATE_LIMIT`),
//! * generic 408/502/503/504,
//! * tlsfetch-style EAGAIN / "Resource temporarily unavailable",
//! * connection-reset / refused / timed-out strings,
//! * `gRPC code::Unavailable` (when wrapped),
//!
//! We do NOT retry: JSON-RPC method-not-found, parse errors, 4xx-other.
//!
//! The classifier is pluggable via [`RetryClassifier`] so callers with custom
//! error types (gRPC `Status`, anyhow chains) can plug in.

use core::time::Duration;

use crate::String;

/// Whether a failed attempt looks worth retrying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransientErrorClass {
    /// Try again after the standard backoff.
    Transient,
    /// Don't retry — propagate immediately.
    Permanent,
}

/// Pluggable error classifier. Implement for whatever error type the caller
/// uses. The default impl in this crate covers `crate::AlkanesError`.
pub trait RetryClassifier<E> {
    fn classify(error: &E) -> TransientErrorClass;
}

/// Default backoff schedule: attempt 1 immediate, then 200ms + jitter,
/// then 500ms + jitter. After 3 attempts we surrender.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delays_ms: [u64; 2],
    pub jitter_ms: [u64; 2],
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delays_ms: [200, 500],
            jitter_ms: [100, 200],
        }
    }
}

impl RetryPolicy {
    /// No retries — useful in tests that want to assert single-shot behavior.
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            base_delays_ms: [0, 0],
            jitter_ms: [0, 0],
        }
    }

    /// Compute the sleep before attempt `n` (1-indexed). Attempt 1 always
    /// sleeps zero; attempt 2 sleeps base[0]+rand(jitter[0]); attempt 3
    /// sleeps base[1]+rand(jitter[1]). Anything beyond uses the last entry.
    pub fn delay_for_attempt(&self, n: u32, jitter_seed: u64) -> Duration {
        if n <= 1 {
            return Duration::from_millis(0);
        }
        let idx = ((n - 2) as usize).min(self.base_delays_ms.len() - 1);
        let base = self.base_delays_ms[idx];
        let jitter_max = self.jitter_ms[idx.min(self.jitter_ms.len() - 1)];
        // Deterministic xorshift jitter; we don't need crypto quality.
        let mut s = jitter_seed.wrapping_add(n as u64);
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let jitter = if jitter_max == 0 { 0 } else { s % jitter_max };
        Duration::from_millis(base + jitter)
    }
}

/// Convenience: classify by string content. Used by the default classifier
/// when an error doesn't expose structured info (most JSON-RPC errors arrive
/// as `AlkanesError::JsonRpc(String)`).
pub fn classify_by_message(msg: &str) -> TransientErrorClass {
    // Lowercase once, scan for known transient markers.
    let m = msg.to_ascii_lowercase();
    const TRANSIENT_MARKERS: &[&str] = &[
        "ip_rate_limit",
        "rate limit",
        "rate-limit",
        "rate_limit_exceeded",
        "ratelimit",
        " 408 ",
        " 502 ",
        " 503 ",
        " 504 ",
        "gateway timeout",
        "bad gateway",
        "service unavailable",
        "request timeout",
        "resource temporarily unavailable",
        "eagain",
        "os error 11",
        "connection refused",
        "connection reset",
        "connection closed",
        "timed out",
        "timeout",
        "unavailable",
        "deadline exceeded",
    ];
    for marker in TRANSIENT_MARKERS {
        if m.contains(marker) {
            return TransientErrorClass::Transient;
        }
    }
    TransientErrorClass::Permanent
}

/// Default classifier for [`crate::AlkanesError`].
#[cfg(feature = "std")]
pub struct DefaultClassifier;

#[cfg(feature = "std")]
impl RetryClassifier<crate::AlkanesError> for DefaultClassifier {
    fn classify(error: &crate::AlkanesError) -> TransientErrorClass {
        use crate::AlkanesError as E;
        match error {
            E::JsonRpc(s) | E::RpcError(s) | E::Network(s) | E::Io(s) => classify_by_message(s),
            // Everything else is a logic bug or schema mismatch — don't burn retries on it.
            _ => TransientErrorClass::Permanent,
        }
    }
}

/// Run `op` with retries. `op` is a closure that produces a fresh future
/// each invocation (so the request can be reissued).
///
/// On a transient error we sleep per [`RetryPolicy`]; on a permanent error
/// we return immediately. If we run out of attempts we return the last
/// error verbatim.
///
/// The async sleep uses `tokio::time::sleep` because that's already a
/// transitive workspace dep; if you need wasm support, gate this behind a
/// runtime feature later.
#[cfg(feature = "std")]
pub async fn retry_transient<F, Fut, T, E, C>(
    label: &str,
    policy: RetryPolicy,
    mut op: F,
) -> core::result::Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: core::future::Future<Output = core::result::Result<T, E>>,
    E: core::fmt::Debug,
    C: RetryClassifier<E>,
{
    let mut last_err: Option<E> = None;
    for attempt in 1..=policy.max_attempts {
        if attempt > 1 {
            let seed = label.as_bytes().iter().fold(0u64, |acc, &b| {
                acc.wrapping_mul(31).wrapping_add(b as u64)
            });
            let sleep = policy.delay_for_attempt(attempt, seed);
            log::debug!(
                "retry_transient[{label}] attempt {attempt}/{} sleeping {sleep:?}",
                policy.max_attempts
            );
            tokio::time::sleep(sleep).await;
        }
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let class = C::classify(&e);
                if class == TransientErrorClass::Permanent {
                    log::debug!("retry_transient[{label}] permanent error, no retry: {e:?}");
                    return Err(e);
                }
                log::debug!(
                    "retry_transient[{label}] transient on attempt {attempt}: {e:?}"
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err.expect("loop ran at least once"))
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug)]
    struct StringErr(String);

    struct StringClassifier;
    impl RetryClassifier<StringErr> for StringClassifier {
        fn classify(e: &StringErr) -> TransientErrorClass {
            classify_by_message(&e.0)
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn retries_transient_then_succeeds() {
        let calls = AtomicU32::new(0);
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delays_ms: [1, 2],
            jitter_ms: [0, 0],
        };
        let result: Result<u32, StringErr> = retry_transient::<_, _, _, _, StringClassifier>(
            "test",
            policy,
            || {
                let prev = calls.fetch_add(1, Ordering::SeqCst);
                async move {
                    if prev < 2 {
                        Err(StringErr("IP_RATE_LIMIT".into()))
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn permanent_error_short_circuits() {
        let calls = AtomicU32::new(0);
        let policy = RetryPolicy::default();
        let result: Result<u32, StringErr> = retry_transient::<_, _, _, _, StringClassifier>(
            "test",
            policy,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err::<u32, _>(StringErr("Method not found".into())) }
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn surrenders_after_max_attempts() {
        let calls = AtomicU32::new(0);
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delays_ms: [1, 1],
            jitter_ms: [0, 0],
        };
        let result: Result<u32, StringErr> = retry_transient::<_, _, _, _, StringClassifier>(
            "test",
            policy,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err::<u32, _>(StringErr("503 service unavailable".into())) }
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn classifier_catches_rate_limit() {
        assert_eq!(
            classify_by_message("IP_RATE_LIMIT exceeded"),
            TransientErrorClass::Transient
        );
        assert_eq!(
            classify_by_message("Rate limit exceeded (20 req/min)"),
            TransientErrorClass::Transient
        );
        assert_eq!(
            classify_by_message("Method not found"),
            TransientErrorClass::Permanent
        );
        assert_eq!(
            classify_by_message("Invalid params"),
            TransientErrorClass::Permanent
        );
    }
}
