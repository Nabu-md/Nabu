//! # StreamingIndicator — lightweight streaming status indicator
//!
//! Provides visual indicators for active streaming sessions: a spinner,
//! a "streaming" label, and a token count. These indicators are intentionally
//! minimal to avoid distracting the user during token delivery.

use dioxus::prelude::*;

use crate::components::ui::feedback::{Spinner, SpinnerSize, StatusDot, StatusKind};

/// Size variant for the streaming indicator.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StreamingIndicatorSize {
    #[default]
    Sm,
    Md,
    Lg,
}

impl StreamingIndicatorSize {
    fn spinner_size(self) -> SpinnerSize {
        match self {
            StreamingIndicatorSize::Sm => SpinnerSize::Sm,
            StreamingIndicatorSize::Md => SpinnerSize::Md,
            StreamingIndicatorSize::Lg => SpinnerSize::Lg,
        }
    }

    fn css_class(self) -> &'static str {
        match self {
            StreamingIndicatorSize::Sm => "streaming-indicator-sm",
            StreamingIndicatorSize::Md => "streaming-indicator-md",
            StreamingIndicatorSize::Lg => "streaming-indicator-lg",
        }
    }
}

/// A compact indicator shown alongside active streaming content.
///
/// Renders a spinning status dot + "Streaming…" label and an optional live
/// token count. The component is stateless — it receives its data via props
/// and re-renders only when those props change.
#[component]
pub fn StreamingIndicator(
    /// Size variant (defaults to `Sm`).
    #[props(optional)]
    size: StreamingIndicatorSize,
    /// Current token count (defaults to 0).
    #[props(optional)]
    token_count: Option<u64>,
    /// Extra CSS classes (optional).
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let s = size;
    let extra = class.unwrap_or_default();
    let count_text = match token_count {
        Some(n) if n > 0 => format!(" · {} token{}", n, if n == 1 { "" } else { "s" }),
        _ => String::new(),
    };
    let count_a11y = match &token_count {
        Some(n) if *n > 0 => format!("{} tokens delivered", n),
        _ => "streaming in progress".to_string(),
    };

    rsx! {
        span {
            class: "streaming-indicator {s.css_class()} {extra}",
            role: "status",
            "aria-label": "Streaming{count_a11y}",
            "aria-live": "polite",
        }
        StatusDot {
            kind: StatusKind::Info,
            label: "Streaming".to_string(),
            pulse: true,
        }
        span {
            class: "streaming-indicator-label",
            "Streaming…",
        }
        {if !count_text.is_empty() {
            rsx! {
                span {
                    class: "streaming-indicator-count",
                    "aria-hidden": "true",
                    "{count_text}",
                }
            }
        } else {
            rsx! {}
        }}
        Spinner {
            size: s.spinner_size(),
            label: "Streaming",
        }
    }
}
