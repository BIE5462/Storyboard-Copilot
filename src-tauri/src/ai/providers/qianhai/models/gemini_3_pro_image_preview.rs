use super::shared::GoogleCompatibleImagePreviewAdapter;

inventory::submit! {
    crate::ai::providers::qianhai::models::RegisteredQianhaiModel {
        build: || Box::new(GoogleCompatibleImagePreviewAdapter::new(
            "qianhai/gemini-3-pro-image-preview",
            &[
                "qianhai/gemini-3-pro-image-preview",
                "gemini-3-pro-image-preview",
            ],
        )),
    }
}
