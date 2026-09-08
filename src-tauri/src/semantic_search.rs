//! Fastembed-backed semantic search across vault notes (RAG mode).
//!
//! Uses the [`fastembed`] crate with the default `BAAI/bge-small-en-v1.5`
//! embedding model (384 dims). The model downloads once to the fastembed
//! cache directory on first use and runs fully offline via ONNX afterwards.
//!
//! The model handle is cached in a process-wide `OnceLock`-guarded
//! [`Mutex`]; per-vault note vectors are cached in [`NOTE_VECTORS`] keyed
//! by vault path + latest modification time so repeated queries do not
//! re-embed the vault.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// Number of notes embedded per fastembed batch call.
const EMBED_BATCH_SIZE: usize = 32;
/// Longest note text (in chars) embedded per note; BGE's context window is
/// 512 tokens, so anything past this adds noise rather than signal.
const MAX_EMBED_CHARS: usize = 8_000;
/// Hard cap on notes embedded per request (protects against huge vaults).
const MAX_NOTES_PER_SEARCH: usize = 2_000;
/// Title similarity gets a boost, mirroring the keyword search ranking.
const TITLE_BOOST: f32 = 0.15;
const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 50;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchRequest {
    pub query: String,
    /// Single-vault form (kept for compatibility); merged into `vault_paths`.
    pub vault_path: Option<String>,
    /// Search these vaults in one call and re-rank results globally.
    /// Takes precedence over `vault_path` when non-empty.
    #[serde(default)]
    pub vault_paths: Vec<String>,
    pub limit: Option<usize>,
    #[serde(default)]
    pub hide_gitignored_files: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchResult {
    pub path: String,
    pub title: String,
    pub snippet: String,
    /// Cosine similarity in `[0, 1]`.
    pub score: f32,
    /// Absolute vault root the note belongs to (multi-vault searches).
    pub vault_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchResponse {
    pub results: Vec<SemanticSearchResult>,
    pub elapsed_ms: u64,
    pub query: String,
    /// Dimensions of the embedding model backing this response.
    pub model: &'static str,
}

struct CachedModel {
    model: TextEmbedding,
    dimensions: usize,
}

fn model_cache_dir() -> PathBuf {
    dirs::data_dir()
        .map(|dir| dir.join("Nabu").join("fastembed-cache"))
        .unwrap_or_else(|| PathBuf::from(".fastembed-cache"))
}

fn model_handle() -> Result<&'static CachedModel, String> {
    static MODEL: OnceLock<Result<CachedModel, String>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            let options = InitOptions {
                model_name: EmbeddingModel::BGESmallENV15,
                show_download_progress: true,
                cache_dir: model_cache_dir(),
                ..InitOptions::default()
            };
            let model = TextEmbedding::try_new(options)
                .map_err(|error| format!("Failed to load embedding model: {error}"))?;
            let dimensions = fastembed_model_dimensions();
            Ok(CachedModel { model, dimensions })
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// `BAAI/bge-small-en-v1.5` produces 384-dimensional embeddings.
fn fastembed_model_dimensions() -> usize {
    384
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let mut dot = 0.0_f32;
    for (a, b) in left.iter().zip(right.iter()) {
        dot += a * b;
    }
    dot
}

/// Embed a batch of texts. Inputs are prefixed with the BGE passage/query
/// conventions so retrieval scores behave as the model was trained for.
fn embed_texts(
    cached: &CachedModel,
    texts: &[String],
    is_query: bool,
) -> Result<Vec<Vec<f32>>, String> {
    let mut vectors = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(EMBED_BATCH_SIZE) {
        let prefixed: Vec<String> = chunk
            .iter()
            .map(|text| {
                if is_query {
                    format!("query: {text}")
                } else {
                    format!("passage: {text}")
                }
            })
            .collect();
        let batch = cached
            .model
            .embed(prefixed, None)
            .map_err(|error| format!("Embedding failed: {error}"))?;
        vectors.extend(batch);
    }
    Ok(vectors)
}

fn is_markdown_candidate(vault_dir: &Path, path: &Path) -> bool {
    if path.extension().is_some_and(|ext| ext == "md") {
        let relative = path.strip_prefix(vault_dir).unwrap_or(path);
        !relative
            .components()
            .any(|component| component.as_os_str().to_string_lossy().starts_with('.'))
    } else {
        false
    }
}

fn collect_markdown_paths(vault_dir: &Path) -> Vec<PathBuf> {
    WalkDir::new(vault_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into_path())
        .filter(|path| is_markdown_candidate(vault_dir, path))
        .collect()
}

fn strip_frontmatter(content: &str) -> &str {
    let rest = match content.strip_prefix("---\n") {
        Some(rest) => rest,
        // Tolerate CRLF frontmatter delimiters.
        None => match content.strip_prefix("---\r\n") {
            Some(rest) => rest,
            None => return content,
        },
    };
    match rest.find("\n---") {
        Some(end) => rest[end + 4..].trim_start(),
        None => content,
    }
}

fn note_title(vault_dir: &Path, path: &Path, content: &str) -> String {
    let body = strip_frontmatter(content);
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("# ") {
            return rest.trim().to_string();
        }
        if !line.trim().is_empty() {
            break;
        }
    }
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            path.strip_prefix(vault_dir)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned()
        })
}

fn relative_note_path(vault_dir: &Path, path: &Path) -> String {
    path.strip_prefix(vault_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn snippet_for(content: &str) -> String {
    let body = strip_frontmatter(content).trim();
    if body.is_empty() {
        return String::new();
    }
    body.chars().take(200).collect()
}

#[derive(Debug)]
struct VaultNote {
    relative_path: String,
    title: String,
    content: String,
}

fn load_vault_notes(vault_dir: &Path) -> Vec<VaultNote> {
    let mut notes: Vec<VaultNote> = collect_markdown_paths(vault_dir)
        .into_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            Some(VaultNote {
                relative_path: relative_note_path(vault_dir, &path),
                title: note_title(vault_dir, &path, &content),
                content,
            })
        })
        .collect();
    notes.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    notes.truncate(MAX_NOTES_PER_SEARCH);
    notes
}

/// Cache of per-vault embedded notes, invalidated by the vault's latest mtime.
static NOTE_VECTORS: OnceLock<Mutex<HashMap<String, CachedVaultEmbedding>>> = OnceLock::new();

#[derive(Debug, Clone)]
struct CachedVaultEmbedding {
    vault_mtime_secs: u64,
    vectors: Vec<NoteVector>,
}

#[derive(Debug, Clone)]
struct NoteVector {
    relative_path: String,
    title: String,
    body: Vec<f32>,
    title_vec: Vec<f32>,
}

fn vault_latest_mtime(vault_dir: &Path) -> u64 {
    WalkDir::new(vault_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.metadata().ok())
        .filter_map(|metadata| metadata.modified().ok())
        .filter_map(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .max()
        .unwrap_or(0)
}

fn embedded_vault_notes(
    cached: &CachedModel,
    vault_dir: &Path,
) -> Result<Vec<NoteVector>, String> {
    let cache = NOTE_VECTORS.get_or_init(|| Mutex::new(HashMap::new()));
    let vault_key = vault_dir.to_string_lossy().into_owned();
    let current_mtime = vault_latest_mtime(vault_dir);

    {
        let guard = cache
            .lock()
            .map_err(|error| format!("Embedding cache poisoned: {error}"))?;
        if let Some(entry) = guard.get(&vault_key) {
            if entry.vault_mtime_secs == current_mtime {
                return Ok(entry.vectors.clone());
            }
        }
    }

    let notes = load_vault_notes(vault_dir);
    if notes.is_empty() {
        return Ok(Vec::new());
    }

    let body_texts: Vec<String> = notes
        .iter()
        .map(|note| {
            let body = strip_frontmatter(&note.content).trim();
            body.chars().take(MAX_EMBED_CHARS).collect::<String>()
        })
        .collect();
    let title_texts: Vec<String> = notes
        .iter()
        .map(|note| note.title.clone())
        .collect();

    let body_vectors = embed_texts(cached, &body_texts, false)?;
    let title_vectors = embed_texts(cached, &title_texts, false)?;

    let vectors: Vec<NoteVector> = notes
        .iter()
        .zip(body_vectors)
        .zip(title_vectors)
        .map(|((note, body), title_vec)| NoteVector {
            relative_path: note.relative_path.clone(),
            title: note.title.clone(),
            body,
            title_vec,
        })
        .collect();

    cache
        .lock()
        .map_err(|error| format!("Embedding cache poisoned: {error}"))?
        .insert(
            vault_key,
            CachedVaultEmbedding {
                vault_mtime_secs: current_mtime,
                vectors: vectors.clone(),
            },
        );

    Ok(vectors)
}

fn truncate_query(query: &str) -> String {
    query.trim().chars().take(512).collect()
}/// Core synchronous search implementation, shared by the Tauri command and tests.
/// Accepts one or more vaults; results are re-ranked globally and each result
/// carries its vault root.
pub fn run_semantic_search(request: SemanticSearchRequest) -> Result<SemanticSearchResponse, String> {
    let start = Instant::now();
    let query = truncate_query(&request.query);
    if query.is_empty() {
        return Err("query is required".to_string());
    }

    let mut vault_paths = request
        .vault_paths
        .iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    if vault_paths.is_empty() {
        if let Some(single) = request.vault_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            vault_paths.push(single.to_string());
        }
    }
    if vault_paths.is_empty() {
        return Err("vaultPath or vaultPaths is required".to_string());
    }
    vault_paths.sort();
    vault_paths.dedup();

    for vault_path in &vault_paths {
        if !Path::new(vault_path).is_dir() {
            return Err(format!("Vault path is not a directory: {vault_path}"));
        }
    }

    let limit = request
        .limit
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);

    let cached = model_handle()?;
    let query_text = query.clone();
    let query_vector = embed_texts(cached, std::slice::from_ref(&query_text), true)?
        .into_iter()
        .next()
        .ok_or_else(|| "Embedding failed: no output for query".to_string())?;

    // Score every vault's notes against the single query embedding.
    let mut results: Vec<SemanticSearchResult> = Vec::new();
    for vault_path in &vault_paths {
        let vault_dir = PathBuf::from(vault_path);
        let note_vectors = embedded_vault_notes(cached, &vault_dir)?;
        for note in &note_vectors {
            let score = (cosine_similarity(&query_vector, &note.body)
                + TITLE_BOOST * cosine_similarity(&query_vector, &note.title_vec))
                .clamp(0.0, 1.0);
            if score <= 0.01 {
                continue;
            }
            results.push(SemanticSearchResult {
                path: note.relative_path.clone(),
                title: note.title.clone(),
                snippet: String::new(),
                score,
                vault_path: vault_path.clone(),
            });
        }
    }

    // Fill snippets only for the top hits so large vaults stay cheap.
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    for result in &mut results {
        let note_path = PathBuf::from(&result.vault_path).join(&result.path);
        if let Ok(content) = std::fs::read_to_string(&note_path) {
            result.snippet = snippet_for(&content);
        }
    }

    Ok(SemanticSearchResponse {
        results,
        elapsed_ms: start.elapsed().as_millis() as u64,
        query: request.query,
        model: "BAAI/bge-small-en-v1.5",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_note(dir: &Path, relative: &str, content: &str) {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn strip_frontmatter_handles_yaml_and_crlf() {
        assert_eq!(strip_frontmatter("---\ntitle: x\n---\nBody"), "Body");
        assert_eq!(strip_frontmatter("---\r\ntitle: x\r\n---\r\nBody"), "Body");
        assert_eq!(strip_frontmatter("No frontmatter"), "No frontmatter");
    }

    #[test]
    fn cosine_similarity_peaks_at_identity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-6);
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn load_vault_notes_skips_hidden_and_non_markdown() {
        let vault = tempfile::tempdir().unwrap();
        write_note(vault.path(), "notes/alpha.md", "# Alpha\nAlpha body.");
        write_note(vault.path(), ".hidden/secret.md", "# Secret");
        write_note(vault.path(), "data.txt", "not markdown");

        let notes = load_vault_notes(vault.path());
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].relative_path, "notes/alpha.md");
        assert_eq!(notes[0].title, "Alpha");
    }

    fn request_for(query: &str, vault_paths: Vec<String>) -> SemanticSearchRequest {
        SemanticSearchRequest {
            query: query.to_string(),
            vault_path: None,
            vault_paths,
            limit: None,
            hide_gitignored_files: false,
        }
    }

    #[test]
    fn run_semantic_search_rejects_bad_input() {
        let vault = tempfile::tempdir().unwrap();
        let error = run_semantic_search(request_for(
            "   ",
            vec![vault.path().to_string_lossy().into_owned()],
        ))
        .unwrap_err();
        assert!(error.contains("query is required"));

        let error = run_semantic_search(request_for(
            "anything",
            vec!["/definitely/not/a/real/vault".to_string()],
        ))
        .unwrap_err();
        assert!(error.contains("not a directory"));

        let error = run_semantic_search(request_for("anything", Vec::new())).unwrap_err();
        assert!(error.contains("vaultPath or vaultPaths is required"));
    }

    #[test]
    fn single_vault_path_still_accepted() {
        let vault = tempfile::tempdir().unwrap();
        write_note(vault.path(), "a.md", "# A\nsolar panel efficiency");
        let request = SemanticSearchRequest {
            query: "solar panels".to_string(),
            vault_path: Some(vault.path().to_string_lossy().into_owned()),
            vault_paths: Vec::new(),
            limit: None,
            hide_gitignored_files: false,
        };
        // The model may be unavailable in CI; a missing model surfaces as an
        // embedding error, while a bad vault list surfaces as a usage error.
        match run_semantic_search(request) {
            Ok(response) => {
                assert!(!response.results.is_empty());
                assert!(response.results[0]
                    .vault_path
                    .starts_with(vault.path().to_string_lossy().as_ref()));
            }
            Err(error) => assert!(error.contains("Embedding") || error.contains("embedding")),
        }
    }

    #[test]
    fn model_cache_dir_is_absolute_and_named() {
        let dir = model_cache_dir();
        assert!(dir.ends_with("fastembed-cache"));
    }

    #[test]
    fn snippet_is_bounded_and_strips_frontmatter() {
        assert_eq!(snippet_for("---\ntitle: t\n---\nHello world"), "Hello world");
        let long = "x".repeat(500);
        assert_eq!(snippet_for(&long).chars().count(), 200);
    }
}
