use crate::database::{Channel, ModelInfo};
use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
pub struct CreateChannelParams {
    pub name: String,
    pub api_type: String,
    pub base_url: String,
    pub api_key: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateChannelParams {
    pub id: String,
    pub name: Option<String>,
    pub api_type: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn list_channels(state: State<'_, AppState>) -> Result<Vec<Channel>, AppError> {
    state.db.list_channels()
}

#[tauri::command]
pub fn create_channel(state: State<'_, AppState>, params: CreateChannelParams) -> Result<Channel, AppError> {
    state.db.create_channel(
        &params.name,
        &params.api_type,
        &params.base_url,
        &params.api_key,
        params.notes.as_deref(),
    )
}

#[tauri::command]
pub fn update_channel(state: State<'_, AppState>, params: UpdateChannelParams) -> Result<Channel, AppError> {
    state.db.update_channel(
        &params.id,
        params.name.as_deref(),
        params.api_type.as_deref(),
        params.base_url.as_deref(),
        params.api_key.as_deref(),
        params.enabled,
        params.notes.as_deref(),
    )?;
    state.db.get_channel(&params.id)
}

#[tauri::command]
pub fn delete_channel(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.db.delete_channel(&id)
}

#[tauri::command]
pub async fn fetch_models(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<Vec<ModelInfo>, AppError> {
    let channel = state.db.get_channel(&channel_id)?;

    // Fetch models from upstream API
    let models = fetch_models_from_api(&channel.api_type, &channel.base_url, &channel.api_key).await?;

    // Update available_models in DB
    state.db.update_channel_models(&channel_id, &models, &channel.selected_models)?;

    Ok(models)
}

#[tauri::command]
pub fn select_models(
    state: State<'_, AppState>,
    channel_id: String,
    model_names: Vec<String>,
) -> Result<(), AppError> {
    let channel = state.db.get_channel(&channel_id)?;
    state.db.update_channel_models(&channel_id, &channel.available_models, &model_names)?;
    state.db.sync_entries_for_channel(&channel_id, &model_names)?;
    Ok(())
}

async fn fetch_models_from_api(
    api_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<ModelInfo>, AppError> {
    let url = match api_type {
        "openai" | "custom" => format!("{}/v1/models", base_url.trim_end_matches('/')),
        "claude" => format!("{}/v1/models", base_url.trim_end_matches('/')),
        "gemini" => format!(
            "{}/v1beta/models?key={}",
            base_url.trim_end_matches('/'),
            api_key
        ),
        "azure" => format!(
            "{}/openai/deployments?api-version=2024-02-01",
            base_url.trim_end_matches('/')
        ),
        _ => return Err(AppError::Validation(format!("Unknown api_type: {api_type}"))),
    };

    let client = reqwest::Client::new();
    let mut request = client.get(&url);

    if api_type != "gemini" {
        request = request.bearer_auth(api_key);
    }

    let response = request
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Failed to fetch models: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!(
            "Failed to fetch models: {status} - {body}"
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Network(format!("Failed to parse models response: {e}")))?;

    // Parse models from different API formats
    let models = match api_type {
        "openai" | "custom" | "azure" => {
            // OpenAI format: { data: [{ id, owned_by }] }
            body.get("data")
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| {
                            let id = m.get("id")?.as_str()?.to_string();
                            Some(ModelInfo {
                                name: id.clone(),
                                id,
                                owned_by: m.get("owned_by").and_then(|v| v.as_str()).map(String::from),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "claude" => {
            // Anthropic format: { data: [{ id, display_name }] }
            body.get("data")
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| {
                            let id = m.get("id")?.as_str()?.to_string();
                            Some(ModelInfo {
                                name: id.clone(),
                                id,
                                owned_by: m.get("owned_by").and_then(|v| v.as_str()).map(String::from),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "gemini" => {
            // Gemini format: { models: [{ name, displayName }] }
            body.get("models")
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| {
                            let name = m.get("name")?.as_str()?.to_string();
                            // Gemini names are like "models/gemini-pro"
                            let id = name.strip_prefix("models/").unwrap_or(&name).to_string();
                            Some(ModelInfo {
                                name: id.clone(),
                                id,
                                owned_by: m.get("owned_by").and_then(|v| v.as_str()).map(String::from),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        _ => vec![],
    };

    // Deduplicate by name
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<ModelInfo> = models
        .into_iter()
        .filter(|m| seen.insert(m.name.clone()))
        .collect();

    Ok(deduped)
}
