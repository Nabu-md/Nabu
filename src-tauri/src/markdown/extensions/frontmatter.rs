use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub properties: HashMap<String, String>,
}

impl Frontmatter {
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim().strip_prefix("---")?.trim();
        if let Some(end_idx) = trimmed.find("\n---") {
            let raw = trimmed[..end_idx].trim();
            let mut properties = HashMap::new();
            for line in raw.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }
                if let Some(sep) = line.find(':') {
                    let key = line[..sep].trim().to_string();
                    let value = line[sep + 1..].trim().to_string();
                    properties.insert(key, value);
                }
            }
            Some(Self { properties })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_yaml_frontmatter() {
        let input = "---\ntitle: hello\naliases:\n- a\n- b\ntags:\n- t\n---\n# doc";
        let fm = Frontmatter::parse(input).unwrap();
        assert_eq!(fm.properties.get("title"), Some(&"hello".to_string()));
    }

    #[test]
    fn no_frontmatter_returns_none() {
        assert!(Frontmatter::parse("# doc").is_none());
    }
}
