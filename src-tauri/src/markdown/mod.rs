mod parser;
mod document;
mod errors;
pub mod extensions;

pub use parser::parse;
pub use document::Document;
pub use errors::ParseError;

#[cfg(test)]
mod tests;