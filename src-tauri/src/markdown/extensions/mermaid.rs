#[derive(Debug, Clone, PartialEq)]
pub struct MermaidBlock {
    pub language: String,
    pub content: String,
    pub source_span: (usize, usize),
}

pub fn extract_mermaid(input: &str) -> Vec<MermaidBlock> {
    let mut blocks = Vec::new();
    let mut start = 0;
    while let Some(open) = input[start..].find("```mermaid") {
        let abs_start = start + open;
        let after_open = abs_start + 11;
        if let Some(close) = input[after_open..].find("```") {
            let span_end = after_open + close + 3;
            let content = &input[after_open..after_open + close];
            blocks.push(MermaidBlock {
                language: "mermaid".into(),
                content: content.into(),
                source_span: (abs_start, span_end),
            });
            start = span_end;
        } else {
            break;
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_mermaid_block() {
        let input = "```mermaid\ngraph LR\nA --> B\n```";
        let blocks = extract_mermaid(input);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content, "graph LR\nA --> B\n");
        assert_eq!(blocks[0].language, "mermaid");
    }
    #[test]
    fn malformed_mermaid_ignored() {
        let input = "```mermaid\nbroken";
        let blocks = extract_mermaid(input);
        assert!(blocks.is_empty());
    }
}
