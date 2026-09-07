use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

const MAX_CACHED_CLIPBOARD_ENTRIES: usize = 20;
const MAX_TEXT_PREVIEW_CHARS: usize = 200;

/// Global in-memory clipboard cache. Populated whenever the app writes to the
/// system clipboard (`copy_text_to_clipboard`, `copy_image_to_vault`) and
/// refreshed with the current system clipboard contents on read. A global
/// (rather than managed Tauri state) keeps the cache reachable from command
/// helpers that do not hold an `AppHandle`.
pub(crate) struct ClipboardCache(pub(crate) Mutex<VecDeque<ClipboardEntry>>);

impl Default for ClipboardCache {
    fn default() -> Self {
        Self(Mutex::new(VecDeque::new()))
    }
}

fn global_cache() -> &'static ClipboardCache {
    static CACHE: OnceLock<ClipboardCache> = OnceLock::new();
    CACHE.get_or_init(ClipboardCache::default)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ClipboardEntry {
    pub id: String,
    pub kind: ClipboardEntryKind,
    pub preview: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardEntryKind {
    Text,
    Image,
}

fn new_entry(kind: ClipboardEntryKind, preview: String) -> ClipboardEntry {
    ClipboardEntry {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        preview: truncate_preview(&preview),
        created_at: Utc::now(),
    }
}

fn truncate_preview(preview: &str) -> String {
    let single_line = preview.replace('\n', " ").replace('\r', " ");
    if single_line.chars().count() <= MAX_TEXT_PREVIEW_CHARS {
        return single_line;
    }
    let mut truncated: String = single_line.chars().take(MAX_TEXT_PREVIEW_CHARS).collect();
    truncated.push('…');
    truncated
}

pub(crate) fn cache_clipboard_text(text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Ok(entries) = global_cache().0.lock() {
        if entries
            .front()
            .is_some_and(|entry| entry.kind == ClipboardEntryKind::Text && entry.preview == truncate_preview(trimmed))
        {
            return;
        }
    }
    push_entry(ClipboardEntryKind::Text, trimmed.to_string());
}

pub(crate) fn cache_clipboard_image(source_path: &str) {
    let filename = source_path.rsplit('/').next().unwrap_or(source_path);
    push_entry(ClipboardEntryKind::Image, filename.to_string());
}

fn push_entry(kind: ClipboardEntryKind, preview: String) {
    if let Ok(mut entries) = global_cache().0.lock() {
        entries.push_front(new_entry(kind, preview));
        while entries.len() > MAX_CACHED_CLIPBOARD_ENTRIES {
            entries.pop_back();
        }
    }
}

pub(crate) fn recent_entries(count: usize) -> Vec<ClipboardEntry> {
    let count = count.clamp(0, MAX_CACHED_CLIPBOARD_ENTRIES);
    global_cache()
        .0
        .lock()
        .map(|entries| entries.iter().take(count).cloned().collect())
        .unwrap_or_default()
}

pub(crate) fn entry_by_id(id: &str) -> Option<ClipboardEntry> {
    global_cache()
        .0
        .lock()
        .ok()
        .and_then(|entries| entries.iter().find(|entry| entry.id == id).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The cache is a process-global, so tests must run serially to avoid
    // stepping on each other's entries.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_cache() {
        if let Ok(mut entries) = global_cache().0.lock() {
            entries.clear();
        }
    }

    #[test]
    fn text_entries_are_cached_most_recent_first() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_cache();
        cache_clipboard_text("first");
        cache_clipboard_text("second");

        let entries = recent_entries(10);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].preview, "second");
        assert_eq!(entries[1].preview, "first");
        assert_eq!(entries[0].kind, ClipboardEntryKind::Text);
    }

    #[test]
    fn duplicate_consecutive_text_is_deduplicated() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_cache();
        cache_clipboard_text("same");
        cache_clipboard_text("same");
        assert_eq!(recent_entries(10).len(), 1);
    }

    #[test]
    fn empty_text_is_ignored() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_cache();
        cache_clipboard_text("   ");
        assert!(recent_entries(10).is_empty());
    }

    #[test]
    fn cache_is_bounded() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_cache();
        for index in 0..50 {
            cache_clipboard_text(&format!("entry-{index}"));
        }
        assert_eq!(recent_entries(100).len(), MAX_CACHED_CLIPBOARD_ENTRIES);
    }

    #[test]
    fn preview_is_truncated_and_single_line() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_cache();
        let long = format!("{}\ntail", "x".repeat(500));
        cache_clipboard_text(&long);

        let entry = recent_entries(1).pop().unwrap();
        assert!(!entry.preview.contains('\n'));
        assert!(entry.preview.ends_with('…'));
        assert!(entry.preview.chars().count() <= MAX_TEXT_PREVIEW_CHARS + 1);
    }

    #[test]
    fn image_entries_use_filename_preview() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_cache();
        cache_clipboard_image("/Users/luca/vault/attachments/photo.png");

        let entry = recent_entries(1).pop().unwrap();
        assert_eq!(entry.kind, ClipboardEntryKind::Image);
        assert_eq!(entry.preview, "photo.png");
    }

    #[test]
    fn restore_lookup_finds_entry_by_id() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_cache();
        cache_clipboard_text("restore me");
        let entry = recent_entries(1).pop().unwrap();

        let found = entry_by_id(&entry.id).unwrap();
        assert_eq!(found.preview, "restore me");
        assert!(entry_by_id("missing-id").is_none());
    }
}