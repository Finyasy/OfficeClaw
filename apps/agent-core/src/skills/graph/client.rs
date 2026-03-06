use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct GraphClientConfig {
    pub base_url: String,
}

#[derive(Clone)]
pub struct GraphClient {
    http: reqwest::Client,
    config: GraphClientConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphClientError {
    pub message: String,
    pub retryable: bool,
}

impl GraphClient {
    pub fn new(config: GraphClientConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
        }
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        access_token: &str,
    ) -> Result<T, GraphClientError> {
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);
        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|error| GraphClientError {
                message: error.to_string(),
                retryable: true,
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(GraphClientError {
                message: format!("graph request failed with status {}", status),
                retryable: status.is_server_error() || status.as_u16() == 429,
            });
        }

        response
            .json::<T>()
            .await
            .map_err(|error| GraphClientError {
                message: error.to_string(),
                retryable: false,
            })
    }

    pub async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        access_token: &str,
        body: &B,
    ) -> Result<T, GraphClientError> {
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);
        let response = self
            .http
            .post(url)
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await
            .map_err(|error| GraphClientError {
                message: error.to_string(),
                retryable: true,
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(GraphClientError {
                message: format!("graph request failed with status {}", status),
                retryable: status.is_server_error() || status.as_u16() == 429,
            });
        }

        response
            .json::<T>()
            .await
            .map_err(|error| GraphClientError {
                message: error.to_string(),
                retryable: false,
            })
    }

    pub async fn post_no_content<B: Serialize>(
        &self,
        path: &str,
        access_token: &str,
        body: &B,
    ) -> Result<(), GraphClientError> {
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);
        let response = self
            .http
            .post(url)
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await
            .map_err(|error| GraphClientError {
                message: error.to_string(),
                retryable: true,
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(GraphClientError {
                message: format!("graph request failed with status {}", status),
                retryable: status.is_server_error() || status.as_u16() == 429,
            });
        }

        Ok(())
    }
}
