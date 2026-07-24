use crate::markdown::extensions::visitor::Visitor;
use pulldown_cmark::{Event, Tag};

#[derive(Debug, Clone, PartialEq)]
pub enum EmbedType {
    Note,
    Image,
    Pdf,
}

impl EmbedType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "svg" => EmbedType::Image,
            "pdf" => EmbedType::Pdf,
            _ => EmbedType::Note,
        }
    }
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
    fn visit(&mut self, event: &Event<'_>) {
        if let Event::Start(Tag::Image { dest_url: dest, .. }) = event {
            let target = dest.to_string();
            let ext = target.split('.').last().unwrap_or("");
            self.embeds.push(Embed {
                embed_type: EmbedType::from_extension(ext),
                target,
            });
        }
    }
}

pub fn extract_embeds(input: &str) -> Vec<Embed> {
    let mut embeds = Vec::new();
    let mut start = 0;
    while let Some(open) = input[start..].find("![[") {
        let abs_start = start + open;
        if let Some(close) = input[abs_start..].find("]]") {
            let span_end = abs_start + close + 2;
            let target = input[abs_start + 3..abs_start + close].to_string();
            let ext = target.split('.').last().unwrap_or("");
            embeds.push(Embed {
                embed_type: EmbedType::from_extension(ext),
                target,
            });
            start = span_end;
        } else {
            break;
        }
    }
    
    let mut visitor = EmbedVisitor::new();
    visitor.visit_str(input);
    embeds.extend(visitor.embeds);
    
    embeds
}
