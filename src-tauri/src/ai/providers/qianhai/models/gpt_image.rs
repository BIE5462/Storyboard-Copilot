use serde_json::json;

use crate::ai::error::AIError;
use crate::ai::GenerateRequest;

use super::super::adapter::{
    PreparedMultipartPart, PreparedRequest, PreparedRequestBody, PreparedResponseKind,
    QianhaiModelAdapter,
};
use super::shared::{collect_reference_image_files, truncate_for_log};

const GPT_IMAGE_MAX_REFERENCE_IMAGES: usize = 10;
const GPT_IMAGE_DEFAULT_SIZE: &str = "1024x1024";

pub struct OpenAiCompatibleImageAdapter {
    canonical_model: &'static str,
    aliases: &'static [&'static str],
}

impl OpenAiCompatibleImageAdapter {
    pub fn new(canonical_model: &'static str, aliases: &'static [&'static str]) -> Self {
        Self {
            canonical_model,
            aliases,
        }
    }

    fn request_model_name(&self) -> &'static str {
        self.canonical_model
            .split_once('/')
            .map(|(_, model)| model)
            .unwrap_or(self.canonical_model)
    }

    fn resolve_size(request: &GenerateRequest) -> String {
        let trimmed = request.size.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("Auto") {
            return "auto".to_string();
        }

        trimmed.to_string()
    }
}

impl QianhaiModelAdapter for OpenAiCompatibleImageAdapter {
    fn model_aliases(&self) -> &'static [&'static str] {
        self.aliases
    }

    fn build_request(
        &self,
        request: &GenerateRequest,
        base_url: &str,
    ) -> Result<PreparedRequest, AIError> {
        let has_reference_images = request
            .reference_images
            .as_ref()
            .map(|images| !images.is_empty())
            .unwrap_or(false);
        let request_model = self.request_model_name();
        let size = Self::resolve_size(request);
        let base_url = base_url.trim_end_matches('/');

        if has_reference_images {
            let reference_files = collect_reference_image_files(request, GPT_IMAGE_MAX_REFERENCE_IMAGES)?;
            if reference_files.is_empty() {
                return Err(AIError::InvalidRequest(
                    "Reference images are present but no valid image file payload was found"
                        .to_string(),
                ));
            }

            let mut parts = vec![
                PreparedMultipartPart::Text {
                    name: "model".to_string(),
                    value: request_model.to_string(),
                },
                PreparedMultipartPart::Text {
                    name: "prompt".to_string(),
                    value: request.prompt.clone(),
                },
                PreparedMultipartPart::Text {
                    name: "size".to_string(),
                    value: size.clone(),
                },
            ];

            parts.extend(reference_files.into_iter().map(|file| PreparedMultipartPart::File {
                name: "image".to_string(),
                file_name: file.file_name,
                mime_type: file.mime_type,
                bytes: file.bytes,
            }));

            let summary = format!(
                "model: {}, mode: edit, images: {}, size: {}, prompt: {}",
                self.canonical_model,
                parts
                    .iter()
                    .filter(|part| matches!(part, PreparedMultipartPart::File { .. }))
                    .count(),
                size,
                truncate_for_log(request.prompt.as_str(), 100)
            );

            return Ok(PreparedRequest {
                endpoint: format!("{}/v1/images/edits", base_url),
                body: PreparedRequestBody::Multipart(parts),
                response_kind: PreparedResponseKind::OpenAiImageData,
                summary,
            });
        }

        let body = json!({
            "model": request_model,
            "prompt": request.prompt.as_str(),
            "size": if size.is_empty() { GPT_IMAGE_DEFAULT_SIZE } else { size.as_str() },
            "format": "png",
            "n": 1,
        });
        let summary = format!(
            "model: {}, mode: generate, size: {}, prompt: {}",
            self.canonical_model,
            size,
            truncate_for_log(request.prompt.as_str(), 100)
        );

        Ok(PreparedRequest {
            endpoint: format!("{}/v1/images/generations", base_url),
            body: PreparedRequestBody::Json(body),
            response_kind: PreparedResponseKind::OpenAiImageData,
            summary,
        })
    }
}

inventory::submit! {
    crate::ai::providers::qianhai::models::RegisteredQianhaiModel {
        build: || Box::new(OpenAiCompatibleImageAdapter::new(
            "qianhai/gpt-image-2",
            &[
                "qianhai/gpt-image-2",
                "gpt-image-2",
            ],
        )),
    }
}

inventory::submit! {
    crate::ai::providers::qianhai::models::RegisteredQianhaiModel {
        build: || Box::new(OpenAiCompatibleImageAdapter::new(
            "qianhai/gpt-image-2-all",
            &[
                "qianhai/gpt-image-2-all",
                "gpt-image-2-all",
            ],
        )),
    }
}
