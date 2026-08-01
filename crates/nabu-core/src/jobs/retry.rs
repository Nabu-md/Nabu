use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Configurable retry policy for jobs.
/// Supports exponential backoff with jitter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_retries: u32,

    /// Base delay in seconds (first retry delay)
    pub base_delay_seconds: u64,

    /// Backoff multiplier (exponential: delay * multiplier^attempt)
    pub backoff_multiplier: f64,

    /// Maximum delay in seconds (cap the backoff)
    pub max_delay_seconds: u64,

    /// Whether to add random jitter to delay
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_seconds: 5,
            backoff_multiplier: 2.0,
            max_delay_seconds: 3600, // 1 hour
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Standard retry policy for user-facing operations.
    pub fn standard() -> Self {
        Self {
            max_retries: 3,
            base_delay_seconds: 10,
            backoff_multiplier: 2.0,
            max_delay_seconds: 300, // 5 minutes
            jitter: true,
        }
    }

    /// Aggressive retry policy for critical operations.
    pub fn aggressive() -> Self {
        Self {
            max_retries: 5,
            base_delay_seconds: 2,
            backoff_multiplier: 2.0,
            max_delay_seconds: 60, // 1 minute
            jitter: true,
        }
    }

    /// Patient retry policy for background operations.
    pub fn patient() -> Self {
        Self {
            max_retries: 10,
            base_delay_seconds: 30,
            backoff_multiplier: 3.0,
            max_delay_seconds: 86400, // 24 hours
            jitter: true,
        }
    }

    /// Never retry.
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            base_delay_seconds: 0,
            backoff_multiplier: 1.0,
            max_delay_seconds: 0,
            jitter: false,
        }
    }

    /// Calculate the delay before the next retry attempt.
    /// Returns the duration to wait before retrying.
    pub fn retry_delay(&self, attempt: u32) -> Duration {
        if attempt >= self.max_retries {
            return Duration::zero();
        }

        let base = self.base_delay_seconds as f64;
        let multiplier = self.backoff_multiplier.powi(attempt as i32);
        let mut delay_secs = (base * multiplier).min(self.max_delay_seconds as f64);

        if self.jitter {
            // Add ±25% jitter
            let jitter_amount = delay_secs * 0.25;
            // Use a deterministic simple jitter based on attempt
            let jitter_frac = (attempt as f64 * 0.618033988749895).fract() * 2.0 - 1.0; // -1..1
            delay_secs += jitter_amount * jitter_frac;
        }

        Duration::seconds(delay_secs as i64).max(Duration::seconds(1))
    }

    /// Calculate the next scheduled time for a retry.
    pub fn next_retry_time(&self, attempt: u32) -> DateTime<Utc> {
        Utc::now() + self.retry_delay(attempt)
    }

    /// Whether another retry is allowed.
    pub fn can_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

/// Predefined retry policies for different processor types.
pub mod policies {
    use super::RetryPolicy;

    /// OCR retry: 3 retries, 10s base, 2x backoff
    pub fn ocr() -> RetryPolicy {
        RetryPolicy::standard()
    }

    /// Whisper retry: 5 retries, 30s base, 3x backoff (transcription is expensive)
    pub fn whisper() -> RetryPolicy {
        RetryPolicy::patient()
    }

    /// Metadata extraction retry: 3 retries, 5s base, 2x backoff
    pub fn metadata() -> RetryPolicy {
        RetryPolicy {
            max_retries: 3,
            base_delay_seconds: 5,
            backoff_multiplier: 2.0,
            max_delay_seconds: 60,
            jitter: true,
        }
    }

    /// AI/embedding retry: 5 retries, 15s base, 2x backoff
    pub fn ai() -> RetryPolicy {
        RetryPolicy {
            max_retries: 5,
            base_delay_seconds: 15,
            backoff_multiplier: 2.0,
            max_delay_seconds: 300,
            jitter: true,
        }
    }

    /// Persistence retry: aggressive, low latency
    pub fn persistence() -> RetryPolicy {
        RetryPolicy::aggressive()
    }
}
