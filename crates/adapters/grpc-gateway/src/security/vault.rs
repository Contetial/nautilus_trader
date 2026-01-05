//! Encrypted credential vault using AES-256-GCM

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{SaltString, rand_core::OsRng as Argon2OsRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Vault errors
#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Vault is locked")]
    Locked,

    #[error("Invalid PIN")]
    InvalidPin,

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Storage error: {0}")]
    StorageError(String),
}

/// Stored broker credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerCredentials {
    pub broker_id: String,
    pub api_key: String,
    pub api_secret: String,
    pub access_token: Option<String>,
    pub extra: HashMap<String, String>,
}

/// Encrypted credential vault
pub struct CredentialVault {
    cipher: Option<Aes256Gcm>,
    credentials: HashMap<String, BrokerCredentials>,
    vault_path: String,
    is_locked: bool,
}

impl CredentialVault {
    /// Create a new vault instance
    pub fn new(vault_path: &str) -> Self {
        Self {
            cipher: None,
            credentials: HashMap::new(),
            vault_path: vault_path.to_string(),
            is_locked: true,
        }
    }

    /// Initialize vault with a new PIN (first time setup)
    pub fn initialize(&mut self, pin: &str) -> Result<(), VaultError> {
        let key = self.derive_key(pin)?;
        self.cipher = Some(Aes256Gcm::new(&key.into()));
        self.is_locked = false;
        self.save()?;
        Ok(())
    }

    /// Unlock vault with PIN
    pub fn unlock(&mut self, pin: &str) -> Result<(), VaultError> {
        let key = self.derive_key(pin)?;
        self.cipher = Some(Aes256Gcm::new(&key.into()));

        // Try to load and decrypt existing credentials
        if let Err(e) = self.load() {
            self.cipher = None;
            return Err(e);
        }

        self.is_locked = false;
        Ok(())
    }

    /// Lock vault (clear decrypted credentials from memory)
    pub fn lock(&mut self) {
        self.cipher = None;
        self.credentials.clear();
        self.is_locked = true;
    }

    /// Check if vault is locked
    pub fn is_locked(&self) -> bool {
        self.is_locked
    }

    /// Store broker credentials
    pub fn store_credentials(&mut self, creds: BrokerCredentials) -> Result<(), VaultError> {
        if self.is_locked {
            return Err(VaultError::Locked);
        }

        self.credentials.insert(creds.broker_id.clone(), creds);
        self.save()
    }

    /// Get broker credentials
    pub fn get_credentials(&self, broker_id: &str) -> Result<Option<&BrokerCredentials>, VaultError> {
        if self.is_locked {
            return Err(VaultError::Locked);
        }

        Ok(self.credentials.get(broker_id))
    }

    /// Remove broker credentials
    pub fn remove_credentials(&mut self, broker_id: &str) -> Result<(), VaultError> {
        if self.is_locked {
            return Err(VaultError::Locked);
        }

        self.credentials.remove(broker_id);
        self.save()
    }

    /// List all broker IDs
    pub fn list_brokers(&self) -> Result<Vec<String>, VaultError> {
        if self.is_locked {
            return Err(VaultError::Locked);
        }

        Ok(self.credentials.keys().cloned().collect())
    }

    /// Derive encryption key from PIN using Argon2
    fn derive_key(&self, pin: &str) -> Result<[u8; 32], VaultError> {
        let salt = SaltString::generate(&mut Argon2OsRng);
        let argon2 = Argon2::default();

        let hash = argon2
            .hash_password(pin.as_bytes(), &salt)
            .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

        let hash_bytes = hash.hash.ok_or(VaultError::EncryptionError("No hash".to_string()))?;
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash_bytes.as_bytes()[..32]);

        Ok(key)
    }

    /// Encrypt and save vault to disk
    fn save(&self) -> Result<(), VaultError> {
        let cipher = self.cipher.as_ref().ok_or(VaultError::Locked)?;

        let data = serde_json::to_vec(&self.credentials)
            .map_err(|e| VaultError::StorageError(e.to_string()))?;

        let nonce = Nonce::from_slice(b"unique nonce"); // TODO: Generate random nonce
        let encrypted = cipher
            .encrypt(nonce, data.as_ref())
            .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

        std::fs::write(&self.vault_path, encrypted)
            .map_err(|e| VaultError::StorageError(e.to_string()))?;

        Ok(())
    }

    /// Load and decrypt vault from disk
    fn load(&mut self) -> Result<(), VaultError> {
        let cipher = self.cipher.as_ref().ok_or(VaultError::Locked)?;

        let encrypted = match std::fs::read(&self.vault_path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // No vault file yet, start fresh
                self.credentials = HashMap::new();
                return Ok(());
            }
            Err(e) => return Err(VaultError::StorageError(e.to_string())),
        };

        let nonce = Nonce::from_slice(b"unique nonce"); // TODO: Use stored nonce
        let decrypted = cipher
            .decrypt(nonce, encrypted.as_ref())
            .map_err(|_| VaultError::InvalidPin)?;

        self.credentials = serde_json::from_slice(&decrypted)
            .map_err(|e| VaultError::StorageError(e.to_string()))?;

        Ok(())
    }
}
