use async_trait::async_trait;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CryptoError {
    pub message: String,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CryptoError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedEnvelope {
    pub ciphertext: Vec<u8>,
    pub key_version: String,
}

#[async_trait]
pub trait EnvelopeCipher: Send + Sync {
    async fn seal(&self, plaintext: &[u8]) -> Result<SealedEnvelope, CryptoError>;
    async fn open(&self, key_version: &str, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError>;
}

#[derive(Debug, Clone)]
pub struct PlaintextEnvelopeCipher {
    key_version: String,
}

impl PlaintextEnvelopeCipher {
    pub fn new(key_version: impl Into<String>) -> Self {
        Self {
            key_version: key_version.into(),
        }
    }
}

impl Default for PlaintextEnvelopeCipher {
    fn default() -> Self {
        Self::new("dev-local")
    }
}

#[async_trait]
impl EnvelopeCipher for PlaintextEnvelopeCipher {
    async fn seal(&self, plaintext: &[u8]) -> Result<SealedEnvelope, CryptoError> {
        Ok(SealedEnvelope {
            ciphertext: plaintext.to_vec(),
            key_version: self.key_version.clone(),
        })
    }

    async fn open(&self, _key_version: &str, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        Ok(ciphertext.to_vec())
    }
}

#[derive(Clone)]
pub struct KeyVaultEnvelopeCipher {
    http: reqwest::Client,
    config: KeyVaultConfig,
    credential: KeyVaultCredential,
}

#[derive(Debug, Clone)]
pub struct KeyVaultConfig {
    pub vault_uri: String,
    pub kek_name: String,
    pub api_version: String,
}

#[derive(Debug, Clone)]
pub enum KeyVaultCredential {
    StaticToken(String),
    ManagedIdentity(ManagedIdentityConfig),
}

#[derive(Debug, Clone)]
pub struct ManagedIdentityConfig {
    pub endpoint: Option<String>,
    pub secret_header: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LocalEnvelope {
    alg: String,
    wrapped_dek: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Debug, Deserialize)]
struct KeyVaultWrapResponse {
    value: String,
    kid: String,
}

impl KeyVaultEnvelopeCipher {
    pub fn new(config: KeyVaultConfig, credential: KeyVaultCredential) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
            credential,
        }
    }
}

#[async_trait]
impl EnvelopeCipher for KeyVaultEnvelopeCipher {
    async fn seal(&self, plaintext: &[u8]) -> Result<SealedEnvelope, CryptoError> {
        let mut dek = [0_u8; 32];
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut dek);
        OsRng.fill_bytes(&mut nonce);

        let cipher = Aes256Gcm::new_from_slice(&dek).map_err(|error| CryptoError {
            message: error.to_string(),
        })?;
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?;

        let wrapped = self.wrap_key(&dek).await?;
        let envelope = LocalEnvelope {
            alg: "A256GCM".to_string(),
            wrapped_dek: wrapped.value,
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        };

        Ok(SealedEnvelope {
            ciphertext: serde_json::to_vec(&envelope).map_err(|error| CryptoError {
                message: error.to_string(),
            })?,
            key_version: key_version_from_kid(&wrapped.kid),
        })
    }

    async fn open(&self, key_version: &str, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let envelope: LocalEnvelope = serde_json::from_slice(ciphertext).map_err(|error| {
            CryptoError {
                message: error.to_string(),
            }
        })?;
        let dek = self.unwrap_key(key_version, &envelope.wrapped_dek).await?;
        let nonce = URL_SAFE_NO_PAD.decode(envelope.nonce).map_err(|error| CryptoError {
            message: error.to_string(),
        })?;
        let encrypted = URL_SAFE_NO_PAD
            .decode(envelope.ciphertext)
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?;

        let cipher = Aes256Gcm::new_from_slice(&dek).map_err(|error| CryptoError {
            message: error.to_string(),
        })?;

        cipher
            .decrypt(Nonce::from_slice(&nonce), encrypted.as_ref())
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })
    }
}

impl KeyVaultEnvelopeCipher {
    async fn wrap_key(&self, dek: &[u8]) -> Result<KeyVaultWrapResponse, CryptoError> {
        let token = self.access_token().await?;
        let url = format!(
            "{}/keys/{}/wrapkey?api-version={}",
            self.config.vault_uri.trim_end_matches('/'),
            self.config.kek_name,
            self.config.api_version
        );

        self.http
            .post(url)
            .bearer_auth(token)
            .json(&serde_json::json!({
                "alg": "RSA-OAEP-256",
                "value": URL_SAFE_NO_PAD.encode(dek)
            }))
            .send()
            .await
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?
            .json::<KeyVaultWrapResponse>()
            .await
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })
    }

    async fn unwrap_key(&self, key_version: &str, wrapped_dek: &str) -> Result<Vec<u8>, CryptoError> {
        let token = self.access_token().await?;
        let url = format!(
            "{}/keys/{}/{}/unwrapkey?api-version={}",
            self.config.vault_uri.trim_end_matches('/'),
            self.config.kek_name,
            key_version,
            self.config.api_version
        );

        let response = self
            .http
            .post(url)
            .bearer_auth(token)
            .json(&serde_json::json!({
                "alg": "RSA-OAEP-256",
                "value": wrapped_dek
            }))
            .send()
            .await
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?;

        let wrapped = response
            .get("value")
            .and_then(|value| value.as_str())
            .ok_or_else(|| CryptoError {
                message: "unwrapkey response missing value".to_string(),
            })?;

        URL_SAFE_NO_PAD.decode(wrapped).map_err(|error| CryptoError {
            message: error.to_string(),
        })
    }

    async fn access_token(&self) -> Result<String, CryptoError> {
        match &self.credential {
            KeyVaultCredential::StaticToken(token) => Ok(token.clone()),
            KeyVaultCredential::ManagedIdentity(config) => access_token_from_managed_identity(
                &self.http,
                config,
            )
            .await,
        }
    }
}

async fn access_token_from_managed_identity(
    http: &reqwest::Client,
    config: &ManagedIdentityConfig,
) -> Result<String, CryptoError> {
    if let (Some(endpoint), Some(secret_header)) = (&config.endpoint, &config.secret_header) {
        let response = http
            .get(endpoint)
            .query(&[
                ("resource", "https://vault.azure.net"),
                ("api-version", "2019-08-01"),
            ])
            .header("X-IDENTITY-HEADER", secret_header)
            .send()
            .await
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| CryptoError {
                message: error.to_string(),
            })?;

        return response
            .get("access_token")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or_else(|| CryptoError {
                message: "managed identity response missing access_token".to_string(),
            });
    }

    let response = http
        .get("http://169.254.169.254/metadata/identity/oauth2/token")
        .query(&[
            ("resource", "https://vault.azure.net"),
            ("api-version", "2018-02-01"),
        ])
        .header("Metadata", "true")
        .send()
        .await
        .map_err(|error| CryptoError {
            message: error.to_string(),
        })?
        .error_for_status()
        .map_err(|error| CryptoError {
            message: error.to_string(),
        })?
        .json::<serde_json::Value>()
        .await
        .map_err(|error| CryptoError {
            message: error.to_string(),
        })?;

    response
        .get("access_token")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| CryptoError {
            message: "managed identity response missing access_token".to_string(),
        })
}

fn key_version_from_kid(kid: &str) -> String {
    kid.rsplit('/').next().unwrap_or(kid).to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        key_version_from_kid, EnvelopeCipher, KeyVaultCredential, LocalEnvelope,
        PlaintextEnvelopeCipher,
    };

    #[tokio::test]
    async fn plaintext_cipher_round_trips_payloads() {
        let cipher = PlaintextEnvelopeCipher::new("dev-test");
        let sealed = cipher.seal(b"secret").await.unwrap();

        assert_eq!(sealed.key_version, "dev-test");
        assert_eq!(
            cipher
                .open(&sealed.key_version, &sealed.ciphertext)
                .await
                .unwrap(),
            b"secret"
        );
    }

    #[test]
    fn key_version_is_extracted_from_kid() {
        assert_eq!(
            key_version_from_kid(
                "https://example.vault.azure.net/keys/teams-agent-kek/abc123-version"
            ),
            "abc123-version"
        );
    }

    #[test]
    fn local_envelope_is_serializable() {
        let envelope = LocalEnvelope {
            alg: "A256GCM".to_string(),
            wrapped_dek: "wrapped".to_string(),
            nonce: "nonce".to_string(),
            ciphertext: "ciphertext".to_string(),
        };

        let bytes = serde_json::to_vec(&envelope).unwrap();
        let parsed: LocalEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.alg, "A256GCM");
    }

    #[test]
    fn static_token_credential_can_be_constructed() {
        let credential = KeyVaultCredential::StaticToken("token".to_string());
        match credential {
            KeyVaultCredential::StaticToken(value) => assert_eq!(value, "token"),
            KeyVaultCredential::ManagedIdentity(_) => panic!("expected static token"),
        }
    }
}
