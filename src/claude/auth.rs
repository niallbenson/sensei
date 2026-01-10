//! API key management using system keyring

use keyring::Entry;

use super::error::ClaudeError;

/// Service name for keyring storage
const SERVICE_NAME: &str = "sensei-tui";
/// Entry name for the API key
const API_KEY_ENTRY: &str = "anthropic-api-key";

/// Manages Claude API key storage in system keyring
pub struct ApiKeyManager;

impl ApiKeyManager {
    /// Get the API key from system keyring
    pub fn get_api_key() -> Result<String, ClaudeError> {
        let entry = Entry::new(SERVICE_NAME, API_KEY_ENTRY)
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        entry.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => ClaudeError::ApiKeyNotFound,
            _ => ClaudeError::KeyringError(e.to_string()),
        })
    }

    /// Store the API key in system keyring
    pub fn set_api_key(key: &str) -> Result<(), ClaudeError> {
        // Validate key format
        if !Self::validate_key_format(key) {
            return Err(ClaudeError::InvalidApiKey);
        }

        let entry = Entry::new(SERVICE_NAME, API_KEY_ENTRY)
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        entry.set_password(key).map_err(|e| ClaudeError::KeyringError(e.to_string()))
    }

    /// Check if an API key is stored
    pub fn has_api_key() -> bool {
        Self::get_api_key().is_ok()
    }

    /// Delete the stored API key
    pub fn delete_api_key() -> Result<(), ClaudeError> {
        let entry = Entry::new(SERVICE_NAME, API_KEY_ENTRY)
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        entry.delete_credential().map_err(|e| ClaudeError::KeyringError(e.to_string()))
    }

    /// Validate API key format
    fn validate_key_format(key: &str) -> bool {
        // Anthropic API keys start with "sk-ant-"
        key.starts_with("sk-ant-") && key.len() > 20
    }

    /// Mask an API key for display (show first and last 4 chars)
    pub fn mask_key(key: &str) -> String {
        if key.len() <= 12 {
            return "*".repeat(key.len());
        }
        let prefix = &key[..8];
        let suffix = &key[key.len() - 4..];
        format!("{}...{}", prefix, suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_key_format() {
        assert!(ApiKeyManager::validate_key_format("sk-ant-api03-abcdefghijklmnop"));
        assert!(!ApiKeyManager::validate_key_format("invalid-key"));
        assert!(!ApiKeyManager::validate_key_format("sk-ant-short"));
    }

    #[test]
    fn mask_key() {
        let key = "sk-ant-api03-abcdefghijklmnopqrstuvwxyz";
        let masked = ApiKeyManager::mask_key(key);
        assert!(masked.starts_with("sk-ant-a"));
        assert!(masked.ends_with("wxyz"));
        assert!(masked.contains("..."));
    }
}
