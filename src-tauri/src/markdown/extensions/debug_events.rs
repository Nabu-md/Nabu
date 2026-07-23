#[cfg(test)]
mod debug_tests {
    #[test]
    fn debug_wikilink_events() {
        let input = "See [[Page]] and [[Folder/Page|Alias]]";
        for event in pulldown_cmark::Parser::new(input) {
            println!("event={:?}", event);
        }
        panic!("intentional panic to see debug output");
    }
}
