mod visitor;
mod wikilinks;
mod embeds;
mod tasks;
mod callouts;
mod mermaid;
mod math;
mod frontmatter;
mod footnotes;
mod html;

pub use visitor::Visitor;
pub use wikilinks::{extract_wikilinks, Wikilink};
pub use embeds::{extract_embeds, Embed, EmbedType};
pub use tasks::{extract_tasks, Task};
pub use callouts::{extract_callouts, Callout, CalloutKind};
pub use mermaid::{extract_mermaid, MermaidBlock};
pub use math::{extract_math, Math, MathKind};
pub use frontmatter::Frontmatter;
pub use footnotes::{extract_footnotes, FootnoteDefinition, FootnoteReference};
pub use html::{extract_html, HtmlBlock};

#[cfg(test)]
mod tests;
