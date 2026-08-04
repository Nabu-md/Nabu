//! Input primitives — text, textarea, search, password, number.
//!
//! All inputs are controlled: pass an `RwSignal` for two-way binding. Each
//! supports a label, hint, and error state (`.field-error` / `.input-error`).

use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;

/// Renders the field chrome (label + control + hint/error) around any control.
fn field_chrome(
    label: Option<&'static str>,
    control: impl IntoView + 'static,
    hint: Option<&'static str>,
    error: Option<&'static str>,
) -> impl IntoView {
    let error_view =
        error.map(|e| view! { <span class="field-error" role="alert">{e}</span> }.into_any());
    view! {
        <label class="field">
            {label.map(|l| view! { <span class="field-label">{l}</span> }.into_any())}
            {control}
            {error_view}
            {hint.map(|h| view! { <span class="field-hint">{h}</span> }.into_any())}
        </label>
    }
}

/// Shared optional props for all inputs.
#[derive(Clone, Copy)]
pub struct InputProps {
    pub label: Option<&'static str>,
    pub hint: Option<&'static str>,
    pub error: Option<&'static str>,
    pub placeholder: Option<&'static str>,
    pub class: Option<&'static str>,
    pub disabled: bool,
}

impl Default for InputProps {
    fn default() -> Self {
        Self {
            label: None,
            hint: None,
            error: None,
            placeholder: None,
            class: None,
            disabled: false,
        }
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

/// Text input with two-way binding to an `RwSignal<String>`.
#[component]
pub fn TextInput(
    /// Two-way bound value.
    value: RwSignal<String>,
    /// Optional label shown above the input.
    #[prop(optional)]
    label: Option<&'static str>,
    /// Optional helper text below.
    #[prop(optional)]
    hint: Option<&'static str>,
    /// Error message — also enables the error styling.
    #[prop(optional)]
    error: Option<&'static str>,
    /// Placeholder text.
    #[prop(optional)]
    placeholder: Option<&'static str>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[prop(optional)]
    disabled: bool,
    /// Called on every keystroke with the new value.
    #[prop(optional)]
    on_input: Option<Callback<String>>,
) -> impl IntoView {
    let classes = input_classes(error, class);
    let on_input_cb = on_input;
    let control = view! {
        <input
            type="text"
            class=classes
            prop:value=value
            placeholder=placeholder
            disabled=disabled
            on:input=move |ev| {
                let new_value = event_target_value(&ev);
                value.set(new_value.clone());
                if let Some(cb) = on_input_cb.as_ref() {
                    cb.run(new_value);
                }
            }
        />
    };
    field_chrome(label, control, hint, error)
}

/// Multi-line text area with two-way binding.
#[component]
pub fn Textarea(
    /// Two-way bound value.
    value: RwSignal<String>,
    /// Optional label.
    #[prop(optional)]
    label: Option<&'static str>,
    /// Optional helper text.
    #[prop(optional)]
    hint: Option<&'static str>,
    /// Error message.
    #[prop(optional)]
    error: Option<&'static str>,
    /// Placeholder.
    #[prop(optional)]
    placeholder: Option<&'static str>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[prop(optional)]
    disabled: bool,
    /// Minimum number of visible rows.
    #[prop(optional)]
    rows: Option<u32>,
    /// Called on every keystroke.
    #[prop(optional)]
    on_input: Option<Callback<String>>,
) -> impl IntoView {
    let mut base = String::from("textarea");
    if error.is_some() {
        base.push_str(" input-error");
    }
    if let Some(extra) = class {
        base.push(' ');
        base.push_str(extra);
    }
    let on_input_cb = on_input;
    let control = view! {
        <textarea
            class=base
            prop:value=value
            placeholder=placeholder
            disabled=disabled
            rows=rows.unwrap_or(4)
            on:input=move |ev| {
                let new_value = event_target_value(&ev);
                value.set(new_value.clone());
                if let Some(cb) = on_input_cb.as_ref() {
                    cb.run(new_value);
                }
            }
        ></textarea>
    };
    field_chrome(label, control, hint, error)
}

/// Search input — text input with a magnifier affordance.
#[component]
pub fn SearchInput(
    /// Two-way bound value.
    value: RwSignal<String>,
    /// Placeholder.
    #[prop(optional)]
    placeholder: Option<&'static str>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[prop(optional)]
    disabled: bool,
    /// Called on every keystroke.
    #[prop(optional)]
    on_input: Option<Callback<String>>,
) -> impl IntoView {
    let classes = input_classes(None, class);
    let on_input_cb = on_input;
    view! {
        <div class="relative w-full">
            <span class="absolute left-2 top-1/2 -translate-y-1/2 text-gray-500 text-xs" aria-hidden="true">
                {render_icon_view(Icon::Search)}
            </span>
            <input
                type="search"
                class=format!("{classes} pl-7")
                prop:value=value
                placeholder=placeholder
                disabled=disabled
                on:input=move |ev| {
                    let new_value = event_target_value(&ev);
                    value.set(new_value.clone());
                    if let Some(cb) = on_input_cb.as_ref() {
                        cb.run(new_value);
                    }
                }
            />
        </div>
    }
}

/// Password input — text input with show/hide toggle.
#[component]
pub fn PasswordInput(
    /// Two-way bound value.
    value: RwSignal<String>,
    /// Optional label.
    #[prop(optional)]
    label: Option<&'static str>,
    /// Optional helper text.
    #[prop(optional)]
    hint: Option<&'static str>,
    /// Error message.
    #[prop(optional)]
    error: Option<&'static str>,
    /// Placeholder.
    #[prop(optional)]
    placeholder: Option<&'static str>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[prop(optional)]
    disabled: bool,
) -> impl IntoView {
    let (revealed, set_revealed) = signal(false);
    let classes = input_classes(error, class);
    let control = view! {
        <div class="relative w-full">
            <input
                type=move || if revealed.get() { "text" } else { "password" }
                class=format!("{classes} pr-8")
                prop:value=value
                placeholder=placeholder
                disabled=disabled
                on:input=move |ev| {
                    let new_value = event_target_value(&ev);
                    value.set(new_value.clone());
                }
            />
            <button
                type="button"
                class="absolute right-1 top-1/2 -translate-y-1/2 px-1 text-xs text-gray-500 hover:text-gray-300"
                aria-label=move || if revealed.get() { "Hide password" } else { "Show password" }
                on:click=move |_| set_revealed.update(|r| *r = !*r)
            >
                {move || if revealed.get() {
                    render_icon_view(Icon::EyeOff)
                } else {
                    render_icon_view(Icon::Eye)
                }}
            </button>
        </div>
    };
    field_chrome(label, control, hint, error)
}

/// Number input with min/max/step bounds and two-way binding.
#[component]
pub fn NumberInput(
    /// Two-way bound value.
    value: RwSignal<f64>,
    /// Optional label.
    #[prop(optional)]
    label: Option<&'static str>,
    /// Optional helper text.
    #[prop(optional)]
    hint: Option<&'static str>,
    /// Error message.
    #[prop(optional)]
    error: Option<&'static str>,
    /// Minimum value.
    #[prop(optional)]
    min: Option<f64>,
    /// Maximum value.
    #[prop(optional)]
    max: Option<f64>,
    /// Step increment.
    #[prop(optional)]
    step: Option<f64>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables the input.
    #[prop(optional)]
    disabled: bool,
) -> impl IntoView {
    let classes = input_classes(error, class);
    let control = view! {
        <input
            type="number"
            class=classes
            prop:value=move || value.get().to_string()
            min=min
            max=max
            step=step.unwrap_or(1.0)
            disabled=disabled
            on:input=move |ev| {
                let raw = event_target_value(&ev);
                if let Ok(parsed) = raw.parse::<f64>() {
                    value.set(parsed);
                }
            }
        />
    };
    field_chrome(label, control, hint, error)
}
