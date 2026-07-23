use crate::markdown::document::Document;
use crate::markdown::errors::ParseError;

pub fn parse(markdown: &str) -> Result<Document, ParseError> {
    Ok(Document::new(markdown.to_string()))
}