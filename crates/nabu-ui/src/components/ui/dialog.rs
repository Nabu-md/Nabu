//! Dialog system — modal, confirm, alert, prompt.
//!
//! Replaces native `window.alert` / `confirm` / `prompt` dialogs. All dialogs
//! render a fixed overlay with `role="dialog"`, `aria-modal`, an Escape-to-close
//! handler, and click-outside-to-close (except alert).

use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;

/// Sizes for [`Dialog`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DialogSize {
    #[default]
    Md,
    Lg,
}

impl DialogSize {
    fn classes(self) -> &'static str {
        match self {
            DialogSize::Md => "",
            DialogSize::Lg => "dialog-lg",
        }
    }
}

/// Low-level modal dialog. Most callers should use [`ConfirmDialog`],
/// [`AlertDialog`] or [`PromptDialog`].
#[component]
pub fn Dialog(
    /// Two-way bound open state.
    open: RwSignal<bool>,
    /// Dialog title.
    title: String,
    /// Called when the dialog is dismissed.
    #[prop(optional)]
    on_close: Option<Callback<()>>,
    /// Size variant.
    #[prop(optional)]
    size: DialogSize,
    children: ChildrenFn,
) -> impl IntoView {
    let on_close = on_close;
    let close_click = on_close.clone();
    let close_key = on_close.clone();
    let size_classes = size.classes();
    view! {
        {move || if open.get() {
            view! {
                <div
                    class="dialog-overlay"
                    role="presentation"
                    on:click=move |_| {
                        open.set(false);
                        if let Some(cb) = on_close.as_ref() {
                            cb.run(());
                        }
                    }
                >
                    <div
                        class=format!("dialog {size_classes}")
                        role="dialog"
                        aria-modal="true"
                        aria-label=title.clone()
                        tabindex="-1"
                        on:click=move |ev| ev.stop_propagation()
                        on:keydown=move |ev| {
                            if ev.key() == "Escape" {
                                open.set(false);
                                if let Some(cb) = close_key.as_ref() {
                                    cb.run(());
                                }
                            }
                        }
                    >
                        <div class="dialog-header">
                            <h2 class="dialog-title">{title.clone()}</h2>
                            <button type="button" class="dialog-close" aria-label="Close dialog" on:click=move |_| {
                                open.set(false);
                                if let Some(cb) = close_click.as_ref() {
                                    cb.run(());
                                }
                            }>
                                {render_icon_view(Icon::X)}
                            </button>
                        </div>
                        <div class="dialog-body">
                            {children()}
                        </div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}

/// Confirmation dialog — a message with Cancel / Confirm buttons.
#[component]
pub fn ConfirmDialog(
    /// Two-way bound open state.
    open: RwSignal<bool>,
    /// Dialog title.
    title: String,
    /// Message body.
    message: String,
    /// Reactive message body — overrides `message` when provided, so callers
    /// can keep the dialog copy fresh (e.g. item counts that change as the
    /// selection does) without re-creating the component.
    #[prop(optional)]
    message_signal: Option<Memo<String>>,
    /// Confirm button label.
    #[prop(optional)]
    confirm_label: Option<&'static str>,
    /// Cancel button label.
    #[prop(optional)]
    cancel_label: Option<&'static str>,
    /// Danger styling for the confirm button.
    #[prop(optional)]
    danger: bool,
    /// Called when the user confirms.
    #[prop(optional)]
    on_confirm: Option<Callback<()>>,
    /// Called when the user cancels.
    #[prop(optional)]
    on_cancel: Option<Callback<()>>,
) -> impl IntoView {
    let confirm_cb = on_confirm;
    let cancel_cb = on_cancel;
    let cancel_key = cancel_cb.clone();
    let cancel_btn = cancel_cb.clone();
    let confirm_btn = confirm_cb.clone();
    view! {
        {move || if open.get() {
            view! {
                <div
                    class="dialog-overlay"
                    role="presentation"
                    on:click=move |_| {
                        open.set(false);
                        if let Some(cb) = cancel_cb.as_ref() {
                            cb.run(());
                        }
                    }
                >
                    <div
                        class="dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-label=title.clone()
                        tabindex="-1"
                        on:click=move |ev| ev.stop_propagation()
                        on:keydown=move |ev| {
                            if ev.key() == "Escape" {
                                open.set(false);
                                if let Some(cb) = cancel_key.as_ref() {
                                    cb.run(());
                                }
                            }
                        }
                    >
                        <div class="dialog-header">
                            <h2 class="dialog-title">{title.clone()}</h2>
                        </div>
                        <div class="dialog-body">
                            {message_signal.map(|m| m.get()).unwrap_or_else(|| message.clone())}
                        </div>
                        <div class="dialog-footer">
                            <Button
                                variant=ButtonVariant::Ghost
                                on_click=Callback::new(move |_| {
                                    open.set(false);
                                    if let Some(cb) = cancel_btn.as_ref() {
                                        cb.run(());
                                    }
                                })
                            >
                                {cancel_label.unwrap_or("Cancel")}
                            </Button>
                            <Button
                                variant=if danger { ButtonVariant::Destructive } else { ButtonVariant::Primary }
                                on_click=Callback::new(move |_| {
                                    open.set(false);
                                    if let Some(cb) = confirm_btn.as_ref() {
                                        cb.run(());
                                    }
                                })
                            >
                                {confirm_label.unwrap_or("Confirm")}
                            </Button>
                        </div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}

/// Alert dialog — a message with a single OK button.
#[component]
pub fn AlertDialog(
    /// Two-way bound open state.
    open: RwSignal<bool>,
    /// Dialog title.
    title: String,
    /// Message body.
    message: String,
    /// OK button label.
    #[prop(optional)]
    ok_label: Option<&'static str>,
    /// Called when the user dismisses the alert.
    #[prop(optional)]
    on_close: Option<Callback<()>>,
) -> impl IntoView {
    let close_cb = on_close;
    let close_key = close_cb.clone();
    let close_btn = close_cb.clone();
    view! {
        {move || if open.get() {
            view! {
                <div
                    class="dialog-overlay"
                    role="presentation"
                    on:click=move |_| {
                        open.set(false);
                        if let Some(cb) = close_cb.as_ref() {
                            cb.run(());
                        }
                    }
                >
                    <div
                        class="dialog"
                        role="alertdialog"
                        aria-modal="true"
                        aria-label=title.clone()
                        tabindex="-1"
                        on:click=move |ev| ev.stop_propagation()
                        on:keydown=move |ev| {
                            if ev.key() == "Escape" {
                                open.set(false);
                                if let Some(cb) = close_key.as_ref() {
                                    cb.run(());
                                }
                            }
                        }
                    >
                        <div class="dialog-header">
                            <h2 class="dialog-title">{title.clone()}</h2>
                        </div>
                        <div class="dialog-body">{message.clone()}</div>
                        <div class="dialog-footer">
                            <Button
                                variant=ButtonVariant::Primary
                                on_click=Callback::new(move |_| {
                                    open.set(false);
                                    if let Some(cb) = close_btn.as_ref() {
                                        cb.run(());
                                    }
                                })
                            >
                                {ok_label.unwrap_or("OK")}
                            </Button>
                        </div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}

/// Prompt dialog — a message with a text input and OK / Cancel buttons.
#[component]
pub fn PromptDialog(
    /// Two-way bound open state.
    open: RwSignal<bool>,
    /// Dialog title.
    title: String,
    /// Message body.
    message: String,
    /// Initial / default input value.
    #[prop(optional)]
    default_value: Option<String>,
    /// Confirm button label.
    #[prop(optional)]
    confirm_label: Option<&'static str>,
    /// Cancel button label.
    #[prop(optional)]
    cancel_label: Option<&'static str>,
    /// Called with the entered value when the user confirms.
    #[prop(optional)]
    on_submit: Option<Callback<String>>,
    /// Called when the user cancels.
    #[prop(optional)]
    on_cancel: Option<Callback<()>>,
) -> impl IntoView {
    let (input_value, set_input_value) = signal(default_value.unwrap_or_default());
    let submit_cb = on_submit;
    let cancel_cb = on_cancel;
    let cancel_key = cancel_cb.clone();
    let cancel_btn = cancel_cb.clone();
    let submit_btn = submit_cb.clone();
    let submit_key = submit_cb.clone();
    view! {
        {move || if open.get() {
            view! {
                <div
                    class="dialog-overlay"
                    role="presentation"
                    on:click=move |_| {
                        open.set(false);
                        if let Some(cb) = cancel_cb.as_ref() {
                            cb.run(());
                        }
                    }
                >
                    <div
                        class="dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-label=title.clone()
                        tabindex="-1"
                        on:click=move |ev| ev.stop_propagation()
                        on:keydown=move |ev| {
                            if ev.key() == "Escape" {
                                open.set(false);
                                if let Some(cb) = cancel_key.as_ref() {
                                    cb.run(());
                                }
                            }
                        }
                    >
                        <div class="dialog-header">
                            <h2 class="dialog-title">{title.clone()}</h2>
                        </div>
                        <div class="dialog-body">
                            <p class="mb-2">{message.clone()}</p>
                            <input
                                type="text"
                                class="input"
                                prop:value=input_value
                                on:input=move |ev| set_input_value.set(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    if ev.key() == "Enter" {
                                        open.set(false);
                                        if let Some(cb) = submit_key.as_ref() {
                                            cb.run(input_value.get());
                                        }
                                    }
                                }
                            />
                        </div>
                        <div class="dialog-footer">
                            <Button
                                variant=ButtonVariant::Ghost
                                on_click=Callback::new(move |_| {
                                    open.set(false);
                                    if let Some(cb) = cancel_btn.as_ref() {
                                        cb.run(());
                                    }
                                })
                            >
                                {cancel_label.unwrap_or("Cancel")}
                            </Button>
                            <Button
                                variant=ButtonVariant::Primary
                                on_click=Callback::new(move |_| {
                                    open.set(false);
                                    if let Some(cb) = submit_btn.as_ref() {
                                        cb.run(input_value.get());
                                    }
                                })
                            >
                                {confirm_label.unwrap_or("OK")}
                            </Button>
                        </div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}
