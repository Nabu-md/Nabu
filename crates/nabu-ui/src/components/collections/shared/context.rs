use leptos::prelude::*;

#[derive(Clone, PartialEq, Default)]
pub struct SearchState {
    pub query: String,
}

pub type ViewContext = Signal<SearchState>;
