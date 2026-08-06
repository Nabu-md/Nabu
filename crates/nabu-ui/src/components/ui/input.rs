//! Input primitives — text, textarea, search, password, number.
//!
//! All inputs are controlled: pass an `Signal<T>` for two-way binding. Each
//! supports a label, hint, and error state (`.field-error` / `.input-error`).

use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

/// Renders the field chrome (label + control + hint/error) around any control.
fn field_chrome(
    label: Option<&'static str>,
    control: Element,
    hint: Option<&'static str>,
    error: Option<&'static str>,
) -> Element {
    rsx! {
        label { class: "field" }
        {label.map(|l| rsx! { span { class: "field-label", "{l}" } })}
        {control}
        {error.map(|e| rsx! { span { class: "field-error", role: "alert", "{e}" } })}
        {hint.map(|h| rsx! { span { class: "field-hint", "{h}" } })}
    }
}

fn input_classes(error: Option<&str>, extra: Option<&str>) -> String {
    let mut base = String::from("input");
    if error.is_some() {
        base.push_str(" input-error");
    }
    if let Some(extra) = extra {
        base.push(' ');
        base.push_str(extra);
    }
    base
}

/// Text input with two-way binding to a `Signal<String>`.
#[component]
pub fn TextInput(
    /// Two-way bound value.
    value: Signal<String>,
    /// Optional label shown above the input.
    #[props(optional)]
    label: Option<&'static str>,
    /// Optional helper text below.
    #[props(optional)]
    hint: Option<&'static str>,
    /// Error message — also enables the error styling.
    #[props(optional)]
    error: Option<&'static str>,
    /// Placeholder text.
    #[props(optional)]
    placeholder: Option<&'static str>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[props(optional)]
    disabled: bool,
    /// Called on every keystroke with the new value.
    #[props(optional)]
    on_input: Option<EventHandler<String>>,
) -> Element {
    let classes = input_classes(error, class);
    let mut val = value;
    let on_input_cb = on_input;
    let control = rsx! {
        input {
            r#type: "text",
            class: classes,
            value: "{val.read()}",
            placeholder: placeholder,
            disabled: disabled,
            onchange: move |ev: FormEvent| {
                let new_value = ev.value();
                val.set(new_value.clone());
                if let Some(cb) = on_input_cb.as_ref() {
                    cb.call(new_value);
                }
            },
        }
    };
    field_chrome(label, control, hint, error)
}

/// Multi-line text area with two-way binding.
#[component]
pub fn Textarea(
    /// Two-way bound value.
    value: Signal<String>,
    /// Optional label.
    #[props(optional)]
    label: Option<&'static str>,
    /// Optional helper text.
    #[props(optional)]
    hint: Option<&'static str>,
    /// Error message.
    #[props(optional)]
    error: Option<&'static str>,
    /// Placeholder.
    #[props(optional)]
    placeholder: Option<&'static str>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[props(optional)]
    disabled: bool,
    /// Minimum number of visible rows.
    #[props(optional)]
    rows: Option<u32>,
    /// Called on every keystroke.
    #[props(optional)]
    on_input: Option<EventHandler<String>>,
) -> Element {
    let mut base = String::from("textarea");
    if error.is_some() {
        base.push_str(" input-error");
    }
    if let Some(extra) = class {
        base.push(' ');
        base.push_str(extra);
    }
    let mut val = value;
    let on_input_cb = on_input;
    let control = rsx! {
        textarea {
            class: base,
            value: "{val.read()}",
            placeholder: placeholder,
            disabled: disabled,
            rows: rows.unwrap_or(4),
            onchange: move |ev: FormEvent| {
                let new_value = ev.value();
                val.set(new_value.clone());
                if let Some(cb) = on_input_cb.as_ref() {
                    cb.call(new_value);
                }
            },
        }
    };
    field_chrome(label, control, hint, error)
}

/// Search input — text input with a magnifier affordance.
#[component]
pub fn SearchInput(
    /// Two-way bound value.
    value: Signal<String>,
    /// Placeholder.
    #[props(optional)]
    placeholder: Option<&'static str>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[props(optional)]
    disabled: bool,
    /// Called on every keystroke.
    #[props(optional)]
    on_input: Option<EventHandler<String>>,
) -> Element {
    let classes = input_classes(None, class);
    let mut val = value;
    let on_input_cb = on_input;
    rsx! {
        div { class: "relative w-full" }
        span {
            class: "absolute left-2 top-1/2 -translate-y-1/2 text-gray-500 text-xs",
            "aria-hidden": "true",
            {render_icon_view(Icon::Search)}
        }
        input {
            r#type: "search",
            class: "{classes} pl-7",
            value: "{val.read()}",
            placeholder: placeholder,
            disabled: disabled,
            onchange: move |ev: FormEvent| {
                let new_value = ev.value();
                val.set(new_value.clone());
                if let Some(cb) = on_input_cb.as_ref() {
                    cb.call(new_value);
                }
            },
        }
    }
}

/// Password input — text input with show/hide toggle.
#[component]
pub fn PasswordInput(
    /// Two-way bound value.
    value: Signal<String>,
    /// Optional label.
    #[props(optional)]
    label: Option<&'static str>,
    /// Optional helper text.
    #[props(optional)]
    hint: Option<&'static str>,
    /// Error message.
    #[props(optional)]
    error: Option<&'static str>,
    /// Placeholder.
    #[props(optional)]
    placeholder: Option<&'static str>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[props(optional)]
    disabled: bool,
) -> Element {
    let mut revealed = use_signal(|| false);
    let classes = input_classes(error, class);
    let mut val = value;
    rsx! {
        div { class: "relative w-full" }
        input {
            r#type: if *revealed.read() { "text" } else { "password" },
            class: "{classes} pr-8",
            value: "{val.read()}",
            placeholder: placeholder,
            disabled: disabled,
            onchange: move |ev: FormEvent| {
                val.set(ev.value());
            },
        }
        button {
            r#type: "button",
            class: "absolute right-1 top-1/2 -translate-y-1/2 px-1 text-xs text-gray-500 hover:text-gray-300",
            "aria-label": if *revealed.read() { "Hide password" } else { "Show password" },
            onclick: move |_| revealed.toggle(),
            if *revealed.read() {
                {render_icon_view(Icon::EyeOff)}
            } else {
                {render_icon_view(Icon::Eye)}
            }
        }
    }
}

/// Number input with min/max/step bounds and two-way binding.
#[component]
pub fn NumberInput(
    /// Two-way bound value.
    value: Signal<f64>,
    /// Optional label.
    #[props(optional)]
    label: Option<&'static str>,
    /// Optional helper text.
    #[props(optional)]
    hint: Option<&'static str>,
    /// Error message.
    #[props(optional)]
    error: Option<&'static str>,
    /// Minimum value.
    #[props(optional)]
    min: Option<f64>,
    /// Maximum value.
    #[props(optional)]
    max: Option<f64>,
    /// Step increment.
    #[props(optional)]
    step: Option<f64>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[props(optional)]
    disabled: bool,
) -> Element {
    let classes = input_classes(error, class);
    let mut val = value;
    rsx! {
        input {
            r#type: "number",
            class: classes,
            value: "{*val.read()}",
            min: min,
            max: max,
            step: step.unwrap_or(1.0),
            disabled: disabled,
            onchange: move |ev: FormEvent| {
                let raw = ev.value();
                if let Ok(parsed) = raw.parse::<f64>() {
                    val.set(parsed);
                }
            },
        }
    }
}
