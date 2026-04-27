use base64::{engine::general_purpose::STANDARD, Engine};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, GenericImageView};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

use crate::ai::error::AIError;
use crate::ai::GenerateRequest;

use super::super::adapter::{
    PreparedRequest, PreparedRequestBody, PreparedResponseKind, QianhaiModelAdapter,
};

pub struct GoogleCompatibleImagePreviewAdapter {
    canonical_model: &'static str,
    aliases: &'static [&'static str],
}

pub(crate) struct ResolvedReferenceImageFile {
    pub file_name: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

const QIANHAI_REFERENCE_IMAGE_MAX_DIMENSION: u32 = 512;
const QIANHAI_REFERENCE_IMAGE_MAX_BYTES: usize = 400 * 1024;
const QIANHAI_REFERENCE_IMAGE_JPEG_QUALITY: u8 = 82;
pub(crate) const QIANHAI_DYNAMIC_MODEL_NAME_KEY: &str = "qianhai_model_name";

impl GoogleCompatibleImagePreviewAdapter {
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
}

fn decode_file_url_path(value: &str) -> String {
    let raw = value.trim_start_matches("file://");
    let decoded = urlencoding::decode(raw)
        .map(|result| result.into_owned())
        .unwrap_or_else(|_| raw.to_string());
    let normalized = if decoded.starts_with('/')
        && decoded.len() > 2
        && decoded.as_bytes().get(2) == Some(&b':')
    {
        &decoded[1..]
    } else {
        &decoded
    };

    normalized.to_string()
}

fn infer_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/png",
    }
}

fn default_extension_for_mime_type(mime_type: &str) -> &'static str {
    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

fn file_name_for_reference_path(path: &Path, fallback_index: usize, mime_type: &str) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
        .unwrap_or_else(|| {
            format!(
                "reference_{}.{}",
                fallback_index + 1,
                default_extension_for_mime_type(mime_type)
            )
        })
}

pub(crate) fn resolve_reference_image_file(
    source: &str,
    index: usize,
) -> Result<ResolvedReferenceImageFile, AIError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(AIError::InvalidRequest(
            "Qianhai reference image source cannot be empty".to_string(),
        ));
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Err(AIError::InvalidRequest(
            "Qianhai GPT image edits require data:image payloads or local image files"
                .to_string(),
        ));
    }

    if let Some((meta, payload)) = trimmed.split_once(',') {
        if meta.starts_with("data:") && meta.ends_with(";base64") && !payload.is_empty() {
            let mime_type = meta
                .trim_start_matches("data:")
                .trim_end_matches(";base64")
                .trim();
            let resolved_mime_type = if mime_type.is_empty() {
                "image/png"
            } else {
                mime_type
            };
            let bytes = STANDARD.decode(payload).map_err(|error| {
                AIError::InvalidRequest(format!(
                    "Qianhai reference image base64 payload is invalid: {}",
                    error
                ))
            })?;

            return Ok(ResolvedReferenceImageFile {
                file_name: format!(
                    "reference_{}.{}",
                    index + 1,
                    default_extension_for_mime_type(resolved_mime_type)
                ),
                mime_type: resolved_mime_type.to_string(),
                bytes,
            });
        }
    }

    let path = if trimmed.starts_with("file://") {
        PathBuf::from(decode_file_url_path(trimmed))
    } else {
        PathBuf::from(trimmed)
    };
    let mime_type = infer_mime_type(&path);
    let bytes = std::fs::read(&path)?;

    Ok(ResolvedReferenceImageFile {
        file_name: file_name_for_reference_path(&path, index, mime_type),
        mime_type: mime_type.to_string(),
        bytes,
    })
}

pub(crate) fn collect_reference_image_files(
    request: &GenerateRequest,
    max_images: usize,
) -> Result<Vec<ResolvedReferenceImageFile>, AIError> {
    let Some(reference_images) = request.reference_images.as_ref() else {
        return Ok(Vec::new());
    };

    if reference_images.len() > max_images {
        return Err(AIError::InvalidRequest(format!(
            "Qianhai GPT image edits support at most {} reference images, received {}",
            max_images,
            reference_images.len()
        )));
    }

    reference_images
        .iter()
        .enumerate()
        .map(|(index, source)| resolve_reference_image_file(source, index))
        .collect()
}

fn optimize_reference_image_bytes(bytes: Vec<u8>, mime_type: &str) -> (String, Vec<u8>) {
    let Ok(image) = image::load_from_memory(&bytes) else {
        return (mime_type.to_string(), bytes);
    };

    let (width, height) = image.dimensions();
    let needs_resize = width > QIANHAI_REFERENCE_IMAGE_MAX_DIMENSION
        || height > QIANHAI_REFERENCE_IMAGE_MAX_DIMENSION;
    let needs_reencode = bytes.len() > QIANHAI_REFERENCE_IMAGE_MAX_BYTES;

    if !needs_resize && !needs_reencode {
        return (mime_type.to_string(), bytes);
    }

    let processed = if needs_resize {
        image.resize(
            QIANHAI_REFERENCE_IMAGE_MAX_DIMENSION,
            QIANHAI_REFERENCE_IMAGE_MAX_DIMENSION,
            FilterType::Lanczos3,
        )
    } else {
        image
    };

    let rgb = processed.to_rgb8();
    let (encoded_width, encoded_height) = rgb.dimensions();
    let mut encoded = Vec::new();
    let mut encoder =
        JpegEncoder::new_with_quality(&mut encoded, QIANHAI_REFERENCE_IMAGE_JPEG_QUALITY);

    if encoder
        .encode(
            &rgb,
            encoded_width,
            encoded_height,
            ExtendedColorType::Rgb8,
        )
        .is_ok()
    {
        ("image/jpeg".to_string(), encoded)
    } else {
        (mime_type.to_string(), bytes)
    }
}

fn resolve_inline_image_part(source: &str) -> Option<Value> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some((meta, payload)) = trimmed.split_once(',') {
        if meta.starts_with("data:") && meta.ends_with(";base64") && !payload.is_empty() {
            let mime_type = meta
                .trim_start_matches("data:")
                .trim_end_matches(";base64")
                .trim();
            let resolved_mime_type = if mime_type.is_empty() {
                "image/png"
            } else {
                mime_type
            };

            if let Ok(decoded_bytes) = STANDARD.decode(payload) {
                let (optimized_mime_type, optimized_bytes) =
                    optimize_reference_image_bytes(decoded_bytes, resolved_mime_type);

                return Some(json!({
                    "inlineData": {
                        "mimeType": optimized_mime_type,
                        "data": STANDARD.encode(optimized_bytes),
                    }
                }));
            }

            return Some(json!({
                "inlineData": {
                    "mimeType": resolved_mime_type,
                    "data": payload,
                }
            }));
        }
    }

    let path = if trimmed.starts_with("file://") {
        PathBuf::from(decode_file_url_path(trimmed))
    } else {
        PathBuf::from(trimmed)
    };

    let bytes = std::fs::read(&path).ok()?;
    let mime_type = infer_mime_type(&path);
    let (optimized_mime_type, optimized_bytes) = optimize_reference_image_bytes(bytes, mime_type);

    Some(json!({
        "inlineData": {
            "mimeType": optimized_mime_type,
            "data": STANDARD.encode(optimized_bytes),
        }
    }))
}

pub(crate) fn truncate_for_log(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }

    input.chars().take(max_chars).collect::<String>()
}

pub(crate) fn resolve_runtime_model_name(request: &GenerateRequest) -> Result<String, AIError> {
    request
        .extra_params
        .as_ref()
        .and_then(|params| params.get(QIANHAI_DYNAMIC_MODEL_NAME_KEY))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| {
            AIError::InvalidRequest(format!(
                "Qianhai runtime model name is missing for {}",
                request.model
            ))
        })
}

pub(crate) fn collect_reference_image_urls(
    request: &GenerateRequest,
) -> Result<Vec<String>, AIError> {
    let mut urls = Vec::new();

    if let Some(reference_images) = request.reference_images.as_ref() {
        for source in reference_images {
            let trimmed = source.trim();
            if trimmed.is_empty() {
                return Err(AIError::InvalidRequest(
                    "Qianhai reference image source cannot be empty".to_string(),
                ));
            }

            if trimmed.starts_with("http://")
                || trimmed.starts_with("https://")
                || trimmed.starts_with("data:image/")
            {
                urls.push(trimmed.to_string());
                continue;
            }

            return Err(AIError::InvalidRequest(format!(
                "Qianhai reference images must be http(s) URLs or data:image payloads: {}",
                truncate_for_log(trimmed, 80)
            )));
        }
    }

    Ok(urls)
}

impl QianhaiModelAdapter for GoogleCompatibleImagePreviewAdapter {
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

        let mut parts = Vec::new();

        if let Some(reference_images) = request.reference_images.as_ref() {
            let image_parts = reference_images
                .iter()
                .take(5)
                .filter_map(|image| resolve_inline_image_part(image))
                .collect::<Vec<Value>>();

            if has_reference_images && image_parts.is_empty() {
                return Err(AIError::InvalidRequest(
                    "Reference images are present but no valid inlineData payload was found"
                        .to_string(),
                ));
            }

            parts.extend(image_parts);
        }

        parts.push(json!({ "text": request.prompt.as_str() }));

        let mut generation_config = Map::new();
        generation_config.insert("responseModalities".to_string(), json!(["IMAGE", "TEXT"]));

        let mut image_config = Map::new();
        if !request.aspect_ratio.trim().is_empty() && request.aspect_ratio != "Auto" {
            image_config.insert(
                "aspectRatio".to_string(),
                Value::String(request.aspect_ratio.clone()),
            );
        } else if !request.size.trim().is_empty() && request.size != "Auto" {
            image_config.insert("aspectRatio".to_string(), Value::String("1:1".to_string()));
        }

        if !request.size.trim().is_empty() && request.size != "Auto" {
            image_config.insert("imageSize".to_string(), Value::String(request.size.clone()));
        }

        if !image_config.is_empty() {
            generation_config.insert("imageConfig".to_string(), Value::Object(image_config));
        }

        let mode_label = if has_reference_images { "edit" } else { "generate" };
        let endpoint = format!(
            "{}/v1beta/models/{}:generateContent",
            base_url.trim_end_matches('/'),
            self.request_model_name()
        );
        let body = json!({
            "contents": [{
                "role": "user",
                "parts": parts,
            }],
            "generationConfig": Value::Object(generation_config),
        });
        let summary = format!(
            "model: {}, mode: {}, images: {}, size: {}, aspect_ratio: {}, prompt: {}",
            self.canonical_model(),
            mode_label,
            request.reference_images.as_ref().map(|images| images.len()).unwrap_or(0),
            request.size.as_str(),
            request.aspect_ratio.as_str(),
            truncate_for_log(request.prompt.as_str(), 100)
        );

        Ok(PreparedRequest {
            endpoint,
            body: PreparedRequestBody::Json(body),
            response_kind: PreparedResponseKind::GeminiInlineImage,
            summary,
        })
    }
}
