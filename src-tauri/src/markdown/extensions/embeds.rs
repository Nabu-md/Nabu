use super::Wikilink;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbedType {
    Note,
    Image,
    Pdf,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Embed {
    pub embed_type: EmbedType,
    pub destination: String,
    pub source_span: (usize, usize),
}

pub fn extract_embeds(input: &str) -> Vec<Embed> {
    let mut embeds = Vec::new();
    let mut start = 0;
    while let Some(open) = input[start..].find("![") {
        let abs_start = start + open;
        if let Some(close) = input[abs_start..].find("]]") {
            let span_end = abs_start + close + 2;
            let destination = input[abs_start + 3..abs_start + close].to_string();
            let lower = destination.to_lowercase();
            let embed_type = if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".gif") || lower.ends_with(".svg") {
                EmbedType::Image
            } else if lower.ends_with(".pdf") {
                EmbedType::Pdf
            } else {
                EmbedType::Note
            };
            embeds.push(Embed {
                embed_type,
                destination,
                source_span: (abs_start, span_end),
            });
            start = span_end;
        } else {
            break;
        }
    }
    embeds
}