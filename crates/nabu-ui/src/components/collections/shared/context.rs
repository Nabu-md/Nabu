use yew::prelude::*;

#[derive(Clone, PartialEq, Default)]
pub struct SearchState {
    pub query: String,
    // Add filters/sort later
}

pub type ViewContext = UseStateHandle<SearchState>;
