use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Returns an ISO 8601 timestamp with millisecond precision.
///
/// This is a shared utility used by both [`CaptureEngine`] and [`IngestionPipeline`]
/// to ensure consistent timestamp formatting across the capture pipeline.
///
/// # Format
///
/// The timestamp is formatted as `{seconds}.{millis}Z`, where:
/// - `seconds` is the number of seconds since the Unix epoch
/// - `millis` is the fractional seconds component, zero-padded to 3 digits
///
/// This format is compatible with most JSON and database timestamp representations.
pub fn current_timestamp() -> String {
    let now = SystemTime::now();
    let duration = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    format!("{}.{:03}Z", secs, millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_has_correct_format() {
        let ts = current_timestamp();
        assert!(ts.ends_with("Z"));
        assert!(ts.contains("."));
        
        let without_z = ts.trim_end_matches('Z');
        let parts: Vec<&str> = without_z.split(".").collect();
        assert_eq!(parts.len(), 2);
        
        let millis_part = parts[1];
        assert_eq!(millis_part.len(), 3);
    }

    #[test]
    fn timestamp_is_recent() {
        let ts = current_timestamp();
        let secs_part: u64 = ts.split(".").next().unwrap().parse().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Should be within 1 second of current time
        assert!(now.saturating_sub(secs_part) <= 1);
    }
}