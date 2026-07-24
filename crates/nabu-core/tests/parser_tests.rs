use nabu_core::parser::parse_markdown_to_html;

#[test]
fn test_markdown_parsing() {
    let markdown = "# Hello World";
    let html = parse_markdown_to_html(markdown);
    assert_eq!(html.trim(), "<h1>Hello World</h1>");
}
