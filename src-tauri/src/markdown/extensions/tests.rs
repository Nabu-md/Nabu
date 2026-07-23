use crate::markdown::extensions::embeds::EmbedType;
use crate::markdown::extensions::callouts::CalloutKind;
use crate::markdown::extensions::mermaid::MermaidBlock;
use crate::markdown::extensions::math::MathKind;
use crate::markdown::extensions::html::HtmlBlock;
use crate::markdown::extensions::{extract_wikilinks, extract_embeds, extract_tasks, extract_callouts, extract_mermaid, extract_math, Frontmatter, extract_footnotes, extract_html};

#[test]
fn extract_wikilinks_basic() {
    let input = "See [[Page]] and [[Folder/Page|Alias]]";
    let links = extract_wikilinks(input);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].destination, "Page");
    assert!(links[0].alias.is_none());
    assert_eq!(links[1].destination, "Folder/Page");
    assert_eq!(links[1].alias, Some("Alias".to_string()));
}

#[test]
fn extract_embeds_note_and_media() {
    let input = "Embed ![[Note]] and ![[Image.png]]";
    let embeds = extract_embeds(input);
    assert_eq!(embeds.len(), 2);
    assert_eq!(embeds[0].embed_type, EmbedType::Note);
    assert_eq!(embeds[1].embed_type, EmbedType::Image);
}

#[test]
fn extract_tasks_detects_markers() {
    let input = "- [ ] todo\n- [x] done";
    let tasks = extract_tasks(input);
    assert_eq!(tasks.len(), 2);
    assert!(!tasks[0].completed);
    assert!(tasks[1].completed);
}

#[test]
fn extract_callouts_detects_kinds() {
    let input = "> [!NOTE]\n> content";
    let callouts = extract_callouts(input);
    assert_eq!(callouts.len(), 1);
    assert_eq!(callouts[0].kind, CalloutKind::Note);
}

#[test]
fn extract_mermaid_blocks() {
    let input = "```mermaid\ngraph LR\nA --> B\n```";
    let blocks = extract_mermaid(input);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].content, "graph LR\nA --> B\n");
}

#[test]
fn extract_inline_and_display_math() {
    let input = "Value $1+2$ and display $$show$$";
    let items = extract_math(input);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, MathKind::Inline);
    assert_eq!(items[1].kind, MathKind::Block);
}

#[test]
fn extracts_frontmatter() {
    let input = "---\ntitle: hello\ntags:\n- a\n---\n# doc";
    let fm = Frontmatter::parse(input).unwrap();
    assert_eq!(fm.properties.get("title"), Some(&"hello".to_string()));
}

#[test]
fn footnote_references_and_definitions() {
    let input = "Hello[^note]\n\n[^note]: My footnote\n";
    let notes = extract_footnotes(input);
    assert_eq!(notes.references.len(), 1);
    assert_eq!(notes.definitions.len(), 1);
}

#[test]
fn preserves_raw_html_blocks() {
    let input = "<div>\ncontent\n</div>";
    let blocks = extract_html(input);
    assert_eq!(blocks.len(), 3);
    assert!(blocks.iter().any(|b| b.html.contains("<div>")), "html blocks: {:?}", blocks.iter().map(|b| &b.html).collect::<Vec<_>>());
}
