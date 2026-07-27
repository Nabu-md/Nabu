mod callouts;
mod embeds;
mod footnotes;
mod frontmatter;
mod html;
mod math;
mod mermaid;
mod tasks;
mod visitor;
mod wikilinks;

pub use callouts::extract_callouts;
pub use embeds::extract_embeds;
pub use footnotes::extract_footnotes;
pub use frontmatter::extract_frontmatter;
pub use html::extract_html;
pub use math::extract_math;
pub use mermaid::extract_mermaid;
pub use tasks::extract_tasks;
pub use visitor::Visitor;
pub use wikilinks::extract_wikilinks;

#[cfg(test)]
mod tests;
