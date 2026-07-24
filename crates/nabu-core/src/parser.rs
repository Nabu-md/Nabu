use regex::Regex;
use pulldown_cmark::{Parser, Options, html};

pub fn parse_markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

pub fn extract_tags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    // Inline #tags
    let re = regex::Regex::new(r"#([a-zA-Z0-9_-]+)").unwrap();
    for cap in re.captures_iter(content) {
        tags.push(cap[1].to_string());
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_parsing() {
        let html = parse_markdown_to_html("# Title");
        assert_eq!(html.trim(), "<h1>Title</h1>");
    }

    #[test]
    fn test_task_list_parsing() {
        let html = parse_markdown_to_html("- [ ] Task");
        // pulldown-cmark GFM task items are usually rendered as <input type="checkbox">
        assert!(html.contains("checkbox"));
    }
}
