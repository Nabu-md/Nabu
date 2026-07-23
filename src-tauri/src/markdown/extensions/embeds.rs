use super::Wikilink;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbedType {
    Note,
    Image,
    Pdf,
    Unknown,
}

impl EmbedType {
    pub fn note() -> Self { Self::Note }
    pub fn image() -> Self { Self::Image }
    pub fn pdf() -> Self { Self::Pdf }
    pub fn is_note(&self) -> bool { matches!(self, Self::Note) }
    pub fn is_image(&self) -> bool { matches!(self, Self::Image) }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Embed {
    pub embed_type: EmbedType,
    pub destination: String,
    pub source_span: (usize, usize),
}

impl Embed {
    pub fn new(destination: impl Into<String>, embed_type: EmbedType, source_span: (usize, usize)) -> Self {
        Self { destination: destination.into(), embed_type, source_span }
    }

    pub fn is_image(&self) -> bool {
        self.embed_type.is_image()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_new_sets_fields() {
        let embed = Embed::new("Note", EmbedType::note(), (0, 9));
        assert_eq!(embed.destination, "Note");
        assert!(embed.embed_type.is_note());
        assert!(!embed.is_image());
        assert_eq!(embed.source_span, (0, 9));
    }

    #[test]
    fn image_embed_classification() {
        let embed = Embed::new("Image.png", EmbedType::image(), (0, 13));
        assert!(embed.is_image());
    }
}
