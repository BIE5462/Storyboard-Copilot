use serde_json::Value;

use crate::ai::error::AIError;
use crate::ai::GenerateRequest;

#[derive(Clone, Debug)]
pub enum PreparedMultipartPart {
    Text {
        name: String,
        value: String,
    },
    File {
        name: String,
        file_name: String,
        mime_type: String,
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug)]
pub enum PreparedRequestBody {
    Json(Value),
    Multipart(Vec<PreparedMultipartPart>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedResponseKind {
    GeminiInlineImage,
    GrokImageSource,
    OpenAiImageData,
}

pub struct PreparedRequest {
    pub endpoint: String,
    pub body: PreparedRequestBody,
    pub response_kind: PreparedResponseKind,
    pub summary: String,
}

pub trait QianhaiModelAdapter: Send + Sync {
    fn model_aliases(&self) -> &'static [&'static str];

    fn canonical_model(&self) -> &'static str {
        self.model_aliases()
            .iter()
            .find(|model| model.contains('/'))
            .copied()
            .or_else(|| self.model_aliases().first().copied())
            .unwrap_or("unknown")
    }

    fn matches(&self, model: &str) -> bool {
        self.model_aliases().iter().any(|alias| alias == &model)
    }

    fn build_request(
        &self,
        request: &GenerateRequest,
        base_url: &str,
    ) -> Result<PreparedRequest, AIError>;
}
