use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::digest::{Context, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::config::Config;

use crate::appservice::HttpClient;

#[derive(Debug, Clone)]
pub struct AuthService {
    crypto: MatrixPasswordCrypto,
    encryption_key: EncryptionKey,
    client: ruma::Client<HttpClient>,
}

impl AuthService {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let crypto = MatrixPasswordCrypto::new();
        let encryption_key = EncryptionKey::new(&config.encryption.secret, None)?;

        let client = ruma::Client::builder()
            .homeserver_url(config.matrix.homeserver.clone())
            .build::<HttpClient>()
            .await?;
        
        Ok(Self {
            crypto,
            encryption_key,
            client
        })
    }

    pub fn generate_matrix_password(&self, length: usize) -> Result<String, EncryptionError> {
        self.crypto.generate_matrix_password(length)
    }
    
    pub fn encrypt_matrix_password(
        &self,
        password: &str,
    ) -> Result<EncryptedData, EncryptionError> {
        self.crypto.encrypt_matrix_password(password, &self.encryption_key)
    }

    pub fn decrypt_matrix_password(
        &self,
        encrypted_data: &EncryptedData,
    ) -> Result<String, EncryptionError> {
        self.crypto.decrypt_matrix_password(encrypted_data, &self.encryption_key)
    }
}

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Failed to generate random data")]
    RandomGeneration,
    #[error("Invalid key derivation input")]
    InvalidKeyInput,
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },
    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

impl From<ring::error::Unspecified> for EncryptionError {
    fn from(_: ring::error::Unspecified) -> Self {
        EncryptionError::RandomGeneration
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

#[derive(Debug, Clone, ZeroizeOnDrop)]
pub struct EncryptionKey {
    key: [u8; 32],
}

impl EncryptionKey {
    /// Derive an encryption key from the application secret using HKDF-like approach
    pub fn new(secret: &str, salt: Option<&[u8]>) -> Result<Self, EncryptionError> {

        if secret.is_empty() {
            return Err(EncryptionError::InvalidKeyInput);
        }

        // Use a default salt if none provided
        let default_salt = b"matrixbird_password_encryption";
        let salt = salt.unwrap_or(default_salt);

        // Create a proper key derivation using SHA256
        let mut context = Context::new(&SHA256);
        context.update(salt);
        context.update(secret.as_bytes());
        context.update(b"encryption_key"); // Domain separation
        
        let digest = context.finish();
        let mut key = [0u8; 32];
        key.copy_from_slice(digest.as_ref());

        Ok(Self { key })
    }

    /// Get the raw key bytes (use carefully)
    fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

#[derive(Debug, Clone)]
pub struct MatrixPasswordCrypto {
    rng: SystemRandom,
}

impl Default for MatrixPasswordCrypto {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixPasswordCrypto {
    pub fn new() -> Self {
        Self {
            rng: SystemRandom::new(),
        }
    }

    /// Generate a cryptographically secure random string for Matrix passwords
    pub fn generate_matrix_password(&self, length: usize) -> Result<String, EncryptionError> {

        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        
        let mut password_bytes = vec![0u8; length];
        self.rng.fill(&mut password_bytes)?;
        
        let password: String = password_bytes
            .into_iter()
            .map(|byte| ALPHABET[(byte as usize) % ALPHABET.len()] as char)
            .collect();
            
        Ok(password)
    }

    /// Encrypt a Matrix password using AES-256-GCM
    pub fn encrypt_matrix_password(
        &self,
        password: &str,
        key: &EncryptionKey,
    ) -> Result<EncryptedData, EncryptionError> {
        // Generate a random nonce
        let mut nonce_bytes = [0u8; NONCE_LEN];
        self.rng.fill(&mut nonce_bytes)?;
        
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        
        // Create the encryption key
        let unbound_key = UnboundKey::new(&AES_256_GCM, key.as_bytes())
            .map_err(|_| EncryptionError::EncryptionFailed)?;
        let sealing_key = LessSafeKey::new(unbound_key);

        // Prepare data for encryption
        let mut in_out = password.as_bytes().to_vec();
        
        // Encrypt the data (this appends the authentication tag)
        sealing_key
            .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
            .map_err(|_| EncryptionError::EncryptionFailed)?;

        Ok(EncryptedData {
            ciphertext: in_out,
            nonce: nonce_bytes.to_vec(),
        })
    }

    /// Decrypt a Matrix password using AES-256-GCM
    pub fn decrypt_matrix_password(
        &self,
        encrypted_data: &EncryptedData,
        key: &EncryptionKey,
    ) -> Result<String, EncryptionError> {
        // Validate nonce length
        if encrypted_data.nonce.len() != NONCE_LEN {
            return Err(EncryptionError::InvalidNonceLength {
                expected: NONCE_LEN,
                actual: encrypted_data.nonce.len(),
            });
        }

        // Convert nonce back to fixed-size array
        let mut nonce_bytes = [0u8; NONCE_LEN];
        nonce_bytes.copy_from_slice(&encrypted_data.nonce);
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        // Create the decryption key
        let unbound_key = UnboundKey::new(&AES_256_GCM, key.as_bytes())
            .map_err(|_| EncryptionError::DecryptionFailed)?;
        let opening_key = LessSafeKey::new(unbound_key);

        // Prepare data for decryption
        let mut in_out = encrypted_data.ciphertext.clone();

        // Decrypt the data (this also verifies the authentication tag)
        let plaintext = opening_key
            .open_in_place(nonce, Aad::empty(), &mut in_out)
            .map_err(|_| EncryptionError::DecryptionFailed)?;

        // Convert back to string
        let password = String::from_utf8(plaintext.to_vec())?;
        
        // Zero out the plaintext buffer for security
        in_out.zeroize();
        
        Ok(password)
    }

    /// Convenience method: generate and encrypt a Matrix password in one go
    pub fn generate_and_encrypt_matrix_password(
        &self,
        key: &EncryptionKey,
        password_length: usize,
    ) -> Result<(String, EncryptedData), EncryptionError> {
        let password = self.generate_matrix_password(password_length)?;
        let encrypted_data = self.encrypt_matrix_password(&password, key)?;
        Ok((password, encrypted_data))
    }
}

