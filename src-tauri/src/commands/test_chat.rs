use crate::error::AppError;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::proxy::ProxyState;
use axum::http::HeaderMap;
use serde_json::json;

#[derive(Debug, Serialize)]
pub struct TestChatResponse {
    pub content: String,
    /// Total request latency in milliseconds
    pub latency_ms: u64,
    /// Token usage from the response
    pub usage: Option<TestChatUsage>,
}

#[derive(Debug, Serialize)]
pub struct TestChatUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestChatMessage {
    pub role: String,
    pub content: String,
}

#[tauri::command]
pub async fn test_chat(
    state: State<'_, AppState>,
    entry_id: String,
    messages: Vec<TestChatMessage>,
) -> Result<TestChatResponse, AppError> {
    let db = state.db.clone();

    // Get all enabled entries
    let entries = db.get_enabled_entries_for_routing()?;
    let entry = entries
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| AppError::NotFound(format!("Entry {entry_id} not found")))?
        .clone();

    // Build the chat body (non-streaming)
    let body = json!({
        "model": entry.model,
        "messages": messages,
        "stream": false,
    });

    // Create ProxyState
    let proxy_state = ProxyState {
        db,
        circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
    };

    let headers = HeaderMap::new();

    // Resolve and forward
    let resolved = crate::proxy::resolve(&entry.model, &entries, &proxy_state.circuit_breakers).await;
    if resolved.is_empty() {
        return Err(AppError::Validation(format!(
            "No available provider for model: {}",
            entry.model
        )));
    }

    let start = Instant::now();

    let response = crate::proxy::forward_with_retry(
        &proxy_state,
        &resolved,
        &body,
        &headers,
        &entry.model,
        None,
        false,
    )
    .await
    .map_err(|e| AppError::Proxy(e.to_string()))?;

    let latency_ms = start.elapsed().as_millis() as u64;

    // Extract the text from the response body
    let bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read response body: {e}")))?;

    let json_body: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Internal(format!("Failed to parse response: {e}")))?;

    // Extract content from OpenAI format response
    let content = json_body
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    // Extract usage
    let usage = json_body.get("usage").map(|u| TestChatUsage {
        prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        completion_tokens: u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        total_tokens: u.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
    });

    Ok(TestChatResponse { content, latency_ms, usage })
}
