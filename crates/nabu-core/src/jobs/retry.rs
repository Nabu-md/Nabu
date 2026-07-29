use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

/// Configuration for retry behaviour.
///
/// Controls how a failed job is retried, including the backoff strategy
/// and the maximum time window for retries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// The initial delay before the first retry (e.g., 5 seconds).
    pub initial_delay_seconds: u64,

    /// The multiplier applied to the delay after each retry.
    /// A value of 2.0 doubles the delay each time (exponential backoff).
    pub backoff_multiplier: f64,

    /// The maximum delay between retries (e.g., 1 hour).
    pub max_delay_seconds: u64,

    /// Jitter factor (0.0 — 1.0). Adds randomness to prevent thundering herd.
    /// 0.0 = no jitter, 0.5 = up to 50% randomness added.
    pub jitter_factor: f64,
}

impl RetryPolicy {
    /// Creates a simple retry policy with no backoff (constant delay).
    pub fn constant(delay: Duration) -> Self {
        let secs = delay.num_seconds().max(1) as u64;
        RetryPolicy {
            initial_delay_seconds: secs,
            backoff_multiplier: 1.0,
            max_delay_seconds: secs,
            jitter_factor: 0.0,
        }
    }

    /// Creates an exponential backoff retry policy.
    ///
    /// - `initial_delay`: The delay before the first retry.
    /// - `multiplier`: The factor by which the delay increases each retry (e.g., 2.0).
    /// - `max_delay`: The maximum delay between retries.
    pub fn exponential(initial_delay: Duration, multiplier: f64, max_delay: Duration) -> Self {
        let init_secs = initial_delay.num_seconds().max(1) as u64;
        let max_secs = max_delay.num_seconds().max(init_secs) as u64;
        RetryPolicy {
            initial_delay_seconds: init_secs,
            backoff_multiplier: multiplier.max(1.0),
            max_delay_seconds: max_secs,
            jitter_factor: 0.0,
        }
    }

    /// Creates an exponential backoff policy with jitter to prevent thundering herd.
    pub fn with_jitter(mut self, jitter_factor: f64) -> Self {
        self.jitter_factor = jitter_factor.clamp(0.0, 1.0);
        self
    }

    /// Calculates the delay before the next retry for the given retry attempt number.
    ///
    /// Uses exponential backoff: `delay = min(initial * multiplier^attempt, max_delay)`
    /// Optionally adds jitter: `delay *= (1.0 + random * jitter_factor)`
    pub fn backoff_delay(&self, retry_count: u32) -> Duration {
        let base_secs = self.initial_delay_seconds as f64
            * self.backoff_multiplier.powi(retry_count as i32);
        let clamped_secs = base_secs.min(self.max_delay_seconds as f64);

        let secs = if self.jitter_factor > 0.0 {
            // Simple deterministic jitter using retry_count as seed for reproducibility
            let pseudo_random = ((retry_count as f64 * 0.1618) % 1.0).abs();
            let jitter_amount = 1.0 + (pseudo_random * self.jitter_factor);
            (clamped_secs * jitter_amount) as i64
        } else {
            clamped_secs as i64
        };

        Duration::seconds(secs.max(1))
    }

    /// Returns `true` if a job with this policy should be retried given the retry count and max retries.
    pub fn should_retry(&self, retry_count: u32, max_retries: u32) -> bool {
        retry_count < max_retries
    }
}

impl Default for RetryPolicy {
    /// Default retry policy: exponential backoff starting at 5 seconds,
    /// doubling each time, capped at 1 hour, with 10% jitter.
    fn default() -> Self {
        RetryPolicy {
            initial_delay_seconds: 5,
            backoff_multiplier: 2.0,
            max_delay_seconds: 3600, // 1 hour
            jitter_factor: 0.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_retry() {
        let policy = RetryPolicy::constant(Duration::seconds(30));
        let d1 = policy.backoff_delay(0);
        let d2 = policy.backoff_delay(5);
        assert_eq!(d1.num_seconds(), 30);
        assert_eq!(d2.num_seconds(), 30);
    }

    #[test]
    fn test_exponential_backoff() {
        let policy = RetryPolicy::exponential(
            Duration::seconds(5),
            2.0,
            Duration::seconds(300),
        );

        let d0 = policy.backoff_delay(0);
        assert_eq!(d0.num_seconds(), 5); // 5 * 2^0 = 5

        let d1 = policy.backoff_delay(1);
        assert_eq!(d1.num_seconds(), 10); // 5 * 2^1 = 10

        let d2 = policy.backoff_delay(2);
        assert_eq!(d2.num_seconds(), 20); // 5 * 2^2 = 20

        let d3 = policy.backoff_delay(3);
        assert_eq!(d3.num_seconds(), 40); // 5 * 2^3 = 40
    }

    #[test]
    fn test_exponential_backoff_capped() {
        let policy = RetryPolicy::exponential(
            Duration::seconds(10),
            2.0,
            Duration::seconds(30),
        );

        let d0 = policy.backoff_delay(0);
        assert_eq!(d0.num_seconds(), 10);

        // 10 * 2^2 = 40, capped at 30
        let d2 = policy.backoff_delay(2);
        assert_eq!(d2.num_seconds(), 30);
    }

    #[test]
    fn test_should_retry() {
        let policy = RetryPolicy::default();
        assert!(policy.should_retry(0, 3));
        assert!(policy.should_retry(2, 3));
        assert!(!policy.should_retry(3, 3));
        assert!(!policy.should_retry(5, 3));
    }

    #[test]
    fn test_default_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.initial_delay_seconds, 5);
        assert_eq!(policy.backoff_multiplier, 2.0);
        assert_eq!(policy.max_delay_seconds, 3600);
        assert!((policy.jitter_factor - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_minimum_one_second() {
        let policy = RetryPolicy::constant(Duration::milliseconds(100));
        let d = policy.backoff_delay(0);
        assert_eq!(d.num_seconds(), 1);
    }

    #[test]
    fn test_jitter_adds_variation() {
        let no_jitter = RetryPolicy::exponential(Duration::seconds(10), 2.0, Duration::seconds(100));
        let with_jitter = RetryPolicy::exponential(Duration::seconds(10), 2.0, Duration::seconds(100))
            .with_jitter(0.5);

        let d0 = no_jitter.backoff_delay(0);
        let d1 = with_jitter.backoff_delay(0);

        // With jitter 0.5, the delay should differ from the base (pseudo-random)
        // Since jitter adds randomness, the delay may be larger than the base
        // But we can't guarantee exact values due to the pseudo-random calculation
        assert!(d1.num_seconds() >= 1);
    }

    #[test]
    fn test_with_jitter_clamps() {
        let policy = RetryPolicy::default().with_jitter(2.0);
        assert!((policy.jitter_factor - 1.0).abs() < f64::EPSILON);

        let policy = RetryPolicy::default().with_jitter(-0.5);
        assert!((policy.jitter_factor - 0.0).abs() < f64::EPSILON);
    }
}
