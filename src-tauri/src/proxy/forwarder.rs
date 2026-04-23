use super::circuit_breaker::CircuitBreaker;
use super::handlers::ProxyError;
use super::server::ProxyState;
use crate::database::{AccessKey, ApiEntry};
use axum::body::Body;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use bytes::Bytes;
use futures::Stream;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::task::Poll;
use std::time::Instant;

struct ForwardResult {
    response: axum::response::Response,
    prompt_tokens: i64,
    completion_tokens: i64,
    first_token_ms: i64,
    status_code: i32,
}

/// Forward a request to the resolved entries with retry/failover.
pub async fn forward_with_retry(
    state: &ProxyState,
    entries: &[ApiEntry],
    body: &Value,
    _original_headers: &HeaderMap,
    requested_model: &str,
    access_key: Option<&AccessKey>,
    is_stream: bool,
) -> Result<axum::response::Response, ProxyError> {
    let mut last_error = None;

    for entry in entries {
        let start = Instant::now();

        // Check circuit breaker
        {
            let breakers = state.circuit_breakers.read().await;
            if let Some(cb) = breakers.get(&entry.id) {
                if !cb.is_available() {
                    continue;
                }
            }
        }

        match forward_single(state, entry, body, requested_model, access_key, is_stream).await {
            Ok(result) => {
                let elapsed = start.elapsed();

                // Record success
                record_circuit_success(state, &entry.id).await;

                if !is_stream {
                    let latency_ms = elapsed.as_millis() as i64;
                    log_usage(
                        &state.db,
                        access_key,
                        entry,
                        requested_model,
                        is_stream,
                        result.prompt_tokens,
                        result.completion_tokens,
                        result.first_token_ms,
                        latency_ms,
                        result.status_code,
                        true,
                        None,
                    );
                }

                return Ok(result.response);
            }
            Err(e) => {
                let elapsed = start.elapsed();
                let latency_ms = elapsed.as_millis() as i64;

                // Record failure
                record_circuit_failure(state, &entry.id).await;

                // Write usage log
                log_usage(
                    &state.db,
                    access_key,
                    entry,
                    requested_model,
                    is_stream,
                    0,
                    0,
                    0,
                    latency_ms,
                    502,
                    false,
                    Some(&e.to_string()),
                );

                last_error = Some(e);
                continue;
            }
        }
    }

    Err(last_error.unwrap_or(ProxyError::AllProvidersFailed))
}

async fn forward_single(
    state: &ProxyState,
    entry: &ApiEntry,
    body: &Value,
    requested_model: &str,
    access_key: Option<&AccessKey>,
    is_stream: bool,
) -> Result<ForwardResult, ProxyError> {
    // Get channel info from DB
    let channel = state
        .db
        .get_channel(&entry.channel_id)
        .map_err(|e| ProxyError::Internal(e.to_string()))?;

    let base_url = channel.base_url.trim_end_matches('/');
    let url = format!("{}/v1/chat/completions", base_url);
    let upstream_body = build_upstream_body(body, &entry.model);

    // Build the request
    let client = reqwest::Client::new();
    let mut request = client
        .post(&url)
        .bearer_auth(&channel.api_key)
        .json(&upstream_body);

    if is_stream {
        request = request.header("Accept", "text/event-stream");
    }

    let response = request
        .send()
        .await
        .map_err(|e| ProxyError::Internal(format!("Request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let error_body = response.text().await.unwrap_or_default();
        return Err(ProxyError::Internal(format!(
            "Upstream error {status}: {error_body}"
        )));
    }

    if is_stream {
        let status_code = response.status().as_u16() as i32;
        let response = build_streaming_response(
            state,
            entry,
            access_key,
            requested_model,
            response,
            status_code,
        );

        return Ok(ForwardResult {
            response,
            prompt_tokens: 0,
            completion_tokens: 0,
            first_token_ms: 0,
            status_code,
        });
    } else {
        let status_code = response.status().as_u16() as i32;

        // Non-stream: forward the JSON response
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| ProxyError::Internal(format!("Failed to parse response: {e}")))?;
        let (prompt_tokens, completion_tokens) = extract_usage_tokens(&response_body);

        Ok(ForwardResult {
            response: axum::Json(response_body).into_response(),
            prompt_tokens,
            completion_tokens,
            first_token_ms: 0,
            status_code,
        })
    }
}

fn build_upstream_body(body: &Value, actual_model: &str) -> Value {
    let mut upstream_body = body.clone();
    if let Some(object) = upstream_body.as_object_mut() {
        object.insert("model".to_string(), Value::String(actual_model.to_string()));
    }
    upstream_body
}

fn extract_usage_tokens(body: &Value) -> (i64, i64) {
    let usage = body.get("usage");
    let prompt_tokens = usage
        .and_then(|v| v.get("prompt_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let completion_tokens = usage
        .and_then(|v| v.get("completion_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);

    (prompt_tokens, completion_tokens)
}

fn build_streaming_response(
    state: &ProxyState,
    entry: &ApiEntry,
    access_key: Option<&AccessKey>,
    requested_model: &str,
    response: reqwest::Response,
    status_code: i32,
) -> axum::response::Response {
    let start = Instant::now();
    let db = state.db.clone();
    let entry = entry.clone();
    let access_key = access_key.cloned();
    let requested_model = requested_model.to_string();
    let first_token_ms = Arc::new(AtomicI64::new(0));
    let prompt_tokens = Arc::new(AtomicI64::new(0));
    let completion_tokens = Arc::new(AtomicI64::new(0));
    let seen_first_chunk = Arc::new(AtomicBool::new(false));
    let logged = Arc::new(AtomicBool::new(false));
    let mut sse_buffer = String::new();
    let mut upstream_stream = Box::pin(response.bytes_stream());

    let body_stream = futures::stream::poll_fn(move |cx| -> Poll<Option<Result<Bytes, std::io::Error>>> {
        match upstream_stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                if !seen_first_chunk.swap(true, Ordering::Relaxed) {
                    first_token_ms.store(start.elapsed().as_millis() as i64, Ordering::Relaxed);
                }

                append_and_parse_sse(
                    &mut sse_buffer,
                    &chunk,
                    &prompt_tokens,
                    &completion_tokens,
                );

                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(err))) => {
                if !logged.swap(true, Ordering::Relaxed) {
                    log_usage(
                        &db,
                        access_key.as_ref(),
                        &entry,
                        &requested_model,
                        true,
                        prompt_tokens.load(Ordering::Relaxed),
                        completion_tokens.load(Ordering::Relaxed),
                        first_token_ms.load(Ordering::Relaxed),
                        start.elapsed().as_millis() as i64,
                        502,
                        false,
                        Some(&format!("Stream error: {err}")),
                    );
                }

                Poll::Ready(Some(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    err,
                ))))
            }
            Poll::Ready(None) => {
                if !logged.swap(true, Ordering::Relaxed) {
                    log_usage(
                        &db,
                        access_key.as_ref(),
                        &entry,
                        &requested_model,
                        true,
                        prompt_tokens.load(Ordering::Relaxed),
                        completion_tokens.load(Ordering::Relaxed),
                        first_token_ms.load(Ordering::Relaxed),
                        start.elapsed().as_millis() as i64,
                        status_code,
                        true,
                        None,
                    );
                }

                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    });

    axum::http::Response::builder()
        .status(axum::http::StatusCode::from_u16(status_code as u16).unwrap_or(axum::http::StatusCode::OK))
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .body(Body::from_stream(body_stream))
        .unwrap()
}

fn append_and_parse_sse(
    buffer: &mut String,
    chunk: &Bytes,
    prompt_tokens: &Arc<AtomicI64>,
    completion_tokens: &Arc<AtomicI64>,
) {
    buffer.push_str(&String::from_utf8_lossy(chunk));

    while let Some(line_end) = buffer.find('\n') {
        let mut line = buffer.drain(..=line_end).collect::<String>();
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }

        let Some(payload) = line.strip_prefix("data: ") else {
            continue;
        };

        if payload == "[DONE]" {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(payload) else {
            continue;
        };

        let (prompt, completion) = extract_usage_tokens(&value);
        if prompt > 0 {
            prompt_tokens.store(prompt, Ordering::Relaxed);
        }
        if completion > 0 {
            completion_tokens.store(completion, Ordering::Relaxed);
        }
    }
}

async fn record_circuit_success(state: &ProxyState, entry_id: &str) {
    let mut breakers = state.circuit_breakers.write().await;
    let cb = breakers
        .entry(entry_id.to_string())
        .or_insert_with(CircuitBreaker::new);
    cb.record_success();
}

async fn record_circuit_failure(state: &ProxyState, entry_id: &str) {
    let mut breakers = state.circuit_breakers.write().await;
    let settings = state.db.get_settings().ok();
    let threshold = settings
        .as_ref()
        .map(|s| s.circuit_failure_threshold as u32)
        .unwrap_or(4);

    let cb = breakers
        .entry(entry_id.to_string())
        .or_insert_with(CircuitBreaker::new);
    cb.record_failure(threshold);
}

fn log_usage(
    db: &crate::database::Database,
    access_key: Option<&AccessKey>,
    entry: &ApiEntry,
    requested_model: &str,
    is_stream: bool,
    prompt_tokens: i64,
    completion_tokens: i64,
    first_token_ms: i64,
    latency_ms: i64,
    status_code: i32,
    success: bool,
    error_message: Option<&str>,
) {
    let log_type = if success { 2 } else { 5 };
    let content = error_message.unwrap_or("");
    let token_name = access_key.map(|ak| ak.name.as_str()).unwrap_or("anonymous");
    let use_time = ((latency_ms as f64) / 1000.0).ceil() as i64;
    let request_id = "";
    let log_group = "default";
    let other = if success {
        format!(
            "{{\"requested_model\":\"{}\",\"resolved_model\":\"{}\",\"first_token_ms\":{},\"status_code\":{},\"success\":true}}",
            requested_model, entry.model, first_token_ms, status_code
        )
    } else {
        format!(
            "{{\"requested_model\":\"{}\",\"resolved_model\":\"{}\",\"first_token_ms\":{},\"status_code\":{},\"success\":false}}",
            requested_model, entry.model, first_token_ms, status_code
        )
    };

    let _ = db.insert_usage_log(
        log_type,
        content,
        access_key.map(|ak| ak.id.as_str()),
        access_key.map(|ak| ak.name.as_str()).unwrap_or("anonymous"),
        token_name,
        &entry.id,
        &entry.channel_id,
        entry.channel_name.as_deref().unwrap_or("unknown"),
        &entry.model,
        requested_model,
        0,
        is_stream,
        prompt_tokens,
        completion_tokens,
        latency_ms,
        first_token_ms,
        use_time,
        status_code,
        success,
        request_id,
        log_group,
        &other,
        error_message,
        None,
    );
}
