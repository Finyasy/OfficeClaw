use async_trait::async_trait;

use crate::domain::ProactiveDeliveryRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProactiveNotifierError {
    pub message: String,
}

#[async_trait]
pub trait ProactiveNotifier: Send + Sync {
    async fn send(&self, request: &ProactiveDeliveryRequest) -> Result<(), ProactiveNotifierError>;
}

#[derive(Clone)]
pub struct HttpProactiveNotifier {
    http: reqwest::Client,
    base_url: String,
}

impl HttpProactiveNotifier {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl ProactiveNotifier for HttpProactiveNotifier {
    async fn send(&self, request: &ProactiveDeliveryRequest) -> Result<(), ProactiveNotifierError> {
        let url = format!("{}/api/proactive", self.base_url.trim_end_matches('/'));
        self.http
            .post(url)
            .json(request)
            .send()
            .await
            .map_err(|error| ProactiveNotifierError {
                message: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| ProactiveNotifierError {
                message: error.to_string(),
            })?;
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct NoopProactiveNotifier;

#[async_trait]
impl ProactiveNotifier for NoopProactiveNotifier {
    async fn send(&self, _request: &ProactiveDeliveryRequest) -> Result<(), ProactiveNotifierError> {
        Ok(())
    }
}
