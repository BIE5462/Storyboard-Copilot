use super::super::adapter::QianhaiModelAdapter;

automod::dir!("src/ai/providers/qianhai/models");

pub struct RegisteredQianhaiModel {
    pub build: fn() -> Box<dyn QianhaiModelAdapter>,
}

inventory::collect!(RegisteredQianhaiModel);

pub fn collect_adapters() -> Vec<Box<dyn QianhaiModelAdapter>> {
    inventory::iter::<RegisteredQianhaiModel>
        .into_iter()
        .map(|entry| (entry.build)())
        .collect()
}
