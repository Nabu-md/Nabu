#[cfg(test)]
mod tests {
    use crate::models::properties::{PropertyDefinition, PropertyType, PropertyValue};

    #[test]
    fn test_property_validation() {
        let text_def = PropertyDefinition {
            id: "text_prop".to_string(),
            display_name: "Text Prop".to_string(),
            property_type: PropertyType::Text,
            description: None,
            default_value: None,
            options: None,
        };
        assert!(text_def.validate(&PropertyValue::Text("hello".to_string())));
        assert!(!text_def.validate(&PropertyValue::Number(1.0)));

        let select_def = PropertyDefinition {
            id: "select_prop".to_string(),
            display_name: "Select Prop".to_string(),
            property_type: PropertyType::Select,
            description: None,
            default_value: None,
            options: Some(vec!["A".to_string(), "B".to_string()]),
        };
        assert!(select_def.validate(&PropertyValue::Select("A".to_string())));
        assert!(!select_def.validate(&PropertyValue::Select("C".to_string())));
    }
}
