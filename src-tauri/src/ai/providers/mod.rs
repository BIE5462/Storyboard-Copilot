use std::sync::Arc;

use super::AIProvider;

pub mod ppio;
pub mod qianhai;
pub mod grsai;
pub mod kie;
pub mod fal;
pub mod dashscope;

pub use dashscope::DashScopeProvider;
pub use fal::FalProvider;
pub use grsai::GrsaiProvider;
pub use kie::KieProvider;
pub use ppio::PPIOProvider;
pub use qianhai::QianhaiProvider;

pub fn build_default_providers() -> Vec<Arc<dyn AIProvider>> {
    vec![
        Arc::new(PPIOProvider::new()),
        Arc::new(QianhaiProvider::new()),
        Arc::new(DashScopeProvider::new()),
        Arc::new(GrsaiProvider::new()),
        Arc::new(KieProvider::new()),
        Arc::new(FalProvider::new()),
    ]
}
