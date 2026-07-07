//! Vault — credential and secret management.
//!
//! Supports multiple backends:
//! - `VaultService`: AES-256-GCM encrypted storage (default)
//! - `KeyringVault`: OS-native keychain fallback (legacy)
//! - `TauriStoreVault`: Tauri secure storage (legacy)
//! - `EnvVault`: environment variables (VPS/CI mode)

pub mod env;
pub mod keyring;
pub mod service;
pub mod tauri_store;

pub use env::EnvVault;
pub use keyring::KeyringVault;
pub use service::VaultError;
pub use service::VaultService;
pub use tauri_store::TauriStoreVault;
