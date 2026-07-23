use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct FootnoteDefinition {
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FootnoteReference {
    pub label: String,
}

#[derive(Debug, Default)]
pub struct FootnoteVisitor {
    pub definitions: Vec<FootnoteDefinition>,
    pub references: Vec<FootnoteReference>,
    pub labels: HashMap<String, usize>,
}

impl FootnoteVisitor {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn extract_footnotes(input: &str) -> FootnoteVisitor {
    let mut visitor = FootnoteVisitor::new();
    for raw_line in input.lines() {
        let line = raw_line.trim();
        if let Some(rest) = line.strip_prefix("[^") {
            if let Some(end) = rest.find(']') {
                let label = rest[..end].to_string();
                visitor.references.push(FootnoteReference { label: label.clone() });
                let rest = &rest[end + 1..];
                if rest.starts_with(":") {
                    let text = rest[1..].trim().trim_start().to_string();
                    visitor.labels.insert(label.clone(), visitor.definitions.len());
                    visitor.definitions.push(FootnoteDefinition { label, text });
                }
            }
        }
    }
    visitor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footnote_definition_and_reference() {
        let input = "Hello[^note]\n\n[^note]: My footnote\n";
        let footnotes = extract_footnotes(input);
        assert_eq!(footnotes.definitions.len(), 1);
        assert_eq!(footnotes.references.len(), 1);
        assert_eq!(footnotes.definitions[0].label, "note");
        assert_eq!(footnotes.references[0].label, "note");
    }
}
