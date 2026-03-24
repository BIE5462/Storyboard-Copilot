use super::shared::GoogleCompatibleImagePreviewAdapter;

inventory::submit! {
    crate::ai::providers::qianhai::models::RegisteredQianhaiModel {
        build: || Box::new(GoogleCompatibleImagePreviewAdapter::new(
            "qianhai/gemini-3.1-flash-image-preview",
            &[
                "qianhai/gemini-3.1-flash-image-preview",
                "gemini-3.1-flash-image-preview",
            ],
        )),
    }
}
