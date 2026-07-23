#[derive(Debug, Clone, PartialEq)]
pub struct Wikilink {
    pub destination: String,
    pub alias: Option<String>,
    pub display_text: String,
    pub source_span: (usize, usize),
}

pub fn extract_wikilinks(input: &str) -> Vec<Wikilink> {
    let mut links = Vec::new();
    let mut start = 0;
    while let Some(open) = input[start..].find("[[") {
        let abs_start = start + open;
        if let Some(close) = input[abs_start..].find("]]") {
            let span_end = abs_start + close + 2;
            let content = &input[abs_start + 2..abs_start + close];
            if let Some(pipe_idx) = content.find('|') {
                let destination = content[..pipe_idx].to_string();
                let alias = Some(content[pipe_idx + 1..].to_string());
                let display_text = alias.clone().unwrap();
                links.push(Wikilink {
                    destination,
                    alias,
                    display_text,
                    source_span: (abs_start, span_end),
                });
            } else {
                let destination = content.to_string();
                let alias = None;
                let display_text = destination.clone();
                links.push(Wikilink {
                    destination,
                    alias,
                    display_text,
                    source_span: (abs_start, span_end),
                });
            }
            start = span_end;
        } else {
            break;
        }
    }
    links
}