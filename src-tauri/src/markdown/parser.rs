use crate::markdown::extensions::{extract_frontmatter, extract_wikilinks, extract_tasks, extract_callouts, extract_mermaid, extract_math, extract_html, extract_embeds, extract_footnotes};
use super::model::{AstNode, Document};
use super::errors::ParseError;
use pulldown_cmark::Parser;

pub fn parse(markdown: &str) -> Result<Document, ParseError> {
    // Parse frontmatter first
    let _frontmatter = extract_frontmatter(markdown);
    let _ = extract_wikilinks(markdown);
    let _ = extract_tasks(markdown);
    let _ = extract_callouts(markdown);
    let _ = extract_mermaid(markdown);
    let _ = extract_math(markdown);
    let _ = extract_html(markdown);
    let _ = extract_embeds(markdown);
    let _ = extract_footnotes(markdown);
    
    let _parser = Parser::new(markdown);
    let mut root_children = Vec::new();
    let _stack: Vec<Vec<AstNode>> = Vec::new(); 
    
    // Simplistic tree builder for demonstration
    // We'll traverse events and build AstNode hierarchy
    root_children.push(AstNode::Paragraph { children: vec![AstNode::Text { value: markdown.to_string() }] });

    Ok(Document::new(markdown.to_string(), AstNode::Root { children: root_children }))
}
