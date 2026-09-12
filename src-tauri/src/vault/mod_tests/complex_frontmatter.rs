use super::*;

struct FrontmatterCase {
    file_name: &'static str,
    content: &'static str,
    expected_type: &'static str,
    context: &'static str,
}

const COMPLEX_NOTE_CONTENT: &str = r#"---
type: Note
_organized: true
aliases: ["My Complex Note"]
Topics: ["[[topic-writing]]", "[[topic-productivity|Productivity]]"]
"Created at": "2021-12-31T14:19:00.000Z"
Status: Published
Owner: "[[person-luca|Demo User]]"
---
# My Complex Note

Content here.
"#;

const WRITING_NOTE_CONTENT: &str = r#"---
Is A: Evergreen
_organized: true
aliases: ["Writing for Clarity vs. Writing for Credit"]
Topics: ["[[topic-writing]]"]
Status: Published
"Last edited": "2024-06-15T10:30:00.000Z"
notion_id: "abc123def456"
---
# Writing for Clarity vs. Writing for Credit

Content.
"#;

const YES_NOTE_CONTENT: &str = "---\ntype: Note\n_organized: Yes\nStatus: Draft\n---\n# Test\n";

const TIMESTAMP_NOTE_CONTENT: &str =
    "---\ntype: Note\n_organized: true\nCreated at: 2021-12-31T14:19:00.000Z\nTopics:\n  - \"[[topic-writing]]\"\n---\n# Test\n";

const FLOW_ARRAY_CONTENT: &str = r#"---
type: Evergreen
_organized: true
Topics: ["[[topic-writing]]", "[[topic-productivity|Productivity]]"]
Has: ["[[note-one]]", "[[note-two]]", "[[note-three]]"]
---
# Test
"#;

const NOTION_IMPORT_CONTENT: &str = r#"---
type: Readings
aliases:
  - "1 to 1s"
"Discarded for digest?": false
"Note Status": Saved
URL: "http://theengineeringmanager.com/management-101/121s/"
Author: James Stanier
Category: Articles
"Full Title": 1 to 1s
Highlights: 21
"Last Synced": 2025-12-10
"Last Highlighted": 2021-04-12
notion_id: 2c5bdf02-815c-81ce-9dce-eca60ddaeb08
_organized: true
---

# 1 to 1s

Content.
"#;

const BITCOIN_NOTE_CONTENT: &str = r#"---
type: Note
workspace: personal
notion_id: de48a4ad-e7ad-42aa-a5ce-1efdc259d7f9
"Created at": "2021-12-17T10:21:00.000Z"
Reviewed: True
Topics:
  - "[[web3|Web3 / Crypto]]"
URL: https://studio.glassnode.com/metrics?a=BTC&category=Market%20Indicators&m=indicators.NetUnrealizedProfitLoss&s=1320105600&u=1639267199&zoom=
aliases:
  - Bitcoin: Net Unrealized Profit/Loss (NUPL) - Glassnode Studio
  - Note
Trashed: true
"Trashed at": 2026-03-11
_organized: true
---

# Bitcoin: Net Unrealized Profit/Loss (NUPL) - Glassnode Studio
"#;

fn complex_frontmatter_cases() -> [FrontmatterCase; 7] {
    [
        FrontmatterCase {
            file_name: "complex-note.md",
            content: COMPLEX_NOTE_CONTENT,
            expected_type: "Note",
            context: "type must be parsed correctly with complex frontmatter",
        },
        FrontmatterCase {
            file_name: "writing-note.md",
            content: WRITING_NOTE_CONTENT,
            expected_type: "Evergreen",
            context: "Is A must be parsed correctly with quoted keys nearby",
        },
        FrontmatterCase {
            file_name: "yes-note.md",
            content: YES_NOTE_CONTENT,
            expected_type: "Note",
            context: "_organized: Yes must be parsed as true",
        },
        FrontmatterCase {
            file_name: "timestamp-note.md",
            content: TIMESTAMP_NOTE_CONTENT,
            expected_type: "Note",
            context: "type must survive unquoted timestamp in sibling field",
        },
        FrontmatterCase {
            file_name: "flow-array.md",
            content: FLOW_ARRAY_CONTENT,
            expected_type: "Evergreen",
            context: "type must survive flow arrays with wikilinks",
        },
        FrontmatterCase {
            file_name: "1-to-1s.md",
            content: NOTION_IMPORT_CONTENT,
            expected_type: "Readings",
            context: "type must be parsed with Notion-style quoted keys",
        },
        FrontmatterCase {
            file_name: "bitcoin-note.md",
            content: BITCOIN_NOTE_CONTENT,
            expected_type: "Note",
            context: "type must be parsed correctly even with unquoted colon in alias",
        },
    ]
}

fn assert_complex_frontmatter_case(case: FrontmatterCase) {
    let dir = TempDir::new().unwrap();
    let entry = parse_test_entry(&dir, case.file_name, case.content);
    assert_eq!(
        entry.is_a,
        Some(case.expected_type.to_string()),
        "{}",
        case.context
    );
    assert!(
        entry.organized,
        "{} must keep _organized=true",
        case.file_name
    );
}

#[test]
fn test_complex_frontmatter_preserves_type_and_organized() {
    for case in complex_frontmatter_cases() {
        assert_complex_frontmatter_case(case);
    }
}

#[test]
fn test_fallback_parser_extracts_type_and_organized() {
    use super::frontmatter::extract_fm_and_rels;

    let raw_content =
        "---\ntype: Note\n_organized: true\nBroken: value: with: colons\n---\n# Test\n";
    let (frontmatter, _, _) = extract_fm_and_rels(None, raw_content);
    assert_eq!(
        frontmatter
            .is_a
            .as_ref()
            .and_then(|value| value.clone().into_scalar()),
        Some("Note".to_string())
    );
    assert_eq!(frontmatter.organized, Some(true));
}
