use serde_json::json;

use crate::ai::error::AIError;
use crate::ai::GenerateRequest;

use super::super::adapter::{PreparedRequest, QianhaiModelAdapter};
use super::shared::{
    collect_reference_image_urls, resolve_runtime_model_name, truncate_for_log,
};

pub struct GrokImageAdapter;

impl QianhaiModelAdapter for GrokImageAdapter {
    fn model_aliases(&self) -> &'static [&'static str] {
        &["qianhai/grok-image", "grok-image"]
    }

    fn build_request(
        &self,
        request: &GenerateRequest,
        base_url: &str,
    ) -> Result<PreparedRequest, AIError> {
        let request_model = resolve_runtime_model_name(request)?;
        let reference_images = collect_reference_image_urls(request)?;
        let endpoint = format!(
            "{}/v1/chat/completions",
            base_url.trim_end_matches('/')
        );

        let content = if reference_images.is_empty() {
            json!(request.prompt.as_str())
        } else {
            let mut parts = reference_images
                .into_iter()
                .map(|url| {
                    json!({
                        "type": "image_url",
                        "image_url": {
                            "url": url
                        }
                    })
                })
                .collect::<Vec<_>>();
            parts.push(json!({
                "type": "text",
                "text": request.prompt.as_str()
            }));
            json!(parts)
        };

        let body = json!({
            "model": request_model,
            "messages": [{
                "role": "user",
                "content": content
            }],
            "size": request.size.as_str(),
            "response_format": {
                "type": "url"
            }
        });

        let summary = format!(
            "model: {}, mode: {}, size: {}, prompt: {}",
            request.model,
            if request
                .reference_images
                .as_ref()
                .map(|images| !images.is_empty())
                .unwrap_or(false)
            {
                "edit"
            } else {
                "generate"
            },
            request.size.as_str(),
            truncate_for_log(request.prompt.as_str(), 100)
        );

        Ok(PreparedRequest {
            endpoint,
            body,
            summary,
        })
    }
}

inventory::submit! {
    crate::ai::providers::qianhai::models::RegisteredQianhaiModel {
        build: || Box::new(GrokImageAdapter),
    }
}
