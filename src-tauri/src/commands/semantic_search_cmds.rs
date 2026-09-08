use crate::semantic_search::{run_semantic_search, SemanticSearchRequest, SemanticSearchResponse};

/// Tauri command: fastembed-backed semantic search over a vault (RAG mode).
///
/// The MCP `search_notes_semantic` tool reaches this via the frontend relay
/// (ws-bridge tool_request → useAiActivity → `invoke('search_notes_semantic')`).
/// The blocking embedding work runs on a dedicated thread so the async
/// runtime is not stalled.
#[tauri::command]
pub async fn search_notes_semantic(
    request: SemanticSearchRequest,
) -> Result<SemanticSearchResponse, String> {
    tauri::async_runtime::spawn_blocking(move || run_semantic_search(request))
        .await
        .map_err(|error| format!("Semantic search task failed: {error}"))?
}
