use crate::database::{Channel, ModelInfo};
use crate::error::AppError;
use crate::proxy::protocol::get_adapter;
use crate::AppState;
use crate::TRAY_ID;
use crate::build_tray_menu;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

#[derive(Clone)]
struct ProbeSuccess {
    models: Vec<ModelInfo>,
    corrected_base_url: String,
}

#[derive(Clone)]
struct EndpointGuess {
    detected_type: String,
    corrected_base_url: String,
}

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
pub fn update_channel(app: tauri::AppHandle, state: State<'_, AppState>, params: UpdateChannelParams) -> Result<Channel, AppError> {
    // If disabling, also disable all associated API pool entries
    if let Some(false) = params.enabled {
        state.db.disable_entries_for_channel(&params.id)?;
    }

    state.db.update_channel(
        &params.id,
        params.name.as_deref(),
        params.api_type.as_deref(),
        params.base_url.as_deref(),
        params.api_key.as_deref(),
        params.enabled,
        params.notes.as_deref(),
    )?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    state.db.get_channel(&params.id)
}

#[tauri::command]
pub fn delete_channel(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.db.delete_channel(&id)?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct ProbeResult {
    pub reachable: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
    pub detected_type: Option<String>,
    pub message: String,
}

#[tauri::command]
pub async fn probe_url(url: String) -> Result<ProbeResult, AppError> {
    let url = url.trim_end_matches('/').trim();
    if url.is_empty() {
        return Ok(ProbeResult { reachable: false, status_code: None, latency_ms: 0,
            detected_type: None, message: "Empty URL".into() });
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(format!("HTTP client: {e}")))?;

    let start = std::time::Instant::now();
    match client.head(url).send().await {
        Ok(r) => {
            let s = r.status().as_u16();
            let ms = start.elapsed().as_millis() as u64;
            Ok(ProbeResult { reachable: s < 500, status_code: Some(s), latency_ms: ms,
                detected_type: None, message: format!("{s} ({ms}ms)") })
        }
        Err(_) => {
            let _start2 = std::time::Instant::now();
            match client.get(url).send().await {
                Ok(r) => {
                    let s = r.status().as_u16();
                    let ms = start.elapsed().as_millis() as u64;
                    Ok(ProbeResult { reachable: s < 500, status_code: Some(s), latency_ms: ms,
                        detected_type: None, message: format!("{s} ({ms}ms)") })
                }
                Err(e) => {
                    let ms = start.elapsed().as_millis() as u64;
                    Ok(ProbeResult { reachable: false, status_code: None, latency_ms: ms,
                        detected_type: None, message: e.to_string() })
                }
            }
        }
    }
}

/// Smart model fetch: tries preferred type first, then auto-detects.
/// Returns detected type and models in one call.
#[derive(Serialize)]
pub struct FetchModelsResult {
    pub detected_type: String,
    pub corrected_base_url: String,
    pub models: Vec<ModelInfo>,
    pub message: String,
}

#[tauri::command]
pub async fn fetch_models_direct(
    _state: State<'_, AppState>,
    api_type: String,
    base_url: String,
    api_key: String,
    verified: Option<bool>,
) -> Result<FetchModelsResult, AppError> {
    let base_url = normalize_base_url(&base_url);
    if base_url.is_empty() {
        return Err(AppError::Network("Empty URL".into()));
    }

    smart_fetch_models(&api_type, &base_url, &api_key, verified.unwrap_or(false)).await
}

#[tauri::command]
pub async fn fetch_models(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<Vec<ModelInfo>, AppError> {
    let channel = state.db.get_channel(&channel_id)?;

    let result = smart_fetch_models(
        &channel.api_type,
        &channel.base_url,
        &channel.api_key,
        false,
    ).await?;

    if channel.api_type != result.detected_type || normalize_base_url(&channel.base_url) != result.corrected_base_url {
        state.db.update_channel_endpoint(&channel_id, &result.detected_type, &result.corrected_base_url)?;
    }

    state.db.update_channel_models(&channel_id, &result.models, &channel.selected_models)?;

    Ok(result.models)
}

async fn smart_fetch_models(
    api_type: &str,
    base_url: &str,
    api_key: &str,
    verified: bool,
) -> Result<FetchModelsResult, AppError> {
    let base_url = normalize_base_url(base_url);

    let endpoint_guess = if verified {
        Some(EndpointGuess {
            detected_type: api_type.to_string(),
            corrected_base_url: base_url.clone(),
        })
    } else {
        detect_endpoint_guess(api_type, &base_url, api_key).await
    };

    let fetch_seed_type = endpoint_guess
        .as_ref()
        .map(|g| g.detected_type.as_str())
        .unwrap_or(api_type);
    let fetch_seed_base_url = endpoint_guess
        .as_ref()
        .map(|g| g.corrected_base_url.as_str())
        .unwrap_or(base_url.as_str());

    let (models, actual_type, actual_base_url) = fetch_models_with_fallback(fetch_seed_type, fetch_seed_base_url, api_key).await?;

    let corrected_type = endpoint_guess
        .as_ref()
        .map(|g| g.detected_type.clone())
        .unwrap_or_else(|| resolve_detected_type(actual_type, &actual_base_url));
    let corrected_base_url = endpoint_guess
        .as_ref()
        .map(|g| g.corrected_base_url.clone())
        .unwrap_or(actual_base_url);
    let count = models.len();

    Ok(FetchModelsResult {
        message: format!("Detected: {corrected_type} ({count} models)"),
        detected_type: corrected_type,
        corrected_base_url,
        models,
    })
}

async fn detect_endpoint_guess(
    api_type: &str,
    base_url: &str,
    api_key: &str,
) -> Option<EndpointGuess> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;

    let candidates = build_base_url_candidates(&base_url);
    let try_types = build_try_types(api_type);

    for candidate_base_url in candidates {
        for current_type in &try_types {
            let adapter = get_adapter(current_type);
            let urls = build_models_url_variants(adapter.as_ref(), &candidate_base_url, api_key);
            for url in &urls {
                match try_models_endpoint(&client, adapter.as_ref(), url, api_key).await {
                    Ok(models) if !models.is_empty() => {
                        if !is_authoritative_detection_success(current_type, url) {
                            continue;
                        }
                        let corrected_base_url = canonical_base_url_for_success(current_type, &candidate_base_url, url);
                        let detected = resolve_detected_type(current_type, &corrected_base_url);
                        log::info!("[detect_endpoint] OK via {url}, type={detected}, base_url={corrected_base_url}");
                        return Some(EndpointGuess {
                            detected_type: detected,
                            corrected_base_url,
                        });
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            if let Some(probe) = try_chat_probe(&client, adapter.as_ref(), &candidate_base_url, api_key, current_type).await {
                let probe_url = adapter.build_chat_url(
                    &candidate_base_url,
                    match *current_type {
                        "claude" => "claude-3-5-sonnet-20241022",
                        "gemini" => "gemini-2.0-flash",
                        _ => "gpt-4o-mini",
                    },
                );
                if !is_authoritative_detection_success(current_type, &probe_url) {
                    continue;
                }
                let detected = resolve_detected_type(current_type, &probe.corrected_base_url);
                log::info!("[detect_endpoint] Chat probe OK, type={detected}, base_url={}", probe.corrected_base_url);
                return Some(EndpointGuess {
                    detected_type: detected,
                    corrected_base_url: probe.corrected_base_url,
                });
            }
        }
    }

    None
}

async fn fetch_models_with_fallback(
    preferred_type: &str,
    preferred_base_url: &str,
    api_key: &str,
) -> Result<(Vec<ModelInfo>, &'static str, String), AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(format!("HTTP client: {e}")))?;

    let candidates = build_base_url_candidates(preferred_base_url);
    let try_types = build_try_types(preferred_type);
    let mut last_err = String::new();

    for candidate_base_url in candidates {
        for current_type in &try_types {
            let adapter = get_adapter(current_type);
            let urls = build_models_url_variants(adapter.as_ref(), &candidate_base_url, api_key);
            for url in &urls {
                match try_models_endpoint(&client, adapter.as_ref(), url, api_key).await {
                    Ok(models) if !models.is_empty() => {
                        let corrected_base_url = canonical_base_url_for_success(current_type, &candidate_base_url, url);
                        let models = dedup_models(models);
                        log::info!("[fetch_models] OK via {url}, type={current_type}, base_url={} ({} models)", corrected_base_url, models.len());
                        return Ok((models, current_type, corrected_base_url));
                    }
                    Ok(_) => {}
                    Err(e) => { last_err = e; }
                }
            }

            if let Some(probe) = try_chat_probe(&client, adapter.as_ref(), &candidate_base_url, api_key, current_type).await {
                let models = dedup_models(probe.models);
                log::info!("[fetch_models] Chat probe OK, type={current_type}, base_url={} ({} models)", probe.corrected_base_url, models.len());
                return Ok((models, current_type, probe.corrected_base_url));
            }
        }
    }

    Err(AppError::Network(format!(
        "Could not fetch models. Check URL and API Key. Last: {last_err}"
    )))
}

fn build_try_types(preferred_type: &str) -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    let normalized: &'static str = match preferred_type {
        "openai" => "openai",
        "gemini" => "gemini",
        "claude" => "claude",
        "azure" => "azure",
        "custom" => "custom",
        _ => "custom",
    };
    let mut v = Vec::new();
    for t in [normalized, "openai", "custom", "gemini", "claude", "azure"] {
        let actual = if t == "openai" && normalized == "custom" { "custom" } else { t };
        if seen.insert(actual) {
            v.push(actual);
        }
    }
    v
}

fn is_authoritative_detection_success(api_type: &str, success_url: &str) -> bool {
    match api_type {
        // Prevent generic OpenAI-compatible gateways from being misclassified as Gemini.
        "gemini" => success_url.contains("/v1beta/openai/"),
        // Azure must really hit the deployments API.
        "azure" => success_url.contains("/openai/deployments"),
        _ => true,
    }
}

fn normalize_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.is_empty() {
        String::new()
    } else {
        format!("https://{trimmed}")
    }
}

fn build_base_url_candidates(base_url: &str) -> Vec<String> {
    let normalized = normalize_base_url(base_url);
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for candidate in [
        normalized.clone(),
        trim_known_api_suffix(&normalized),
    ] {
        if !candidate.is_empty() && seen.insert(candidate.clone()) {
            candidates.push(candidate);
        }
    }

    candidates
}

fn canonical_base_url_for_success(api_type: &str, fallback_base_url: &str, success_url: &str) -> String {
    let success = success_url.trim();

    if api_type == "gemini" {
        if let Some((base, _)) = success.split_once("/v1beta/openai/") {
            return base.trim_end_matches('/').to_string();
        }
    }

    if api_type == "claude" {
        if let Some((base, _)) = success.split_once("/v1/") {
            return base.trim_end_matches('/').to_string();
        }
    }

    if api_type == "azure" {
        if let Some((base, _)) = success.split_once("/openai/deployments") {
            return base.trim_end_matches('/').to_string();
        }
    }

    if api_type == "openai" || api_type == "custom" {
        if let Some(stripped) = success.strip_suffix("/models") {
            return stripped.trim_end_matches('/').to_string();
        }
        if let Some(stripped) = success.strip_suffix("/chat/completions") {
            return stripped.trim_end_matches('/').to_string();
        }
    }

    normalize_base_url(fallback_base_url)
}

fn trim_known_api_suffix(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let suffixes = [
        "/v1/chat/completions",
        "/chat/completions",
        "/v1/messages",
        "/v1/models",
        "/models",
        "/v1beta/openai/chat/completions",
        "/v1beta/openai/models",
        "/openai/deployments",
    ];
    for suffix in suffixes {
        if let Some(stripped) = base.strip_suffix(suffix) {
            return stripped.trim_end_matches('/').to_string();
        }
    }
    base.to_string()
}

fn resolve_detected_type(detected: &str, base_url: &str) -> String {
    if detected == "openai" && !base_url.contains("api.openai.com") {
        "custom".into()
    } else {
        detected.into()
    }
}

#[tauri::command]
pub fn select_models(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    channel_id: String,
    model_names: Vec<String>,
) -> Result<(), AppError> {
    let channel = state.db.get_channel(&channel_id)?;
    state.db.update_channel_models(&channel_id, &channel.available_models, &model_names)?;
    state.db.sync_entries_for_channel(&channel_id, &model_names)?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(())
}

/// Generate model list URL candidates: adapter standard + common variants
fn build_models_url_variants(
    adapter: &(dyn crate::proxy::protocol::ProtocolAdapter + Send + Sync),
    base_url: &str,
    api_key: &str,
) -> Vec<String> {
    let mut urls = vec![adapter.build_models_url(base_url, api_key)];
    let base = base_url.trim_end_matches('/');
    for v in &["/models", "/api/models", "/api/v1/models", "/v2/models"] {
        let u = format!("{base}{v}");
        if !urls.contains(&u) { urls.push(u); }
    }
    urls
}

/// Try a single models endpoint, return parsed models or error string
async fn try_models_endpoint(
    client: &reqwest::Client,
    adapter: &(dyn crate::proxy::protocol::ProtocolAdapter + Send + Sync),
    url: &str,
    api_key: &str,
) -> Result<Vec<ModelInfo>, String> {
    let resp = adapter.apply_auth(client.get(url), api_key)
        .send().await.map_err(|e| format!("{e}"))?;
    let status = resp.status();
    if !status.is_success() {
        // Try to extract model list from error body (some APIs include it)
        let body = resp.text().await.unwrap_or_default();
        if let Some(m) = extract_models_from_json(&body) {
            return Ok(m);
        }
        return Err(format!("HTTP {status}"));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("{e}"))?;
    Ok(adapter.parse_models_response(&body).into_iter()
        .map(|(id, owned_by)| ModelInfo { name: id.clone(), id, owned_by })
        .collect())
}

/// Try to extract model list from a JSON body (even error responses)
fn extract_models_from_json(body: &str) -> Option<Vec<ModelInfo>> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let arr = json.get("data")?.as_array()?;
    let models: Vec<ModelInfo> = arr.iter()
        .filter_map(|m| {
            let id = m.get("id")?.as_str()?.to_string();
            if id.eq_ignore_ascii_case("auto") {
                return None;
            }
            let owned = m.get("owned_by").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            Some(ModelInfo { name: id.clone(), id, owned_by: Some(owned) })
        })
        .collect();
    if models.is_empty() { None } else { Some(models) }
}

/// Chat probe: send a tiny request to verify the API works
async fn try_chat_probe(
    client: &reqwest::Client,
    adapter: &(dyn crate::proxy::protocol::ProtocolAdapter + Send + Sync),
    base_url: &str,
    api_key: &str,
    api_type: &str,
) -> Option<ProbeSuccess> {
    let test_model = match api_type {
        "claude" => "claude-3-5-sonnet-20241022",
        "gemini" => "gemini-2.0-flash",
        _ => "gpt-4o-mini",
    };
    let chat_url = adapter.build_chat_url(base_url, test_model);
    let body = serde_json::json!({"model": test_model, "messages": [{"role":"user","content":"hi"}], "max_tokens": 1});
    let req = adapter.apply_auth(
        client.post(&chat_url).header("Content-Type", "application/json"), api_key);
    match req.json(&body).send().await {
        Ok(resp) => {
            let s = resp.status().as_u16();
            if s < 500 {
                let corrected_base_url = canonical_base_url_for_success(api_type, base_url, &chat_url);
                // Server responded → API works, return known models
                if let Ok(text) = resp.text().await {
                    if let Some(m) = extract_models_from_json(&text) {
                        return Some(ProbeSuccess { models: m, corrected_base_url });
                    }
                }
                return Some(ProbeSuccess {
                    models: known_models_for_type(api_type),
                    corrected_base_url,
                });
            }
            None
        }
        Err(_) => None,
    }
}

/// Commonly known models per API type
fn known_models_for_type(api_type: &str) -> Vec<ModelInfo> {
    let list: &[(&str, &str)] = match api_type {
        "openai" => &[
            ("gpt-4o","openai"),("gpt-4o-mini","openai"),("gpt-4-turbo","openai"),
            ("gpt-3.5-turbo","openai"),("o1","openai"),("o1-mini","openai"),
            ("o1-preview","openai"),("o3-mini","openai"),("o4-mini","openai"),
        ],
        "claude" => &[
            ("claude-sonnet-4-20250514","anthropic"),("claude-3-5-sonnet-20241022","anthropic"),
            ("claude-3-5-haiku-20241022","anthropic"),("claude-3-opus-20240229","anthropic"),
        ],
        "gemini" => &[
            ("gemini-2.5-pro-preview-05-06","google"),("gemini-2.0-flash","google"),
            ("gemini-1.5-pro","google"),("gemini-1.5-flash","google"),
        ],
        "azure" => &[
            ("gpt-4o","azure"),("gpt-4o-mini","azure"),("gpt-4-turbo","azure"),
        ],
        _ => &[
            ("gpt-4o","openai"),("gpt-4o-mini","openai"),("gpt-3.5-turbo","openai"),
            ("claude-3-5-sonnet-20241022","anthropic"),("claude-3-5-haiku-20241022","anthropic"),
            ("gemini-2.0-flash","google"),("deepseek-chat","deepseek"),("deepseek-reasoner","deepseek"),
            ("qwen-turbo","alibaba"),("glm-4-flash","zhipu"),
        ],
    };
    list.iter().map(|&(name, owner)| ModelInfo {
        name: name.into(), id: name.into(), owned_by: Some(owner.into()),
    }).collect()
}

fn dedup_models(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    models
        .into_iter()
        .filter(|m| !m.id.eq_ignore_ascii_case("auto") && !m.name.eq_ignore_ascii_case("auto"))
        .filter(|m| seen.insert(m.name.clone()))
        .collect()
}

