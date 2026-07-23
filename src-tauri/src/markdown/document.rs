use pulldown_cmark::{Event, Parser};

#[derive(Debug, Clone, Default)]
pub struct Document {
    pub source: String,
}

impl Document {
    pub fn new(source: String) -> Self {
        Self { source }
    }

    pub fn events(&self) -> impl Iterator<Item = Event<'_>> {
        Parser::new(&self.source).map(|e| e)
    }

    pub fn event_count(&self) -> usize {
        self.events().count()
    }
}