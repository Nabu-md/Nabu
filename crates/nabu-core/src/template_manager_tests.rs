#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_legacy_template() {
        let dir = tempdir().unwrap();
        let template_dir = dir.path().join(".nabu/templates");
        fs::create_dir_all(&template_dir).unwrap();
        fs::write(template_dir.join("legacy.md"), "Legacy Content").unwrap();

        let manager = TemplateManager::new(dir.path());
        // This should fail to parse as YAML, which is correct for legacy
        let result = manager.load_template("legacy");
        assert!(result.is_err());
    }
}
