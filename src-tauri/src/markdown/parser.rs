use crate::markdown::model::{AstNode, Document};
use crate::markdown::errors::ParseError;
use pulldown_cmark::Parser;

pub fn parse(markdown: &str) -> Result<Document, ParseError> {
    let parser = Parser::new(markdown);
    let mut root_children = Vec::new();
    let mut stack: Vec<Vec<AstNode>> = Vec::new(); 
    
    // Simplistic tree builder for demonstration
    // We'll traverse events and build AstNode hierarchy
    root_children.push(AstNode::Paragraph { children: vec![AstNode::Text { value: markdown.to_string() }] });

    Ok(Document::new(markdown.to_string(), AstNode::Root { children: root_children }))
}
