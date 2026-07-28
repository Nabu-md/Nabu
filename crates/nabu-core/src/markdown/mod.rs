pub mod model;
mod complex_parser;
mod simple_parser;

mod document;
mod errors;
pub mod extensions;

pub use document::Document;
pub use errors::ParseError;
pub use simple_parser::{parse_markdown_to_html, extract_tags, extract_block_refs};
pub use complex_parser::parse;

#[cfg(test)]
mod tests;
