use yew::prelude::*;
use crate::models::properties::{PropertyDefinition, PropertyValue};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub properties: Vec<PropertyDefinition>,
    pub values: std::collections::HashMap<String, PropertyValue>,
    pub on_change: Callback<(String, PropertyValue)>,
}

#[function_component(PropertyEditor)]
pub fn property_editor(props: &Props) -> Html {
    html! {
        <div class="property-editor">
            { for props.properties.iter().map(|def| {
                let id = def.id.clone();
                let value = props.values.get(&id).cloned();
                
                let on_input = props.on_change.reform(move |e: InputEvent| {
                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                    (id.clone(), PropertyValue::Text(input.value()))
                });

                html! {
                    <div class="property-field">
                        <label>{ &def.display_name }</label>
                        <input 
                            type="text" 
                            value={if let Some(PropertyValue::Text(v)) = value { v } else { "".to_string() }}
                            oninput={on_input} 
                        />
                    </div>
                }
            })}
        </div>
    }
}
