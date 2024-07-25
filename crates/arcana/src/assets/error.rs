use std::sync::Arc;

/// Error type for asset loading.
pub struct Error(pub Arc<dyn std::error::Error + Send + Sync>);
