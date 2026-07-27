pub mod model;
mod parser;

mod document;
mod errors;
pub mod extensions;

pub use document::Document;
pub use errors::ParseError;
pub use parser::parse;

#[cfg(test)]
mod tests;
