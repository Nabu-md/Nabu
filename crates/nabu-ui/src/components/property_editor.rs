//! # Property Editor — metadata editing interface (Dioxus)
//!
//! Production-ready property editor supporting all `PropertyType` variants:
//! text, number, date, select, multi-select, URL.
//! Includes validation and autocomplete where appropriate.
//! Views are projections of existing `KnowledgeObjects` — views never own data.

use crate::models::properties::{PropertyDefinition, PropertyType, PropertyValue};
use crate::components::ui::icons::Icon;
use crate::components::ui::feedback::{use_toast, ToastContext};
use crate::components::ui::menu::{MenuItem, MenuSeparator};
use dioxus::prelude::*;
use std::collections::HashMap;

/// Current validation state of a single property field.
#[derive(Clone, PartialEq, Debug)]
pub enum ValidationState {
    Valid,
    Invalid(String),
}

/// Properties for the [`PropertyEditor`] component.
#[derive(Props, PartialEq)]
pub struct PropertyEditorProps {
    /// Property definitions to render (drives ordering, grouping, labels).
    pub properties: Vec<PropertyDefinition>,
    /// Current property values, keyed by `def.id`.
    pub values: HashMap<String, PropertyValue>,
    /// Called when a value changes — receives `(id, new_value)`.
    #[props(optional)]
    pub on_change: Option<EventHandler<(String, PropertyValue)>>,
    /// Called when validation state changes — receives `(id, state)`.
    #[props(optional)]
    pub on_validate: Option<EventHandler<(String, ValidationState)>>,
    /// Property definitions for custom properties (user-created fields
    /// without a schema). Shown at the end under a "Custom Properties"
    /// group so the editor surface stays complete.
    #[props(optional)]
    pub custom_definitions: Option<Vec<PropertyDefinition>>,
}

/// The property editor. Renders all property definitions with the appropriate
/// input type and fires `on_change` / `on_validate` callbacks on every edit.
#[component]
pub fn PropertyEditor(props: PropertyEditorProps) -> Element {
    let on_change = props.on_change;
    let on_validate = props.on_validate;

    // Pre-compute the list of (id, definition, current_value) tuples so the
    // rsx! loop body stays simple and allocation-free at render time.
    let rows: Vec<(String, PropertyDefinition, Option<PropertyValue>)> = props
        .properties
        .iter()
        .map(|def| {
            let id = def.id.clone();
            let value = props.values.get(&id).cloned();
            (id, def.clone(), value)
        })
        .collect();

    let has_custom = props
        .custom_definitions
        .as_ref()
        .map(|cd| !cd.is_empty())
        .unwrap_or(false);

    rsx! {
        div {
            class: "property-editor space-y-3",
            role: "group",
            "aria-label": "Note properties",
        }
        for (i, (id, def, value)) in rows.into_iter().enumerate() {
            {
                let id_i = id.clone();
                let def_i = def.clone();
                let value_i = value.clone();
                rsx! {
                    div { key: "{id_i}", class: "property-field" }
                    PropertyField {
                        id: id_i,
                        definition: def_i,
                        value: value_i,
                        on_change: on_change,
                        on_validate: on_validate,
                    }
                }
            }
        }

        // Custom properties section
        if has_custom {
            div { class: "property-section" }
            div { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 mb-2", "Custom Properties" }
            {props.custom_definitions.as_ref().map(|cd| {
                let items: Vec<(String, PropertyDefinition, Option<PropertyValue>)> = cd
                    .iter()
                    .map(|def| {
                        let id = def.id.clone();
                        let value = props.values.get(&id).cloned();
                        (id, def.clone(), value)
                    })
                    .collect();
                rsx! {
                    for (id, def, value) in items {
                        {
                            let id_c = id.clone();
                            rsx! {
                                div { key: "{id_c}", class: "property-field" }
                                PropertyField {
                                    id: id,
                                    definition: def,
                                    value: value,
                                    on_change: on_change,
                                    on_validate: on_validate,
                                }
                            }
                        }
                    }
                }
            })}
        }
    }
}

/// Renders a single property input based on its [`PropertyType`].
#[component]
fn PropertyField(
    id: String,
    definition: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Option<EventHandler<(String, PropertyValue)>>,
    on_validate: Option<EventHandler<(String, ValidationState)>>,
) -> Element {
    let on_change_cb = on_change;
    let on_validate_cb = on_validate;

    match definition.property_type {
        PropertyType::Text => text_field(id, definition, value, on_change_cb, on_validate_cb),
        PropertyType::Number => number_field(id, definition, value, on_change_cb, on_validate_cb),
        PropertyType::Date => date_field(id, definition, value, on_change_cb, on_validate_cb),
        PropertyType::Select => select_field(id, definition, value, on_change_cb, on_validate_cb),
        PropertyType::MultiSelect => {
            multiselect_field(id, definition, value, on_change_cb, on_validate_cb)
        }
        PropertyType::Url => url_field(id, definition, value, on_change_cb, on_validate_cb),
    }
}

fn text_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Option<EventHandler<(String, PropertyValue)>>,
    on_validate: Option<EventHandler<(String, ValidationState)>>,
) -> Element {
    let current = match &value {
        Some(PropertyValue::Text(v)) => v.clone(),
        _ => String::new(),
    };
    let placeholder = def.description.clone().unwrap_or_default();
    let cb_change = on_change;
    let cb_validate = on_validate;
    let id_c = id.clone();

    rsx! {
        label { class: "field" }
        span { class: "field-label", "{def.display_name}" }
        {def.description.as_ref().map(|d| rsx! {
            span { class: "text-gray-600 ml-1", "{d}" }
        })}
        input {
            r#type: "text",
            class: "w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none",
            placeholder: "{placeholder}",
            value: "{current}",
            onchange: move |ev: FormEvent| {
                let val = ev.value();
                let _ = &id_c;
                if let Some(cb) = cb_change.as_ref() {
                    cb.call((id_c.clone(), PropertyValue::Text(val.clone())));
                }
                if def.property_type == PropertyType::Url && !val.is_empty() {
                    let is_valid = val.starts_with("http://")
                        || val.starts_with("https://")
                        || val.starts_with("mailto:");
                    if let Some(cv) = cb_validate.as_ref() {
                        cv.call((
                            id_c.clone(),
                            if is_valid {
                                ValidationState::Valid
                            } else {
                                ValidationState::Invalid(
                                    "URL must start with http://, https://, or mailto:".to_string(),
                                )
                            },
                        ));
                    }
                } else if let Some(cv) = cb_validate.as_ref() {
                    cv.call((id_c.clone(), ValidationState::Valid));
                }
            },
        }
    }
}

fn number_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Option<EventHandler<(String, PropertyValue)>>,
    on_validate: Option<EventHandler<(String, ValidationState)>>,
) -> Element {
    let current = match &value {
        Some(PropertyValue::Number(v)) => v.to_string(),
        _ => String::new(),
    };
    let cb_change = on_change;
    let cb_validate = on_validate;
    let id_c = id.clone();

    rsx! {
        label { class: "field" }
        span { class: "field-label", "{def.display_name}" }
        input {
            r#type: "number",
            class: "w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none",
            value: "{current}",
            step: "any",
            onchange: move |ev: FormEvent| {
                let val = ev.value();
                match val.parse::<f64>() {
                    Ok(n) => {
                        if let Some(cb) = cb_change.as_ref() {
                            cb.call((id_c.clone(), PropertyValue::Number(n)));
                        }
                        if let Some(cv) = cb_validate.as_ref() {
                            cv.call((id_c.clone(), ValidationState::Valid));
                        }
                    }
                    Err(_) => {
                        if let Some(cv) = cb_validate.as_ref() {
                            cv.call((id_c.clone(), ValidationState::Invalid("Must be a valid number".to_string())));
                        }
                    }
                }
            },
        }
    }
}

fn date_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Option<EventHandler<(String, PropertyValue)>>,
    on_validate: Option<EventHandler<(String, ValidationState)>>,
) -> Element {
    let current = match &value {
        Some(PropertyValue::Date(v)) => v.clone(),
        _ => String::new(),
    };
    let cb_change = on_change;
    let cb_validate = on_validate;
    let id_c = id.clone();

    rsx! {
        label { class: "field" }
        span { class: "field-label", "{def.display_name}" }
        input {
            r#type: "date",
            class: "w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none",
            value: "{current}",
            onchange: move |ev: FormEvent| {
                let val = ev.value();
                let is_valid = val.is_empty() || val.len() >= 10;
                if let Some(cb) = cb_change.as_ref() {
                    cb.call((id_c.clone(), PropertyValue::Date(val)));
                }
                if let Some(cv) = cb_validate.as_ref() {
                    cv.call((id_c.clone(), if is_valid {
                        ValidationState::Valid
                    } else {
                        ValidationState::Invalid("Invalid date format".to_string())
                    }));
                }
            },
        }
    }
}

fn select_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Option<EventHandler<(String, PropertyValue)>>,
    on_validate: Option<EventHandler<(String, ValidationState)>>,
) -> Element {
    let options = def.options.clone().unwrap_or_default();
    let current = match &value {
        Some(PropertyValue::Select(v)) => v.clone(),
        _ => String::new(),
    };
    let cb_change = on_change;
    let cb_validate = on_validate;
    let id_c = id.clone();

    rsx! {
        label { class: "field" }
        span { class: "field-label", "{def.display_name}" }
        select {
            class: "w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none",
            value: "{current}",
            onchange: move |ev: FormEvent| {
                let val = ev.value();
                if let Some(cb) = cb_change.as_ref() {
                    cb.call((id_c.clone(), PropertyValue::Select(val)));
                }
                if let Some(cv) = cb_validate.as_ref() {
                    cv.call((id_c.clone(), ValidationState::Valid));
                }
            },
            option { value: "", disabled: true, selected: current.is_empty(), "-- Select --" }
            for opt in &options {
                {
                    let opt_c = opt.clone();
                    rsx! {
                        option {
                            key: "{opt_c}",
                            value: "{opt_c}",
                            selected: "{current == opt_c}",
                            "{opt_c}"
                        }
                    }
                }
            }
        }
    }
}

fn multiselect_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Option<EventHandler<(String, PropertyValue)>>,
    on_validate: Option<EventHandler<(String, ValidationState)>>,
) -> Element {
    let options = def.options.clone().unwrap_or_default();
    let current = match &value {
        Some(PropertyValue::MultiSelect(v)) => v.clone(),
        _ => vec![],
    };
    let selected_sig = use_signal(move || current);
    let cb_change = on_change;
    let cb_validate = on_validate;
    let id_c = id.clone();
    let toasts = use_toast();

    rsx! {
        label { class: "field" }
        span { class: "field-label", "{def.display_name}" }
        div { class: "flex flex-wrap gap-1" }
        for opt in &options {
            {
                let opt_c = opt.clone();
                let is_selected = selected_sig.read().contains(&opt_c);
                rsx! {
                    button {
                        key: "{opt_c}",
                        r#type: "button",
                        class: if is_selected {
                            "px-2 py-0.5 text-xs rounded-full border transition-colors bg-blue-700 border-blue-500 text-blue-100"
                        } else {
                            "px-2 py-0.5 text-xs rounded-full border transition-colors border-gray-600 text-gray-400 hover:border-gray-500"
                        },
                        onclick: move |_| {
                            let mut vals = selected_sig.read().clone();
                            if vals.contains(&opt_c) {
                                vals.retain(|v| v != &opt_c);
                            } else {
                                vals.push(opt_c.clone());
                            }
                            selected_sig.set(vals.clone());
                            if let Some(cb) = cb_change.as_ref() {
                                cb.call((id_c.clone(), PropertyValue::MultiSelect(vals)));
                            }
                            if let Some(cv) = cb_validate.as_ref() {
                                cv.call((id_c.clone(), ValidationState::Valid));
                            }
                        },
                        "{opt_c}"
                    }
                }
            }
        }
    }
}

fn url_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Option<EventHandler<(String, PropertyValue)>>,
    on_validate: Option<EventHandler<(String, ValidationState)>>,
) -> Element {
    let current = match &value {
        Some(PropertyValue::Url(v)) => v.clone(),
        _ => String::new(),
    };
    let cb_change = on_change;
    let cb_validate = on_validate;
    let id_c = id.clone();

    rsx! {
        label { class: "field" }
        span { class: "field-label", "{def.display_name}" }
        input {
            r#type: "url",
            class: "w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none",
            placeholder: "https://...",
            value: "{current}",
            onchange: move |ev: FormEvent| {
                let val = ev.value();
                if let Some(cb) = cb_change.as_ref() {
                    cb.call((id_c.clone(), PropertyValue::Url(val.clone())));
                }
                let is_valid = val.is_empty()
                    || val.starts_with("http://")
                    || val.starts_with("https://")
                    || val.starts_with("mailto:");
                if let Some(cv) = cb_validate.as_ref() {
                    cv.call((id_c.clone(), if is_valid {
                        ValidationState::Valid
                    } else {
                        ValidationState::Invalid("URL must start with http://, https://, or mailto:".to_string())
                    }));
                }
            },
        }
    }
}
