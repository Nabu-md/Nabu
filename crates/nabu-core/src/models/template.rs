use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::models::properties::PropertyValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_folder: Option<String>,
    pub frontmatter_defaults: HashMap<String, String>,
    pub property_presets: HashMap<String, PropertyValue>,
    pub body: String,
    pub object_type: Option<String>,
}
