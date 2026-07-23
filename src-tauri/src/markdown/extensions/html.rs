use crate::markdown::extensions::visitor::Visitor;
use pulldown_cmark::Event;

#[derive(Debug, Clone, PartialEq)]
pub struct HtmlBlock {
    pub html: String,
    pub source_span: (usize, usize),
}

#[derive(Debug, Default)]
pub struct HtmlVisitor {
    pub blocks: Vec<HtmlBlock>,
}

impl HtmlVisitor {
    pub fn new() -> Self { Self::default() }
}

impl Visitor for HtmlVisitor {
    fn visit(&mut self, event: &Event<'_>) {
        if let Event::Html(text) = event {
            self.blocks.push(HtmlBlock { html: text.to_string(), source_span: (0, 0) });
        }
    }
}

pub fn extract_html(input: &str) -> Vec<HtmlBlock> {
    let mut visitor = HtmlVisitor::new();
    visitor.visit_str(input);
    visitor.blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_html_blocks() {
        let input = "<div>\ncontent\n</div>";
        let blocks = extract_html(input);
        assert_eq!(blocks.len(), 3);
        let joined: String = blocks.iter().map(|b| b.html.as_str()).collect();
        assert!(joined.contains("<div>"), "joined html: {}", joined);
    }
}
