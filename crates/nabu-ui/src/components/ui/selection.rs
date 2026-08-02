//! Selection controls — checkbox, radio, switch, segmented control, select.
//!
//! Checkbox / radio / switch use native inputs for accessibility
//! (`accent-color` styling from the design system); segmented and select use
//! styled buttons / native select.

use leptos::prelude::*;

/// Checkbox with two-way binding to `RwSignal<bool>`.
#[component]
pub fn Checkbox(
    /// Two-way bound checked state.
    checked: RwSignal<bool>,
    /// Accessible label text.
    label: String,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the control.
    #[prop(optional)]
    disabled: bool,
    /// Called when the value changes.
    #[prop(optional)]
    on_change: Option<Callback<bool>>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let cb = on_change;
    view! {
        <label class=format!("check-field{extra}")>
            <input
                type="checkbox"
                prop:checked=checked
                disabled=disabled
                on:change=move |ev| {
                    let checked_now = event_target_checked(&ev);
                    checked.set(checked_now);
                    if let Some(cb) = cb.as_ref() {
                        cb.run(checked_now);
                    }
                }
            />
            <span>{label}</span>
        </label>
    }
}

/// Radio button group item. Pair with a shared `name` and selected value signal.
#[component]
pub fn Radio(
    /// Radio group name.
    name: String,
    /// Value this radio represents.
    value: String,
    /// Currently selected value signal.
    selected: RwSignal<String>,
    /// Accessible label text.
    label: String,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the control.
    #[prop(optional)]
    disabled: bool,
    /// Called when this radio becomes selected.
    #[prop(optional)]
    on_change: Option<Callback<String>>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let value_for_check = value.clone();
    let cb = on_change;
    view! {
        <label class=format!("radio-field{extra}")>
            <input
                type="radio"
                name=name
                prop:checked=move || selected.get() == value_for_check
                value=value.clone()
                disabled=disabled
                on:change=move |_| {
                    selected.set(value.clone());
                    if let Some(cb) = cb.as_ref() {
                        cb.run(value.clone());
                    }
                }
            />
            <span>{label}</span>
        </label>
    }
}

/// Switch (toggle) with two-way binding to `RwSignal<bool>`.
#[component]
pub fn Switch(
    /// Two-way bound checked state.
    checked: RwSignal<bool>,
    /// Accessible label text.
    label: String,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the control.
    #[prop(optional)]
    disabled: bool,
    /// Called when the value changes.
    #[prop(optional)]
    on_change: Option<Callback<bool>>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let cb = on_change;
    view! {
        <label class=format!("switch{extra}")>
            <input
                type="checkbox"
                role="switch"
                aria-label=label
                prop:checked=checked
                disabled=disabled
                on:change=move |ev| {
                    let checked_now = event_target_checked(&ev);
                    checked.set(checked_now);
                    if let Some(cb) = cb.as_ref() {
                        cb.run(checked_now);
                    }
                }
            />
            <span class="switch-track" aria-hidden="true"></span>
            <span class="switch-thumb" aria-hidden="true"></span>
        </label>
    }
}

/// One option of a [`Segmented`] control.
#[derive(Clone)]
pub struct SegmentedOption {
    pub value: String,
    pub label: String,
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
    /// Options to display.
    options: Vec<SegmentedOption>,
    /// Currently selected value signal.
    selected: RwSignal<String>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the whole control.
    #[prop(optional)]
    disabled: bool,
    /// Called when selection changes.
    #[prop(optional)]
    on_change: Option<Callback<String>>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let cb = on_change;
    view! {
        <div class=format!("segmented{extra}") role="radiogroup">
            {options.into_iter().map(|opt| {
                let value_checked = opt.value.clone();
                let value_class = opt.value.clone();
                let value_click = opt.value.clone();
                let label = opt.label.clone();
                let on_click_cb = cb;
                view! {
                    <button
                        type="button"
                        role="radio"
                        aria-checked=move || selected.get() == value_checked
                        class=move || if selected.get() == value_class { "segmented-active" } else { "" }
                        disabled=disabled
                        on:click=move |_| {
                            selected.set(value_click.clone());
                            if let Some(cb) = on_click_cb.as_ref() {
                                cb.run(value_click.clone());
                            }
                        }
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

/// One option of a [`Select`] dropdown.
#[derive(Clone)]
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
    /// Options to display.
    options: Vec<SelectOption>,
    /// Two-way bound selected value.
    value: RwSignal<String>,
    /// Optional label.
    #[prop(optional)]
    label: Option<&'static str>,
    /// Optional helper text.
    #[prop(optional)]
    hint: Option<&'static str>,
    /// Optional error message.
    #[prop(optional)]
    error: Option<&'static str>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the control.
    #[prop(optional)]
    disabled: bool,
    /// Called when selection changes.
    #[prop(optional)]
    on_change: Option<Callback<String>>,
) -> impl IntoView {
    let mut base = String::from("input");
    if error.is_some() {
        base.push_str(" input-error");
    }
    if let Some(extra) = class {
        base.push(' ');
        base.push_str(extra);
    }
    let cb = on_change;
    let control = view! {
        <select
            class=base
            prop:value=value
            disabled=disabled
            on:change=move |ev| {
                let new_value = event_target_value(&ev);
                value.set(new_value.clone());
                if let Some(cb) = cb.as_ref() {
                    cb.run(new_value);
                }
            }
        >
            {options.into_iter().map(|opt| view! {
                <option value=opt.value.clone()>{opt.label}</option>
            }).collect_view()}
        </select>
    };
    view! {
        <label class="field">
            {label.map(|l| view! { <span class="field-label">{l}</span> }.into_any())}
            {control}
            {error.map(|e| view! { <span class="field-error" role="alert">{e}</span> }.into_any())}
            {hint.map(|h| view! { <span class="field-hint">{h}</span> }.into_any())}
        </label>
    }
}
