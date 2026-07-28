use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PropertyType {
    Text,
    Number,
    Date,
    Select,
    MultiSelect,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PropertyValue {
    Text(String),
    Number(f64),
    Date(String), // ISO 8601
    Select(String),
    MultiSelect(Vec<String>),
    Url(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropertyDefinition {
    pub id: String,
    pub display_name: String,
    pub property_type: PropertyType,
    pub description: Option<String>,
    pub default_value: Option<PropertyValue>,
    pub options: Option<Vec<String>>, // For Select/MultiSelect
}

impl PropertyDefinition {
    pub fn validate(&self, value: &PropertyValue) -> bool {
        match (&self.property_type, value) {
            (PropertyType::Text, PropertyValue::Text(_)) => true,
            (PropertyType::Number, PropertyValue::Number(_)) => true,
            (PropertyType::Date, PropertyValue::Date(_)) => true,
            (PropertyType::Select, PropertyValue::Select(s)) => {
                self.options.as_ref().map_or(false, |opts| opts.contains(s))
            }
            (PropertyType::MultiSelect, PropertyValue::MultiSelect(vs)) => {
                self.options.as_ref().map_or(false, |opts| vs.iter().all(|v| opts.contains(v)))
            }
            (PropertyType::Url, PropertyValue::Url(_)) => true,
            _ => false,
        }
    }
}

