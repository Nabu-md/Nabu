mod parser;
mod document;
mod errors;

pub use parser::parse;
pub use document::Document;
pub use errors::ParseError;

#[cfg(test)]
mod tests;