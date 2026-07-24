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
pub use wikilinks::extract_wikilinks;
pub use embeds::extract_embeds;
pub use tasks::extract_tasks;
pub use callouts::extract_callouts;
pub use mermaid::extract_mermaid;
pub use math::extract_math;
pub use frontmatter::extract_frontmatter;
pub use footnotes::extract_footnotes;
pub use html::extract_html;

#[cfg(test)]
mod tests;
