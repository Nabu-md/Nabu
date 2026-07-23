use pulldown_cmark::Event;

pub trait Visitor {
    fn visit(&mut self, event: &Event<'_>);
    fn visit_str(&mut self, input: &str) {
        let parser = pulldown_cmark::Parser::new(input);
        for event in parser {
            self.visit(&event);
        }
    }
}