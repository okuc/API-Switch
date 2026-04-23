use super::circuit_breaker::CircuitBreaker;
use crate::database::ApiEntry;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Resolve which entries to try for a given model request.
/// Returns an ordered list of entries to attempt.
pub async fn resolve(
    model: &str,
    entries: &[ApiEntry],
    circuit_breakers: &RwLock<HashMap<String, CircuitBreaker>>,
) -> Vec<ApiEntry> {
    let breakers = circuit_breakers.read().await;

    // Filter out circuit-open entries
    let available: Vec<&ApiEntry> = entries
        .iter()
        .filter(|e| {
            if let Some(cb) = breakers.get(&e.id) {
                cb.is_available()
            } else {
                true
            }
        })
        .collect();

    if model == "auto" {
        // Return all available entries sorted by sort_index (priority)
        return available.into_iter().cloned().collect();
    }

    // Find entries matching the requested model
    let matched: Vec<ApiEntry> = available
        .iter()
        .filter(|e| e.model == model)
        .map(|e| (*e).clone())
        .collect();

    if matched.is_empty() {
        // Fallback: use all available entries (auto behavior)
        available.into_iter().cloned().collect()
    } else {
        matched
    }
}
