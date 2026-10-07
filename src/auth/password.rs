use std::sync::OnceLock;

use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use phc::PasswordHash;


#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("stored password hash is malformed")]
    MalformedHash(String),
    #[error("password verification failed")]
    Verify(String),
    #[error("password task panicked")]
    Join(#[from] tokio::task::JoinError)
}

pub async fn verify(hash: String, password: String, pepper: Option<String>) -> Result<bool, PasswordError> {
    tokio::task::spawn_blocking(move || verify_blocking(&hash, &password, pepper.as_ref()))
    .await
    .inspect_err(|e| tracing::error!("tokio join error: {e}"))?
}

/// Used to prevent timing attacks: still hash-check something,
/// but discard the result after
pub async fn verify_or_dummy(hash: Option<String>, password: String, pepper: Option<String>) -> Result<bool, PasswordError> {
    tokio::task::spawn_blocking(move || {
        let exists = hash.is_some();
        let hash = hash.as_deref().unwrap_or_else(|| dummy_hash(pepper.as_ref()));

        Ok(verify_blocking(hash, &password, pepper.as_ref())? && exists)
    })
    .await
    .inspect_err(|e| tracing::error!("tokio join error: {e}"))?
}

pub fn argon2(pepper: Option<&String>) -> Argon2<'_> {
    match pepper {
        Some(pepper) => Argon2::new_with_secret(
            pepper.as_bytes(),
            Algorithm::default(),
            Version::default(),
            Params::default(),
        )
        .unwrap(), // This only fails if the secret is too long, it'll be fine!
        None => Argon2::default(),
    }
}

fn verify_blocking(hash: &str, password: &str, pepper: Option<&String>) -> Result<bool, PasswordError> {
    let parsed_hash = PasswordHash::new(hash)
        .inspect_err(|e| tracing::error!("failed to parse password hash - pretty bad: {e}"))
        .map_err(|_| PasswordError::MalformedHash(hash.into()))?;

    match argon2(pepper).verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::PasswordInvalid) => Ok(false),
        Err(e) => Err(PasswordError::Verify(e.to_string()))
    }
}

fn dummy_hash(pepper: Option<&String>) -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();

    HASH.get_or_init(|| {
        argon2(pepper)
            .hash_password(b"soggymod")
            .expect("hashing a static str can't fail")
            .to_string()
    })
}

