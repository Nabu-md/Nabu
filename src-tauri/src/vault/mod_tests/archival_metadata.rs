use super::*;

fn parse_archived_entry(file_name: &str, content: &str) -> VaultEntry {
    let dir = TempDir::new().unwrap();
    parse_test_entry(&dir, file_name, content)
}

#[test]
fn test_parse_archived_truthy_aliases() {
    let cases = [
        (
            "old-quarter.md",
            "---\narchived: true\n---\n# Old Quarter\n",
            "lowercase archived alias must be parsed",
        ),
        (
            "old-quarter-2.md",
            "---\nArchived: true\n---\n# Old Quarter\n",
            "titlecase archived alias must be parsed",
        ),
        (
            "old.md",
            "---\nArchived: Yes\n---\n# Old\n",
            "Archived: Yes must be parsed as true",
        ),
        (
            "old2.md",
            "---\narchived: yes\n---\n# Old\n",
            "archived: yes must be parsed as true",
        ),
        (
            "old3.md",
            "---\nArchived: YES\n---\n# Old\n",
            "Archived: YES must be parsed as true",
        ),
        (
            "old-new.md",
            "---\n_archived: true\n---\n# Old\n",
            "_archived canonical key must be parsed",
        ),
    ];

    for (file_name, content, message) in cases {
        let entry = parse_archived_entry(file_name, content);
        assert!(entry.archived, "{message}");
    }
}

#[test]
fn test_parse_archived_falsy_inputs_and_absence() {
    let cases = [
        (
            "active2.md",
            "---\nArchived: No\n---\n# Active\n",
            "Archived: No must be parsed as false",
        ),
        (
            "active3.md",
            "---\nArchived: \"false\"\n---\n# Active\n",
            "Archived: false must be parsed as false",
        ),
        (
            "active4.md",
            "---\nArchived: 0\n---\n# Active\n",
            "Archived: 0 must be parsed as false",
        ),
        (
            "active5.md",
            "---\nIs A: Note\n---\n# Active\n",
            "absent archived must default to false",
        ),
    ];

    for (file_name, content, message) in cases {
        let entry = parse_archived_entry(file_name, content);
        assert!(!entry.archived, "{message}");
    }
}

#[test]
fn test_parse_favorite_fields() {
    let favorite = parse_archived_entry(
        "fav.md",
        "---\n_favorite: true\n_favorite_index: 3\n---\n# Fav\n",
    );
    assert!(favorite.favorite);
    assert_eq!(favorite.favorite_index, Some(3));

    let not_favorite = parse_archived_entry("not-fav.md", "---\ntype: Note\n---\n# Not Fav\n");
    assert!(!not_favorite.favorite);
    assert_eq!(not_favorite.favorite_index, None);
}

#[test]
fn test_parse_visible_values() {
    let cases = [
        (
            "journal.md",
            "---\ntype: Type\nvisible: false\n---\n# Journal\n",
            Some(false),
        ),
        (
            "project.md",
            "---\ntype: Type\nvisible: true\n---\n# Project\n",
            Some(true),
        ),
        ("missing.md", "---\ntype: Type\n---\n# Project\n", None),
    ];

    for (file_name, content, expected) in cases {
        let entry = parse_archived_entry(file_name, content);
        assert_eq!(
            entry.visible, expected,
            "unexpected visible value for {file_name}"
        );
    }
}

#[test]
fn test_archived_true_with_extra_non_string_fields() {
    let entry = parse_archived_entry(
        "archived-extra.md",
        "---\nArchived: true\norder: 5\n---\n# Archived Note\n",
    );
    assert!(entry.archived);
}

#[test]
fn test_fallback_parser_extracts_archived_from_malformed_yaml() {
    let dir = TempDir::new().unwrap();
    let frontmatter = [
        "---",
        "type: Essay",
        "Notes:",
        "  - \"[[slug|{\"Broken\": 'quotes'}]]\"",
        "Archived: true",
        "---",
        "",
        "# Archived Essay",
    ];
    let content = frontmatter.join("\n");
    create_test_file(dir.path(), "archived-essay.md", &content);
    let entry = parse_md_file(&dir.path().join("archived-essay.md"), None).unwrap();
    assert!(entry.archived);
    assert_eq!(entry.is_a, Some("Essay".to_string()));
}

#[test]
fn test_visible_not_in_relationships_or_properties() {
    let entry = parse_archived_entry(
        "journal.md",
        "---\ntype: Type\nvisible: false\n---\n# Journal\n",
    );
    assert!(!entry.relationships.contains_key("visible"));
    assert!(!entry.properties.contains_key("visible"));
}

#[test]
fn test_roundtrip_type_aliases_parse_correctly() {
    let cases = [
        ("quarter/q1.md", "---\ntype: Quarter\n---\n# Q1 2026\n"),
        ("quarter/q1-is-a.md", "---\nIs A: Quarter\n---\n# Q1 2026\n"),
        (
            "quarter/q1-snake.md",
            "---\nis_a: Quarter\n---\n# Q1 2026\n",
        ),
    ];

    for (file_name, content) in cases {
        let dir = TempDir::new().unwrap();
        let entry = parse_test_entry(&dir, file_name, content);
        assert_eq!(entry.is_a, Some("Quarter".to_string()));
    }
}

#[test]
fn test_string_or_list_normalization_keeps_type_and_scalar_fields() {
    let single_status = parse_archived_entry(
        "test.md",
        "---\ntype: Project\nStatus:\n  - Active\n---\n# Test\n",
    );
    assert_eq!(single_status.status, Some("Active".to_string()));
    assert_eq!(single_status.is_a, Some("Project".to_string()));

    let scalar_fields = parse_archived_entry(
        "scalar.md",
        "---\ntype: Project\nOwner: Demo User\nCadence: Daily\nStatus: Done\n---\n# Test\n",
    );
    assert_eq!(
        scalar_fields
            .properties
            .get("Owner")
            .and_then(|value| value.as_str()),
        Some("Demo User")
    );
    assert_eq!(
        scalar_fields
            .properties
            .get("Cadence")
            .and_then(|value| value.as_str()),
        Some("Daily")
    );
    assert_eq!(scalar_fields.status, Some("Done".to_string()));

    let absent_fields = parse_archived_entry("absent.md", "---\ntype: Note\n---\n# Test\n");
    assert_eq!(absent_fields.status, None);
}

#[test]
fn test_array_field_does_not_break_type_detection() {
    let entry = parse_archived_entry(
        "array-fields.md",
        "---\ntype: Responsibility\nOwner:\n  - Demo User\nCadence:\n  - Weekly\nStatus:\n  - Active\n---\n# My Responsibility\n",
    );
    assert_eq!(entry.is_a, Some("Responsibility".to_string()));
    assert_eq!(entry.status, Some("Active".to_string()));
}
