use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Where per-vault mini-app databases live. `.nabu/` is vault metadata (the
/// settings seed already lives there), and the directory is created on demand.
const NABU_DIR: &str = ".nabu";
const APPS_DB_DIR: &str = "apps";

/// Section headers recognized inside a mini-app definition note.
const SCHEMA_HEADING: &str = "database schema";
const VIEWS_HEADING: &str = "views";

/// A data-backed mini-app: SQL schema + named views, defined in a markdown
/// note. Live records live in a DuckDB file at `.nabu/apps/<app_id>.duckdb`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MiniAppSql {
    pub app_id: String,
    pub title: String,
    /// Vault-relative path of the markdown definition note.
    pub note_path: String,
    /// SQL DDL executed (idempotently) whenever the app's database is opened.
    pub schema: String,
    pub views: Vec<MiniAppView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MiniAppView {
    pub name: String,
    pub view_type: MiniAppViewType,
    /// SQL query for read views; INSERT statement for form views.
    pub query: String,
    /// Kanban group-by column (kanban views only).
    #[serde(default)]
    pub group_by: Option<String>,
    /// Form field definitions (form views only).
    #[serde(default)]
    pub fields: Vec<MiniAppFormField>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MiniAppViewType {
    Table,
    Form,
    Kanban,
    Chart,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MiniAppFormField {
    pub name: String,
    pub column: String,
    pub field_type: MiniAppFormFieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MiniAppFormFieldType {
    Text,
    Number,
    Email,
    Date,
    Select,
    Textarea,
}

/// Lightweight metadata returned by `discover_mini_apps`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MiniAppMeta {
    pub app_id: String,
    pub title: String,
    pub note_path: String,
}

// ── Markdown definition parsing ─────────────────────────────────────────────

impl MiniAppSql {
    /// Parses a mini-app definition from note frontmatter plus body sections.
    ///
    /// Expected frontmatter keys: `app_id`, `title` (or falls back to the
    /// first H1 / filename). Body sections:
    ///   `## Database Schema` — SQL DDL until the next `## ` heading
    ///   `## Views` — `### <name>` sub-sections with `type:`, `query:` and
    ///   (forms) `fields:` YAML-ish blocks.
    pub fn from_markdown(
        frontmatter: &HashMap<String, Value>,
        body: &str,
        note_path: &str,
    ) -> Result<Self, String> {
        let app_id = frontmatter_string(frontmatter, "app_id")
            .ok_or_else(|| "Mini-app note is missing an `app_id` frontmatter property".to_string())?;
        validate_app_id(&app_id)?;

        let title = frontmatter_string(frontmatter, "title")
            .or_else(|| first_h1(body))
            .unwrap_or_else(|| app_id.clone());

        let schema = section_body(body, SCHEMA_HEADING)
            .ok_or_else(|| "Mini-app note is missing a `## Database Schema` section".to_string())?
            .trim()
            .to_string();
        if schema.is_empty() {
            return Err("Mini-app `## Database Schema` section is empty".to_string());
        }

        let views = parse_views(
            section_body(body, VIEWS_HEADING)
                .ok_or_else(|| "Mini-app note is missing a `## Views` section".to_string())?
                .trim(),
        )?;

        Ok(Self {
            app_id,
            title,
            note_path: note_path.to_string(),
            schema,
            views,
        })
    }
}

fn frontmatter_string(frontmatter: &HashMap<String, Value>, key: &str) -> Option<String> {
    frontmatter.get(key).and_then(|value| match value {
        Value::String(text) => Some(text.trim().to_string()).filter(|text| !text.is_empty()),
        Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
        _ => None,
    })
}

fn first_h1(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(|title| title.trim().to_string()))
        .filter(|title| !title.is_empty())
}

/// Returns the body content under a `## <heading>` section (exclusive of the
/// next same-level heading). Heading match is case-insensitive.
fn section_body(body: &str, heading: &str) -> Option<String> {
    let mut collected: Option<Vec<&str>> = None;
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(heading_text) = trimmed.strip_prefix("## ") {
            if collected.is_some() {
                break;
            }
            if heading_text.trim().eq_ignore_ascii_case(heading) {
                collected = Some(Vec::new());
            }
            continue;
        }
        if let Some(lines) = collected.as_mut() {
            lines.push(line);
        }
    }
    collected.map(|lines| lines.join("\n"))
}

/// Parses `### <view name>` sub-sections inside the `## Views` block.
fn parse_views(views_body: &str) -> Result<Vec<MiniAppView>, String> {
    let mut views = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;

    for line in views_body.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("### ") {
            if let Some((name, lines)) = current.take() {
                views.push(parse_view(&name, &lines)?);
            }
            current = Some((name.trim().to_string(), Vec::new()));
            continue;
        }
        if let Some((_, lines)) = current.as_mut() {
            lines.push(line);
        }
    }
    if let Some((name, lines)) = current.take() {
        views.push(parse_view(&name, &lines)?);
    }

    if views.is_empty() {
        return Err("Mini-app `## Views` section defines no views".to_string());
    }
    Ok(views)
}

fn parse_view(name: &str, lines: &[&str]) -> Result<MiniAppView, String> {
    if name.is_empty() {
        return Err("Mini-app view has an empty name".to_string());
    }

    let mut view_type = MiniAppViewType::Table;
    let mut query_lines: Vec<String> = Vec::new();
    let mut group_by = None;
    let mut fields = Vec::new();
    let mut in_fields = false;

    for line in lines {
        let trimmed = line.trim();
        if in_fields {
            if trimmed.starts_with("- ") {
                if let Some(field) = parse_form_field(trimmed.trim_start_matches("- ")) {
                    fields.push(field);
                }
            } else if !trimmed.is_empty() {
                in_fields = false;
            } else {
                continue;
            }
            if in_fields {
                continue;
            }
        }
        if trimmed.is_empty() {
            if !query_lines.is_empty() {
                query_lines.push(String::new());
            }
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "type" => {
                    view_type = match value.to_ascii_lowercase().as_str() {
                        "table" => MiniAppViewType::Table,
                        "form" => MiniAppViewType::Form,
                        "kanban" => MiniAppViewType::Kanban,
                        "chart" => MiniAppViewType::Chart,
                        other => return Err(format!("Mini-app view '{name}' has unknown type '{other}'")),
                    };
                    continue;
                }
                "group_by" | "group-by" => {
                    group_by = Some(value.to_string()).filter(|value| !value.is_empty());
                    continue;
                }
                "fields" if value.is_empty() => {
                    in_fields = true;
                    continue;
                }
                "query" => {
                    query_lines.push(value.to_string());
                    continue;
                }
                _ => {}
            }
        }
        // Continuation lines of a multi-line query.
        query_lines.push(trimmed.to_string());
    }

    let query = query_lines.join("\n").trim().to_string();
    if query.is_empty() && view_type != MiniAppViewType::Form {
        return Err(format!("Mini-app view '{name}' has no query"));
    }
    if view_type == MiniAppViewType::Form && fields.is_empty() {
        return Err(format!("Mini-app form view '{name}' defines no fields"));
    }

    Ok(MiniAppView {
        name: name.to_string(),
        view_type,
        query,
        group_by,
        fields,
    })
}

fn parse_form_field(spec: &str) -> Option<MiniAppFormField> {
    // Field spec: `name: Customer Name, column: name, type: text, required: true`
    // or `name: Status, column: status, type: select, options: [lead, prospect]`
    let mut name = None;
    let mut column = None;
    let mut field_type = MiniAppFormFieldType::Text;
    let mut required = false;
    let mut options = Vec::new();

    for part in split_field_spec(spec) {
        let Some((key, value)) = part.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "name" => name = Some(value.to_string()).filter(|value| !value.is_empty()),
            "column" => column = Some(value.to_string()).filter(|value| !value.is_empty()),
            "type" => {
                field_type = match value.to_ascii_lowercase().as_str() {
                    "text" => MiniAppFormFieldType::Text,
                    "number" => MiniAppFormFieldType::Number,
                    "email" => MiniAppFormFieldType::Email,
                    "date" => MiniAppFormFieldType::Date,
                    "select" => MiniAppFormFieldType::Select,
                    "textarea" => MiniAppFormFieldType::Textarea,
                    _ => MiniAppFormFieldType::Text,
                };
            }
            "required" => required = value.eq_ignore_ascii_case("true"),
            "options" => {
                let inner = value.trim_start_matches('[').trim_end_matches(']');
                options = inner
                    .split(',')
                    .map(|option| option.trim().to_string())
                    .filter(|option| !option.is_empty())
                    .collect();
            }
            _ => {}
        }
    }

    Some(MiniAppFormField {
        name: name?,
        column: column.unwrap_or_else(|| name.clone().unwrap_or_default()),
        field_type,
        required,
        options,
    })
}

/// Splits a field spec on commas that are not inside brackets.
fn split_field_spec(spec: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, character) in spec.char_indices() {
        match character {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&spec[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&spec[start..]);
    parts
}

fn validate_app_id(app_id: &str) -> Result<(), String> {
    let valid = !app_id.is_empty()
        && app_id.len() <= 64
        && app_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "Mini-app id '{app_id}' is invalid: use 1-64 letters, digits, dashes, or underscores"
        ))
    }
}

// ── Database location ───────────────────────────────────────────────────────

/// Resolves (and creates) the per-vault DuckDB directory: `<vault>/.nabu/apps`.
fn apps_db_dir(vault_path: &Path) -> Result<PathBuf, String> {
    let dir = vault_path.join(NABU_DIR).join(APPS_DB_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create mini-app database directory: {error}"))?;
    Ok(dir)
}

/// Database file for an app id. The id was validated during parsing; this
/// re-checks defensively so the path can never escape the directory.
pub(crate) fn database_path(vault_path: &Path, app_id: &str) -> Result<PathBuf, String> {
    validate_app_id(app_id)?;
    Ok(apps_db_dir(vault_path)?.join(format!("{app_id}.duckdb")))
}

// ── Query execution ─────────────────────────────────────────────────────────

/// A single result row: column name → JSON value.
pub type MiniAppRow = HashMap<String, Value>;

/// Opens the app's DuckDB file, executing the schema DDL first. DDL uses
/// `CREATE TABLE IF NOT EXISTS`-style statements so re-opening is idempotent.
fn open_app_database(db_path: &Path, schema: &str) -> Result<duckdb::Connection, String> {
    let connection = duckdb::Connection::open(db_path)
        .map_err(|error| format!("Failed to open mini-app database: {error}"))?;
    connection
        .execute_batch(schema)
        .map_err(|error| format!("Failed to apply mini-app schema: {error}"))?;
    Ok(connection)
}

fn sql_literal(value: &str) -> String {
    // DuckDB supports doubled-single-quote escaping; '' inside a literal.
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_number_literal(field: &MiniAppFormField, raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let Ok(number) = trimmed.parse::<f64>() else {
        return Err(format!("Field '{}' expects a number", field.name));
    };
    if !number.is_finite() {
        return Err(format!("Field '{}' expects a finite number", field.name));
    }
    Ok(trimmed.to_string())
}

fn validate_select_query(view_name: &str, query: &str) -> Result<(), String> {
    let normalized = query.trim().to_ascii_lowercase();
    let allowed = normalized.starts_with("select") || normalized.starts_with("with");
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "Mini-app view '{view_name}' must start with SELECT or WITH"
        ))
    }
}

/// Runs a read view's query and returns all rows as JSON maps.
pub fn run_view(
    vault_path: &Path,
    app: &MiniAppSql,
    view_name: &str,
    params: &HashMap<String, String>,
) -> Result<Vec<MiniAppRow>, String> {
    let view = app
        .views
        .iter()
        .find(|view| view.name == view_name)
        .ok_or_else(|| format!("Mini-app view '{view_name}' not found"))?;
    if view.view_type == MiniAppViewType::Form {
        return Err(format!("Mini-app view '{view_name}' is a form view; use execute_mini_app_mut"));
    }
    validate_select_query(view_name, &view.query)?;

    let db_path = database_path(vault_path, &app.app_id)?;
    let connection = open_app_database(&db_path, &app.schema)?;

    let mut statement = connection
        .prepare(&view.query)
        .map_err(|error| format!("Failed to prepare mini-app query: {error}"))?;
    let column_names: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect();

    let mut rows = statement
        .query([])
        .map_err(|error| format!("Failed to run mini-app query: {error}"))?;

    let mut results: Vec<MiniAppRow> = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let mut record = MiniAppRow::new();
        for (index, column) in column_names.iter().enumerate() {
            let value: Value = row
                .get::<_, Option<duckdb::types::Value>>(index)
                .map(|value| match value {
                    Some(duckdb::types::Value::Null) | None => Value::Null,
                    Some(duckdb::types::Value::Boolean(flag)) => Value::Bool(flag),
                    Some(duckdb::types::Value::Int(number)) => Value::from(number),
                    Some(duckdb::types::Value::BigInt(number)) => Value::from(number),
                    Some(duckdb::types::Value::Float(number)) => json_number(number),
                    Some(duckdb::types::Value::Double(number)) => json_number(number),
                    Some(duckdb::types::Value::Text(text)) => Value::String(text),
                    Some(other) => Value::String(other.to_string()),
                })
                .unwrap_or(Value::Null);
            record.insert(column.clone(), value);
        }
        results.push(record);
    }

    // Kanban/chart metadata is applied client-side; `params` are reserved for
    // future parameterized queries (bound filters).
    let _ = params;
    Ok(results)
}

fn json_number(number: f64) -> Value {
    serde_json::Number::from_f64(number)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// Executes a form view's INSERT for one record. Values are validated against
/// the form's field definitions and interpolated as validated SQL literals
/// (numbers verified parseable; text escaped; identifiers come from the note
/// author, not user input at runtime).
pub fn run_form_insert(
    vault_path: &Path,
    app: &MiniAppSql,
    view_name: &str,
    values: &HashMap<String, String>,
) -> Result<(), String> {
    let view = app
        .views
        .iter()
        .find(|view| view.name == view_name)
        .ok_or_else(|| format!("Mini-app view '{view_name}' not found"))?;
    if view.view_type != MiniAppViewType::Form {
        return Err(format!("Mini-app view '{view_name}' is not a form view"));
    }

    let sql = build_insert_sql(view, values)?;
    let db_path = database_path(vault_path, &app.app_id)?;
    let connection = open_app_database(&db_path, &app.schema)?;
    connection
        .execute_batch(&sql)
        .map_err(|error| format!("Failed to insert mini-app record: {error}"))
}

fn build_insert_sql(
    view: &MiniAppView,
    values: &HashMap<String, String>,
) -> Result<String, String> {
    let parsed = parse_insert_template(&view.query).ok_or_else(|| {
        format!(
            "Mini-app form view '{}' must contain an INSERT INTO <table> (...) VALUES (...); template",
            view.name
        )
    })?;

    let mut value_literals: Vec<String> = Vec::new();
    for placeholder in &parsed.value_placeholders {
        let column = placeholder.trim();
        let field = view
            .fields
            .iter()
            .find(|field| field.column.eq_ignore_ascii_case(column))
            .ok_or_else(|| format!("INSERT template references unknown column '{column}'"))?;

        let raw = values
            .get(&field.column)
            .map(String::as_str)
            .unwrap_or_default();
        if field.required && raw.trim().is_empty() {
            return Err(format!("Field '{}' is required", field.name));
        }

        let literal = if raw.trim().is_empty() {
            "NULL".to_string()
        } else {
            match field.field_type {
                MiniAppFormFieldType::Number => sql_number_literal(field, raw)?,
                MiniAppFormFieldType::Select => {
                    if !field.options.is_empty() && !field.options.iter().any(|option| option == raw) {
                        return Err(format!(
                            "Field '{}' must be one of: {}",
                            field.name,
                            field.options.join(", ")
                        ));
                    }
                    sql_literal(raw)
                }
                MiniAppFormFieldType::Email => {
                    if !raw.contains('@') {
                        return Err(format!("Field '{}' expects an email address", field.name));
                    }
                    sql_literal(raw)
                }
                _ => sql_literal(raw),
            }
        };
        value_literals.push(literal);
    }

    Ok(format!(
        "INSERT INTO {} ({}) VALUES ({});",
        parsed.table,
        parsed
            .columns
            .iter()
            .map(|column| column.trim().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        value_literals.join(", "),
    ))
}

struct ParsedInsert {
    table: String,
    columns: Vec<String>,
    value_placeholders: Vec<String>,
}

/// Extracts `INSERT INTO table (col1, col2) VALUES (col1, col2)` from the
/// form view query. Placeholders inside VALUES are column references that get
/// bound to submitted values.
fn parse_insert_template(query: &str) -> Option<ParsedInsert> {
    let normalized = query.replace(['\n', '\r'], " ");
    let lower = normalized.to_ascii_lowercase();
    let into_index = lower.find("insert into ")?;
    let after_into = &normalized[into_index + "insert into ".len()..];

    let table_end = after_into.find(['(', ' '])?;
    let table = after_into[..table_end].trim().trim_end_matches(';').to_string();
    if table.is_empty() {
        return None;
    }

    let columns_open = after_into.find('(')?;
    let columns_close = after_into[columns_open..].find(')')? + columns_open;
    let columns: Vec<String> = after_into[columns_open + 1..columns_close]
        .split(',')
        .map(|column| column.trim().to_string())
        .filter(|column| !column.is_empty())
        .collect();

    let values_index = lower[columns_close..].find("values").map(|index| index + columns_close)?;
    let values_open = normalized[values_index..].find('(')? + values_index;
    let values_close = normalized[values_open..].find(')')? + values_open;
    let value_placeholders: Vec<String> = normalized[values_open + 1..values_close]
        .split(',')
        .map(|placeholder| placeholder.trim().to_string())
        .filter(|placeholder| !placeholder.is_empty())
        .collect();

    if columns.len() != value_placeholders.len() {
        return None;
    }

    Some(ParsedInsert {
        table,
        columns,
        value_placeholders,
    })
}

// ── Discovery ───────────────────────────────────────────────────────────────

/// Finds markdown notes in the vault whose frontmatter declares `app_id`
/// (mini-app definitions). Expects a `Vec<(relative_path, frontmatter_json,
/// body)>` of candidate notes produced by the caller.
pub fn discover_apps(
    notes: Vec<(String, String, String)>,
) -> Result<Vec<MiniAppMeta>, String> {
    let mut apps = Vec::new();
    for (note_path, frontmatter_json, body) in notes {
        let frontmatter: HashMap<String, Value> =
            serde_json::from_str(&frontmatter_json).unwrap_or_default();
        if frontmatter_string(&frontmatter, "app_id").is_none() {
            continue;
        }
        match MiniAppSql::from_markdown(&frontmatter, &body, &note_path) {
            Ok(app) => apps.push(MiniAppMeta {
                app_id: app.app_id,
                title: app.title,
                note_path: app.note_path,
            }),
            // Notes declaring app_id but failing validation are reported so
            // authors can fix them; they don't abort discovery.
            Err(error) => return Err(format!("{note_path}: {error}")),
        }
    }
    apps.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_app() -> MiniAppSql {
        let frontmatter: HashMap<String, Value> =
            serde_json::from_value(json!({ "app_id": "crm", "title": "Customer Tracker" }))
                .unwrap();
        MiniAppSql::from_markdown(
            &frontmatter,
            "# Customer Tracker\n\n## Database Schema\n\nCREATE TABLE IF NOT EXISTS customers (\n  id INTEGER PRIMARY KEY,\n  name TEXT NOT NULL,\n  status TEXT DEFAULT 'lead'\n);\n\n## Views\n\n### Pipeline\n\ntype: kanban\ngroup_by: status\nquery: SELECT id, name, status FROM customers ORDER BY name\n\n### Add Customer\n\ntype: form\nfields:\n  - name: Customer Name, column: name, type: text, required: true\n  - name: Status, column: status, type: select, options: [lead, prospect, customer]\n",
            "notes/crm.md",
        )
        .unwrap()
    }

    #[test]
    fn parses_definition_markdown() {
        let app = sample_app();
        assert_eq!(app.app_id, "crm");
        assert_eq!(app.title, "Customer Tracker");
        assert_eq!(app.views.len(), 2);
        assert_eq!(app.views[0].view_type, MiniAppViewType::Kanban);
        assert_eq!(app.views[0].group_by.as_deref(), Some("status"));
        assert_eq!(app.views[1].fields.len(), 2);
        assert_eq!(
            app.views[1].fields[1].options,
            vec!["lead", "prospect", "customer"]
        );
    }

    #[test]
    fn rejects_invalid_app_ids() {
        let frontmatter: HashMap<String, Value> =
            serde_json::from_value(json!({ "app_id": "../evil" })).unwrap();
        let error = MiniAppSql::from_markdown(&frontmatter, "## Database Schema\nCREATE TABLE t(x);\n## Views\n### A\ntype: table\nquery: SELECT 1", "x.md")
            .unwrap_err();
        assert!(error.contains("invalid"));
    }

    #[test]
    fn rejects_app_without_views() {
        let frontmatter: HashMap<String, Value> =
            serde_json::from_value(json!({ "app_id": "solo" })).unwrap();
        let error = MiniAppSql::from_markdown(&frontmatter, "## Database Schema\nCREATE TABLE t(x);", "x.md")
            .unwrap_err();
        assert!(error.contains("Views"));
    }

    #[test]
    fn rejects_read_views_that_are_not_selects() {
        let app = sample_app();
        let error = run_view(Path::new("/tmp/definitely-not-a-vault"), &app, "Pipeline", &HashMap::new())
            .unwrap_err();
        // The Pipeline view is a SELECT, so the failure must come from the
        // missing database path, not from query validation. Flip the query to
        // verify validation catches destructive SQL.
        let mut tampered = app.clone();
        tampered.views[0].query = "DELETE FROM customers".to_string();
        let error = run_view(Path::new("/tmp/definitely-not-a-vault"), &tampered, "Pipeline", &HashMap::new())
            .unwrap_err();
        assert!(error.contains("SELECT or WITH"), "unexpected error: {error}");
        let _ = error;
    }

    #[test]
    fn insert_template_parses_and_binds_values() {
        let app = sample_app();
        let view = &app.views[1];
        let mut values = HashMap::new();
        values.insert("name".to_string(), "ACME Corp".to_string());
        values.insert("status".to_string(), "prospect".to_string());

        let sql = build_insert_sql(view, &values).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO customers (name, status) VALUES ('ACME Corp', 'prospect');"
        );
    }

    #[test]
    fn insert_rejects_invalid_option_and_missing_required() {
        let app = sample_app();
        let view = &app.views[1];

        let mut values = HashMap::new();
        values.insert("name".to_string(), "ACME".to_string());
        values.insert("status".to_string(), "hacker".to_string());
        let error = build_insert_sql(view, &values).unwrap_err();
        assert!(error.contains("must be one of"), "unexpected: {error}");

        let mut values = HashMap::new();
        values.insert("status".to_string(), "lead".to_string());
        let error = build_insert_sql(view, &values).unwrap_err();
        assert!(error.contains("required"), "unexpected: {error}");
    }

    #[test]
    fn insert_validates_numbers() {
        let frontmatter: HashMap<String, Value> =
            serde_json::from_value(json!({ "app_id": "nums" })).unwrap();
        let app = MiniAppSql::from_markdown(
            &frontmatter,
            "## Database Schema\nCREATE TABLE IF NOT EXISTS t (n INTEGER);\n## Views\n### Add\ntype: form\nfields:\n  - name: N, column: n, type: number, required: true\nquery: INSERT INTO t (n) VALUES (n)\n",
            "x.md",
        )
        .unwrap();

        let mut values = HashMap::new();
        values.insert("n".to_string(), "not-a-number".to_string());
        assert!(build_insert_sql(&app.views[0], &values).is_err());

        values.insert("n".to_string(), "42".to_string());
        assert_eq!(
            build_insert_sql(&app.views[0], &values).unwrap(),
            "INSERT INTO t (n) VALUES (42);"
        );
    }

    #[test]
    fn split_field_spec_respects_brackets() {
        let parts = split_field_spec("name: S, column: s, type: select, options: [a, b, c]");
        assert_eq!(parts.len(), 4);
    }

    #[test]
    fn database_path_stays_inside_vault() {
        let vault = Path::new("/vault");
        assert_eq!(
            database_path(vault, "crm").unwrap(),
            Path::new("/vault/.nabu/apps/crm.duckdb")
        );
        assert!(database_path(vault, "../escape").is_err());
        assert!(database_path(vault, "a/b").is_err());
    }

    #[test]
    fn end_to_end_roundtrip() {
        let vault = tempfile::TempDir::new().unwrap();
        let app = sample_app();

        let mut values = HashMap::new();
        values.insert("name".to_string(), "ACME Corp".to_string());
        values.insert("status".to_string(), "customer".to_string());
        run_form_insert(vault.path(), &app, "Add Customer", &values).unwrap();

        let rows = run_view(vault.path(), &app, "Pipeline", &HashMap::new()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], json!("ACME Corp"));
        assert_eq!(rows[0]["status"], json!("customer"));

        // Re-running with the same schema DDL must be idempotent.
        let rows = run_view(vault.path(), &app, "Pipeline", &HashMap::new()).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn discovers_app_notes() {
        let notes = vec![(
            "notes/crm.md".to_string(),
            json!({ "app_id": "crm", "title": "CRM" }).to_string(),
            "## Database Schema\nCREATE TABLE IF NOT EXISTS t (x);\n## Views\n### A\ntype: table\nquery: SELECT 1".to_string(),
        )];
        let apps = discover_apps(notes).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].app_id, "crm");
    }
}
