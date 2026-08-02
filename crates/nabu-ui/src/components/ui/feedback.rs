//! Feedback primitives — toasts, banners, alerts, badges, progress, spinners,
//! skeletons, status dots.

use leptos::prelude::*;
use std::time::Duration;

/// Kind for toast / badge / alert / status styling.
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

    fn icon(self) -> &'static str {
        match self {
            ToastKind::Info => "ℹ️",
            ToastKind::Success => "✅",
            ToastKind::Warning => "⚠️",
            ToastKind::Error => "❌",
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

/// One toast entry.
#[derive(Clone)]
pub struct ToastItem {
    pub id: String,
    pub kind: ToastKind,
    pub title: String,
    pub message: Option<String>,
    /// Optional clickable action rendered as a button inside the toast
    /// (e.g. an "Undo" button after an item is moved to trash).
    pub action: Option<ToastAction>,
    /// Persistent toasts stay until dismissed (no auto-dismiss timer) and are
    /// listed in the notification center.
    pub persistent: bool,
}

/// A clickable action attached to a toast.
#[derive(Clone)]
pub struct ToastAction {
    pub label: String,
    pub on_click: Callback<()>,
}

impl ToastAction {
    pub fn new(label: impl Into<String>, on_click: Callback<()>) -> Self {
        Self {
            label: label.into(),
            on_click,
        }
    }
}

/// Shared toast store, provided via [`ToastProvider`].
#[derive(Clone, Copy)]
pub struct ToastContext {
    pub toasts: RwSignal<Vec<ToastItem>>,
}

impl ToastContext {
    /// Pushes a toast and auto-dismisses it after a short lifetime.
    pub fn push(
        self,
        kind: ToastKind,
        title: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.push_inner(kind, title, message, None, Some(5000));
    }

    /// Pushes a toast with a clickable action button and a longer lifetime so
    /// the user has time to act on it.
    pub fn push_with_action(
        self,
        kind: ToastKind,
        title: impl Into<String>,
        message: impl Into<String>,
        action: ToastAction,
    ) {
        self.push_inner(kind, title, message, Some(action), Some(10_000));
    }

    /// Pushes a persistent notification with a clickable action — stays until
    /// dismissed, and the action stays available the whole time (e.g. a
    /// "Retry" button on an index failure that keeps failing). Use sparingly
    /// for important, long-lived state that needs a decision.
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
        toasts.update(|list| {
            list.push(ToastItem {
                id: id.clone(),
                kind,
                title,
                message: (!message_str.is_empty()).then_some(message_str),
                action,
                persistent: duration_ms.is_none(),
            });
        });
        if let Some(ms) = duration_ms {
            set_timeout(
                move || toasts.update(|list| list.retain(|t| t.id != id)),
                Duration::from_millis(ms),
            );
        }
    }

    /// Dismisses a toast by id (used by the notification center).
    pub fn dismiss(self, id: &str) {
        let id = id.to_string();
        self.toasts.update(|list| list.retain(|t| t.id != id));
    }

    /// Dismisses every toast with the given title — used to clear a stale
    /// persistent warning once the underlying condition has resolved (e.g. a
    /// successful retry after an index failure).
    pub fn dismiss_by_title(self, title: &str) {
        let title = title.to_string();
        self.toasts.update(|list| list.retain(|t| t.title != title));
    }

    /// True when a toast with this title is currently shown (used to dedupe
    /// repeated failure notifications).
    pub fn has_toast_with_title(self, title: &str) -> bool {
        self.toasts.get().iter().any(|t| t.title == title)
    }

    /// Removes every toast, persistent ones included.
    pub fn clear_all(self) {
        self.toasts.set(Vec::new());
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

/// Provides the toast context to the subtree and renders the toast region.
#[component]
pub fn ToastProvider(children: ChildrenFn) -> impl IntoView {
    let toasts = RwSignal::new(Vec::<ToastItem>::new());
    provide_context(ToastContext { toasts });
    view! {
        {children()}
        <ToastRegion />
    }
}

/// A single toast / notification row, shared by [`ToastRegion`] and the
/// notification panel so dismiss and action handling stay in one place.
#[component]
fn ToastItemView(toast: ToastItem, class: String) -> impl IntoView {
    let id = toast.id.clone();
    let kind = toast.kind;
    let title = toast.title.clone();
    let message = toast.message.clone();
    let action = toast.action.clone();
    let toasts = expect_context::<ToastContext>();
    view! {
        <div class=class role="status">
            <span aria-hidden="true">{kind.icon()}</span>
            <div class="flex flex-col gap-0.5 min-w-0">
                <div class="text-sm font-medium text-gray-100">{title}</div>
                {message.map(|m| view! { <div class="text-xs text-gray-400">{m}</div> }.into_any())}
            </div>
            {action.map(|a| {
                let label = a.label;
                let on_click = a.on_click;
                view! {
                    <button
                        type="button"
                        class="toast-action"
                        on:click=move |_| on_click.run(())
                    >
                        {label}
                    </button>
                }.into_any()
            })}
            <button
                type="button"
                class="toast-close"
                aria-label="Dismiss notification"
                on:click=move |_| toasts.dismiss(&id)
            >
                "✕"
            </button>
        </div>
    }
}

/// Renders the toast stack (fixed bottom-right).
#[component]
pub fn ToastRegion() -> impl IntoView {
    let context = expect_context::<ToastContext>();
    view! {
        <div class="toast-region" role="region" aria-live="polite" aria-label="Notifications">
            {move || context.toasts.get().into_iter().map(|toast| {
                let class = format!("toast {}", toast.kind.toast_class());
                view! { <ToastItemView toast=toast class=class /> }
            }).collect_view()}
        </div>
    }
}

/// A bell button with an unread-count badge that opens the notification
/// center. Rendered in the NavBar; the panel lists active notifications with
/// dismiss controls (persistent ones stay until cleared).
#[component]
pub fn NotificationBell() -> impl IntoView {
    let context = expect_context::<ToastContext>();
    let (open, set_open) = signal(false);
    // Close when the user presses Escape.
    let overlay_ref = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        if open.get() {
            set_timeout(
                move || {
                    if let Some(el) = overlay_ref.get() {
                        let _ = el.focus();
                    }
                },
                std::time::Duration::from_millis(10),
            );
        }
    });
    view! {
        <div class="relative">
            <button
                type="button"
                class="navbar-action"
                title="Notifications"
                aria-label="Notifications"
                aria-expanded=move || open.get()
                on:click=move |_| set_open.update(|v| *v = !*v)
            >
                "🔔"
                {move || {
                    let count = context.toasts.get().len();
                    if count > 0 {
                        view! { <span class="notif-badge" aria-hidden="true">{count}</span> }.into_any()
                    } else {
                        view! {}.into_any()
                    }
                }}
            </button>
            {move || if open.get() {
                view! {
                    <div
                        node_ref=overlay_ref
                        tabindex="-1"
                        class="notif-overlay"
                        role="dialog"
                        aria-modal="false"
                        aria-label="Notifications"
                        on:click=move |_| set_open.set(false)
                        on:keydown=move |ev| if ev.key() == "Escape" { set_open.set(false) }
                    >
                        <div class="notif-panel" on:click=move |ev| ev.stop_propagation()>
                            <div class="notif-panel-header">
                                <span class="text-sm font-medium">"Notifications"</span>
                                {move || if context.toasts.get().is_empty() {
                                    view! {}.into_any()
                                } else {
                                    view! {
                                        <button
                                            type="button"
                                            class="btn btn-sm btn-ghost"
                                            on:click=move |_| context.clear_all()
                                        >
                                            "Clear all"
                                        </button>
                                    }.into_any()
                                }}
                            </div>
                            <div class="notif-panel-body">
                                {move || {
                                    let toasts = context.toasts.get();
                                    if toasts.is_empty() {
                                        view! {
                                            <div class="dash-empty">"No notifications — you're all caught up."</div>
                                        }.into_any()                                    } else {
                                        toasts.into_iter().map(|toast| {
                                            let class = format!("notif-item {}", toast.kind.toast_class());
                                            view! { <ToastItemView toast=toast class=class /> }
                                        }).collect_view().into_any()
                                    }
                                }}
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </div>
    }
}

/// Retrieves the toast context. Call inside a [`ToastProvider`] subtree.
pub fn use_toast() -> ToastContext {
    expect_context::<ToastContext>()
}

/// Banner — a prominent, dismissible status strip.
#[component]
pub fn Banner(
    /// Status kind.
    kind: ToastKind,
    /// Message text.
    message: String,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Called when the dismiss button is clicked.
    #[prop(optional)]
    on_dismiss: Option<Callback<()>>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("banner {}{extra}", kind.alert_class()) role="status">
            <span aria-hidden="true">{kind.icon()}</span>
            <span class="flex-1">{message}</span>
            {on_dismiss.map(|cb| view! {
                <button type="button" class="toast-close" aria-label="Dismiss" on:click=move |_| cb.run(())>
                    "✕"
                </button>
            }.into_any())}
        </div>
    }
}

/// Alert — a non-dismissible status box (inline).
#[component]
pub fn Alert(
    /// Status kind.
    kind: ToastKind,
    /// Optional title.
    #[prop(optional)]
    title: Option<String>,
    /// Message text.
    message: String,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("alert {}{extra}", kind.alert_class()) role="alert">
            <span aria-hidden="true">{kind.icon()}</span>
            <div class="flex flex-col gap-0.5">
                {title.map(|t| view! { <div class="text-sm font-medium">{t}</div> }.into_any())}
                <div class="text-sm">{message}</div>
            </div>
        </div>
    }
}

/// Badge kind.
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

/// Badge — a small status pill.
#[component]
pub fn Badge(
    /// Kind.
    kind: BadgeKind,
    /// Label text.
    label: String,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <span class=format!("badge {}{extra}", kind.class())>{label}</span>
    }
}

/// Progress bar with a 0.0–1.0 value signal.
#[component]
pub fn Progress(
    /// Progress value 0.0–1.0.
    value: RwSignal<f64>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let pct = move || {
        let v = value.get().clamp(0.0, 1.0);
        format!("{:.1}%", v * 100.0)
    };
    view! {
        <div
            class=format!("progress{extra}")
            role="progressbar"
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow=move || {
                let v = value.get().clamp(0.0, 1.0);
                (v * 100.0) as i64
            }
        >
            <div class="progress-fill" style=move || format!("width: {};", pct())></div>
        </div>
    }
}

/// Spinner size.
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

/// Spinner — an indeterminate loading indicator.
#[component]
pub fn Spinner(
    /// Size.
    #[prop(optional)]
    size: SpinnerSize,
    /// Accessible label.
    #[prop(optional)]
    label: Option<&'static str>,
) -> impl IntoView {
    view! {
        <span
            class=format!("spinner {}", size.class())
            role="status"
            aria-label=label.unwrap_or("Loading")
        ></span>
    }
}

/// Skeleton — a shimmering placeholder block.
#[component]
pub fn Skeleton(
    /// Width (CSS value, e.g. "100%").
    #[prop(optional)]
    width: Option<&'static str>,
    /// Height (CSS value, e.g. "16px").
    #[prop(optional)]
    height: Option<&'static str>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
) -> impl IntoView {
    let style = format!(
        "width: {}; height: {};",
        width.unwrap_or("100%"),
        height.unwrap_or("14px")
    );
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("skeleton{extra}") style=style aria-hidden="true"></div>
    }
}

/// Status dot kind.
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

/// Status indicator — a small coloured dot with optional pulse.
#[component]
pub fn StatusDot(
    /// Kind.
    kind: StatusKind,
    /// Accessible label.
    label: String,
    /// Enables the pulse animation.
    #[prop(optional)]
    pulse: bool,
) -> impl IntoView {
    let extra = if pulse { " status-dot-pulse" } else { "" };
    view! {
        <span
            class=format!("status-dot {}{extra}", kind.class())
            role="status"
            aria-label=label
        ></span>
    }
}

// ── Loading states ───────────────────────────────────────────────────────

/// A centered, unobtrusive loading block with a spinner and optional label.
/// Use this instead of blank screens while an async workflow is in flight.
#[component]
pub fn LoadingBlock(
    /// Optional label shown under the spinner.
    #[prop(optional)]
    label: Option<&'static str>,
    /// Size of the spinner.
    #[prop(optional)]
    size: SpinnerSize,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("loading-block{extra}") role="status" aria-live="polite">
            <Spinner size=size label=label.unwrap_or("Loading") />
            {label.map(|l| view! { <div class="loading-block-label">{l}</div> }.into_any())}
        </div>
    }
}

/// An absolute-overlay loading veil for content that is being (re)loaded in
/// place — the parent must be `position: relative`.
#[component]
pub fn LoadingOverlay(
    /// Label shown inside the veil.
    #[prop(optional)]
    label: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div class="loading-overlay" role="status" aria-live="polite">
            <LoadingBlock label=label.unwrap_or("Loading…") />
        </div>
    }
}

/// A full-height centered loading screen (e.g. for the app boot screen).
#[component]
pub fn LoadingScreen(
    /// Label shown under the spinner.
    #[prop(optional)]
    label: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div class="loading-screen">
            <LoadingBlock label=label.unwrap_or("Loading…") size=SpinnerSize::Lg />
        </div>
    }
}

/// A stack of skeleton lines used while a list/table is loading.
#[component]
pub fn SkeletonList(
    /// Number of skeleton rows to render.
    #[prop(optional)]
    rows: Option<usize>,
) -> impl IntoView {
    let n = rows.unwrap_or(5);
    view! {
        <div class="skeleton-list" aria-hidden="true">
            {(0..n).map(|_| view! {
                <div class="skeleton-list-row">
                    <Skeleton width="100%" height="16px" />
                </div>
            }).collect_view()}
        </div>
    }
}

// ── Error state ──────────────────────────────────────────────────────────

/// A full-width error panel with a plain-language explanation, expandable
/// technical details, and an optional retry action. Replaces silent failures.
#[component]
pub fn ErrorPanel(
    /// Plain-language summary (e.g. "Couldn't load your notes").
    title: String,
    /// What went wrong in plain language.
    message: String,
    /// Optional technical detail shown behind an expander.
    #[prop(optional)]
    details: Option<String>,
    /// Optional retry handler.
    #[prop(optional)]
    on_retry: Option<Callback<()>>,
    /// Optional recovery guidance shown as a callout.
    #[prop(optional)]
    recovery: Option<String>,
) -> impl IntoView {
    view! {
        <div class="error-panel panel" role="alert">
            <div class="flex items-start gap-3">
                <span class="text-xl" aria-hidden="true">"⚠️"</span>
                <div class="flex flex-col gap-1 min-w-0 flex-1">
                    <div class="text-sm font-semibold text-gray-100">{title}</div>
                    <div class="text-sm text-gray-400">{message}</div>
                    {recovery.map(|r| view! {
                        <div class="error-recovery">"💡 " {r}</div>
                    }.into_any())}
                    {details.map(|d| view! {
                        <details class="error-details">
                            <summary class="error-details-summary">"Technical details"</summary>
                            <pre class="error-details-pre">{d}</pre>
                        </details>
                    }.into_any())}
                    {if let Some(retry) = on_retry {
                        view! {
                            <div class="mt-1">
                                <button
                                    type="button"
                                    class="btn btn-sm"
                                    on:click=move |_| retry.run(())
                                >
                                    "↻ Retry"
                                </button>
                            </div>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }}
                </div>
            </div>
        </div>
    }
}

// ── Background tasks (progress indicators) ───────────────────────────────

/// One in-flight background task.
#[derive(Clone, PartialEq)]
pub struct TaskInfo {
    pub id: String,
    pub label: String,
    /// `Some(0.0–1.0)` when determinate progress is known, `None` for
    /// indeterminate tasks.
    pub progress: Option<f64>,
}

/// Shared registry of long-running background tasks, provided at the app root
/// and rendered by [`TaskIndicator`] in the NavBar.
#[derive(Clone, Copy)]
pub struct TaskContext {
    pub tasks: RwSignal<Vec<TaskInfo>>,
}

impl TaskContext {
    /// Registers a task, returning its id for later progress/removal updates.
    pub fn start(self, label: impl Into<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.tasks.update(|t| {
            t.push(TaskInfo {
                id: id.clone(),
                label: label.into(),
                progress: None,
            });
        });
        id
    }

    /// Updates a task's determinate progress (0.0–1.0).
    pub fn progress(self, id: &str, value: f64) {
        self.tasks.update(|t| {
            if let Some(task) = t.iter_mut().find(|x| x.id == id) {
                task.progress = Some(value.clamp(0.0, 1.0));
            }
        });
    }

    /// Removes a task (completed, failed or cancelled).
    pub fn finish(self, id: &str) {
        let id = id.to_string();
        self.tasks.update(|t| t.retain(|x| x.id != id));
    }
}

/// Provides the background-task context (call once at the app root).
pub fn provide_tasks() {
    provide_context(TaskContext {
        tasks: RwSignal::new(Vec::new()),
    });
}

/// Retrieves the background-task context.
pub fn use_tasks() -> TaskContext {
    expect_context::<TaskContext>()
}

/// A compact NavBar indicator that appears while background tasks are active
/// (spinner for indeterminate, mini progress bar for determinate tasks).
#[component]
pub fn TaskIndicator() -> impl IntoView {
    let tasks = use_tasks();
    view! {
        <span class="task-indicator" role="status" aria-live="polite">
            {move || {
                let list = tasks.tasks.get();
                if list.is_empty() {
                    view! {}.into_any()
                } else {
                    // Average only over the determinate tasks; an indeterminate
                    // sibling shouldn't drag the shown percentage down.
                    let determinate_count = list.iter().filter(|t| t.progress.is_some()).count();
                    let determinate = if determinate_count > 0 {
                        list.iter().filter_map(|t| t.progress).sum::<f64>()
                            / determinate_count as f64
                    } else {
                        0.0
                    };
                    let label = list
                        .first()
                        .map(|t| t.label.clone())
                        .unwrap_or_else(|| "Working…".to_string());
                    let title_text = label.clone();
                    view! {
                        <span class="task-indicator-inner" title=title_text>
                            {if list.iter().any(|t| t.progress.is_some()) {
                                view! {
                                    <span class="task-indicator-bar">
                                        <span
                                            class="task-indicator-fill"
                                            style=move || format!("width: {:.0}%;", determinate * 100.0)
                                        ></span>
                                    </span>
                                }.into_any()
                            } else {
                                view! { <Spinner size=SpinnerSize::Sm label="Working" /> }.into_any()
                            }}
                            <span class="task-indicator-label">{label}</span>
                        </span>
                    }.into_any()
                }
            }}
        </span>
    }
}
