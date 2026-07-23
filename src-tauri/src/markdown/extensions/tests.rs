use crate::markdown::extensions::embeds::EmbedType;
use crate::markdown::extensions::callouts::CalloutKind;
use crate::markdown::extensions::{extract_wikilinks, extract_embeds, extract_tasks, extract_callouts};
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