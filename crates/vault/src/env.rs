//! Environment variable vault — reads API keys from env vars.
//!
//! Supports `env:VARIABLE_NAME` and `keyring:service` prefixes
//! in forge.toml configuration values.
//!
//! Priority order:
//! 1. `env:VAR_NAME` → reads from environment variable
//! 2. `keyring:service` → reads from OS keychain (not yet implemented)
//! 3. Plain string → treated as the literal API key (WARNING: insecure)

use std::collections::HashMap;

/// Environment variable vault — resolves API key references.
pub struct EnvVault {
    /// Cached key lookups.
    cache: HashMap<String, Option<String>>,
}

impl EnvVault {
    /// Create a new env vault.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Resolve a provider API key from its forge.toml value.
    ///
    /// Supports:
    /// - `"env:VAR_NAME"` → reads `VAR_NAME` from environment
    /// - `"keyring:service"` → reads from OS keychain (stub)
    /// - `"sk-xxxxx"` → literal key (insecure, logs warning)
    pub fn resolve_api_key(&mut self, raw_value: &str) -> Result<Option<String>, VaultError> {
        // Check cache first
        if let Some(cached) = self.cache.get(raw_value) {
            return Ok(cached.clone());
        }

        let resolved = if let Some(var_name) = raw_value.strip_prefix("env:") {
            // Read from environment variable
            match std::env::var(var_name) {
                Ok(value) => {
                    if value.is_empty() {
                        tracing::warn!("Environment variable {} is empty", var_name);
                        None
                    } else {
                        tracing::debug!("Resolved API key from env:{}", var_name);
                        Some(value)
                    }
                }
                Err(std::env::VarError::NotPresent) => {
                    tracing::warn!(
                        "Environment variable {} not set. \
                         Set it in your shell or .env file.",
                        var_name
                    );
                    None
                }
                Err(e) => {
                    return Err(VaultError::EnvReadError {
                        variable: var_name.to_string(),
                        reason: e.to_string(),
                    });
                }
            }
        } else if let Some(_service) = raw_value.strip_prefix("keyring:") {
            // Keyring lookup — not yet implemented
            tracing::debug!("Keyring lookup not yet implemented, skipping");
            None
        } else {
            // Literal key — insecure, warn
            if raw_value.starts_with("sk-") || raw_value.starts_with("xai-") {
                tracing::warn!(
                    "⚠️  API key appears to be a plaintext secret in config. \
                     Consider using env:VARIABLE_NAME instead."
                );
            }
            Some(raw_value.to_string())
        };

        self.cache.insert(raw_value.to_string(), resolved.clone());
        Ok(resolved)
    }

    /// Resolve multiple provider API keys from a config map.
    pub fn resolve_all(
        &mut self,
        providers: &HashMap<String, String>,
    ) -> HashMap<String, Option<String>> {
        providers
            .iter()
            .map(|(name, raw_key)| {
                let resolved = self.resolve_api_key(raw_key)
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to resolve API key for {}: {}", name, e);
                        None
                    });
                (name.clone(), resolved)
            })
            .collect()
    }

    /// Clear the cache (e.g., after config reload).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Load a `.env` file (simple key=value parser, no external deps).
    pub fn load_dotenv(path: &std::path::Path) -> Result<u32, VaultError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| VaultError::FileReadError {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;

        let mut loaded = 0u32;
        for line in content.lines() {
            let line = line.trim();
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Parse KEY=VALUE
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                // Remove surrounding quotes
                let value = value
                    .strip_prefix('"').and_then(|v| v.strip_suffix('"'))
                    .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                    .unwrap_or(value);

                // Only set if not already in env (don't override existing)
                if std::env::var(key).is_err() {
                    // SAFETY: This is called during startup/init, not in concurrent contexts
                    // where data races could occur. The dotenv file is read once at startup.
                    unsafe { std::env::set_var(key, value) };
                    loaded += 1;
                }
            }
        }

        tracing::info!("Loaded {} variables from {}", loaded, path.display());
        Ok(loaded)
    }
}

impl Default for EnvVault {
    fn default() -> Self {
        Self::new()
    }
}

/// Vault error types.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Failed to read environment variable {variable}: {reason}")]
    EnvReadError { variable: String, reason: String },

    #[error("Failed to read file {path}: {reason}")]
    FileReadError { path: String, reason: String },

    #[error("Keyring not available: {0}")]
    KeyringUnavailable(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_variable() {
        unsafe { std::env::set_var("TEST_API_KEY_XYZZY", "test-secret-123") };
        let mut vault = EnvVault::new();
        let result = vault.resolve_api_key("env:TEST_API_KEY_XYZZY").unwrap();
        assert_eq!(result, Some("test-secret-123".to_string()));
    }

    #[test]
    fn test_resolve_missing_env_variable() {
        let mut vault = EnvVault::new();
        let result = vault.resolve_api_key("env:NONEXISTENT_VAR_ABC123").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_literal_key() {
        let mut vault = EnvVault::new();
        let result = vault.resolve_api_key("my-custom-key").unwrap();
        assert_eq!(result, Some("my-custom-key".to_string()));
    }

    #[test]
    fn test_resolve_caching() {
        unsafe { std::env::set_var("CACHED_KEY_TEST_789", "cached-value") };
        let mut vault = EnvVault::new();
        let r1 = vault.resolve_api_key("env:CACHED_KEY_TEST_789").unwrap();
        let r2 = vault.resolve_api_key("env:CACHED_KEY_TEST_789").unwrap();
        assert_eq!(r1, r2);
        assert_eq!(vault.cache.len(), 1);
    }

    #[test]
    fn test_load_dotenv() {
        let dir = std::env::temp_dir().join(format!("vault-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let env_path = dir.join(".env");
        std::fs::write(&env_path, "# Comment\nTEST_DOTENV_KEY=hello123\nOTHER_KEY=\"quoted\"\n").unwrap();

        let loaded = EnvVault::load_dotenv(&env_path).unwrap();
        assert!(loaded >= 1);
        assert_eq!(std::env::var("TEST_DOTENV_KEY").unwrap(), "hello123");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_resolve_all() {
        unsafe { std::env::set_var("RESOLVE_ALL_TEST_KEY", "resolved") };
        let mut providers = HashMap::new();
        providers.insert("test".to_string(), "env:RESOLVE_ALL_TEST_KEY".to_string());
        providers.insert("other".to_string(), "literal-key".to_string());

        let mut vault = EnvVault::new();
        let results = vault.resolve_all(&providers);
        assert_eq!(results.get("test").unwrap(), &Some("resolved".to_string()));
        assert_eq!(results.get("other").unwrap(), &Some("literal-key".to_string()));
    }
}
