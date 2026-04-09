mod adapter;
mod models;
mod registry;

use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use crate::ai::error::AIError;
use crate::ai::AIProvider;

use registry::QianhaiModelRegistry;

const QIANHAI_PROVIDER_ROUTE: &str = "qianhai";
const QIANHAI_GROK_PROVIDER_ROUTE: &str = "qianhai-grok";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QianhaiCredentialSlot {
    Gemini,
    Grok,
}

pub struct QianhaiProvider {
    client: Client,
    gemini_api_key: Arc<RwLock<Option<String>>>,
    grok_api_key: Arc<RwLock<Option<String>>>,
    base_url: String,
    model_registry: QianhaiModelRegistry,
}

impl QianhaiProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            gemini_api_key: Arc::new(RwLock::new(None)),
            grok_api_key: Arc::new(RwLock::new(None)),
            base_url: "https://api.qianhai.online".to_string(),
            model_registry: QianhaiModelRegistry::new(),
        }
    }

    pub async fn set_api_key(&self, provider: &str, api_key: String) {
        let slot = resolve_credential_slot_for_provider(provider);
        let key_store = match slot {
            QianhaiCredentialSlot::Gemini => &self.gemini_api_key,
            QianhaiCredentialSlot::Grok => &self.grok_api_key,
        };

        {
            let current_key = key_store.read().await;
            if current_key.as_deref() == Some(api_key.as_str()) {
                return;
            }
        }

        let mut current_key = key_store.write().await;
        *current_key = Some(api_key);
    }

    async fn api_key_for_model(&self, model: &str) -> Result<String, AIError> {
        let slot = resolve_credential_slot_for_model(model);
        let key_store = match slot {
            QianhaiCredentialSlot::Gemini => &self.gemini_api_key,
            QianhaiCredentialSlot::Grok => &self.grok_api_key,
        };

        key_store
            .read()
            .await
            .clone()
            .ok_or_else(|| AIError::InvalidRequest(api_key_missing_message(slot).to_string()))
    }

    async fn post_with_retry(
        &self,
        endpoint: &str,
        api_key: &str,
        body: &Value,
    ) -> Result<reqwest::Response, AIError> {
        const MAX_ATTEMPTS: usize = 3;
        const RETRY_DELAYS_MS: [u64; MAX_ATTEMPTS - 1] = [300, 900];

        for attempt in 1..=MAX_ATTEMPTS {
            let response = self
                .client
                .post(endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(body)
                .send()
                .await;

            match response {
                Ok(response) => return Ok(response),
                Err(error) => {
                    let should_retry = attempt < MAX_ATTEMPTS;
                    if !should_retry {
                        return Err(AIError::Network(error));
                    }

                    let delay_ms = RETRY_DELAYS_MS[attempt - 1];
                    warn!(
                        "[Qianhai API] request attempt {}/{} failed: {}; retrying in {}ms",
                        attempt,
                        MAX_ATTEMPTS,
                        error,
                        delay_ms
                    );
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }

        Err(AIError::Provider(
            "Qianhai request exhausted retry attempts".to_string(),
        ))
    }
}

fn normalize_identifier(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn is_qianhai_grok_model(model: &str) -> bool {
    matches!(
        normalize_identifier(model).as_str(),
        "qianhai/grok-image" | "grok-image"
    )
}

fn resolve_credential_slot_for_provider(provider: &str) -> QianhaiCredentialSlot {
    match normalize_identifier(provider).as_str() {
        QIANHAI_GROK_PROVIDER_ROUTE => QianhaiCredentialSlot::Grok,
        _ => QianhaiCredentialSlot::Gemini,
    }
}

fn resolve_credential_slot_for_model(model: &str) -> QianhaiCredentialSlot {
    if is_qianhai_grok_model(model) {
        return QianhaiCredentialSlot::Grok;
    }

    QianhaiCredentialSlot::Gemini
}

fn api_key_missing_message(slot: QianhaiCredentialSlot) -> &'static str {
    match slot {
        QianhaiCredentialSlot::Gemini => "Qianhai Gemini API key not set",
        QianhaiCredentialSlot::Grok => "Qianhai Grok API key not set",
    }
}

fn truncate_for_log(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }

    input.chars().take(max_chars).collect::<String>()
}

fn is_base64_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=')
}

fn extract_data_url_images_from_text(text: &str) -> Vec<(String, String)> {
    let mut images = Vec::new();
    let mut search_start = 0usize;

    while let Some(relative_start) = text[search_start..].find("data:image/") {
        let start = search_start + relative_start;
        let tail = &text[start..];
        let Some(base64_index) = tail.find(";base64,") else {
            search_start = start + "data:image/".len();
            continue;
        };

        let mime_type = tail["data:".len()..base64_index].trim();
        let payload_start = start + base64_index + ";base64,".len();
        let payload = text[payload_start..]
            .chars()
            .take_while(|ch| is_base64_char(*ch))
            .collect::<String>();

        if !payload.is_empty() {
            images.push((
                if mime_type.is_empty() {
                    "image/png".to_string()
                } else {
                    mime_type.to_string()
                },
                payload.clone(),
            ));
            search_start = payload_start + payload.len();
        } else {
            search_start = payload_start;
        }
    }

    images
}

fn extract_gemini_images_from_response(body: &Value) -> Result<Vec<(String, String)>, AIError> {
    let candidates = body
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| AIError::Provider("Qianhai response missing candidates field".to_string()))?;

    if candidates.is_empty() {
        return Err(AIError::Provider(
            "Qianhai response candidates field is empty".to_string(),
        ));
    }

    let mut images = Vec::new();
    let mut text_content = String::new();

    if let Some(parts) = candidates[0]
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
    {
        for part in parts {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                text_content.push_str(text);
            }

            if let Some(inline_data) = part.get("inlineData") {
                if let Some(data) = inline_data.get("data").and_then(Value::as_str) {
                    let mime_type = inline_data
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("image/png");
                    images.push((mime_type.to_string(), data.to_string()));
                }
            }
        }
    }

    if images.is_empty() && !text_content.is_empty() {
        images.extend(extract_data_url_images_from_text(&text_content));
    }

    if images.is_empty() {
        return Err(AIError::Provider(format!(
            "Qianhai response did not contain image data; text={}",
            truncate_for_log(&text_content, 160)
        )));
    }

    Ok(images)
}

fn is_supported_image_source(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("data:image/")
}

fn extract_markdown_url_at(text: &str, open_bracket_index: usize) -> Option<String> {
    let after_open = open_bracket_index + 1;
    let close_bracket_rel = text[after_open..].find(']')?;
    let close_bracket_index = after_open + close_bracket_rel;

    let remainder = &text[close_bracket_index + 1..];
    let trimmed_remainder = remainder.trim_start();
    if !trimmed_remainder.starts_with('(') {
        return None;
    }

    let leading_ws = remainder.len() - trimmed_remainder.len();
    let url_start = close_bracket_index + 1 + leading_ws + 1;
    let url_end_rel = text[url_start..].find(')')?;
    let candidate = text[url_start..url_start + url_end_rel].trim();
    if candidate.is_empty() {
        return None;
    }

    Some(candidate.to_string())
}

fn extract_markdown_image_url(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for index in 1..bytes.len() {
        if bytes[index] == b'[' && bytes[index - 1] == b'!' {
            if let Some(url) = extract_markdown_url_at(text, index) {
                return Some(url);
            }
        }
    }

    None
}

fn extract_markdown_link_url(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for (index, ch) in bytes.iter().enumerate() {
        if *ch != b'[' {
            continue;
        }

        if index > 0 && bytes[index - 1] == b'!' {
            continue;
        }

        if let Some(url) = extract_markdown_url_at(text, index) {
            return Some(url);
        }
    }

    None
}

fn extract_plain_url(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|token| {
        let candidate = token.trim_matches(|ch: char| {
            matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | '.')
        });
        if candidate.is_empty() || !is_supported_image_source(candidate) {
            return None;
        }

        Some(candidate.to_string())
    })
}

fn collect_grok_content_text(content: &Value) -> Vec<String> {
    match content {
        Value::String(text) => vec![text.to_string()],
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.to_string()),
                Value::Object(map) => map
                    .get("text")
                    .and_then(Value::as_str)
                    .or_else(|| map.get("content").and_then(Value::as_str))
                    .or_else(|| map.get("url").and_then(Value::as_str))
                    .map(|value| value.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn extract_grok_image_source_from_response(body: &Value) -> Result<String, AIError> {
    let content = body
        .pointer("/choices/0/message/content")
        .ok_or_else(|| AIError::Provider("Grok response missing choices[0].message.content".to_string()))?;
    let content_text = collect_grok_content_text(content).join("\n");

    let source = extract_markdown_image_url(&content_text)
        .or_else(|| extract_markdown_link_url(&content_text))
        .or_else(|| extract_plain_url(&content_text))
        .filter(|value| is_supported_image_source(value))
        .ok_or_else(|| {
            AIError::Provider(format!(
                "Grok response did not contain a usable image URL; content={}",
                truncate_for_log(&content_text, 200)
            ))
        })?;

    Ok(source)
}

impl Default for QianhaiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AIProvider for QianhaiProvider {
    fn name(&self) -> &str {
        QIANHAI_PROVIDER_ROUTE
    }

    fn supports_model(&self, model: &str) -> bool {
        self.model_registry.supports(model)
    }

    fn list_models(&self) -> Vec<String> {
        self.model_registry.list_models()
    }

    async fn set_api_key(&self, provider: &str, api_key: String) -> Result<(), AIError> {
        QianhaiProvider::set_api_key(self, provider, api_key).await;
        Ok(())
    }

    async fn generate(&self, request: crate::ai::GenerateRequest) -> Result<String, AIError> {
        let credential_slot = resolve_credential_slot_for_model(request.model.as_str());
        let api_key = self.api_key_for_model(request.model.as_str()).await?;

        let adapter = self
            .model_registry
            .resolve(&request.model)
            .ok_or_else(|| AIError::ModelNotSupported(request.model.clone()))?;

        let prepared = adapter.build_request(&request, &self.base_url)?;

        info!("[Qianhai Request] {}", prepared.summary);
        info!("[Qianhai API] URL: {}", prepared.endpoint);

        let response = self
            .post_with_retry(&prepared.endpoint, &api_key, &prepared.body)
            .await?;

        let status = response.status();
        let raw_response = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(AIError::Provider(format!(
                "Qianhai API error {}: {}",
                status, raw_response
            )));
        }

        let body = serde_json::from_str::<Value>(&raw_response).map_err(|error| {
            AIError::Provider(format!(
                "Qianhai API returned invalid JSON response: {}; raw={}",
                error, raw_response
            ))
        })?;

        let image_source = match credential_slot {
            QianhaiCredentialSlot::Gemini => {
                let images = extract_gemini_images_from_response(&body)?;
                let (mime_type, base64_payload) = images.into_iter().next().ok_or_else(|| {
                    AIError::Provider("Qianhai response missing image payload".to_string())
                })?;
                format!("data:{};base64,{}", mime_type, base64_payload)
            }
            QianhaiCredentialSlot::Grok => extract_grok_image_source_from_response(&body)?,
        };

        info!(
            "[Qianhai Response] Resolved image source for slot {:?}",
            credential_slot
        );

        Ok(image_source)
    }
}
