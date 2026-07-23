use crate::markdown::extensions::visitor::Visitor;
use pulldown_cmark::Event;

#[derive(Debug, Clone, PartialEq)]
pub struct Math {
    pub kind: MathKind,
    pub expression: String,
    pub source_span: (usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathKind {
    Inline,
    Block,
}

#[derive(Debug, Default)]
pub struct MathVisitor {
    pub items: Vec<Math>,
}

impl MathVisitor {
    pub fn new() -> Self { Self::default() }
}

impl Visitor for MathVisitor {
    fn visit(&mut self, event: &Event<'_>) {
        if let Event::InlineMath(text) = event {
            self.items.push(Math { kind: MathKind::Inline, expression: text.to_string(), source_span: (0, 0) });
        } else if let Event::DisplayMath(text) = event {
            self.items.push(Math { kind: MathKind::Block, expression: text.to_string(), source_span: (0, 0) });
        }
    }
}

pub fn extract_math(input: &str) -> Vec<Math> {
    let mut visitor = MathVisitor::new();
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_MATH);
    let parser = pulldown_cmark::Parser::new_ext(input, options);
    for event in parser {
        visitor.visit(&event);
    }
    visitor.items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_math_detected() {
        let input = "Value is $1+2$ today.";
        let items = extract_math(input);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, MathKind::Inline);
        assert_eq!(items[0].expression, "1+2");
    }

    #[test]
    fn display_math_detected() {
        let input = "$$\n1+2\n$$\n";
        let items = extract_math(input);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, MathKind::Block);
        assert_eq!(items[0].expression, "\n1+2\n");
    }
}
