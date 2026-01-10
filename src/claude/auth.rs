//! API key management using system keyring

use super::error::ClaudeError;

/// Service name for keyring storage
const SERVICE_NAME: &str = "sensei-tui";
/// Entry name for the API key
const API_KEY_ENTRY: &str = "anthropic-api-key";

/// Manages Claude API key storage in system keyring
pub struct ApiKeyManager;

impl ApiKeyManager {
    /// Get the API key from system keyring
    #[cfg(target_os = "macos")]
    pub fn get_api_key() -> Result<String, ClaudeError> {
        let output = std::process::Command::new("security")
            .args(["find-generic-password", "-s", SERVICE_NAME, "-a", API_KEY_ENTRY, "-w"])
            .output()
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        if output.status.success() {
            let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if key.is_empty() { Err(ClaudeError::ApiKeyNotFound) } else { Ok(key) }
        } else {
            Err(ClaudeError::ApiKeyNotFound)
        }
    }

    /// Get the API key from system keyring (non-macOS fallback using keyring crate)
    #[cfg(not(target_os = "macos"))]
    pub fn get_api_key() -> Result<String, ClaudeError> {
        let entry = keyring::Entry::new(SERVICE_NAME, API_KEY_ENTRY)
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        entry.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => ClaudeError::ApiKeyNotFound,
            _ => ClaudeError::KeyringError(e.to_string()),
        })
    }

    /// Store the API key in system keyring
    #[cfg(target_os = "macos")]
    pub fn set_api_key(key: &str) -> Result<(), ClaudeError> {
        // Validate key format
        if !Self::validate_key_format(key) {
            return Err(ClaudeError::InvalidApiKey);
        }

        // Delete existing entry first (ignore errors)
        let _ = Self::delete_api_key();

        let output = std::process::Command::new("security")
            .args(["add-generic-password", "-s", SERVICE_NAME, "-a", API_KEY_ENTRY, "-w", key])
            .output()
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ClaudeError::KeyringError(format!("Failed to store key: {}", stderr)))
        }
    }

    /// Store the API key in system keyring (non-macOS fallback)
    #[cfg(not(target_os = "macos"))]
    pub fn set_api_key(key: &str) -> Result<(), ClaudeError> {
        if !Self::validate_key_format(key) {
            return Err(ClaudeError::InvalidApiKey);
        }

        let entry = keyring::Entry::new(SERVICE_NAME, API_KEY_ENTRY)
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        entry.set_password(key).map_err(|e| ClaudeError::KeyringError(e.to_string()))
    }

    /// Check if an API key is stored
    pub fn has_api_key() -> bool {
        Self::get_api_key().is_ok()
    }

    /// Delete the stored API key
    #[cfg(target_os = "macos")]
    pub fn delete_api_key() -> Result<(), ClaudeError> {
        let output = std::process::Command::new("security")
            .args(["delete-generic-password", "-s", SERVICE_NAME, "-a", API_KEY_ENTRY])
            .output()
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            // Item not found is not an error for delete
            Ok(())
        }
    }

    /// Delete the stored API key (non-macOS fallback)
    #[cfg(not(target_os = "macos"))]
    pub fn delete_api_key() -> Result<(), ClaudeError> {
        let entry = keyring::Entry::new(SERVICE_NAME, API_KEY_ENTRY)
            .map_err(|e| ClaudeError::KeyringError(e.to_string()))?;

        entry.delete_credential().map_err(|e| ClaudeError::KeyringError(e.to_string()))
    }

    /// Validate API key format
    fn validate_key_format(key: &str) -> bool {
        // Basic validation - must be non-empty and reasonable length
        // Let the API do the actual validation
        !key.trim().is_empty() && key.len() >= 10
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
        assert!(ApiKeyManager::validate_key_format("some-other-key-format-12345"));
        assert!(!ApiKeyManager::validate_key_format("short"));
        assert!(!ApiKeyManager::validate_key_format(""));
        assert!(!ApiKeyManager::validate_key_format("   "));
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
