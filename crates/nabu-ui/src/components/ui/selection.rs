//! Selection controls — checkbox, radio, switch, segmented control, select.
//!
//! Checkbox / radio / switch use native inputs for accessibility
//! (`accent-color` styling from the design system); segmented and select use
//! styled buttons / native select.

use dioxus::prelude::*;

/// Checkbox with two-way binding to `Signal<bool>`.
#[component]
pub fn Checkbox(
    checked: Signal<bool>,
    label: String,
    #[props(optional)] class: Option<&'static str>,
    #[props(optional)] disabled: bool,
    #[props(optional)] on_change: Option<EventHandler<bool>>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let checked_sig = checked;
    let cb = on_change;
    rsx! {
        label { class: "check-field{extra}" }
        input {
            r#type: "checkbox",
            checked: *checked_sig.read(),
            disabled: disabled,
            onchange: move |ev: FormEvent| {
                let checked_now = ev.checked();
                let mut s = checked_sig;
                s.set(checked_now);
                if let Some(cb) = cb.as_ref() {
                    cb.call(checked_now);
                }
            },
        }
        span { "{label}" }
    }
}

/// Radio button group item.
#[component]
pub fn Radio(
    name: String,
    value: String,
    selected: Signal<String>,
    label: String,
    #[props(optional)] class: Option<&'static str>,
    #[props(optional)] disabled: bool,
    #[props(optional)] on_change: Option<EventHandler<String>>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let value_for_check = value.clone();
    let selected_sig = selected;
    let cb = on_change;
    rsx! {
        label { class: "radio-field{extra}" }
        input {
            r#type: "radio",
            name: name,
            checked: *selected_sig.read() == value_for_check,
            value: value_for_check,
            disabled: disabled,
            onchange: move |_| {
                let mut s = selected_sig;
                s.set(value.clone());
                if let Some(cb) = cb.as_ref() {
                    cb.call(value.clone());
                }
            },
        }
        span { "{label}" }
    }
}

/// Switch (toggle) with two-way binding to `Signal<bool>`.
#[component]
pub fn Switch(
    checked: Signal<bool>,
    label: String,
    #[props(optional)] class: Option<&'static str>,
    #[props(optional)] disabled: bool,
    #[props(optional)] on_change: Option<EventHandler<bool>>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let checked_sig = checked;
    let cb = on_change;
    rsx! {
        label { class: "switch{extra}" }
        input {
            r#type: "checkbox",
            role: "switch",
            "aria-label": label,
            checked: *checked_sig.read(),
            disabled: disabled,
            onchange: move |ev: FormEvent| {
                let checked_now = ev.checked();
                let mut s = checked_sig;
                s.set(checked_now);
                if let Some(cb) = cb.as_ref() {
                    cb.call(checked_now);
                }
            },
        }
        span { class: "switch-track", "aria-hidden": "true" }
        span { class: "switch-thumb", "aria-hidden": "true" }
    }
}

/// One option of a [`Segmented`] control.
#[derive(Clone)]
pub struct SegmentedOption {
    pub value: String,
    pub label: String,
}

impl PartialEq for SegmentedOption {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.label == other.label
    }
}

impl SegmentedOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

/// Segmented control — a row of mutually exclusive buttons.
#[component]
pub fn Segmented(
    options: Vec<SegmentedOption>,
    selected: Signal<String>,
    #[props(optional)] class: Option<&'static str>,
    #[props(optional)] disabled: bool,
    #[props(optional)] on_change: Option<EventHandler<String>>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let selected_sig = selected;
    let cb = on_change;
    rsx! {
        div { class: "segmented{extra}", role: "radiogroup" }
        for opt in options {
            {
                let value_checked = opt.value.clone();
                let value_click = opt.value.clone();
                let label = opt.label.clone();
                rsx! {
                    button {
                        key: "{value_checked}",
                        r#type: "button",
                        role: "radio",
                        "aria-checked": "{value_checked == *selected_sig.read()}",
                        class: if value_checked == *selected_sig.read() { "segmented-active" } else { "" },
                        disabled: disabled,
                        onclick: move |_| {
                            let mut s = selected_sig;
                            s.set(value_click.clone());
                            if let Some(cb) = cb.as_ref() {
                                cb.call(value_click.clone());
                            }
                        },
                        "{label}"
                    }
                }
            }
        }
    }
}

/// One option of a [`Select`] dropdown.
#[derive(Clone, PartialEq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

/// Native select dropdown, styled via the design system.
#[component]
pub fn Select(
    options: Vec<SelectOption>,
    value: Signal<String>,
    #[props(optional)] label: Option<&'static str>,
    #[props(optional)] hint: Option<&'static str>,
    #[props(optional)] error: Option<&'static str>,
    #[props(optional)] class: Option<&'static str>,
    #[props(optional)] disabled: bool,
    #[props(optional)] on_change: Option<EventHandler<String>>,
) -> Element {
    let mut base = String::from("input");
    if error.is_some() {
        base.push_str(" input-error");
    }
    if let Some(extra) = class {
        base.push(' ');
        base.push_str(extra);
    }
    let val = value;
    let cb = on_change;
    rsx! {
        label { class: "field" }
        {label.map(|l| rsx! { span { class: "field-label", "{l}" } })}
        select {
            class: base,
            value: "{val.read()}",
            disabled: disabled,
            onchange: move |ev: FormEvent| {
                let new_value = ev.value();
                let mut s = val;
                s.set(new_value.clone());
                if let Some(cb) = cb.as_ref() {
                    cb.call(new_value);
                }
            },
            for opt in &options {
                option {
                    key: "{opt.value}",
                    value: "{opt.value}",
                    "{opt.label}"
                }
            }
        }
        {error.map(|e| rsx! { span { class: "field-error", role: "alert", "{e}" } })}
        {hint.map(|h| rsx! { span { class: "field-hint", "{h}" } })}
    }
}
