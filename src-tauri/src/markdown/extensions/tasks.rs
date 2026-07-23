use pulldown_cmark::{Event, Options};
use super::Visitor;

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub completed: bool,
    pub text: String,
    pub source_span: (usize, usize),
}

pub struct TaskVisitor {
    pub tasks: Vec<Task>,
}

impl Default for TaskVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskVisitor {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }
}

impl Visitor for TaskVisitor {
    fn visit(&mut self, event: &Event<'_>) {
        if let Event::TaskListMarker(checked) = event {
            self.tasks.push(Task {
                completed: *checked,
                text: String::new(),
                source_span: (0, 0),
            });
        }
    }
}

pub fn extract_tasks(input: &str) -> Vec<Task> {
    let mut visitor = TaskVisitor::new();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = pulldown_cmark::Parser::new_ext(input, options);
    for event in parser {
        visitor.visit(&event);
    }
    visitor.tasks
}