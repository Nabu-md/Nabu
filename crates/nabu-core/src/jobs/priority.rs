use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// Priority levels for job scheduling.
///
/// Higher-priority jobs are always dequeued before lower-priority jobs.
/// Within the same priority level, jobs are dequeued in FIFO order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Critical — executes before all other jobs.
    /// Reserved for system operations that must complete immediately.
    Critical = 5,

    /// High — executes after critical jobs.
    /// Use for user-facing operations where latency matters.
    High = 4,

    /// Normal — the default priority.
    /// Use for standard background operations.
    Normal = 3,

    /// Low — executes after normal jobs.
    /// Use for non-urgent maintenance operations.
    Low = 2,

    /// Background — executes when no higher-priority work exists.
    /// Use for batch processing, indexing, analytics.
    Background = 1,
}

impl Priority {
    /// Returns all priority levels in descending order.
    pub fn all_descending() -> Vec<Priority> {
        vec![
            Priority::Critical,
            Priority::High,
            Priority::Normal,
            Priority::Low,
            Priority::Background,
        ]
    }

    /// Returns a human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Priority::Critical => "Critical",
            Priority::High => "High",
            Priority::Normal => "Normal",
            Priority::Low => "Low",
            Priority::Background => "Background",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_val = *self as i32;
        let other_val = *other as i32;
        self_val.cmp(&other_val)
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
        assert!(Priority::Low > Priority::Background);

        // Same priority is equal
        assert_eq!(Priority::Normal, Priority::Normal);
    }

    #[test]
    fn test_priority_all_descending() {
        let levels = Priority::all_descending();
        assert_eq!(levels.len(), 5);
        assert_eq!(levels[0], Priority::Critical);
        assert_eq!(levels[1], Priority::High);
        assert_eq!(levels[2], Priority::Normal);
        assert_eq!(levels[3], Priority::Low);
        assert_eq!(levels[4], Priority::Background);
    }

    #[test]
    fn test_priority_default() {
        let p: Priority = Default::default();
        assert_eq!(p, Priority::Normal);
    }

    #[test]
    fn test_priority_serialization() {
        let json = serde_json::to_string(&Priority::Critical).unwrap();
        assert_eq!(json, "\"critical\"");

        let p: Priority = serde_json::from_str("\"high\"").unwrap();
        assert_eq!(p, Priority::High);
    }
}
