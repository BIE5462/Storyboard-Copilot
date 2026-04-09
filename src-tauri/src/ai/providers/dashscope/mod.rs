use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::ai::error::AIError;
use crate::ai::{AIProvider, GenerateRequest};

const DASHSCOPE_PROVIDER_NAME: &str = "dashscope";
const DASHSCOPE_GENERATION_ENDPOINT: &str =
    "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
const DASHSCOPE_DEFAULT_SIZE: &str = "1024*1024";
const DASHSCOPE_MAX_REFERENCE_IMAGES: usize = 3;
const DASHSCOPE_SUPPORTED_MODELS: [&str; 2] = ["qwen-image-2.0-pro", "qwen-image-2.0"];
const DASHSCOPE_SUPPORTED_SIZES: [&str; 16] = [
    "1024*1024",
    "1536*1536",
    "768*1152",
    "1024*1536",
    "1152*768",
    "1536*1024",
    "960*1280",
    "1080*1440",
    "1280*960",
    "1440*1080",
    "720*1280",
    "1080*1920",
    "1280*720",
    "1920*1080",
    "1344*576",
    "2048*872",
];

pub struct DashScopeProvider {
    client: Client,
    api_key: Arc<RwLock<Option<String>>>,
}

impl DashScopeProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: Arc::new(RwLock::new(None)),
        }
    }

    fn normalize_identifier(input: &str) -> String {
        input.trim().to_ascii_lowercase()
    }

    fn normalize_requested_model(model: &str) -> Option<&'static str> {
        let normalized = Self::normalize_identifier(model);
        let bare_model = normalized
            .split_once('/')
            .map(|(_, value)| value)
            .unwrap_or(normalized.as_str());

        match bare_model {
            "qwen-image-2.0-pro" => Some("qwen-image-2.0-pro"),
            "qwen-image-2.0" => Some("qwen-image-2.0"),
            _ => None,
        }
    }

    fn normalize_size(value: &str) -> String {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return DASHSCOPE_DEFAULT_SIZE.to_string();
        }

        trimmed.replace('x', "*").replace('X', "*")
    }

    fn resolve_size(request: &GenerateRequest) -> Result<String, AIError> {
        let normalized_size = Self::normalize_size(request.size.as_str());
        if DASHSCOPE_SUPPORTED_SIZES.contains(&normalized_size.as_str()) {
            return Ok(normalized_size);
        }

        Err(AIError::InvalidRequest(format!(
            "DashScope size '{}' is not supported; allowed sizes: {}",
            request.size,
            DASHSCOPE_SUPPORTED_SIZES.join(", ")
        )))
    }

    fn is_supported_reference_image(source: &str) -> bool {
        source.starts_with("http://")
            || source.starts_with("https://")
            || source.starts_with("data:image/")
    }

    fn resolve_reference_images(request: &GenerateRequest) -> Result<Vec<String>, AIError> {
        let reference_images = request.reference_images.clone().unwrap_or_default();
        if reference_images.len() > DASHSCOPE_MAX_REFERENCE_IMAGES {
            return Err(AIError::InvalidRequest(format!(
                "DashScope Qwen supports at most {} reference images, received {}",
                DASHSCOPE_MAX_REFERENCE_IMAGES,
                reference_images.len()
            )));
        }

        let mut normalized = Vec::with_capacity(reference_images.len());
        for source in reference_images {
            let trimmed = source.trim();
            if trimmed.is_empty() {
                continue;
            }

            if !Self::is_supported_reference_image(trimmed) {
                return Err(AIError::InvalidRequest(format!(
                    "DashScope reference image must be http(s) or data:image, received '{}'",
                    trimmed
                )));
            }

            normalized.push(trimmed.to_string());
        }

        Ok(normalized)
    }

    fn build_request_body(
        request: &GenerateRequest,
        model: &str,
        size: &str,
        reference_images: &[String],
    ) -> Value {
        let mut content = reference_images
            .iter()
            .map(|image| json!({ "image": image }))
            .collect::<Vec<_>>();
        content.push(json!({ "text": request.prompt }));

        json!({
            "model": model,
            "input": {
                "messages": [
                    {
                        "role": "user",
                        "content": content,
                    }
                ]
            },
            "parameters": {
                "n": 1,
                "negative_prompt": " ",
                "prompt_extend": true,
                "watermark": false,
                "size": size,
            }
        })
    }

    fn extract_error_details(body: &Value) -> String {
        let mut details = Vec::new();

        if let Some(code) = body.get("code").and_then(Value::as_str) {
            let trimmed = code.trim();
            if !trimmed.is_empty() {
                details.push(format!("code={}", trimmed));
            }
        }

        if let Some(message) = body.get("message").and_then(Value::as_str) {
            let trimmed = message.trim();
            if !trimmed.is_empty() {
                details.push(format!("message={}", trimmed));
            }
        }

        if let Some(request_id) = body.get("request_id").and_then(Value::as_str) {
            let trimmed = request_id.trim();
            if !trimmed.is_empty() {
                details.push(format!("request_id={}", trimmed));
            }
        }

        details.join(", ")
    }

    fn extract_image_url(body: &Value) -> Result<String, AIError> {
        let content = body
            .pointer("/output/choices/0/message/content")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                let details = Self::extract_error_details(body);
                AIError::Provider(if details.is_empty() {
                    "DashScope response missing output.choices[0].message.content".to_string()
                } else {
                    format!(
                        "DashScope response missing output.choices[0].message.content ({})",
                        details
                    )
                })
            })?;

        for item in content {
            if let Some(image_url) = item.get("image").and_then(Value::as_str) {
                let trimmed = image_url.trim();
                if !trimmed.is_empty() {
                    return Ok(trimmed.to_string());
                }
            }
        }

        let details = Self::extract_error_details(body);
        Err(AIError::Provider(if details.is_empty() {
            "DashScope response did not contain an image URL".to_string()
        } else {
            format!("DashScope response did not contain an image URL ({})", details)
        }))
    }
}

impl Default for DashScopeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AIProvider for DashScopeProvider {
    fn name(&self) -> &str {
        DASHSCOPE_PROVIDER_NAME
    }

    fn supports_model(&self, model: &str) -> bool {
        Self::normalize_requested_model(model).is_some()
    }

    fn list_models(&self) -> Vec<String> {
        DASHSCOPE_SUPPORTED_MODELS
            .iter()
            .map(|model| format!("{}/{}", DASHSCOPE_PROVIDER_NAME, model))
            .collect()
    }

    async fn set_api_key(&self, _provider: &str, api_key: String) -> Result<(), AIError> {
        let mut key = self.api_key.write().await;
        *key = Some(api_key);
        Ok(())
    }

    async fn generate(&self, request: GenerateRequest) -> Result<String, AIError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| AIError::InvalidRequest("DashScope API key not set".to_string()))?;
        let resolved_model = Self::normalize_requested_model(request.model.as_str())
            .ok_or_else(|| AIError::ModelNotSupported(request.model.clone()))?;
        let resolved_size = Self::resolve_size(&request)?;
        let reference_images = Self::resolve_reference_images(&request)?;
        let body = Self::build_request_body(
            &request,
            resolved_model,
            resolved_size.as_str(),
            reference_images.as_slice(),
        );

        info!(
            "[DashScope Request] model: {}, size: {}, reference_images: {}",
            resolved_model,
            resolved_size,
            reference_images.len()
        );
        info!("[DashScope API] URL: {}", DASHSCOPE_GENERATION_ENDPOINT);

        let response = self
            .client
            .post(DASHSCOPE_GENERATION_ENDPOINT)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let raw_response = response.text().await.unwrap_or_default();
        let parsed_body = serde_json::from_str::<Value>(&raw_response).map_err(|error| {
            AIError::Provider(format!(
                "DashScope API returned invalid JSON response: {}; raw={}",
                error, raw_response
            ))
        })?;

        if !status.is_success() {
            let details = Self::extract_error_details(&parsed_body);
            return Err(AIError::Provider(if details.is_empty() {
                format!("DashScope API error {}: {}", status, raw_response)
            } else {
                format!("DashScope API error {} ({})", status, details)
            }));
        }

        Self::extract_image_url(&parsed_body)
    }
}
