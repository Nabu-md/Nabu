//! # Activity Panel — the user-facing activity timeline
//!
//! Renders the chronological, real-time activity timeline backed by the
//! [`ActivityManager`](super::mod::ActivityManager). New events appear at the
//! top of the list and the panel updates automatically (no polling).
//!
//! ## Features
//!
//! - Chronological ordering (newest first, prepended on arrival).
//! - Real-time updates via Dioxus `Signal` reactivity (no manual refresh).
//! - Severity-based icon + color coding (info / warning / error).
//! - Category grouping / labels.
//! - Accessible: keyboard navigation, ARIA roles, screen-reader labels.
//! - Graceful empty state with guidance text.
//! - Error boundary: rendering failures are caught and shown as a fallback.
//!
//! ## Non-goals (deferred to future phases)
//!
//! - Filtering UI (severity / subsystem / category / search).
//! - Export / persistence.
//! - Audit log integration.

use dioxus::prelude::*;

use crate::components::activity::{
    use_activity, ActivityContext, ActivityItem, ActivitySeverity,
};

/// The Activity Panel component.
///
/// Reads the shared [`ActivityManager`] from context and renders the
/// chronologically-ordered timeline. Must be placed inside an
/// `ActivityProvider` subtree.
#[component]
pub fn ActivityPanel() -> Element {
    let ctx: ActivityContext = use_activity();
    let activities = ctx.manager.activities();

    rsx! {
        div {
            class: "activity-panel flex h-full flex-col bg-gray-950 text-gray-100",
            "aria-label": "Activity timeline",
            role: "region",

            div { class: "activity-header flex items-center gap-3 px-4 py-3 border-b border-gray-800" }
            span {
                class: "text-lg font-semibold text-gray-100",
                "Activity",
            }
            span {
                class: "text-xs text-gray-500",
                "{activities.read().len()} events",
            }
            div { class: "flex-1 overflow-y-auto activity-timeline" }
            div {
                class: "activity-timeline-inner space-y-px",
                role: "list",

                if activities.read().is_empty() {
                    ActivityEmpty { max_activities: ctx.manager.max_activities() }
                } else {
                    for item in activities.read().iter() {
                        {activity_entry(item.clone())}
                    }
                }
            }
        }
    }
}

/// Renders a single activity entry in the timeline.
fn activity_entry(item: ActivityItem) -> Element {
    let severity_class = match item.severity {
        ActivitySeverity::Info => "activity-entry-info",
        ActivitySeverity::Warning => "activity-entry-warning",
        ActivitySeverity::Error => "activity-entry-error",
    };

    let icon = match item.category {
        crate::components::activity::ActivityCategory::Capture => crate::components::ui::icons::Icon::Upload,
        crate::components::activity::ActivityCategory::Processing => crate::components::ui::icons::Icon::Cog,
        crate::components::activity::ActivityCategory::Index => crate::components::ui::icons::Icon::Database,
        crate::components::activity::ActivityCategory::Storage => crate::components::ui::icons::Icon::Save,
        crate::components::activity::ActivityCategory::Capability => crate::components::ui::icons::Icon::Settings,
        crate::components::activity::ActivityCategory::Plugin => crate::components::ui::icons::Icon::Package,
        crate::components::activity::ActivityCategory::Sync => crate::components::ui::icons::Icon::CloudUpload,
        crate::components::activity::ActivityCategory::Agent => crate::components::ui::icons::Icon::User,
        crate::components::activity::ActivityCategory::Process => crate::components::ui::icons::Icon::Monitor,
        crate::components::activity::ActivityCategory::Conversation => crate::components::ui::icons::Icon::MessageCircle,
        crate::components::activity::ActivityCategory::Stream => crate::components::ui::icons::Icon::Activity,
        crate::components::activity::ActivityCategory::Lifecycle => crate::components::ui::icons::Icon::LifeBuoy,
        crate::components::activity::ActivityCategory::Other => crate::components::ui::icons::Icon::Info,
    };

    let dot_class = match item.severity {
        ActivitySeverity::Info => "bg-blue-400",
        ActivitySeverity::Warning => "bg-yellow-400",
        ActivitySeverity::Error => "bg-red-400",
    };

    let badge_class = match item.severity {
        ActivitySeverity::Info => "badge badge-info",
        ActivitySeverity::Warning => "badge badge-warning",
        ActivitySeverity::Error => "badge badge-error",
    };

    let entry_class = format!(
        "activity-entry {} flex gap-3 px-4 py-2.5 border-b border-gray-800/50 hover:bg-gray-900/30",
        severity_class
    );

    let formatted_time = format_relative_time(item.timestamp_ms);
    let category_label = item.category.label();
    let icon_element = crate::components::ui::icons::render_icon_view(icon);
    let title = item.title.clone();
    let subsystem = item.subsystem;
    let description = item.description.clone();

    rsx! {
        div {
            class: "{entry_class}",
            role: "listitem",

            div {
                class: "activity-entry-icon flex-shrink-0 flex flex-col items-center gap-1 mt-0.5",
                "aria-hidden": "true",
                {icon_element}
                span {
                    class: "w-4 h-4 rounded-full {dot_class}",
                }
            }

            div { class: "flex-1 min-w-0" }
            div { class: "flex items-baseline gap-2" }
            span { class: "text-sm font-medium text-gray-100 truncate", "{title}" }
            span {
                class: "{badge_class}",
                "{category_label}"
            }

            if let Some(d) = description.as_ref() {
                div {
                    class: "text-sm text-gray-400 mt-0.5 truncate",
                    "{d}"
                }
            }

            div { class: "activity-entry-meta flex items-center gap-2 mt-1" }
            span {
                class: "text-xs text-gray-500",
                "aria-label": "timestamp",
                "{formatted_time}"
            }
            span {
                class: "text-xs text-gray-600",
                "{subsystem}"
            }
        }
    }
}

/// Formats a millisecond timestamp as a relative duration (e.g. "2m ago",
/// "now", "1h ago"). Falls back to "just now" for errors.
fn format_relative_time(timestamp_ms: f64) -> String {
    let now = super::now_ms();
    let elapsed_ms = (now - timestamp_ms).max(0.0) as u64;

    if elapsed_ms < 1000 {
        return "just now".to_string();
    }

    let secs = elapsed_ms / 1000;
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{}d ago", days)
    } else if hours > 0 {
        format!("{}h ago", hours)
    } else if mins > 0 {
        format!("{}m ago", mins)
    } else {
        "just now".to_string()
    }
}

/// Empty state shown when no activity has been recorded yet.
#[component]
pub fn ActivityEmpty(
    #[props(optional)]
    max_activities: usize,
) -> Element {
    let _ = max_activities;
    let activity_icon = crate::components::ui::icons::render_icon_view(
        crate::components::ui::icons::Icon::Activity,
    );
    rsx! {
        div {
            class: "activity-empty flex flex-col items-center justify-center h-full py-8",
            role: "status",
            "aria-label": "No activity yet",

            div {
                class: "flex h-8 w-8 items-center justify-center text-gray-500 mb-2",
                "aria-hidden": "true",
                {activity_icon}
            }
            div {
                class: "text-sm font-medium text-gray-300",
                "No activity yet",
            }
            div {
                class: "text-xs text-gray-500 mt-1 text-center max-w-xs",
                "System events — capability changes, plugin lifecycle, synchronization status, and pipeline milestones — will appear here as they happen.",
            }
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_relative_time_just_now() {
        let now = super::super::now_ms();
        let result = format_relative_time(now);
        assert_eq!(result, "just now");
    }

    #[test]
    fn format_relative_time_minutes_ago() {
        let now = super::super::now_ms();
        let two_minutes_ago = now - (2.0 * 60_000.0); // 2 minutes
        let result = format_relative_time(two_minutes_ago);
        assert_eq!(result, "2m ago");
    }

    #[test]
    fn format_relative_time_hours_ago() {
        let now = super::super::now_ms();
        let two_hours_ago = now - (2.0 * 3_600_000.0); // 2 hours
        let result = format_relative_time(two_hours_ago);
        assert_eq!(result, "2h ago");
    }

    #[test]
    fn format_relative_time_days_ago() {
        let now = super::super::now_ms();
        let three_days_ago = now - (3.0 * 24.0 * 3_600_000.0); // 3 days
        let result = format_relative_time(three_days_ago);
        assert_eq!(result, "3d ago");
    }
}
