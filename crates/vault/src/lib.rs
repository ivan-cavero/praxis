//! Vault — credential and secret management.
//!
//! Supports multiple backends:
//! - keyring: OS-native keychain (CLI mode)
//! - tauri-store: Tauri secure storage (desktop mode)
//! - env: environment variables (VPS/CI mode)

pub mod keyring;
pub mod tauri_store;
pub mod env;

pub use keyring::KeyringVault;
pub use tauri_store::TauriStoreVault;
pub use env::EnvVault;
