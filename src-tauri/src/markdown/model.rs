use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AstNode {
    Root { children: Vec<AstNode> },
    Paragraph { children: Vec<AstNode> },
    Heading { depth: usize, children: Vec<AstNode> },
    Text { value: String },
    Emphasis { children: Vec<AstNode> },
    Strong { children: Vec<AstNode> },
    InlineCode { value: String },
    CodeBlock { lang: Option<String>, value: String },
    Blockquote { children: Vec<AstNode> },
    List { ordered: bool, children: Vec<AstNode> },
    ListItem { children: Vec<AstNode> },
    Table { align: Vec<Option<String>>, children: Vec<AstNode> },
    TableRow { children: Vec<AstNode> },
    TableCell { children: Vec<AstNode>, col_span: Option<usize>, row_span: Option<usize> },
    ThematicBreak,
    Html { value: String },
    Link { url: String, title: Option<String>, children: Vec<AstNode> },
    Image { url: String, alt: String, title: Option<String> },
    Break,
    Delete { children: Vec<AstNode> },
    TaskList { children: Vec<AstNode> },
    TaskItem { checked: bool, children: Vec<AstNode> },
    WikiLink { target: String, alias: Option<String> },
    Embed { target: String, embed_type: EmbedType },
    Callout { kind: Option<String>, title: Option<String>, children: Vec<AstNode>, folding: Option<CalloutFolding> },
    Mermaid { language: String, content: String },
    Math { kind: MathKind, expression: String },
    Frontmatter { properties: HashMap<String, String> },
    Footnote { label: String, text: String },
    FootnoteReference { label: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EmbedType { Note, Image, Pdf }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathKind { Inline, Block }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalloutFolding { pub expanded: Option<bool> }

pub fn normalize(node: AstNode) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    match node {
        AstNode::Root { children } => {
            map.insert("type".into(), "root".into());
            map.insert("children".into(), children.into_iter().map(normalize).collect());
        }
        AstNode::Text { value } => {
            map.insert("type".into(), "text".into());
            map.insert("value".into(), value.into());
        }
        AstNode::Paragraph { children } => {
            map.insert("type".into(), "paragraph".into());
            map.insert("children".into(), children.into_iter().map(normalize).collect());
        }
        _ => { map.insert("type".into(), "unknown".into()); }
    }
    serde_json::Value::Object(map)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub source: String,
    pub ast: AstNode,
}

impl Document {
    pub fn new(source: String, ast: AstNode) -> Self {
        Self { source, ast }
    }
}
