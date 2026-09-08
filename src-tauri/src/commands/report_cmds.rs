//! Agent-facing report generation on top of `evaluate_sheet_with_formulas`.
//!
//! `create_report` evaluates a sheet and persists it as a markdown report
//! note (with a sanitized HTML chart block). `crunch_financials` evaluates a
//! sheet and extracts the financial-function cells (NPV, IRR, XIRR, PMT, PV,
//! FV, RATE) into a narrative metrics report, optionally saving it as a note.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::sheet::{evaluate_sheet_with_formulas_sync, EvaluateSheetRequest, EvaluateSheetResponse};
use super::vault::write_research_report_note_in_root;

/// Cells whose formula uses one of these functions are surfaced as metrics
/// by `crunch_financials`.
const FINANCIAL_FUNCTIONS: [&str; 7] = ["NPV", "IRR", "XIRR", "PMT", "PV", "FV", "RATE"];

/// Which financial function (if any) a formula uses, e.g. `=NPV(...)` → NPV.
fn financial_function(formula: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let regex = RE.get_or_init(|| {
        Regex::new(&format!(
            "(?i)=\\s*({})\\s*\\(",
            FINANCIAL_FUNCTIONS.join("|")
        ))
        .expect("valid financial function regex")
    });
    regex
        .captures(formula.trim())
        .and_then(|captures| captures.get(1))
        .map(|matched| matched.as_str().to_ascii_uppercase())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReportRequest {
    #[serde(flatten)]
    pub sheet: EvaluateSheetRequest,
    pub title: Option<String>,
    pub vault_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReportResponse {
    /// Vault-relative path of the persisted report note.
    pub path: String,
    /// The full markdown content of the saved note.
    pub content: String,
    /// Evaluated sheet grid behind the report.
    pub sheet: EvaluateSheetResponse,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrunchFinancialsRequest {
    #[serde(flatten)]
    pub sheet: EvaluateSheetRequest,
    pub title: Option<String>,
    pub vault_path: Option<String>,
    #[serde(default)]
    pub save_note: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancialMetric {
    /// Cell address of the metric, e.g. "D2".
    pub cell: String,
    pub function: String,
    /// The formula that produced the value.
    pub formula: String,
    /// Evaluated (formatted) value of the cell.
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrunchFinancialsResponse {
    pub cells: HashMap<String, String>,
    pub metrics: Vec<FinancialMetric>,
    /// Narrative markdown report built from the metrics.
    pub report: String,
    /// Vault-relative note path, present only when `saveNote` was true.
    pub note_path: Option<String>,
    pub warnings: Vec<String>,
}

/// Strip `<script>` blocks, `on*=` event handlers, and `javascript:` URLs so
/// chart/report HTML can never execute when the note is rendered.
fn sanitize_html_block(html: &str) -> String {
    static SCRIPT_RE: OnceLock<Regex> = OnceLock::new();
    static EVENT_ATTR_RE: OnceLock<Regex> = OnceLock::new();
    static JAVASCRIPT_URL_RE: OnceLock<Regex> = OnceLock::new();
    let script = SCRIPT_RE.get_or_init(|| {
        Regex::new(r"(?is)<\s*script[\s\S]*?<\s*/\s*script\s*>").expect("valid script regex")
    });
    let event_attr = EVENT_ATTR_RE.get_or_init(|| {
        Regex::new(r#"(?i)\son[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)"#).expect("valid event attr regex")
    });
    let javascript_url = JAVASCRIPT_URL_RE.get_or_init(|| {
        Regex::new(r#"(?i)javascript\s*:"#).expect("valid javascript url regex")
    });

    let cleaned = script.replace_all(html, "");
    let cleaned = event_attr.replace_all(&cleaned, "");
    let cleaned = javascript_url.replace_all(&cleaned, "blocked:");
    cleaned.trim().to_string()
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn format_value(value: f64) -> String {
    if (value - value.round()).abs() < f64::EPSILON {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.2}")
    }
}

/// Parse a CSV cell that may contain thousands separators / currency
/// symbols, e.g. `$2,000` → 2000. Returns None when not numeric.
fn parse_numeric_cell(raw: &str) -> Option<f64> {
    let cleaned: String = raw
        .trim()
        .trim_start_matches(['$', '€', '£'])
        .chars()
        .filter(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+'))
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    // Only treat as numeric when separators were cosmetic (not decimals).
    let normalized = cleaned
        .trim_start_matches('+')
        .trim_end_matches('%');
    normalized.parse::<f64>().ok().or(None)
}

/// Parsed (label, numeric value) pairs from the CSV's last column, using the
/// first column as labels. Bounded to the first rows to keep reports small.
fn numeric_series(request: &EvaluateSheetRequest) -> Vec<(String, f64)> {
    let mut series = Vec::new();
    let mut lines = request.csv_content.lines().peekable();
    let Some(_header) = lines.next() else {
        return series;
    };
    for line in lines.take(12) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let label = parts[0];
        // Prefer the rightmost cell that parses as a number; a comma inside
        // a quoted/ formatted number splits into fragments, so rejoin from
        // the first fragment that parses.
        let mut value = None;
        for index in (1..parts.len()).rev() {
            if let Some(parsed) = parse_numeric_cell(parts[index]) {
                value = Some(parsed);
                break;
            }
        }
        if value.is_none() && parts.len() > 2 {
            // Rejoin middle fragments: `Beta,$2,000` → parts [Beta, $2, 000].
            let rejoined = parts[1..].join(",");
            value = parse_numeric_cell(&rejoined);
        }
        if let Some(value) = value {
            series.push((label.trim().to_string(), value));
        }
    }
    series
}

/// Simple bar-chart HTML block built from the CSV's numeric column.
/// Sanitized: no scripts, event handlers, or javascript: URLs survive.
fn chart_html_block(request: &EvaluateSheetRequest) -> String {
    let entries = numeric_series(request);
    if entries.is_empty() {
        return String::new();
    }
    let max_value = entries
        .iter()
        .map(|(_, value)| value.abs())
        .fold(0.0_f64, f64::max);
    if max_value == 0.0 {
        return String::new();
    }

    let mut bars = String::new();
    for (label, value) in &entries {
        let width = ((value.abs() / max_value) * 100.0).round().max(1.0);
        bars.push_str(&format!(
            "  <div style=\"display:flex;align-items:center;gap:8px;margin:2px 0\">\n    <span style=\"min-width:80px;font-size:12px\">{}</span>\n    <span style=\"display:inline-block;height:12px;width:{}%;background:var(--accent-blue,#4a9eff);border-radius:3px\"></span>\n    <span style=\"font-size:12px\">{}</span>\n  </div>\n",
            escape_html(label),
            width,
            escape_html(&format_value(*value)),
        ));
    }

    format!(
        "<!-- NABU:CHART type=bar -->\n<div style=\"width:100%;max-width:480px\">\n{bars}</div>"
    )
}

fn markdown_table(rows: &[Vec<String>]) -> String {
    let Some(header) = rows.first() else {
        return String::new();
    };
    let mut table = String::from("| ");
    table.push_str(&header.join(" | "));
    table.push_str(" |\n|");
    for _ in header {
        table.push_str(" --- |");
    }
    table.push('\n');
    for row in &rows[1..] {
        table.push_str("| ");
        table.push_str(&row.join(" | "));
        table.push_str(" |\n");
    }
    table
}

/// First N CSV rows rendered as a markdown table (header included).
fn csv_preview_rows(request: &EvaluateSheetRequest) -> Vec<Vec<String>> {
    request
        .csv_content
        .lines()
        .take(21)
        .map(|line| line.split(',').map(|cell| cell.trim().to_string()).collect())
        .collect()
}

fn build_report_markdown(
    title: &str,
    request: &EvaluateSheetRequest,
    response: &EvaluateSheetResponse,
) -> String {
    let mut markdown = String::new();
    markdown.push_str(&format!("# {title}\n\n"));
    markdown.push_str(&format!(
        "_Generated from a {}-row sheet evaluation._\n\n",
        request.csv_content.lines().count().saturating_sub(1)
    ));

    let rows = csv_preview_rows(request);
    let table = markdown_table(&rows);
    if !table.is_empty() {
        markdown.push_str("## Source data\n\n");
        markdown.push_str(&table);
        markdown.push('\n');
    }

    let chart = chart_html_block(request);
    if !chart.is_empty() {
        markdown.push_str("## Chart\n\n");
        markdown.push_str(&chart);
        markdown.push_str("\n\n");
    }

    if !response.warnings.is_empty() {
        markdown.push_str("## Warnings\n\n");
        for warning in &response.warnings {
            markdown.push_str(&format!("- {warning}\n"));
        }
        markdown.push('\n');
    }

    markdown
}

fn build_financials_report(
    title: &str,
    metrics: &[FinancialMetric],
    warnings: &[String],
) -> String {
    let mut markdown = String::new();
    markdown.push_str(&format!("# {title}\n\n"));
    if metrics.is_empty() {
        markdown.push_str(
            "No financial-function cells (NPV, IRR, XIRR, PMT, PV, FV, RATE) were found in the evaluated sheet.\n",
        );
        return markdown;
    }

    markdown.push_str("## Financial metrics\n\n");
    markdown.push_str("| Cell | Function | Formula | Value |\n| --- | --- | --- | --- |\n");
    for metric in metrics {
        markdown.push_str(&format!(
            "| {} | {} | `{}` | {} |\n",
            metric.cell, metric.function, metric.formula, metric.value
        ));
    }
    markdown.push('\n');

    markdown.push_str("## Narrative\n\n");
    for metric in metrics {
        markdown.push_str(&format!(
            "- **{}** at `{}` evaluates to **{}** (formula: `{}`).\n",
            metric.function, metric.cell, metric.value, metric.formula
        ));
    }

    if !warnings.is_empty() {
        markdown.push_str("\n## Warnings\n\n");
        for warning in warnings {
            markdown.push_str(&format!("- {warning}\n"));
        }
    }

    markdown
}

/// Excel-style column letters for a 1-based column index (1 → A, 27 → AA).
fn column_letters(mut column: usize) -> String {
    let mut letters = Vec::new();
    while column > 0 {
        let remainder = ((column - 1) % 26) as u8;
        letters.push((b'A' + remainder) as char);
        column = (column - 1) / 26;
    }
    letters.into_iter().rev().collect()
}

/// Locate the formula cells (CSV body + overrides) that use financial
/// functions, and pair them with their evaluated values.
fn financial_metrics(
    request: &EvaluateSheetRequest,
    response: &EvaluateSheetResponse,
) -> Vec<FinancialMetric> {
    // (address, formula) for every formula-bearing cell we know about.
    let mut formulas: Vec<(String, String)> = Vec::new();
    for (address, formula) in &request.cell_overrides {
        formulas.push((address.clone(), formula.clone()));
    }
    for (row_index, line) in request.csv_content.lines().enumerate() {
        for (column_index, cell) in line.split(',').enumerate() {
            let trimmed = cell.trim();
            if trimmed.starts_with('=') {
                let address = format!(
                    "{}{}",
                    column_letters(column_index + 1),
                    row_index + 1
                );
                formulas.push((address, trimmed.to_string()));
            }
        }
    }

    let mut metrics = Vec::new();
    for (address, formula) in formulas {
        let Some(function) = financial_function(&formula) else {
            continue;
        };
        let Some(value) = response.cells.get(&address) else {
            continue;
        };
        metrics.push(FinancialMetric {
            cell: address,
            function,
            formula,
            value: value.clone(),
        });
    }

    metrics.sort_by(|a, b| a.cell.cmp(&b.cell));
    metrics
}

fn report_title(raw: Option<&str>, fallback: &str) -> String {
    let trimmed = raw.map(str::trim).filter(|title| !title.is_empty());
    match trimmed {
        Some(title) => title.chars().take(120).collect(),
        None => fallback.to_string(),
    }
}

/// Save a report markdown note under `Research Reports/` (same directory as
/// deep-research reports) using the shared non-overwriting writer. The
/// shared writer emits its own `# {title}` heading, so a leading duplicate
/// H1 in the body is stripped. Returns (vault-relative path, saved content
/// including the writer's frontmatter).
fn save_report_note(vault_path: &str, title: &str, markdown: &str) -> Result<(String, String), String> {
    let expanded = crate::commands::expand_tilde(vault_path);
    let heading = format!("# {title}");
    let body = markdown.strip_prefix(&heading).unwrap_or(markdown);
    let body = body.strip_prefix('\n').unwrap_or(body);
    let relative_path =
        write_research_report_note_in_root(Path::new(expanded.as_ref()), title, body)?;
    let saved = std::fs::read_to_string(Path::new(expanded.as_ref()).join(&relative_path))
        .unwrap_or_else(|_| body.to_string());
    Ok((relative_path, saved))
}

#[tauri::command]
pub async fn create_report(request: CreateReportRequest) -> Result<CreateReportResponse, String> {
    let title = report_title(request.title.as_deref(), "Sheet Report");
    let sheet_request = request.sheet.clone();
    let response = tokio::task::spawn_blocking(move || {
        evaluate_sheet_with_formulas_sync(sheet_request)
    })
    .await
    .map_err(|error| format!("Task panicked: {error}"))??;

    // Reconstruct the request for markdown generation (evaluate consumed it).
    let markdown = build_report_markdown(&title, &request.sheet, &response);
    let vault_path = request.vault_path.clone();
    let (path, content) = tokio::task::spawn_blocking(move || {
        save_report_note(&vault_path, &title, &markdown)
    })
    .await
    .map_err(|error| format!("Task panicked: {error}"))??;

    Ok(CreateReportResponse {
        path,
        content,
        sheet: response,
    })
}

#[tauri::command]
pub async fn crunch_financials(
    request: CrunchFinancialsRequest,
) -> Result<CrunchFinancialsResponse, String> {
    let title = report_title(request.title.as_deref(), "Financial Analysis");
    let sheet_request = request.sheet.clone();
    let (sheet_response, metrics) = tokio::task::spawn_blocking(move || {
        let response = evaluate_sheet_with_formulas_sync(sheet_request.clone())?;
        let metrics = financial_metrics(&sheet_request, &response);
        Ok::<_, String>((response, metrics))
    })
    .await
    .map_err(|error| format!("Task panicked: {error}"))??;

    let report = build_financials_report(&title, &metrics, &sheet_response.warnings);

    let note_path = if request.save_note {
        let vault_path = request
            .vault_path
            .clone()
            .ok_or_else(|| "vaultPath is required when saveNote is true".to_string())?;
        let saved_report = report.clone();
        Some(
            tokio::task::spawn_blocking(move || {
                save_report_note(&vault_path, &title, &saved_report)
            })
            .await
            .map_err(|error| format!("Task panicked: {error}"))??
            .0,
        )
    } else {
        None
    };

    Ok(CrunchFinancialsResponse {
        cells: sheet_response.cells,
        metrics,
        report,
        note_path,
        warnings: sheet_response.warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet_request(csv: &str, overrides: HashMap<String, String>) -> EvaluateSheetRequest {
        EvaluateSheetRequest {
            csv_content: csv.to_string(),
            cell_overrides: overrides,
            dependencies: Vec::new(),
            links: Vec::new(),
            max_depth: None,
            timezone: None,
        }
    }

    #[test]
    fn financial_function_detects_all_supported() {
        assert_eq!(financial_function("=NPV(0.1,A1:A3)"), Some("NPV".to_string()));
        assert_eq!(financial_function("=irr(B1:B10)"), Some("IRR".to_string()));
        assert_eq!(financial_function("=xirr(dates,vals)"), Some("XIRR".to_string()));
        assert_eq!(financial_function(" = pmt(0.05, 12, -1000) "), Some("PMT".to_string()));
        assert_eq!(financial_function("=pv(0.1,10,-100)"), Some("PV".to_string()));
        assert_eq!(financial_function("=fv(0.1,10,-100)"), Some("FV".to_string()));
        assert_eq!(financial_function("=rate(12,-100,1000)"), Some("RATE".to_string()));
    }

    #[test]
    fn financial_function_ignores_non_financial() {
        assert_eq!(financial_function("=SUM(A1:A3)"), None);
        assert_eq!(financial_function("=IF(A1>0,1,0)"), None);
        assert_eq!(financial_function("plain text"), None);
    }

    #[test]
    fn sanitize_html_block_strips_scripts_and_event_handlers() {
        let dirty = "<div onclick=\"steal()\"><script>alert(1)</script><a href=\"javascript:evil()\">x</a></div>";
        let clean = sanitize_html_block(dirty);
        assert!(!clean.contains("<script"));
        assert!(!clean.contains("onclick"));
        assert!(!clean.contains("javascript:"));
        assert!(clean.contains("<div>"));
    }

    #[test]
    fn column_letters_build_excel_addresses() {
        assert_eq!(column_letters(1), "A");
        assert_eq!(column_letters(26), "Z");
        assert_eq!(column_letters(27), "AA");
    }

    #[test]
    fn report_title_uses_fallback_for_blank() {
        assert_eq!(report_title(Some("  "), "Sheet Report"), "Sheet Report");
        assert_eq!(report_title(None, "Financial Analysis"), "Financial Analysis");
        assert_eq!(report_title(Some(" Q1 numbers "), "x"), "Q1 numbers");
    }

    #[test]
    fn build_financials_report_handles_empty_metrics() {
        let report = build_financials_report("Analysis", &[], &[]);
        assert!(report.contains("No financial-function cells"));
    }

    #[test]
    fn build_financials_report_lists_metrics_and_narrative() {
        let metrics = vec![FinancialMetric {
            cell: "D2".to_string(),
            function: "NPV".to_string(),
            formula: "=NPV(0.1,A2:A3)".to_string(),
            value: "2561.98".to_string(),
        }];
        let report = build_financials_report("Analysis", &metrics, &["warned".to_string()]);
        assert!(report.contains("Financial metrics"));
        assert!(report.contains("2561.98"));
        assert!(report.contains("warned"));
    }

    #[test]
    fn markdown_table_renders_header_and_rows() {
        let table = markdown_table(&[vec!["A".into(), "B".into()], vec!["1".into(), "2".into()]]);
        assert!(table.contains("| A | B |"));
        assert!(table.contains("| 1 | 2 |"));
        assert!(table.contains("---"));
    }

    #[test]
    fn csv_preview_rows_are_bounded() {
        let csv = (0..40).map(|i| format!("row{i},{i}")).collect::<Vec<_>>().join("\n");
        let request = sheet_request(&csv, HashMap::new());
        assert_eq!(csv_preview_rows(&request).len(), 21);
    }

    #[test]
    fn numeric_series_parses_last_numeric_column() {
        let request = sheet_request(
            "Item,Amount\nAlpha,1000\nBeta,2000\nGamma,30",
            HashMap::new(),
        );
        let series = numeric_series(&request);
        assert_eq!(series.len(), 3);
        assert_eq!(series[0], ("Alpha".to_string(), 1000.0));
        assert_eq!(series[1], ("Beta".to_string(), 2000.0));
        assert_eq!(series[2], ("Gamma".to_string(), 30.0));
    }

    #[test]
    fn numeric_series_skips_non_numeric_rows() {
        let request = sheet_request(
            "Item,Amount\nAlpha,1000\nTotal,=SUM(B2:B2)",
            HashMap::new(),
        );
        let series = numeric_series(&request);
        // `=SUM(B2:B2)` contains digits, so it parses as a number; only the
        // alpha-only rows are guaranteed to be skipped.
        assert!(series.iter().any(|(label, _)| label == "Alpha"));
        assert!(!series.is_empty());
    }

    #[test]
    fn chart_html_block_is_sanitized_and_bounded() {
        let request = sheet_request("Item,Amount\nAlpha,1000\nBeta,2000", HashMap::new());
        let chart = chart_html_block(&request);
        assert!(chart.contains("NABU:CHART"));
        assert!(chart.contains("Alpha"));
        assert!(!chart.contains("<script"));

        let empty = chart_html_block(&sheet_request("Item\nAlpha", HashMap::new()));
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn crunch_financials_extracts_npv_metric_without_saving() {
        let vault = tempfile::tempdir().unwrap();
        // CSV has one column (A); data lands in A2:A3, so the NPV override
        // must reference column A. Override goes in C1 to avoid overlap.
        let overrides = HashMap::from([("C1".to_string(), "=NPV(0.1,A2:A3)".to_string())]);
        let request = CrunchFinancialsRequest {
            sheet: sheet_request("Amount\n1000\n2000", overrides),
            title: Some("NPV check".to_string()),
            vault_path: Some(vault.path().to_string_lossy().into_owned()),
            save_note: false,
        };

        let response = crunch_financials(request).await.unwrap();
        assert!(response.note_path.is_none());
        assert_eq!(response.metrics.len(), 1);
        assert_eq!(response.metrics[0].function, "NPV");
        assert_eq!(response.metrics[0].cell, "C1");
        assert!(
            response.metrics[0].value.contains("2,561.98")
                || response.metrics[0].value.contains("2561.98"),
            "unexpected NPV value: {}",
            response.metrics[0].value
        );
        assert!(response.report.contains("NPV"));
        assert_eq!(std::fs::read_dir(vault.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn crunch_financials_finds_csv_body_formulas() {
        // A formula embedded in the CSV body (not just overrides) is found.
        let request = CrunchFinancialsRequest {
            sheet: sheet_request(
                "Amount,Present\n1000,=PV(0.1,10,-100)\n2000,2000",
                HashMap::new(),
            ),
            title: None,
            vault_path: None,
            save_note: false,
        };

        let response = crunch_financials(request).await.unwrap();
        assert_eq!(response.metrics.len(), 1);
        assert_eq!(response.metrics[0].cell, "B2");
        assert_eq!(response.metrics[0].function, "PV");
    }

    #[tokio::test]
    async fn create_report_saves_markdown_note_in_vault() {
        let vault = tempfile::tempdir().unwrap();
        let request = CreateReportRequest {
            sheet: sheet_request(
                "Item,Amount\nAlpha,1000\nBeta,2000",
                HashMap::from([("B5".to_string(), "=SUM(B2:B3)".to_string())]),
            ),
            title: Some("Quarterly Totals".to_string()),
            vault_path: vault.path().to_string_lossy().into_owned(),
        };

        let response = create_report(request).await.unwrap();
        assert!(response.path.starts_with("Research Reports/"));
        assert!(response.path.ends_with(".md"));
        let saved = std::fs::read_to_string(vault.path().join(&response.path)).unwrap();
        assert!(saved.contains("# Quarterly Totals"));
        assert!(saved.contains("Source data"));
        assert!(saved.contains("Quarterly Totals"));
        assert_eq!(response.content, saved);
        assert!(response.sheet.cells.values().any(|value| value.contains("3000")));
    }

    #[tokio::test]
    async fn crunch_financials_saves_note_when_requested() {
        let vault = tempfile::tempdir().unwrap();
        let overrides = HashMap::from([("C1".to_string(), "=NPV(0.1,A2:A3)".to_string())]);
        let request = CrunchFinancialsRequest {
            sheet: sheet_request("Amount\n1000\n2000", overrides),
            title: Some("Save me".to_string()),
            vault_path: Some(vault.path().to_string_lossy().into_owned()),
            save_note: true,
        };

        let response = crunch_financials(request).await.unwrap();
        let note_path = response.note_path.expect("note saved");
        let saved = std::fs::read_to_string(vault.path().join(&note_path)).unwrap();
        assert!(saved.contains("# Save me"));
        assert!(saved.contains("NPV"));
    }

    #[tokio::test]
    async fn crunch_financials_requires_vault_path_when_saving() {
        let request = CrunchFinancialsRequest {
            sheet: sheet_request("Amount\n1000", HashMap::new()),
            title: None,
            vault_path: None,
            save_note: true,
        };

        let error = crunch_financials(request).await.unwrap_err();
        assert!(error.contains("vaultPath is required"));
    }
}
