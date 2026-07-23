use super::*;

#[test]
fn parses_heading() {
    let doc = parse("# Title").unwrap();
    let events: Vec<_> = doc.events().collect();
    assert!(!events.is_empty());
}

#[test]
fn parses_paragraph() {
    let doc = parse("Hello world").unwrap();
    let events: Vec<_> = doc.events().collect();
    assert!(!events.is_empty());
}

#[test]
fn parses_emphasis() {
    let doc = parse("*emphasis*").unwrap();
    let events: Vec<_> = doc.events().collect();
    assert!(!events.is_empty());
}

#[test]
fn parses_strong() {
    let doc = parse("**strong**").unwrap();
    let events: Vec<_> = doc.events().collect();
    assert!(!events.is_empty());
}

#[test]
fn parses_code_block() {
    let doc = parse("```\ncode\n```").unwrap();
    let events: Vec<_> = doc.events().collect();
    assert!(!events.is_empty());
}

#[test]
fn parses_blockquote() {
    let doc = parse("> quote").unwrap();
    let events: Vec<_> = doc.events().collect();
    assert!(!events.is_empty());
}

#[test]
fn parses_list() {
    let doc = parse("- item 1\n- item 2").unwrap();
    let events: Vec<_> = doc.events().collect();
    assert!(!events.is_empty());
}

#[test]
fn parses_malformed_recovers() {
    let doc = parse("# Missing closing *emphasis").unwrap();
    let events: Vec<_> = doc.events().collect();
    assert!(!events.is_empty());
}