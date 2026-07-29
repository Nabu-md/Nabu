use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Priority levels determining job execution order.
/// Higher-priority jobs execute before lower-priority jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Priority {
    /// System-critical operations (must execute immediately)
    Critical = 0,
    /// User-facing operations (OCR, metadata extraction on visible items)
    High = 1,
    /// Standard background processing
    Normal = 2,
    /// Non-urgent processing (batch operations, analytics)
    Low = 3,
    /// Deferred processing (maintenance, garbage collection)
    Background = 4,
}

impl Priority {
    /// All priority levels in descending order of importance.
    pub const ALL: [Priority; 5] = [
        Priority::Critical,
        Priority::High,
        Priority::Normal,
        Priority::Low,
        Priority::Background,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Priority::Critical => "critical",
            Priority::High => "high",
            Priority::Normal => "normal",
            Priority::Low => "low",
            Priority::Background => "background",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "critical" => Some(Priority::Critical),
            "high" => Some(Priority::High),
            "normal" => Some(Priority::Normal),
            "low" => Some(Priority::Low),
            "background" => Some(Priority::Background),
            _ => None,
        }
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        other.value().cmp(&self.value())
    }
}

impl Priority {
    pub fn name(&self) -> &'static str {
        self.label()
    }

    pub fn value(&self) -> u8 {
        match self {
            Priority::Critical => 0,
            Priority::High => 1,
            Priority::Normal => 2,
            Priority::Low => 3,
            Priority::Background => 4,
        }
    }
}

/// A wrapper for items that need priority-based ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorityItem<T> {
    pub priority: Priority,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub item: T,
}

impl<T> PriorityItem<T> {
    pub fn new(priority: Priority, created_at: chrono::DateTime<chrono::Utc>, item: T) -> Self {
        Self {
            priority,
            created_at,
            item,
        }
    }
}

impl<T: PartialEq + Eq> PartialOrd for PriorityItem<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: PartialEq + Eq> Ord for PriorityItem<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then FIFO by created_at
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => self.created_at.cmp(&other.created_at),
            ordering => ordering,
        }
    }
}
