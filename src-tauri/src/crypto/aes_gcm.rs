use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Invalid data length")]
    InvalidDataLength,
    #[error("Invalid key length")]
    InvalidKeyLength,
}

/// AES-256-GCM encryption/decryption
/// Replaces the insecure AES-ECB mode from the original Java implementation
#[derive(Clone)]
pub struct AesGcmCrypto {
    cipher: Aes256Gcm,
}

impl AesGcmCrypto {
    /// Create a new AES-GCM crypto instance from a password
    /// The password is hashed to derive a 32-byte key
    pub fn new(password: &str) -> Result<Self, CryptoError> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let key = hasher.finalize();

        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key);
        let cipher = Aes256Gcm::new(key);

        Ok(Self { cipher })
    }

    /// Create from raw key bytes (must be 32 bytes for AES-256)
    pub fn from_key(key: &[u8]) -> Result<Self, CryptoError> {
        if key.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }

        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(key);
        let cipher = Aes256Gcm::new(key);

        Ok(Self { cipher })
    }

    /// Encrypt plaintext using AES-256-GCM
    /// Returns: nonce (12 bytes) + ciphertext + tag (16 bytes)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    /// Decrypt data encrypted with AES-256-GCM
    /// Input format: nonce (12 bytes) + ciphertext + tag
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if data.len() < 12 + 16 {
            // nonce + minimum tag size
            return Err(CryptoError::InvalidDataLength);
        }

        let nonce = Nonce::from_slice(&data[..12]);
        let ciphertext = &data[12..];

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::DecryptionFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let crypto = AesGcmCrypto::new("test_password").unwrap();
        let plaintext = b"Hello, World!";
        let encrypted = crypto.encrypt(plaintext).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_different_nonces() {
        let crypto = AesGcmCrypto::new("test_password").unwrap();
        let plaintext = b"Hello, World!";
        let encrypted1 = crypto.encrypt(plaintext).unwrap();
        let encrypted2 = crypto.encrypt(plaintext).unwrap();
        // Same plaintext should produce different ciphertext due to random nonce
        assert_ne!(encrypted1, encrypted2);
    }
}
