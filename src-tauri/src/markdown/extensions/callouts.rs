use pulldown_cmark::{Event, Tag};
use super::Visitor;

#[derive(Debug, Clone, PartialEq)]
pub struct Callout {
    pub kind: CalloutKind,
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

pub struct CalloutVisitor {
    pub callouts: Vec<Callout>,
}

impl Default for CalloutVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl CalloutVisitor {
    pub fn new() -> Self {
        Self { callouts: Vec::new() }
    }
}

impl Visitor for CalloutVisitor {
    fn visit(&mut self, event: &Event<'_>) {
        if let Event::Start(tag) = event {
            if let Tag::BlockQuote(kind) = tag {
                if let Some(kind) = kind {
                    let callout_kind = match kind {
                        pulldown_cmark::BlockQuoteKind::Note => CalloutKind::Note,
                        pulldown_cmark::BlockQuoteKind::Tip => CalloutKind::Tip,
                        pulldown_cmark::BlockQuoteKind::Important => CalloutKind::Important,
                        pulldown_cmark::BlockQuoteKind::Warning => CalloutKind::Warning,
                        pulldown_cmark::BlockQuoteKind::Caution => CalloutKind::Caution,
                    };
                    self.callouts.push(Callout {
                        kind: callout_kind,
                        title: None,
                        content: String::new(),
                    });
                }
            }
        }
    }
}

pub fn extract_callouts(input: &str) -> Vec<Callout> {
    let mut visitor = CalloutVisitor::new();
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_GFM);
    let parser = pulldown_cmark::Parser::new_ext(input, options);
    for event in parser {
        visitor.visit(&event);
    }
    visitor.callouts
}