use crate::markdown::extensions::visitor::Visitor;
use pulldown_cmark::Event;

#[derive(Debug, Clone, PartialEq)]
pub enum EmbedType {
    Note,
    Image,
    Pdf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Embed {
    pub embed_type: EmbedType,
    pub target: String,
}

#[derive(Debug, Default)]
pub struct EmbedVisitor {
    pub embeds: Vec<Embed>,
}

impl EmbedVisitor {
    pub fn new() -> Self { Self::default() }
}

impl Visitor for EmbedVisitor {
    fn visit(&mut self, _event: &Event<'_>) {
        // Implement logic to extract embeds
    }
}

pub fn extract_embeds(input: &str) -> Vec<Embed> {
    let mut visitor = EmbedVisitor::new();
    visitor.visit_str(input);
    visitor.embeds
}
