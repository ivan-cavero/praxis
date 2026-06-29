//! Vault — credential and secret management.
//!
//! Supports multiple backends:
//! - `VaultService`: AES-256-GCM encrypted storage (default)
//! - `KeyringVault`: OS-native keychain fallback (legacy)
//! - `TauriStoreVault`: Tauri secure storage (legacy)
//! - `EnvVault`: environment variables (VPS/CI mode)

pub mod keyring;
pub mod tauri_store;
pub mod env;
pub mod service;

pub use service::VaultService;
pub use service::VaultError;
pub use keyring::KeyringVault;
pub use tauri_store::TauriStoreVault;
pub use env::EnvVault;
