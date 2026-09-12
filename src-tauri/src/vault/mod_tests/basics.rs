use super::*;

const FULL_FM_CONTENT: &str = "---\ntitle: Laputa Project\nIs A: Project\naliases:\n  - Laputa\n  - Castle in the Sky\nBelongs to:\n  - Studio Ghibli\nRelated to:\n  - Miyazaki\nStatus: Active\nOwner: Demo User\nCadence: Weekly\n---\n# Laputa Project\n\nThis is a project note.\n";

#[test]
fn test_reload_entry_returns_fresh_data() {
    let dir = TempDir::new().unwrap();
    create_test_file(
        dir.path(),
        "my-note.md",
        "---\ntitle: My Note\nStatus: Active\n---\n# My Note\n\nOriginal.",
    );
    let entry = reload_entry(&dir.path().join("my-note.md")).unwrap();
    assert_eq!(entry.title, "My Note");
    assert_eq!(entry.status, Some("Active".to_string()));

    create_test_file(
        dir.path(),
        "my-note.md",
        "---\ntitle: My Note\nStatus: Done\n---\n# My Note\n\nUpdated.",
    );
    let fresh = reload_entry(&dir.path().join("my-note.md")).unwrap();
    assert_eq!(fresh.status, Some("Done".to_string()));
}

#[test]
fn test_reload_entry_nonexistent_file() {
    let result = reload_entry(std::path::Path::new("/nonexistent/path/note.md"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("does not exist"));
}

#[test]
fn test_parse_full_frontmatter_identity() {
    let dir = TempDir::new().unwrap();
    let entry = parse_test_entry(&dir, "laputa.md", FULL_FM_CONTENT);
    assert_eq!(entry.title, "Laputa Project");
    assert_eq!(entry.is_a, Some("Project".to_string()));
    assert_eq!(entry.filename, "laputa.md");
}

#[test]
fn test_parse_full_frontmatter_lists() {
    let dir = TempDir::new().unwrap();
    let entry = parse_test_entry(&dir, "laputa.md", FULL_FM_CONTENT);
    assert_eq!(entry.aliases, vec!["Laputa", "Castle in the Sky"]);
    assert!(!entry.relationships.contains_key("Belongs to"));
    assert!(!entry.relationships.contains_key("Related to"));
}

#[test]
fn test_parse_full_frontmatter_scalars() {
    let dir = TempDir::new().unwrap();
    let entry = parse_test_entry(&dir, "laputa.md", FULL_FM_CONTENT);
    assert_eq!(entry.status, Some("Active".to_string()));
    assert_eq!(
        entry
            .properties
            .get("Owner")
            .and_then(|value| value.as_str()),
        Some("Demo User")
    );
    assert_eq!(
        entry
            .properties
            .get("Cadence")
            .and_then(|value| value.as_str()),
        Some("Weekly")
    );
}

#[test]
fn test_parse_empty_frontmatter() {
    let dir = TempDir::new().unwrap();
    let entry = parse_test_entry(
        &dir,
        "just-a-title.md",
        "---\n---\n# Just a Title\n\nNo frontmatter fields.",
    );
    assert_eq!(
        (
            entry.title.as_str(),
            entry.aliases.is_empty(),
            entry.belongs_to.is_empty(),
            entry.status.as_deref(),
        ),
        ("Just a Title", true, true, None)
    );
}

#[test]
fn test_parse_no_frontmatter() {
    let dir = TempDir::new().unwrap();
    let content = "# A Note Without Frontmatter\n\nJust markdown.";
    create_test_file(dir.path(), "a-note-without-frontmatter.md", content);

    let entry = parse_md_file(&dir.path().join("a-note-without-frontmatter.md"), None).unwrap();
    assert_eq!(entry.title, "A Note Without Frontmatter");
}

#[test]
fn test_parse_single_string_aliases() {
    let dir = TempDir::new().unwrap();
    let content = "---\naliases: SingleAlias\n---\n# Test\n";
    create_test_file(dir.path(), "single-alias.md", content);

    let entry = parse_md_file(&dir.path().join("single-alias.md"), None).unwrap();
    assert_eq!(entry.aliases, vec!["SingleAlias"]);
}

#[test]
fn test_parse_malformed_yaml() {
    let dir = TempDir::new().unwrap();
    let content = "---\nIs A: [unclosed bracket\n---\n# Malformed\n";
    create_test_file(dir.path(), "malformed.md", content);

    let entry = parse_md_file(&dir.path().join("malformed.md"), None);
    assert!(entry.is_ok());
}

#[test]
fn test_parse_md_file_has_snippet() {
    let dir = TempDir::new().unwrap();
    let content = "---\nIs A: Note\n---\n# Test Note\n\nHello, world! This is a snippet.";
    create_test_file(dir.path(), "test.md", content);

    let entry = parse_md_file(&dir.path().join("test.md"), None).unwrap();
    assert_eq!(entry.snippet, "Hello, world! This is a snippet.");
}

#[test]
fn test_parse_md_file_has_word_count() {
    let dir = TempDir::new().unwrap();
    let content =
        "---\nIs A: Note\n---\n# Test Note\n\nHello world. This is a test with seven words.";
    create_test_file(dir.path(), "test.md", content);

    let entry = parse_md_file(&dir.path().join("test.md"), None).unwrap();
    assert_eq!(entry.word_count, 9);
}

#[test]
fn test_parse_md_file_word_count_empty_body() {
    let dir = TempDir::new().unwrap();
    let content = "---\nIs A: Note\n---\n# Empty Note\n";
    create_test_file(dir.path(), "test.md", content);

    let entry = parse_md_file(&dir.path().join("test.md"), None).unwrap();
    assert_eq!(entry.word_count, 0);
}
