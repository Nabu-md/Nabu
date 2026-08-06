//! Feedback primitives — toasts, banners, alerts, badges, progress, spinners,
//! skeletons, status dots, and background-task indicators.
//!
//! This module owns the full toast/task context stack (replacing the Phase 0
//! stubs that lived in `contexts.rs`). It also provides reusable notification
//! surfaces (`ToastProvider`, `NotificationBell`, `TaskIndicator`).

use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

// ── set_timeout helper ───────────────────────────────────────────────────

/// One-shot timer using `window.setTimeout`. The closure is leaked (via
/// `Closure::forget`) so the JS function stays alive until the timer fires.
pub fn set_timeout<F: FnOnce() + 'static>(f: F, ms: u32) {
    if let Some(window) = web_sys::window() {
        let mut f = Some(f);
        let closure = Closure::wrap(Box::new(move || {
            if let Some(f) = f.take() {
                f();
            }
        }) as Box<dyn FnMut()>);
        let func: &js_sys::Function = JsCast::unchecked_ref(closure.as_ref());
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            func,
            ms as i32,
        );
        closure.forget();
    }
}

// ── ToastKind ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl ToastKind {
    fn toast_class(self) -> &'static str {
        match self {
            ToastKind::Info => "toast-info",
            ToastKind::Success => "toast-success",
            ToastKind::Warning => "toast-warning",
            ToastKind::Error => "toast-error",
        }
    }

    fn icon(self) -> Icon {
        match self {
            ToastKind::Info => Icon::Info,
            ToastKind::Success => Icon::CircleCheck,
            ToastKind::Warning => Icon::Warning,
            ToastKind::Error => Icon::CircleX,
        }
    }

    fn alert_class(self) -> &'static str {
        match self {
            ToastKind::Info => "alert-info",
            ToastKind::Success => "alert-success",
            ToastKind::Warning => "alert-warning",
            ToastKind::Error => "alert-error",
        }
    }
}

// ── Toast data model ──────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub struct ToastItem {
    pub id: String,
    pub kind: ToastKind,
    pub title: String,
    pub message: Option<String>,
    pub action: Option<ToastAction>,
    pub persistent: bool,
}

#[derive(Clone)]
pub struct ToastAction {
    pub label: String,
    pub on_click: Callback<()>,
}

impl PartialEq for ToastAction {
    fn eq(&self, _other: &Self) -> bool {
        self.label == _other.label
    }
}

impl ToastAction {
    pub fn new(label: impl Into<String>, on_click: Callback<()>) -> Self {
        Self {
            label: label.into(),
            on_click,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ToastContext {
    pub toasts: Signal<Vec<ToastItem>>,
}

impl ToastContext {
    pub fn push(self, kind: ToastKind, title: impl Into<String>, message: impl Into<String>) {
        self.push_inner(kind, title, message, None, Some(5000));
    }

    pub fn push_with_action(
        self,
        kind: ToastKind,
        title: impl Into<String>,
        message: impl Into<String>,
        action: ToastAction,
    ) {
        self.push_inner(kind, title, message, Some(action), Some(10_000));
    }

    pub fn push_persistent_with_action(
        self,
        kind: ToastKind,
        title: impl Into<String>,
        message: impl Into<String>,
        action: ToastAction,
    ) {
        self.push_inner(kind, title, message, Some(action), None);
    }

    fn push_inner(
        self,
        kind: ToastKind,
        title: impl Into<String>,
        message: impl Into<String>,
        action: Option<ToastAction>,
        duration_ms: Option<u64>,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let title = title.into();
        let message_str = message.into();
        let toasts = self.toasts;
        toasts.write_unchecked().push(ToastItem {
            id: id.clone(),
            kind,
            title,
            message: (!message_str.is_empty()).then_some(message_str),
            action,
            persistent: duration_ms.is_none(),
        });
        if let Some(ms) = duration_ms {
            let toasts_copy = toasts;
            set_timeout(
                move || {
                    toasts_copy.write_unchecked().retain(|t| t.id != id);
                },
                ms as u32,
            );
        }
    }

    pub fn dismiss(self, id: &str) {
        let id = id.to_string();
        self.toasts.write_unchecked().retain(|t| t.id != id);
    }

    pub fn dismiss_by_title(self, title: &str) {
        let title = title.to_string();
        self.toasts.write_unchecked().retain(|t| t.title != title);
    }

    pub fn has_toast_with_title(self, title: &str) -> bool {
        self.toasts.read().iter().any(|t| t.title == title)
    }

    pub fn clear_all(self) {
        self.toasts.write_unchecked().clear();
    }

    pub fn info(self, title: impl Into<String>, message: impl Into<String>) {
        self.push(ToastKind::Info, title, message);
    }

    pub fn success(self, title: impl Into<String>, message: impl Into<String>) {
        self.push(ToastKind::Success, title, message);
    }

    pub fn warning(self, title: impl Into<String>, message: impl Into<String>) {
        self.push(ToastKind::Warning, title, message);
    }

    pub fn error(self, title: impl Into<String>, message: impl Into<String>) {
        self.push(ToastKind::Error, title, message);
    }
}

pub fn use_toast() -> ToastContext {
    use_context::<ToastContext>()
}

#[component]
pub fn ToastProvider(children: Element) -> Element {
    let toasts = use_signal(Vec::<ToastItem>::new);
    provide_context(ToastContext { toasts });
    rsx! {
        ToastRegion {}
        {children}
    }
}

#[component]
fn ToastRegion() -> Element {
    let ctx = use_toast();
    let toasts = ctx.toasts.clone();
    rsx! {
        div {
            class: "toast-region",
            role: "region",
            "aria-live": "polite",
            "aria-label": "Notifications",
        }
        for toast in toasts.read().iter() {
            {
                let class = format!("toast {}", toast.kind.toast_class());
                rsx! {
                    ToastItemView {
                        toast: toast.clone(),
                        class: class,
                    }
                }
            }
        }
    }
}

#[component]
fn ToastItemView(
    toast: ToastItem,
    class: String,
) -> Element {
    let id = toast.id.clone();
    let kind = toast.kind;
    let title = toast.title.clone();
    let message = toast.message.clone();
    let action = toast.action;
    let toasts = use_toast();
    rsx! {
        div {
            class: class,
            role: "status",
            span { "aria-hidden": "true", {render_icon_view(kind.icon())} }
            div { class: "flex flex-col gap-0.5 min-w-0" }
            div { class: "text-sm font-medium text-gray-100", "{title}" }
            {message.map(|m| rsx! { div { class: "text-xs text-gray-400", "{m}" } })}
            {action.map(|a| {
                let cb = a.on_click;
                let label = a.label;
                rsx! {
                    button {
                        r#type: "button",
                        class: "toast-action",
                        onclick: move |_| cb.call(()),
                        "{label}"
                    }
                }
            })}
            button {
                r#type: "button",
                class: "toast-close",
                "aria-label": "Dismiss notification",
                onclick: move |_| toasts.dismiss(&id),
                {render_icon_view(Icon::X)}
            }
        }
    }
}

// ── Banner & Alert ────────────────────────────────────────────────────────

#[component]
pub fn Banner(
    /// Status kind.
    kind: ToastKind,
    /// Message text.
    message: String,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    /// Called when the dismiss button is clicked.
    #[props(optional)]
    on_dismiss: Option<EventHandler<()>>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div {
            class: "banner {kind.alert_class()}{extra}",
            role: "status",
            span { "aria-hidden": "true", {render_icon_view(kind.icon())} }
            span { class: "flex-1", "{message}" }
            {on_dismiss.map(|cb| rsx! {
                button {
                    r#type: "button",
                    class: "toast-close",
                    "aria-label": "Dismiss",
                    onclick: move |_| cb.call(()),
                    {render_icon_view(Icon::X)}
                }
            })}
        }
    }
}

#[component]
pub fn Alert(
    /// Status kind.
    kind: ToastKind,
    /// Optional title.
    #[props(optional)]
    title: Option<String>,
    /// Message text.
    message: String,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div {
            class: "alert {kind.alert_class()}{extra}",
            role: "alert",
            span { "aria-hidden": "true", {render_icon_view(kind.icon())} }
            div { class: "flex flex-col gap-0.5" }
            {title.map(|t| rsx! { div { class: "text-sm font-medium", "{t}" } })}
            div { class: "text-sm", "{message}" }
        }
    }
}

// ── Badge ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BadgeKind {
    #[default]
    Neutral,
    Success,
    Warning,
    Error,
    Info,
}

impl BadgeKind {
    fn class(self) -> &'static str {
        match self {
            BadgeKind::Neutral => "",
            BadgeKind::Success => "badge-success",
            BadgeKind::Warning => "badge-warning",
            BadgeKind::Error => "badge-error",
            BadgeKind::Info => "badge-info",
        }
    }
}

#[component]
pub fn Badge(
    kind: BadgeKind,
    label: String,
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        span { class: "badge {kind.class()}{extra}", "{label}" }
    }
}

// ── Progress ──────────────────────────────────────────────────────────────

#[component]
pub fn Progress(
    /// Progress value 0.0–1.0.
    value: Signal<f64>,
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let val = value;
    rsx! {
        div {
            class: "progress{extra}",
            role: "progressbar",
            "aria-valuemin": "0",
            "aria-valuemax": "100",
            "aria-valuenow": "{val.read().clamp(0.0, 1.0) * 100.0}",
            div {
                class: "progress-fill",
                style: "width: {val.read().clamp(0.0, 1.0) * 100.0}%",
            }
        }
    }
}

// ── Spinner ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SpinnerSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl SpinnerSize {
    fn class(self) -> &'static str {
        match self {
            SpinnerSize::Sm => "spinner-sm",
            SpinnerSize::Md => "spinner-md",
            SpinnerSize::Lg => "spinner-lg",
        }
    }
}

#[component]
pub fn Spinner(
    #[props(optional)]
    size: SpinnerSize,
    #[props(optional)]
    label: Option<&'static str>,
) -> Element {
    rsx! {
        span {
            class: "spinner {size.class()}",
            role: "status",
            "aria-label": label.unwrap_or("Loading"),
        }
    }
}

/// Skeleton — a shimmering placeholder block.
#[component]
pub fn Skeleton(
    #[props(optional)]
    width: Option<&'static str>,
    #[props(optional)]
    height: Option<&'static str>,
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let style = format!(
        "width: {}; height: {};",
        width.unwrap_or("100%"),
        height.unwrap_or("14px")
    );
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div {
            class: "skeleton{extra}",
            style: style,
            "aria-hidden": "true",
        }
    }
}

// ── Status dot ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StatusKind {
    #[default]
    Neutral,
    Success,
    Warning,
    Error,
    Info,
}

impl StatusKind {
    fn class(self) -> &'static str {
        match self {
            StatusKind::Neutral => "",
            StatusKind::Success => "status-dot-success",
            StatusKind::Warning => "status-dot-warning",
            StatusKind::Error => "status-dot-error",
            StatusKind::Info => "status-dot-info",
        }
    }
}

#[component]
pub fn StatusDot(
    kind: StatusKind,
    label: String,
    #[props(optional)]
    pulse: bool,
) -> Element {
    let extra = if pulse { " status-dot-pulse" } else { "" };
    rsx! {
        span {
            class: "status-dot {kind.class()}{extra}",
            role: "status",
            "aria-label": label,
        }
    }
}

// ── Loading states ───────────────────────────────────────────────────────

#[component]
pub fn LoadingBlock(
    #[props(optional)]
    label: Option<&'static str>,
    #[props(optional)]
    size: SpinnerSize,
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div {
            class: "loading-block{extra}",
            role: "status",
            "aria-live": "polite",
        }
        Spinner { size: size, label: label.unwrap_or("Loading") }
        {label.map(|l| rsx! { div { class: "loading-block-label", "{l}" } })}
    }
}

#[component]
pub fn LoadingOverlay(
    #[props(optional)]
    label: Option<&'static str>,
) -> Element {
    rsx! {
        div {
            class: "loading-overlay",
            role: "status",
            "aria-live": "polite",
            LoadingBlock { label: label.unwrap_or("Loading…") }
        }
    }
}

#[component]
pub fn LoadingScreen(
    #[props(optional)]
    label: Option<&'static str>,
) -> Element {
    rsx! {
        div {
            class: "loading-screen",
            LoadingBlock {
                label: label.unwrap_or("Loading…"),
                size: SpinnerSize::Lg,
            }
        }
    }
}

#[component]
pub fn SkeletonList(
    #[props(optional)]
    rows: Option<usize>,
) -> Element {
    let n = rows.unwrap_or(5);
    rsx! {
        div { class: "skeleton-list", "aria-hidden": "true" }
        for _ in 0..n {
            div { class: "skeleton-list-row" }
            Skeleton { width: "100%", height: "16px" }
        }
    }
}

// ── Error Panel ───────────────────────────────────────────────────────────

#[component]
pub fn ErrorPanel(
    /// Plain-language summary.
    title: String,
    /// What went wrong in plain language.
    message: String,
    /// Optional technical detail shown behind an expander.
    #[props(optional)]
    details: Option<String>,
    /// Optional retry handler.
    #[props(optional)]
    on_retry: Option<EventHandler<()>>,
    /// Optional recovery guidance shown as a callout.
    #[props(optional)]
    recovery: Option<String>,
) -> Element {
    rsx! {
        div {
            class: "error-panel panel",
            role: "alert",
        }
        div { class: "flex items-start gap-3" }
        span { class: "text-xl", "aria-hidden": "true", {render_icon_view(Icon::CircleAlert)} }
        div { class: "flex flex-col gap-1 min-w-0 flex-1" }
        div { class: "text-sm font-semibold text-gray-100", "{title}" }
        div { class: "text-sm text-gray-400", "{message}" }
        {recovery.map(|r| rsx! {
            div { class: "error-recovery" }
            {render_icon_view(Icon::Callout)}
            " {r}"
        })}
        {details.map(|d| rsx! {
            details { class: "error-details" }
            summary { class: "error-details-summary", "Technical details" }
            pre { class: "error-details-pre", "{d}" }
        })}
        {on_retry.map(|cb| rsx! {
            div { class: "mt-1" }
            button {
                r#type: "button",
                class: "btn btn-sm",
                onclick: move |_| cb.call(()),
                {render_icon_view(Icon::RefreshCw)}
                " Retry"
            }
        })}
    }
}

// ── Background tasks (progress indicators) ───────────────────────────────

/// One in-flight background task.
#[derive(Clone, PartialEq)]
pub struct TaskInfo {
    pub id: String,
    pub label: String,
    pub progress: Option<f64>,
}

/// Shared registry of long-running background tasks.
#[derive(Clone, Copy)]
pub struct TaskContext {
    pub tasks: Signal<Vec<TaskInfo>>,
}

impl TaskContext {
    pub fn start(self, label: impl Into<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.tasks.write_unchecked().push(TaskInfo {
            id: id.clone(),
            label: label.into(),
            progress: None,
        });
        id
    }

    pub fn progress(self, id: &str, value: f64) {
        self.tasks.write_unchecked().iter_mut().for_each(|t| {
            if t.id == id {
                t.progress = Some(value.clamp(0.0, 1.0));
            }
        });
    }

    pub fn finish(self, id: &str) {
        let id = id.to_string();
        self.tasks.write_unchecked().retain(|t| t.id != id);
    }
}

/// Provides the background-task context (call once at the app root).
pub fn provide_tasks() {
    provide_context(TaskContext {
        tasks: use_signal(Vec::<TaskInfo>::new),
    });
}

/// Provider component for background-task tracking.
#[component]
pub fn TaskProvider(children: Element) -> Element {
    provide_tasks();
    rsx! { {children} }
}

/// Retrieves the background-task context.
pub fn use_tasks() -> TaskContext {
    use_context::<TaskContext>()
}

/// A compact NavBar indicator that appears while background tasks are active.
#[component]
pub fn TaskIndicator() -> Element {
    let tasks = use_tasks();
    let list = tasks.tasks.clone();
    let any_determinate = list.read().iter().any(|t| t.progress.is_some());
    let label = list.read().first().map(|t| t.label.clone())
        .unwrap_or_else(|| "Working…".to_string());
    rsx! {
        if !list.read().is_empty() {
            span { class: "task-indicator", role: "status", "aria-live": "polite" }
            span { class: "task-indicator-inner", title: "{label}" }
            if any_determinate {
                span { class: "task-indicator-bar" }
                span { class: "task-indicator-fill", style: "width: 100%;" }
            } else {
                Spinner { size: SpinnerSize::Sm, label: "Working" }
            }
            span { class: "task-indicator-label", "{label}" }
        }
    }
}
