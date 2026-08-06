//! Dialog system — modal, confirm, alert, prompt.
//!
//! Replaces native `window.alert` / `confirm` / `prompt` dialogs. All dialogs
//! render a fixed overlay with `role="dialog"`, `aria-modal`, an Escape-to-close
//! handler, and click-outside-to-close (except alert).

use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

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
    open: Signal<bool>,
    /// Dialog title.
    title: String,
    /// Called when the dialog is dismissed.
    #[props(optional)]
    on_close: Option<EventHandler<()>>,
    /// Size variant.
    #[props(optional)]
    size: DialogSize,
    children: Element,
) -> Element {
    let mut open_sig = open;
    let on_close_cb = on_close;
    let size_classes = size.classes();
    rsx! {
        if *open_sig.read() {
            div {
                class: "dialog-overlay",
                role: "presentation",
                onclick: move |_| {
                    open_sig.set(false);
                    if let Some(cb) = on_close_cb.as_ref() {
                        cb.call(());
                    }
                },
                div {
                    class: "dialog {size_classes}",
                    role: "dialog",
                    "aria-modal": "true",
                    "aria-label": title,
                    tabindex: "-1",
                    onclick: |ev: MouseEvent| ev.stop_propagation(),
                    onkeydown: move |ev: KeyboardEvent| {
                        if ev.key() == Key::Escape {
                            open_sig.set(false);
                            if let Some(cb) = on_close_cb.as_ref() {
                                cb.call(());
                            }
                        }
                    },
                    div { class: "dialog-header" }
                    h2 { class: "dialog-title", "{title}" }
                    button {
                        r#type: "button",
                        class: "dialog-close",
                        "aria-label": "Close dialog",
                        onclick: move |_| {
                            open_sig.set(false);
                            if let Some(cb) = on_close_cb.as_ref() {
                                cb.call(());
                            }
                        },
                        {render_icon_view(Icon::X)}
                    }
                    div { class: "dialog-body", {children} }
                }
            }
        }
    }
}

/// Confirmation dialog — a message with Cancel / Confirm buttons.
#[component]
pub fn ConfirmDialog(
    /// Two-way bound open state.
    open: Signal<bool>,
    /// Dialog title.
    title: String,
    /// Message body.
    message: String,
    /// Reactive message body — overrides `message` when provided.
    #[props(optional)]
    message_signal: Option<Signal<String>>,
    /// Confirm button label.
    #[props(optional)]
    confirm_label: Option<&'static str>,
    /// Cancel button label.
    #[props(optional)]
    cancel_label: Option<&'static str>,
    /// Danger styling for the confirm button.
    #[props(optional)]
    danger: bool,
    /// Called when the user confirms.
    #[props(optional)]
    on_confirm: Option<EventHandler<()>>,
    /// Called when the user cancels.
    #[props(optional)]
    on_cancel: Option<EventHandler<()>>,
) -> Element {
    let mut open_sig = open;
    let confirm_cb = on_confirm;
    let cancel_cb = on_cancel;
    let title_for_title = title.clone();

    let msg = if let Some(ms) = message_signal {
        ms.read().clone()
    } else {
        message
    };

    rsx! {
        if *open_sig.read() {
            div {
                class: "dialog-overlay",
                role: "presentation",
                onclick: move |_| {
                    open_sig.set(false);
                    if let Some(cb) = cancel_cb.as_ref() {
                        cb.call(());
                    }
                },
                div {
                    class: "dialog",
                    role: "dialog",
                    "aria-modal": "true",
                    "aria-label": title,
                    tabindex: "-1",
                    onclick: |ev: MouseEvent| ev.stop_propagation(),
                    onkeydown: move |ev: KeyboardEvent| {
                        if ev.key() == Key::Escape {
                            open_sig.set(false);
                            if let Some(cb) = cancel_cb.as_ref() {
                                cb.call(());
                            }
                        }
                    },
                    div { class: "dialog-header" }
                    h2 { class: "dialog-title", "{title_for_title}" }
                    div { class: "dialog-body", "{msg}" }
                    div { class: "dialog-footer" }
                    Button {
                        variant: ButtonVariant::Ghost,
                        on_click: move |_| {
                            open_sig.set(false);
                            if let Some(cb) = cancel_cb.as_ref() {
                                cb.call(());
                            }
                        },
                        {cancel_label.unwrap_or("Cancel")}
                    }
                    Button {
                        variant: if danger { ButtonVariant::Destructive } else { ButtonVariant::Primary },
                        on_click: move |_| {
                            open_sig.set(false);
                            if let Some(cb) = confirm_cb.as_ref() {
                                cb.call(());
                            }
                        },
                        {confirm_label.unwrap_or("Confirm")}
                    }
                }
            }
        }
    }
}

/// Alert dialog — a message with a single OK button.
#[component]
pub fn AlertDialog(
    /// Two-way bound open state.
    open: Signal<bool>,
    /// Dialog title.
    title: String,
    /// Message body.
    message: String,
    /// OK button label.
    #[props(optional)]
    ok_label: Option<&'static str>,
    /// Called when the user dismisses the alert.
    #[props(optional)]
    on_close: Option<EventHandler<()>>,
) -> Element {
    let mut open_sig = open;
    let close_cb = on_close;
    let title_for_title = title.clone();
    rsx! {
        if *open_sig.read() {
            div {
                class: "dialog-overlay",
                role: "presentation",
                onclick: move |_| {
                    open_sig.set(false);
                    if let Some(cb) = close_cb.as_ref() {
                        cb.call(());
                    }
                },
                div {
                    class: "dialog",
                    role: "alertdialog",
                    "aria-modal": "true",
                    "aria-label": title,
                    tabindex: "-1",
                    onclick: |ev: MouseEvent| ev.stop_propagation(),
                    onkeydown: move |ev: KeyboardEvent| {
                        if ev.key() == Key::Escape {
                            open_sig.set(false);
                            if let Some(cb) = close_cb.as_ref() {
                                cb.call(());
                            }
                        }
                    },
                    div { class: "dialog-header" }
                    h2 { class: "dialog-title", "{title_for_title}" }
                    div { class: "dialog-body", "{message}" }
                    div { class: "dialog-footer" }
                    Button {
                        variant: ButtonVariant::Primary,
                        on_click: move |_| {
                            open_sig.set(false);
                            if let Some(cb) = close_cb.as_ref() {
                                cb.call(());
                            }
                        },
                        {ok_label.unwrap_or("OK")}
                    }
                }
            }
        }
    }
}

/// Prompt dialog — a message with a text input and OK / Cancel buttons.
#[component]
pub fn PromptDialog(
    /// Two-way bound open state.
    open: Signal<bool>,
    /// Dialog title.
    title: String,
    /// Message body.
    message: String,
    /// Initial / default input value.
    #[props(optional)]
    default_value: Option<String>,
    /// Confirm button label.
    #[props(optional)]
    confirm_label: Option<&'static str>,
    /// Cancel button label.
    #[props(optional)]
    cancel_label: Option<&'static str>,
    /// Called with the entered value when the user confirms.
    #[props(optional)]
    on_submit: Option<EventHandler<String>>,
    /// Called when the user cancels.
    #[props(optional)]
    on_cancel: Option<EventHandler<()>>,
) -> Element {
    let mut open_sig = open;
    let mut input_value = use_signal(|| default_value.unwrap_or_default());
    let submit_cb = on_submit;
    let cancel_cb = on_cancel;
    let title_for_title = title.clone();
    let message_for_msg = message.clone();
    rsx! {
        if *open_sig.read() {
            div {
                class: "dialog-overlay",
                role: "presentation",
                onclick: move |_| {
                    open_sig.set(false);
                    if let Some(cb) = cancel_cb.as_ref() {
                        cb.call(());
                    }
                },
                div {
                    class: "dialog",
                    role: "dialog",
                    "aria-modal": "true",
                    "aria-label": title,
                    tabindex: "-1",
                    onclick: |ev: MouseEvent| ev.stop_propagation(),
                    onkeydown: move |ev: KeyboardEvent| {
                        if ev.key() == Key::Escape {
                            open_sig.set(false);
                            if let Some(cb) = cancel_cb.as_ref() {
                                cb.call(());
                            }
                        }
                    },
                    div { class: "dialog-header" }
                    h2 { class: "dialog-title", "{title_for_title}" }
                    div { class: "dialog-body" }
                    p { class: "mb-2", "{message_for_msg}" }
                    input {
                        r#type: "text",
                        class: "input",
                        value: "{input_value.read()}",
                        onchange: move |ev: FormEvent| {
                            input_value.set(ev.value());
                        },
                        onkeydown: move |ev: KeyboardEvent| {
                            if ev.key() == Key::Enter {
                                open_sig.set(false);
                                if let Some(cb) = submit_cb.as_ref() {
                                    cb.call(input_value.read().clone());
                                }
                            }
                        },
                    }
                    div { class: "dialog-footer" }
                    Button {
                        variant: ButtonVariant::Ghost,
                        on_click: move |_| {
                            open_sig.set(false);
                            if let Some(cb) = cancel_cb.as_ref() {
                                cb.call(());
                            }
                        },
                        {cancel_label.unwrap_or("Cancel")}
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        on_click: move |_| {
                            open_sig.set(false);
                            if let Some(cb) = submit_cb.as_ref() {
                                cb.call(input_value.read().clone());
                            }
                        },
                        {confirm_label.unwrap_or("OK")}
                    }
                }
            }
        }
    }
}
