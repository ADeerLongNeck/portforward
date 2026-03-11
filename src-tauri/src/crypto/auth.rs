use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid MAC")]
    InvalidMac,
    #[error("Invalid key length")]
    InvalidKeyLength,
}

/// Authentication manager using SHA-256 HMAC
/// Replaces the insecure MD5 authentication from the original Java implementation
pub struct AuthManager {
    server_key: Vec<u8>,
}

impl AuthManager {
    /// Create a new auth manager from a password
    /// The password is hashed with SHA-256 to derive the key
    pub fn new(password: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let server_key = hasher.finalize().to_vec();
        Self { server_key }
    }

    /// Generate a random nonce (challenge)
    pub fn generate_nonce(&self) -> Vec<u8> {
        let mut nonce = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }

    /// Generate authentication response for a given nonce
    /// response = HMAC-SHA256(server_key, nonce)
    pub fn generate_response(&self, nonce: &[u8]) -> Result<Vec<u8>, AuthError> {
        let mut mac = HmacSha256::new_from_slice(&self.server_key)
            .map_err(|_| AuthError::InvalidKeyLength)?;
        mac.update(nonce);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// Verify an authentication response
    /// Uses constant-time comparison to prevent timing attacks
    pub fn verify_response(&self, nonce: &[u8], response: &[u8]) -> bool {
        let expected = match self.generate_response(nonce) {
            Ok(r) => r,
            Err(_) => return false,
        };

        // Constant-time comparison
        if response.len() != expected.len() {
            return false;
        }

        let mut result = 0u8;
        for (a, b) in response.iter().zip(expected.iter()) {
            result |= a ^ b;
        }
        result == 0
    }

    /// Generate a secure session key from the password and a random salt
    pub fn generate_session_key(&self, salt: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(&self.server_key);
        hasher.update(salt);
        hasher.finalize().to_vec()
    }
}

/// Challenge-response authentication flow
pub struct AuthChallenge {
    pub nonce: Vec<u8>,
}

impl AuthChallenge {
    pub fn new() -> Self {
        let mut nonce = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);
        Self { nonce }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { nonce: bytes }
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.nonce
    }
}

impl Default for AuthChallenge {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility function to compute SHA-256 hash
pub fn sha256_hash(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Utility function to compute SHA-256 hash and return as hex string
pub fn sha256_hex(data: &[u8]) -> String {
    let hash = sha256_hash(data);
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_flow() {
        let server_auth = AuthManager::new("server_password");
        let client_auth = AuthManager::new("server_password");

        // Server generates challenge
        let nonce = server_auth.generate_nonce();

        // Client generates response
        let response = client_auth.generate_response(&nonce).unwrap();

        // Server verifies response
        assert!(server_auth.verify_response(&nonce, &response));
    }

    #[test]
    fn test_wrong_password() {
        let server_auth = AuthManager::new("correct_password");
        let client_auth = AuthManager::new("wrong_password");

        let nonce = server_auth.generate_nonce();
        let response = client_auth.generate_response(&nonce).unwrap();

        // Should fail with wrong password
        assert!(!server_auth.verify_response(&nonce, &response));
    }
}
