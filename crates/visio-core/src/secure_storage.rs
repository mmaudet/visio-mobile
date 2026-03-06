//! Cross-platform secure storage abstraction.
//!
//! Provides secure credential storage using platform-native mechanisms:
//! - iOS: Keychain (via C FFI)
//! - Android: EncryptedSharedPreferences (via JNI)
//! - Desktop: keyring crate (libsecret on Linux, Keychain on macOS, Credential Manager on Windows)

use crate::errors::VisioError;
use std::collections::HashMap;
use std::sync::RwLock;

/// Trait for secure storage backends.
/// Implementations are platform-specific.
pub trait SecureStorage: Send + Sync {
    /// Store a value securely.
    fn store(&self, key: &str, value: &str) -> Result<(), VisioError>;
    /// Retrieve a stored value.
    fn retrieve(&self, key: &str) -> Result<Option<String>, VisioError>;
    /// Delete a stored value.
    fn delete(&self, key: &str) -> Result<(), VisioError>;
}

/// In-memory storage for testing or platforms without secure storage.
#[derive(Default)]
pub struct MemoryStorage {
    data: RwLock<HashMap<String, String>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecureStorage for MemoryStorage {
    fn store(&self, key: &str, value: &str) -> Result<(), VisioError> {
        self.data
            .write()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, VisioError> {
        Ok(self
            .data
            .read()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?
            .get(key)
            .cloned())
    }

    fn delete(&self, key: &str) -> Result<(), VisioError> {
        self.data
            .write()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?
            .remove(key);
        Ok(())
    }
}

/// Desktop secure storage using the keyring crate.
/// Uses platform-native storage: libsecret (Linux), Keychain (macOS), Credential Manager (Windows).
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct KeyringStorage {
    service_name: String,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl KeyringStorage {
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl SecureStorage for KeyringStorage {
    fn store(&self, key: &str, value: &str) -> Result<(), VisioError> {
        let entry = keyring::Entry::new(&self.service_name, key)
            .map_err(|e| VisioError::Auth(format!("keyring error: {e}")))?;
        entry
            .set_password(value)
            .map_err(|e| VisioError::Auth(format!("keyring store error: {e}")))
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, VisioError> {
        let entry = keyring::Entry::new(&self.service_name, key)
            .map_err(|e| VisioError::Auth(format!("keyring error: {e}")))?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(VisioError::Auth(format!("keyring retrieve error: {e}"))),
        }
    }

    fn delete(&self, key: &str) -> Result<(), VisioError> {
        let entry = keyring::Entry::new(&self.service_name, key)
            .map_err(|e| VisioError::Auth(format!("keyring error: {e}")))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
            Err(e) => Err(VisioError::Auth(format!("keyring delete error: {e}"))),
        }
    }
}

/// Callback-based storage for mobile platforms.
/// The actual storage is implemented in Swift (iOS) or Kotlin (Android),
/// and called via FFI callbacks.
pub struct CallbackStorage {
    store_fn: Box<dyn Fn(&str, &str) -> Result<(), String> + Send + Sync>,
    retrieve_fn: Box<dyn Fn(&str) -> Result<Option<String>, String> + Send + Sync>,
    delete_fn: Box<dyn Fn(&str) -> Result<(), String> + Send + Sync>,
}

impl CallbackStorage {
    pub fn new<S, R, D>(store: S, retrieve: R, delete: D) -> Self
    where
        S: Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static,
        R: Fn(&str) -> Result<Option<String>, String> + Send + Sync + 'static,
        D: Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    {
        Self {
            store_fn: Box::new(store),
            retrieve_fn: Box::new(retrieve),
            delete_fn: Box::new(delete),
        }
    }
}

impl SecureStorage for CallbackStorage {
    fn store(&self, key: &str, value: &str) -> Result<(), VisioError> {
        (self.store_fn)(key, value).map_err(|e| VisioError::Auth(format!("storage error: {e}")))
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, VisioError> {
        (self.retrieve_fn)(key).map_err(|e| VisioError::Auth(format!("storage error: {e}")))
    }

    fn delete(&self, key: &str) -> Result<(), VisioError> {
        (self.delete_fn)(key).map_err(|e| VisioError::Auth(format!("storage error: {e}")))
    }
}

/// Create the default secure storage for the current platform.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn create_default_storage() -> Box<dyn SecureStorage> {
    Box::new(KeyringStorage::new("io.visio.mobile"))
}

/// For mobile platforms, storage must be provided via callbacks.
/// This function returns a memory storage as a fallback.
#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn create_default_storage() -> Box<dyn SecureStorage> {
    // Mobile platforms should use set_secure_storage() to provide platform-specific storage
    Box::new(MemoryStorage::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_storage_operations() {
        let storage = MemoryStorage::new();

        // Store and retrieve
        storage.store("test_key", "test_value").unwrap();
        assert_eq!(
            storage.retrieve("test_key").unwrap(),
            Some("test_value".to_string())
        );

        // Delete
        storage.delete("test_key").unwrap();
        assert_eq!(storage.retrieve("test_key").unwrap(), None);

        // Delete non-existent key (should not error)
        storage.delete("non_existent").unwrap();
    }

    #[test]
    fn test_callback_storage() {
        use std::sync::{Arc, Mutex};

        let data: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

        let data_store = data.clone();
        let data_retrieve = data.clone();
        let data_delete = data.clone();

        let storage = CallbackStorage::new(
            move |key, value| {
                data_store.lock().unwrap().insert(key.to_string(), value.to_string());
                Ok(())
            },
            move |key| Ok(data_retrieve.lock().unwrap().get(key).cloned()),
            move |key| {
                data_delete.lock().unwrap().remove(key);
                Ok(())
            },
        );

        storage.store("key", "value").unwrap();
        assert_eq!(storage.retrieve("key").unwrap(), Some("value".to_string()));
        storage.delete("key").unwrap();
        assert_eq!(storage.retrieve("key").unwrap(), None);
    }
}
