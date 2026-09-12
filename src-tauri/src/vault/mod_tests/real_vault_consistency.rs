use super::*;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default)]
struct FrontmatterExpectations {
    has_type_with_value: bool,
    has_organized_true: bool,
}

fn frontmatter_block(content: &str) -> Option<&str> {
    if !content.starts_with("---\n") {
        return None;
    }

    let end = content[4..].find("\n---")?;
    Some(&content[4..end + 4])
}

fn frontmatter_expectations(content: &str) -> Option<FrontmatterExpectations> {
    let block = frontmatter_block(content)?;
    let has_type_with_value = block.lines().any(|line| {
        let trimmed = line.trim();
        ["type:", "Is A:", "is_a:"]
            .iter()
            .find_map(|prefix| trimmed.strip_prefix(prefix))
            .is_some_and(|rest| !rest.trim().is_empty())
    });
    let has_organized_true = block.lines().any(|line| {
        matches!(
            line.trim(),
            "_organized: true" | "_organized: True" | "_organized: yes" | "_organized: Yes"
        )
    });

    if !has_type_with_value && !has_organized_true {
        return None;
    }

    Some(FrontmatterExpectations {
        has_type_with_value,
        has_organized_true,
    })
}

fn mismatch_messages(
    path: &Path,
    expectations: FrontmatterExpectations,
    parsed: &VaultEntry,
) -> Vec<String> {
    let mut mismatches = Vec::new();
    if expectations.has_type_with_value && parsed.is_a.is_none() {
        mismatches.push(format!(
            "TYPE MISSING: {} (raw has type with value but parsed isA=None)",
            path.display()
        ));
    }
    if expectations.has_organized_true && !parsed.organized {
        mismatches.push(format!(
            "ORGANIZED MISSING: {} (raw has _organized: true but parsed organized=false)",
            path.display()
        ));
    }
    mismatches
}

fn parse_real_vault_mismatches(vault_path: &Path) -> Vec<String> {
    let mut mismatches = Vec::new();
    let walker = walkdir::WalkDir::new(vault_path)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'));

    for dir_entry in walker.filter_map(|entry| entry.ok()) {
        let path = dir_entry.path();
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "md") {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let Some(expectations) = frontmatter_expectations(&content) else {
            continue;
        };

        match parse_md_file(path, None) {
            Ok(parsed) => mismatches.extend(mismatch_messages(path, expectations, &parsed)),
            Err(error) => mismatches.push(format!("PARSE ERROR: {} -> {}", path.display(), error)),
        }
    }

    mismatches
}

#[test]
fn test_real_vault_type_and_organized_consistency() {
    let vault_path = Path::new("/Users/demo/vault");
    if !vault_path.exists() {
        eprintln!("Skipping: ~/Laputa vault not found");
        return;
    }

    let mismatches = parse_real_vault_mismatches(vault_path);
    if mismatches.is_empty() {
        return;
    }

    let summary = mismatches
        .iter()
        .take(20)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    panic!(
        "Found {} parsing mismatches in real vault:\n{}",
        mismatches.len(),
        summary
    );
}
