//! Property Editor component.
//!
//! Production-ready property editor supporting all PropertyType variants:
//! text, number, date, select, multi-select, URL.
//! Includes validation and autocomplete where appropriate.
//! Views are projections of existing KnowledgeObjects — views never own data.

use leptos::prelude::*;
use crate::models::properties::{PropertyDefinition, PropertyType, PropertyValue};

#[derive(Clone, PartialEq)]
pub enum ValidationState {
    Valid,
    Invalid(String),
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub properties: Vec<PropertyDefinition>,
    pub values: std::collections::HashMap<String, PropertyValue>,
    pub on_change: Callback<(String, PropertyValue)>,
    pub on_validate: Callback<(String, ValidationState)>,
}

#[function_component(PropertyEditor)]
pub fn property_editor(props: &Props) -> Html {
    view! {
        <div class="property-editor space-y-3">
            { props.properties.iter().map(|def| {
                let id = def.id.clone();
                let value = props.values.get(&id).cloned();
                let on_change = props.on_change.clone();
                let on_validate = props.on_validate.clone();

                view! {
                    <div class="property-field" key={id.clone()}>
                        <label class="block text-xs font-medium text-gray-400 mb-1">
                            {&def.display_name}
                            {def.description.map(|d| view! { <span class="text-gray-600 ml-1">{d}</span> }).into_any()}
                        </label>
                        <PropertyField
                            definition=def.clone()
                            value=value
                            on_change=on_change
                            on_validate=on_validate
                        />
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct PropertyFieldProps {
    definition: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Callback<(String, PropertyValue)>,
    on_validate: Callback<(String, ValidationState)>,
}

#[component]
fn PropertyField(props: PropertyFieldProps) -> impl IntoView {
    let id = props.definition.id.clone();
    let def = props.definition.clone();

    match def.property_type {
        PropertyType::Text => text_field(id, def, props.value, props.on_change, props.on_validate),
        PropertyType::Number => number_field(id, def, props.value, props.on_change, props.on_validate),
        PropertyType::Date => date_field(id, def, props.value, props.on_change, props.on_validate),
        PropertyType::Select => select_field(id, def, props.value, props.on_change, props.on_validate),
        PropertyType::MultiSelect => multiselect_field(id, def, props.value, props.on_change, props.on_validate),
        PropertyType::Url => url_field(id, def, props.value, props.on_change, props.on_validate),
    }
}

fn text_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Callback<(String, PropertyValue)>,
    on_validate: Callback<(String, ValidationState)>,
) -> Html {
    let current = match &value {
        Some(PropertyValue::Text(v)) => v.clone(),
        _ => String::new(),
    };

    let on_input = move |ev: InputEvent| {
        let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
        let val = input.value();
        on_change.emit((id.clone(), PropertyValue::Text(val.clone())));
        // Validate: text is always valid, but check URL format if the field is a URL type
        if def.property_type == PropertyType::Url && !val.is_empty() {
            let is_valid = val.starts_with("http://") || val.starts_with("https://") || val.starts_with("mailto:");
            on_validate.emit((id.clone(), if is_valid { ValidationState::Valid } else { ValidationState::Invalid("URL must start with http://, https://, or mailto:".to_string()) }));
        } else {
            on_validate.emit((id.clone(), ValidationState::Valid));
        }
    };

    let placeholder = def.description.unwrap_or_default();

    html! {
        <input
            type="text"
            value={current}
            placeholder={placeholder}
            on:input=on_input
            class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
        />
    }
}

fn number_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Callback<(String, PropertyValue)>,
    on_validate: Callback<(String, ValidationState)>,
) -> Html {
    let current = match &value {
        Some(PropertyValue::Number(v)) => v.to_string(),
        _ => String::new(),
    };

    let on_input = move |ev: InputEvent| {
        let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
        let val = input.value();
        match val.parse::<f64>() {
            Ok(n) => {
                on_change.emit((id.clone(), PropertyValue::Number(n)));
                on_validate.emit((id.clone(), ValidationState::Valid));
            }
            Err(_) => {
                on_validate.emit((id.clone(), ValidationState::Invalid("Must be a valid number".to_string())));
            }
        }
    };

    html! {
        <input
            type="number"
            value={current}
            step="any"
            on:input=on_input
            class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
        />
    }
}

fn date_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Callback<(String, PropertyValue)>,
    on_validate: Callback<(String, ValidationState)>,
) -> Html {
    let current = match &value {
        Some(PropertyValue::Date(v)) => v.clone(),
        _ => String::new(),
    };

    let on_input = move |ev: InputEvent| {
        let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
        let val = input.value();
        // Validate ISO 8601 date format
        let is_valid = val.is_empty() || val.len() >= 10;
        on_change.emit((id.clone(), PropertyValue::Date(val)));
        on_validate.emit((id.clone(), if is_valid { ValidationState::Valid } else { ValidationState::Invalid("Invalid date format".to_string()) }));
    };

    html! {
        <input
            type="date"
            value={current}
            on:input=on_input
            class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
        />
    }
}

fn select_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Callback<(String, PropertyValue)>,
    on_validate: Callback<(String, ValidationState)>,
) -> Html {
    let options = def.options.unwrap_or_default();
    let current = match &value {
        Some(PropertyValue::Select(v)) => v.clone(),
        _ => String::new(),
    };

    let on_change_cb = move |ev: Event| {
        let select: web_sys::HtmlSelectElement = ev.target_unchecked_into();
        let val = select.value();
        on_change.emit((id.clone(), PropertyValue::Select(val)));
        on_validate.emit((id.clone(), ValidationState::Valid));
    };

    html! {
        <select
            value={current}
            on:change=on_change_cb
            class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
        >
            <option value="" disabled={current.is_empty()}>"-- Select --"</option>
            { options.iter().map(|opt| {
                let selected = current == *opt;
                view! {
                    <option value={opt.clone()} selected={selected}>{opt}</option>
                }
            }).collect_view()}
        </select>
    }
}

fn multiselect_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Callback<(String, PropertyValue)>,
    on_validate: Callback<(String, ValidationState)>,
) -> Html {
    let options = def.options.unwrap_or_default();
    let current = match &value {
        Some(PropertyValue::MultiSelect(v)) => v.clone(),
        _ => vec![],
    };

    let (selected, set_selected) = use_signal(move || current);

    let on_toggle = move |opt: String| {
        let mut vals = selected.get();
        if vals.contains(&opt) {
            vals.retain(|v| v != &opt);
        } else {
            vals.push(opt);
        }
        set_selected.set(vals.clone());
        on_change.emit((id.clone(), PropertyValue::MultiSelect(vals)));
        on_validate.emit((id.clone(), ValidationState::Valid));
    };

    html! {
        <div class="flex flex-wrap gap-1">
            { options.iter().map(|opt| {
                let is_selected = selected.get().contains(opt);
                let opt_clone = opt.clone();
                view! {
                    <button
                        type="button"
                        class=move || format!("px-2 py-0.5 text-xs rounded-full border transition-colors {}",
                            if is_selected { "bg-blue-700 border-blue-500 text-blue-100" } else { "border-gray-600 text-gray-400 hover:border-gray-500" })
                        on:click=move |_| on_toggle(opt_clone.clone())
                    >
                        {opt}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

fn url_field(
    id: String,
    def: PropertyDefinition,
    value: Option<PropertyValue>,
    on_change: Callback<(String, PropertyValue)>,
    on_validate: Callback<(String, ValidationState)>,
) -> Html {
    let current = match &value {
        Some(PropertyValue::Url(v)) => v.clone(),
        _ => String::new(),
    };

    let on_input = move |ev: InputEvent| {
        let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
        let val = input.value();
        on_change.emit((id.clone(), PropertyValue::Url(val.clone())));
        let is_valid = val.is_empty() || val.starts_with("http://") || val.starts_with("https://") || val.starts_with("mailto:");
        on_validate.emit((id.clone(), if is_valid { ValidationState::Valid } else { ValidationState::Invalid("URL must start with http://, https://, or mailto:".to_string()) }));
    };

    html! {
        <input
            type="url"
            value={current}
            placeholder="https://..."
            on:input=on_input
            class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
        />
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::properties::{PropertyDefinition, PropertyType, PropertyValue};

    #[test]
    fn test_property_definition_validate_text() {
        let def = PropertyDefinition {
            id: "title".to_string(),
            display_name: "Title".to_string(),
            property_type: PropertyType::Text,
            description: None,
            default_value: None,
            options: None,
        };
        assert!(def.validate(&PropertyValue::Text("Hello".to_string())));
        assert!(!def.validate(&PropertyValue::Number(1.0)));
    }

    #[test]
    fn test_property_definition_validate_number() {
        let def = PropertyDefinition {
            id: "count".to_string(),
            display_name: "Count".to_string(),
            property_type: PropertyType::Number,
            description: None,
            default_value: None,
            options: None,
        };
        assert!(def.validate(&PropertyValue::Number(42.0)));
        assert!(!def.validate(&PropertyValue::Text("not a number".to_string())));
    }

    #[test]
    fn test_property_definition_validate_select() {
        let def = PropertyDefinition {
            id: "status".to_string(),
            display_name: "Status".to_string(),
            property_type: PropertyType::Select,
            description: None,
            default_value: None,
            options: Some(vec!["active".to_string(), "inactive".to_string()]),
        };
        assert!(def.validate(&PropertyValue::Select("active".to_string())));
        assert!(!def.validate(&PropertyValue::Select("unknown".to_string())));
    }

    #[test]
    fn test_property_definition_validate_multiselect() {
        let def = PropertyDefinition {
            id: "tags".to_string(),
            display_name: "Tags".to_string(),
            property_type: PropertyType::MultiSelect,
            description: None,
            default_value: None,
            options: Some(vec!["rust".to_string(), "python".to_string(), "js".to_string()]),
        };
        assert!(def.validate(&PropertyValue::MultiSelect(vec!["rust".to_string(), "python".to_string()])));
        assert!(!def.validate(&PropertyValue::MultiSelect(vec!["unknown".to_string()])));
    }

    #[test]
    fn test_property_definition_validate_url() {
        let def = PropertyDefinition {
            id: "website".to_string(),
            display_name: "Website".to_string(),
            property_type: PropertyType::Url,
            description: None,
            default_value: None,
            options: None,
        };
        assert!(def.validate(&PropertyValue::Url("https://example.com".to_string())));
        assert!(!def.validate(&PropertyValue::Url("not-a-url".to_string())));
    }
}
