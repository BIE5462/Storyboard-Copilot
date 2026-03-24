mod adapter;
mod models;
mod registry;

use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::ai::error::AIError;
use crate::ai::AIProvider;

use registry::QianhaiModelRegistry;

pub struct QianhaiProvider {
    client: Client,
    api_key: Arc<RwLock<Option<String>>>,
    base_url: String,
    model_registry: QianhaiModelRegistry,
}

impl QianhaiProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: Arc::new(RwLock::new(None)),
            base_url: "https://api.qianhai.online".to_string(),
            model_registry: QianhaiModelRegistry::new(),
        }
    }

    pub async fn set_api_key(&self, api_key: String) {
        let mut key = self.api_key.write().await;
        *key = Some(api_key);
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

fn extract_images_from_response(body: &Value) -> Result<Vec<(String, String)>, AIError> {
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

impl Default for QianhaiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AIProvider for QianhaiProvider {
    fn name(&self) -> &str {
        "qianhai"
    }

    fn supports_model(&self, model: &str) -> bool {
        self.model_registry.supports(model)
    }

    fn list_models(&self) -> Vec<String> {
        self.model_registry.list_models()
    }

    async fn set_api_key(&self, api_key: String) -> Result<(), AIError> {
        QianhaiProvider::set_api_key(self, api_key).await;
        Ok(())
    }

    async fn generate(&self, request: crate::ai::GenerateRequest) -> Result<String, AIError> {
        let key = self.api_key.read().await;
        let api_key = key
            .as_ref()
            .ok_or_else(|| AIError::InvalidRequest("API key not set".to_string()))?;

        let adapter = self
            .model_registry
            .resolve(&request.model)
            .ok_or_else(|| AIError::ModelNotSupported(request.model.clone()))?;

        let prepared = adapter.build_request(&request, &self.base_url)?;

        info!("[Qianhai Request] {}", prepared.summary);
        info!("[Qianhai API] URL: {}", prepared.endpoint);

        let response = self
            .client
            .post(&prepared.endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&prepared.body)
            .send()
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
        let images = extract_images_from_response(&body)?;
        let (mime_type, base64_payload) = images
            .into_iter()
            .next()
            .ok_or_else(|| AIError::Provider("Qianhai response missing image payload".to_string()))?;
        let data_url = format!("data:{};base64,{}", mime_type, base64_payload);

        info!(
            "[Qianhai Response] Generated image payload with mime type {}",
            mime_type
        );

        Ok(data_url)
    }
}
