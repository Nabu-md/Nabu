mod visitor;
mod wikilinks;
mod embeds;
mod tasks;
mod callouts;

pub use visitor::Visitor;
pub use wikilinks::{extract_wikilinks, Wikilink};
pub use embeds::{extract_embeds, Embed};
pub use tasks::{extract_tasks, Task};
pub use callouts::{extract_callouts, Callout};

#[cfg(test)]
mod tests;