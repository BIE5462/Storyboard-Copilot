mod adapter;
mod models;
mod registry;

use reqwest::Client;
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::task;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use uuid::Uuid;

use crate::ai::build_provider_http_client;
use crate::ai::error::AIError;
use crate::ai::AIProvider;

use adapter::{PreparedMultipartPart, PreparedRequestBody, PreparedResponseKind};
use registry::QianhaiModelRegistry;

const QIANHAI_PROVIDER_ROUTE: &str = "qianhai";
const QIANHAI_GROK_PROVIDER_ROUTE: &str = "qianhai-grok";
const QIANHAI_GPT_IMAGE_2_ALL_PROVIDER_ROUTE: &str = "qianhai-gpt-image-2-all";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QianhaiCredentialSlot {
    Gemini,
    Grok,
    GptImage2All,
}

pub struct QianhaiProvider {
    client: Client,
    gemini_api_key: Arc<RwLock<Option<String>>>,
    grok_api_key: Arc<RwLock<Option<String>>>,
    gpt_image_2_all_api_key: Arc<RwLock<Option<String>>>,
    base_url: String,
    model_registry: QianhaiModelRegistry,
}

#[derive(Debug)]
struct QianhaiHttpResponse {
    status_code: u16,
    raw_body: String,
}

impl QianhaiHttpResponse {
    fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    fn status_label(&self) -> String {
        self.status_code.to_string()
    }
}

impl QianhaiProvider {
    pub fn new() -> Self {
        Self {
            client: build_provider_http_client(),
            gemini_api_key: Arc::new(RwLock::new(None)),
            grok_api_key: Arc::new(RwLock::new(None)),
            gpt_image_2_all_api_key: Arc::new(RwLock::new(None)),
            base_url: "https://api.qianhai.online".to_string(),
            model_registry: QianhaiModelRegistry::new(),
        }
    }

    pub async fn set_api_key(&self, provider: &str, api_key: String) {
        let slot = resolve_credential_slot_for_provider(provider);
        let key_store = match slot {
            QianhaiCredentialSlot::Gemini => &self.gemini_api_key,
            QianhaiCredentialSlot::Grok => &self.grok_api_key,
            QianhaiCredentialSlot::GptImage2All => &self.gpt_image_2_all_api_key,
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
            QianhaiCredentialSlot::GptImage2All => &self.gpt_image_2_all_api_key,
        };

        key_store
            .read()
            .await
            .clone()
            .ok_or_else(|| AIError::InvalidRequest(api_key_missing_message(slot).to_string()))
    }

    async fn post_json_with_retry(
        &self,
        endpoint: &str,
        api_key: &str,
        body: &Value,
    ) -> Result<QianhaiHttpResponse, AIError> {
        const MAX_ATTEMPTS: usize = 3;
        const RETRY_DELAYS_MS: [u64; MAX_ATTEMPTS - 1] = [300, 900];

        for attempt in 1..=MAX_ATTEMPTS {
            let response = self
                .client
                .post(endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .json(body)
                .send()
                .await;

            match response {
                Ok(response) => {
                    let status_code = response.status().as_u16();
                    let raw_body = response.text().await.unwrap_or_default();
                    return Ok(QianhaiHttpResponse {
                        status_code,
                        raw_body,
                    });
                }
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

    async fn post_multipart_with_retry(
        &self,
        endpoint: &str,
        api_key: &str,
        parts: &[PreparedMultipartPart],
    ) -> Result<QianhaiHttpResponse, AIError> {
        const MAX_ATTEMPTS: usize = 3;
        const RETRY_DELAYS_MS: [u64; MAX_ATTEMPTS - 1] = [300, 900];

        for attempt in 1..=MAX_ATTEMPTS {
            let endpoint = endpoint.to_string();
            let api_key = api_key.to_string();
            let parts = parts.to_vec();
            let response = task::spawn_blocking(move || {
                run_curl_multipart_request(endpoint, api_key, parts)
            })
            .await
            .map_err(|error| {
                AIError::Provider(format!("Qianhai multipart worker failed: {}", error))
            })?;

            match response {
                Ok(response) => return Ok(response),
                Err(error) => {
                    let should_retry = attempt < MAX_ATTEMPTS;
                    if !should_retry {
                        return Err(error);
                    }

                    let delay_ms = RETRY_DELAYS_MS[attempt - 1];
                    warn!(
                        "[Qianhai API] multipart request attempt {}/{} failed: {}; retrying in {}ms",
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
            "Qianhai multipart request exhausted retry attempts".to_string(),
        ))
    }

    async fn post_with_retry(
        &self,
        endpoint: &str,
        api_key: &str,
        body: &PreparedRequestBody,
    ) -> Result<QianhaiHttpResponse, AIError> {
        match body {
            PreparedRequestBody::Json(value) => {
                self.post_json_with_retry(endpoint, api_key, value).await
            }
            PreparedRequestBody::Multipart(parts) => {
                self.post_multipart_with_retry(endpoint, api_key, parts).await
            }
        }
    }
}

fn curl_command_name() -> &'static str {
    if cfg!(windows) {
        "curl.exe"
    } else {
        "curl"
    }
}

fn curl_file_extension_for_mime_type(mime_type: &str) -> &'static str {
    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

fn sanitize_curl_file_name(file_name: &str, fallback: &str) -> String {
    let sanitized = file_name
        .chars()
        .map(|ch| match ch {
            '"' | ';' | '\r' | '\n' | '/' | '\\' => '_',
            _ => ch,
        })
        .collect::<String>();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() || trimmed.chars().all(|ch| matches!(ch, '_' | '.')) {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn escape_curl_config_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_curl_http_response(stdout: &[u8]) -> Result<QianhaiHttpResponse, AIError> {
    const STATUS_MARKER: &str = "\n__STORYBOARD_HTTP_STATUS__:";
    let output = String::from_utf8_lossy(stdout).to_string();
    let Some((raw_body, raw_status)) = output.rsplit_once(STATUS_MARKER) else {
        return Err(AIError::Provider(format!(
            "Qianhai curl response missing HTTP status marker: {}",
            truncate_for_log(output.as_str(), 200)
        )));
    };
    let status_code = raw_status.trim().parse::<u16>().map_err(|error| {
        AIError::Provider(format!(
            "Qianhai curl response had invalid HTTP status '{}': {}",
            raw_status.trim(),
            error
        ))
    })?;

    Ok(QianhaiHttpResponse {
        status_code,
        raw_body: raw_body.to_string(),
    })
}

fn run_curl_multipart_request(
    endpoint: String,
    api_key: String,
    parts: Vec<PreparedMultipartPart>,
) -> Result<QianhaiHttpResponse, AIError> {
    let temp_dir = std::env::temp_dir().join(format!("storyboard-qianhai-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let result = run_curl_multipart_request_with_temp_dir(endpoint, api_key, parts, &temp_dir);
    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

fn run_curl_multipart_request_with_temp_dir(
    endpoint: String,
    api_key: String,
    parts: Vec<PreparedMultipartPart>,
    temp_dir: &PathBuf,
) -> Result<QianhaiHttpResponse, AIError> {
    let mut command = Command::new(curl_command_name());
    command
        .arg("-sS")
        .arg("--connect-timeout")
        .arg("15")
        .arg("--max-time")
        .arg("600")
        .arg("-X")
        .arg("POST")
        .arg(endpoint)
        .arg("-K")
        .arg("-")
        .arg("-w")
        .arg("\n__STORYBOARD_HTTP_STATUS__:%{http_code}")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut file_index = 0usize;
    for part in parts {
        match part {
            PreparedMultipartPart::Text { name, value } => {
                command.arg("--form-string").arg(format!("{}={}", name, value));
            }
            PreparedMultipartPart::File {
                name,
                file_name,
                mime_type,
                bytes,
            } => {
                let fallback_name = format!(
                    "reference_{}.{}",
                    file_index + 1,
                    curl_file_extension_for_mime_type(mime_type.as_str())
                );
                let safe_file_name = sanitize_curl_file_name(file_name.as_str(), fallback_name.as_str());
                let temp_path = temp_dir.join(format!("upload_{}_{}", file_index + 1, safe_file_name));
                std::fs::write(&temp_path, bytes)?;
                command.arg("--form").arg(format!(
                    "{}=@{};type={};filename={}",
                    name,
                    temp_path.display(),
                    mime_type,
                    safe_file_name
                ));
                file_index += 1;
            }
        }
    }

    let mut child = command.spawn().map_err(|error| {
        AIError::Provider(format!(
            "Failed to start curl for Qianhai multipart request: {}",
            error
        ))
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        let config = format!(
            "header = \"Accept: application/json\"\nheader = \"Authorization: Bearer {}\"\n",
            escape_curl_config_value(api_key.as_str())
        );
        stdin.write_all(config.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AIError::Provider(format!(
            "Qianhai curl multipart request failed: {}",
            truncate_for_log(stderr.as_ref(), 300)
        )));
    }

    parse_curl_http_response(output.stdout.as_slice())
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

fn is_qianhai_gpt_image_2_all_model(model: &str) -> bool {
    matches!(
        normalize_identifier(model).as_str(),
        "qianhai/gpt-image-2-all" | "gpt-image-2-all"
    )
}

fn resolve_credential_slot_for_provider(provider: &str) -> QianhaiCredentialSlot {
    match normalize_identifier(provider).as_str() {
        QIANHAI_GROK_PROVIDER_ROUTE => QianhaiCredentialSlot::Grok,
        QIANHAI_GPT_IMAGE_2_ALL_PROVIDER_ROUTE => QianhaiCredentialSlot::GptImage2All,
        _ => QianhaiCredentialSlot::Gemini,
    }
}

fn resolve_credential_slot_for_model(model: &str) -> QianhaiCredentialSlot {
    if is_qianhai_grok_model(model) {
        return QianhaiCredentialSlot::Grok;
    }

    if is_qianhai_gpt_image_2_all_model(model) {
        return QianhaiCredentialSlot::GptImage2All;
    }

    QianhaiCredentialSlot::Gemini
}

fn api_key_missing_message(slot: QianhaiCredentialSlot) -> &'static str {
    match slot {
        QianhaiCredentialSlot::Gemini => "Qianhai Gemini API key not set",
        QianhaiCredentialSlot::Grok => "Qianhai Grok API key not set",
        QianhaiCredentialSlot::GptImage2All => "Qianhai GPT Image 2 All API key not set",
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

fn resolve_openai_image_mime_type(body: &Value) -> &'static str {
    match body
        .get("output_format")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

fn extract_openai_image_source_from_response(body: &Value) -> Result<String, AIError> {
    let data = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| AIError::Provider("Qianhai GPT response missing data field".to_string()))?;

    if data.is_empty() {
        return Err(AIError::Provider(
            "Qianhai GPT response data field is empty".to_string(),
        ));
    }

    for item in data {
        if let Some(base64_payload) = item.get("b64_json").and_then(Value::as_str) {
            let trimmed = base64_payload.trim();
            if !trimmed.is_empty() {
                if trimmed.starts_with("data:image/") {
                    return Ok(trimmed.to_string());
                }

                return Ok(format!(
                    "data:{};base64,{}",
                    resolve_openai_image_mime_type(body),
                    trimmed
                ));
            }
        }

        if let Some(url) = item.get("url").and_then(Value::as_str) {
            let trimmed = url.trim();
            if is_supported_image_source(trimmed) {
                return Ok(trimmed.to_string());
            }
        }
    }

    Err(AIError::Provider(
        "Qianhai GPT response did not contain b64_json or image url".to_string(),
    ))
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

        let is_success = response.is_success();
        let status_label = response.status_label();
        let raw_response = response.raw_body;

        if !is_success {
            return Err(AIError::Provider(format!(
                "Qianhai API error {}: {}",
                status_label, raw_response
            )));
        }

        let body = serde_json::from_str::<Value>(&raw_response).map_err(|error| {
            AIError::Provider(format!(
                "Qianhai API returned invalid JSON response: {}; raw={}",
                error, raw_response
            ))
        })?;

        let image_source = match prepared.response_kind {
            PreparedResponseKind::GeminiInlineImage => {
                let images = extract_gemini_images_from_response(&body)?;
                let (mime_type, base64_payload) = images.into_iter().next().ok_or_else(|| {
                    AIError::Provider("Qianhai response missing image payload".to_string())
                })?;
                format!("data:{};base64,{}", mime_type, base64_payload)
            }
            PreparedResponseKind::GrokImageSource => extract_grok_image_source_from_response(&body)?,
            PreparedResponseKind::OpenAiImageData => {
                extract_openai_image_source_from_response(&body)?
            }
        };

        info!(
            "[Qianhai Response] Resolved image source for slot {:?}",
            credential_slot
        );

        Ok(image_source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn openai_image_response_prefers_b64_json() {
        let body = json!({
            "data": [
                {
                    "b64_json": "iVBORw0KGgo="
                }
            ],
            "output_format": "png"
        });

        let source = extract_openai_image_source_from_response(&body).unwrap();

        assert_eq!(source, "data:image/png;base64,iVBORw0KGgo=");
    }

    #[test]
    fn openai_image_response_accepts_url_fallback() {
        let body = json!({
            "data": [
                {
                    "url": "https://example.test/generated.png"
                }
            ]
        });

        let source = extract_openai_image_source_from_response(&body).unwrap();

        assert_eq!(source, "https://example.test/generated.png");
    }

    #[test]
    fn gpt_image_2_all_uses_independent_credential_slot() {
        assert_eq!(
            resolve_credential_slot_for_provider("qianhai-gpt-image-2-all"),
            QianhaiCredentialSlot::GptImage2All
        );
        assert_eq!(
            resolve_credential_slot_for_model("qianhai/gpt-image-2-all"),
            QianhaiCredentialSlot::GptImage2All
        );
    }

    #[test]
    fn curl_http_response_parser_extracts_body_and_status() {
        let response =
            parse_curl_http_response(br#"{"data":[{"url":"https://example.test/image.png"}]}
__STORYBOARD_HTTP_STATUS__:200"#)
                .unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(
            response.raw_body,
            r#"{"data":[{"url":"https://example.test/image.png"}]}"#
        );
        assert!(response.is_success());
    }

    #[test]
    fn curl_http_response_parser_rejects_missing_status_marker() {
        let error = parse_curl_http_response(br#"{"error":"missing marker"}"#).unwrap_err();

        assert!(matches!(
            error,
            AIError::Provider(message)
                if message.contains("missing HTTP status marker")
        ));
    }

    #[test]
    fn curl_upload_file_names_are_sanitized() {
        assert_eq!(
            sanitize_curl_file_name("..\\bad/name;\".jpg", "fallback.jpg"),
            ".._bad_name__.jpg"
        );
        assert_eq!(sanitize_curl_file_name("  \n  ", "fallback.jpg"), "fallback.jpg");
        assert_eq!(curl_file_extension_for_mime_type("image/jpeg"), "jpg");
        assert_eq!(curl_file_extension_for_mime_type("image/webp"), "webp");
        assert_eq!(curl_file_extension_for_mime_type("application/octet-stream"), "png");
    }
}
