use std::sync::Arc;
use std::time::Duration;
use reqwest::Client;
use crate::error::{AppError, AppResult};

/// Creates a configured HTTP client with shared settings
pub fn create_http_client() -> AppResult<Client> {
    Client::builder()
        .danger_accept_invalid_certs(true)  // Skip certificate verification
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(AppError::Http)
}

/// Creates an Arc-wrapped HTTP client for sharing across handlers
pub fn create_shared_client() -> AppResult<Arc<Client>> {
    create_http_client().map(Arc::new)
}

