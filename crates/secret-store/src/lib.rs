use std::{collections::BTreeMap, sync::Mutex};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

const SERVICE_NAME: &str = "io.nimora.desktop";
const MAX_REFERENCE_BYTES: usize = 160;
const MAX_SECRET_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SecretReference(String);

impl SecretReference {
    /// Parses a stable, non-secret credential reference.
    ///
    /// # Errors
    ///
    /// Returns an error when the reference is not a bounded namespaced identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, SecretStoreError> {
        let value = value.into();
        if value.len() > MAX_REFERENCE_BYTES
            || !value.starts_with("secret:")
            || value.split(':').count() < 3
            || value.split(':').any(|segment| {
                segment.is_empty()
                    || segment.len() > 64
                    || !segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'-' | b'_' | b'.')
                    })
            })
        {
            return Err(SecretStoreError::InvalidReference);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretPresence {
    Present,
    Missing,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretStoreError {
    #[error("secret reference is invalid")]
    InvalidReference,
    #[error("secret value is invalid")]
    InvalidSecret,
    #[error("secret is not present")]
    Missing,
    #[error("system secret store is unavailable")]
    Unavailable,
}

pub trait SecretStore: Send + Sync {
    /// Stores a secret under a non-secret reference.
    ///
    /// # Errors
    ///
    /// Returns an error if the value is empty, oversized, or the backend is unavailable.
    fn put(
        &self,
        reference: &SecretReference,
        secret: Zeroizing<String>,
    ) -> Result<(), SecretStoreError>;

    /// Resolves a secret into zeroizing memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the reference is missing or the backend is unavailable.
    fn resolve(&self, reference: &SecretReference) -> Result<Zeroizing<String>, SecretStoreError>;

    /// Removes a secret. Removing an absent reference is idempotent.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend is unavailable.
    fn delete(&self, reference: &SecretReference) -> Result<(), SecretStoreError>;

    /// Checks presence without exposing the secret.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend is unavailable.
    fn presence(&self, reference: &SecretReference) -> Result<SecretPresence, SecretStoreError>;
}

#[derive(Debug, Default)]
pub struct SystemSecretStore;

impl SystemSecretStore {
    fn entry(reference: &SecretReference) -> Result<keyring::Entry, SecretStoreError> {
        keyring::Entry::new(SERVICE_NAME, reference.as_str())
            .map_err(|_| SecretStoreError::Unavailable)
    }
}

impl SecretStore for SystemSecretStore {
    fn put(
        &self,
        reference: &SecretReference,
        secret: Zeroizing<String>,
    ) -> Result<(), SecretStoreError> {
        validate_secret(&secret)?;
        Self::entry(reference)?
            .set_password(&secret)
            .map_err(|_| SecretStoreError::Unavailable)
    }

    fn resolve(&self, reference: &SecretReference) -> Result<Zeroizing<String>, SecretStoreError> {
        let secret = Self::entry(reference)?
            .get_password()
            .map_err(|error| map_keyring_error(&error))?;
        validate_secret(&secret)?;
        Ok(Zeroizing::new(secret))
    }

    fn delete(&self, reference: &SecretReference) -> Result<(), SecretStoreError> {
        match Self::entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SecretStoreError::Unavailable),
        }
    }

    fn presence(&self, reference: &SecretReference) -> Result<SecretPresence, SecretStoreError> {
        match Self::entry(reference)?.get_password() {
            Ok(mut secret) => {
                secret.zeroize();
                Ok(SecretPresence::Present)
            }
            Err(keyring::Error::NoEntry) => Ok(SecretPresence::Missing),
            Err(_) => Err(SecretStoreError::Unavailable),
        }
    }
}

#[derive(Debug, Default)]
pub struct MemorySecretStore {
    values: Mutex<BTreeMap<SecretReference, Zeroizing<String>>>,
}

impl SecretStore for MemorySecretStore {
    fn put(
        &self,
        reference: &SecretReference,
        secret: Zeroizing<String>,
    ) -> Result<(), SecretStoreError> {
        validate_secret(&secret)?;
        self.values
            .lock()
            .map_err(|_| SecretStoreError::Unavailable)?
            .insert(reference.clone(), secret);
        Ok(())
    }

    fn resolve(&self, reference: &SecretReference) -> Result<Zeroizing<String>, SecretStoreError> {
        self.values
            .lock()
            .map_err(|_| SecretStoreError::Unavailable)?
            .get(reference)
            .map(|secret| Zeroizing::new(secret.to_string()))
            .ok_or(SecretStoreError::Missing)
    }

    fn delete(&self, reference: &SecretReference) -> Result<(), SecretStoreError> {
        self.values
            .lock()
            .map_err(|_| SecretStoreError::Unavailable)?
            .remove(reference);
        Ok(())
    }

    fn presence(&self, reference: &SecretReference) -> Result<SecretPresence, SecretStoreError> {
        Ok(
            if self
                .values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .contains_key(reference)
            {
                SecretPresence::Present
            } else {
                SecretPresence::Missing
            },
        )
    }
}

fn validate_secret(secret: &str) -> Result<(), SecretStoreError> {
    if secret.is_empty() || secret.len() > MAX_SECRET_BYTES || secret.contains('\0') {
        return Err(SecretStoreError::InvalidSecret);
    }
    Ok(())
}

fn map_keyring_error(error: &keyring::Error) -> SecretStoreError {
    if matches!(error, keyring::Error::NoEntry) {
        SecretStoreError::Missing
    } else {
        SecretStoreError::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reference_is_namespaced_and_bounded() {
        assert!(SecretReference::parse("secret:provider:openai-work").is_ok());
        for invalid in [
            "provider:key",
            "secret::key",
            "secret:Provider:key",
            "secret:provider:key/path",
        ] {
            assert_eq!(
                SecretReference::parse(invalid),
                Err(SecretStoreError::InvalidReference)
            );
        }
    }

    #[test]
    fn memory_store_never_requires_plaintext_listing() {
        let store = MemorySecretStore::default();
        let reference = SecretReference::parse("secret:provider:test").expect("reference");
        assert_eq!(store.presence(&reference), Ok(SecretPresence::Missing));
        store
            .put(&reference, Zeroizing::new("token-value".to_owned()))
            .expect("put");
        assert_eq!(store.presence(&reference), Ok(SecretPresence::Present));
        assert_eq!(
            store.resolve(&reference).expect("resolve").as_str(),
            "token-value"
        );
        store.delete(&reference).expect("delete");
        store.delete(&reference).expect("idempotent delete");
        assert_eq!(store.resolve(&reference), Err(SecretStoreError::Missing));
    }

    #[test]
    fn empty_oversized_and_nul_secrets_fail_closed() {
        let store = MemorySecretStore::default();
        let reference = SecretReference::parse("secret:provider:test").expect("reference");
        for secret in [
            String::new(),
            "contains\0nul".to_owned(),
            "x".repeat(MAX_SECRET_BYTES + 1),
        ] {
            assert_eq!(
                store.put(&reference, Zeroizing::new(secret)),
                Err(SecretStoreError::InvalidSecret)
            );
        }
    }

    #[test]
    #[ignore = "requires explicit system credential-store access"]
    fn system_store_round_trip_is_present_resolvable_and_revocable() {
        assert_eq!(
            std::env::var("NIMORA_RUN_SYSTEM_SECRET_STORE_TEST").as_deref(),
            Ok("1"),
            "set NIMORA_RUN_SYSTEM_SECRET_STORE_TEST=1 to run this destructive-cleanup test"
        );
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let reference = SecretReference::parse(format!(
            "secret:test:system-store-{}-{nonce}",
            std::process::id()
        ))
        .expect("unique reference");
        let store = SystemSecretStore;
        let result = (|| {
            assert_eq!(store.presence(&reference)?, SecretPresence::Missing);
            store.put(
                &reference,
                Zeroizing::new(format!("nimora-synthetic-test-{nonce}")),
            )?;
            assert_eq!(store.presence(&reference)?, SecretPresence::Present);
            assert_eq!(
                store.resolve(&reference)?.as_str(),
                format!("nimora-synthetic-test-{nonce}")
            );
            store.delete(&reference)?;
            assert_eq!(store.presence(&reference)?, SecretPresence::Missing);
            store.delete(&reference)
        })();
        let cleanup = store.delete(&reference);
        assert_eq!(cleanup, Ok(()), "system credential cleanup failed");
        assert_eq!(result, Ok(()), "system credential lifecycle failed");
    }
}
