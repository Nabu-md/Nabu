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
}

/// Shared toast store, provided via [`ToastProvider`].
#[derive(Clone, Copy)]
pub struct ToastContext {
    pub toasts: RwSignal<Vec<ToastItem>>,
}

impl ToastContext {
    /// Pushes a toast and auto-dismisses it after `duration`.
    pub fn push(
        self,
        kind: ToastKind,
        title: impl Into<String>,
        message: impl Into<String>,
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
            });
        });
        set_timeout(
            move || toasts.update(|list| list.retain(|t| t.id != id)),
            Duration::from_millis(5000),
        );
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

/// Renders the toast stack (fixed bottom-right).
#[component]
pub fn ToastRegion() -> impl IntoView {
    let context = expect_context::<ToastContext>();
    view! {
        <div class="toast-region" role="region" aria-live="polite" aria-label="Notifications">
            {move || context.toasts.get().into_iter().map(|toast| {
                let id = toast.id.clone();
                let kind = toast.kind;
                let title = toast.title.clone();
                let message = toast.message.clone();
                let toasts = context.toasts;
                view! {
                    <div class=format!("toast {}", kind.toast_class()) role="status">
                        <span aria-hidden="true">{kind.icon()}</span>
                        <div class="flex flex-col gap-0.5 min-w-0">
                            <div class="text-sm font-medium text-gray-100">{title}</div>
                            {message.map(|m| view! { <div class="text-xs text-gray-400">{m}</div> }.into_any())}
                        </div>
                        <button
                            type="button"
                            class="toast-close"
                            aria-label="Dismiss notification"
                            on:click=move |_| {
                                toasts.update(|list| list.retain(|t| t.id != id));
                            }
                        >
                            "✕"
                        </button>
                    </div>
                }
            }).collect_view()}
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
