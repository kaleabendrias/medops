use std::collections::HashMap;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs;

use crate::config::KeyBootstrapConfig;
use crate::contracts::ApiError;

#[derive(Clone)]
pub struct FieldCrypto {
    active_key_version: String,
    keys: HashMap<String, Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::FieldCrypto;
    use crate::config::KeyBootstrapConfig;

    #[tokio::test]
    async fn require_existing_fails_when_key_missing() {
        let key_file = format!("/tmp/nonexistent-field-crypto-key-{}.json", std::process::id());
        let cfg = KeyBootstrapConfig {
            strategy: "require_existing".to_string(),
            key_file,
            key_length: 32,
        };
        let result = FieldCrypto::load_or_bootstrap(&cfg).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_strategy_is_rejected() {
        let cfg = KeyBootstrapConfig {
            strategy: "fallback_only".to_string(),
            key_file: "/tmp/ignored-key.json".to_string(),
            key_length: 32,
        };
        let result = FieldCrypto::load_or_bootstrap(&cfg).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn encrypt_then_decrypt_roundtrip_returns_original_text() {
        let key_file = format!("/tmp/field-crypto-roundtrip-{}.json", std::process::id());
        let cfg = KeyBootstrapConfig {
            strategy: "generate_if_missing".to_string(),
            key_file: key_file.clone(),
            key_length: 32,
        };
        let crypto = FieldCrypto::load_or_bootstrap(&cfg).await.expect("bootstrap");
        let plaintext = "Patient has severe shellfish allergy. Documented during visit.";
        let ciphertext = crypto.encrypt(plaintext).expect("encrypt");
        assert_ne!(ciphertext, plaintext, "ciphertext must differ from plaintext");
        assert!(!ciphertext.contains(plaintext), "plaintext must not appear within ciphertext");
        let decrypted = crypto.decrypt(&ciphertext).expect("decrypt");
        assert_eq!(decrypted, plaintext);
        let _ = std::fs::remove_file(&key_file);
    }

    #[tokio::test]
    async fn ciphertext_format_contains_version_nonce_data() {
        let key_file = format!("/tmp/field-crypto-format-{}.json", std::process::id());
        let cfg = KeyBootstrapConfig {
            strategy: "generate_if_missing".to_string(),
            key_file: key_file.clone(),
            key_length: 32,
        };
        let crypto = FieldCrypto::load_or_bootstrap(&cfg).await.expect("bootstrap");
        let ciphertext = crypto.encrypt("test note").expect("encrypt");
        let parts: Vec<&str> = ciphertext.split(':').collect();
        assert_eq!(parts.len(), 3, "ciphertext must be version:nonce:data");
        assert_eq!(parts[0], crypto.active_key_version());
        let _ = std::fs::remove_file(&key_file);
    }

    #[tokio::test]
    async fn two_encryptions_of_same_text_produce_different_ciphertexts() {
        let key_file = format!("/tmp/field-crypto-nonce-{}.json", std::process::id());
        let cfg = KeyBootstrapConfig {
            strategy: "generate_if_missing".to_string(),
            key_file: key_file.clone(),
            key_length: 32,
        };
        let crypto = FieldCrypto::load_or_bootstrap(&cfg).await.expect("bootstrap");
        let c1 = crypto.encrypt("same note").expect("encrypt1");
        let c2 = crypto.encrypt("same note").expect("encrypt2");
        assert_ne!(c1, c2, "random nonce must produce distinct ciphertexts");
        let _ = std::fs::remove_file(&key_file);
    }

    #[tokio::test]
    async fn decrypt_rejects_tampered_ciphertext() {
        let key_file = format!("/tmp/field-crypto-tamper-{}.json", std::process::id());
        let cfg = KeyBootstrapConfig {
            strategy: "generate_if_missing".to_string(),
            key_file: key_file.clone(),
            key_length: 32,
        };
        let crypto = FieldCrypto::load_or_bootstrap(&cfg).await.expect("bootstrap");
        let ciphertext = crypto.encrypt("sensitive visit note").expect("encrypt");
        let tampered = format!("{}X", ciphertext);
        assert!(crypto.decrypt(&tampered).is_err());
        let _ = std::fs::remove_file(&key_file);
    }

    #[test]
    fn hash_for_lookup_is_deterministic() {
        let h1 = FieldCrypto::hash_for_lookup("MRN-12345");
        let h2 = FieldCrypto::hash_for_lookup("MRN-12345");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64, "SHA256 hex digest must be 64 chars");
    }

    #[test]
    fn hash_for_lookup_trims_whitespace() {
        let h1 = FieldCrypto::hash_for_lookup("  test  ");
        let h2 = FieldCrypto::hash_for_lookup("test");
        assert_eq!(h1, h2);
    }

    #[tokio::test]
    async fn encrypt_empty_string_roundtrips() {
        let key_file = format!("/tmp/field-crypto-empty-{}.json", std::process::id());
        let cfg = KeyBootstrapConfig {
            strategy: "generate_if_missing".to_string(),
            key_file: key_file.clone(),
            key_length: 32,
        };
        let crypto = FieldCrypto::load_or_bootstrap(&cfg).await.expect("bootstrap");
        let ciphertext = crypto.encrypt("").expect("encrypt empty");
        let decrypted = crypto.decrypt(&ciphertext).expect("decrypt empty");
        assert_eq!(decrypted, "");
        let _ = std::fs::remove_file(&key_file);
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyRingDisk {
    active_key_version: String,
    keys: HashMap<String, String>,
}

impl FieldCrypto {
    pub async fn load_or_bootstrap(cfg: &KeyBootstrapConfig) -> Result<Self, Box<dyn std::error::Error>> {
        if cfg.strategy != "generate_if_missing" && cfg.strategy != "require_existing" {
            return Err("security.key_bootstrap.strategy must be generate_if_missing or require_existing".into());
        }

        let key_path = Path::new(&cfg.key_file);
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        if !key_path.exists() && cfg.strategy == "generate_if_missing" {
            let mut bytes = vec![0u8; cfg.key_length];
            rand::thread_rng().fill_bytes(&mut bytes);
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            let mut keys = HashMap::new();
            keys.insert("v1".to_string(), encoded);
            let disk = KeyRingDisk {
                active_key_version: "v1".to_string(),
                keys,
            };
            fs::write(key_path, serde_json::to_string_pretty(&disk)?).await?;
        }

        if !key_path.exists() {
            return Err("encryption key material not found; startup aborted".into());
        }

        let raw = fs::read_to_string(key_path).await?;
        let parsed = if raw.trim_start().starts_with('{') {
            serde_json::from_str::<KeyRingDisk>(&raw)?
        } else {
            let mut keys = HashMap::new();
            keys.insert("v1".to_string(), raw.trim().to_string());
            let disk = KeyRingDisk {
                active_key_version: "v1".to_string(),
                keys,
            };
            fs::write(key_path, serde_json::to_string_pretty(&disk)?).await?;
            disk
        };

        let mut decoded = HashMap::new();
        for (version, key_b64) in parsed.keys {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(key_b64.trim())
                .map_err(|_| "invalid key in keyring")?;
            if bytes.len() != 32 {
                return Err("key length must be 32 bytes".into());
            }
            decoded.insert(version, bytes);
        }

        if !decoded.contains_key(&parsed.active_key_version) {
            return Err("active key version missing from keyring".into());
        }

        Ok(Self {
            active_key_version: parsed.active_key_version,
            keys: decoded,
        })
    }

    pub fn active_key_version(&self) -> &str {
        &self.active_key_version
    }

    pub fn encrypt(&self, plain: &str) -> Result<String, ApiError> {
        let key = self
            .keys
            .get(&self.active_key_version)
            .ok_or(ApiError::Internal)?;
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| ApiError::Internal)?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let encrypted = cipher
            .encrypt(nonce, plain.as_bytes())
            .map_err(|_| ApiError::Internal)?;

        Ok(format!(
            "{}:{}:{}",
            self.active_key_version,
            base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
            base64::engine::general_purpose::STANDARD.encode(encrypted)
        ))
    }

    pub fn decrypt(&self, cipher_text: &str) -> Result<String, ApiError> {
        let mut parts = cipher_text.split(':');
        let version = parts.next().ok_or(ApiError::Internal)?;
        let nonce_b64 = parts.next().ok_or(ApiError::Internal)?;
        let data_b64 = parts.next().ok_or(ApiError::Internal)?;
        if parts.next().is_some() {
            return Err(ApiError::Internal);
        }

        let key = self.keys.get(version).ok_or(ApiError::Internal)?;
        let nonce_bytes = base64::engine::general_purpose::STANDARD
            .decode(nonce_b64)
            .map_err(|_| ApiError::Internal)?;
        let encrypted = base64::engine::general_purpose::STANDARD
            .decode(data_b64)
            .map_err(|_| ApiError::Internal)?;

        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| ApiError::Internal)?;
        let decrypted = cipher
            .decrypt(Nonce::from_slice(&nonce_bytes), encrypted.as_ref())
            .map_err(|_| ApiError::Internal)?;
        String::from_utf8(decrypted).map_err(|_| ApiError::Internal)
    }

    pub fn hash_for_lookup(value: &str) -> String {
        hex::encode(Sha256::digest(value.trim().as_bytes()))
    }
}
