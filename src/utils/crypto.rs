//! Encrypts secrets (a church's Paynow key) before they are stored. AES-256-GCM, random nonce.

use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce, aead::{Aead, AeadCore, OsRng}};
use base64::{Engine, engine::general_purpose::STANDARD};

use crate::errors::{AppError, AppResult};

pub fn parse_key(b64: &str) -> Result<[u8; 32], String> {
    let bytes = STANDARD.decode(b64.trim()).map_err(|_| "PAYMENTS_ENCRYPTION_KEY must be base64".to_string())?;
    bytes.try_into().map_err(|_| "PAYMENTS_ENCRYPTION_KEY must be 32 bytes (openssl rand -base64 32)".to_string())
}

pub fn encrypt(key: &[u8; 32], plain: &str) -> AppResult<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let mut out = nonce.to_vec();
    out.extend(cipher.encrypt(&nonce, plain.as_bytes()).map_err(|_| AppError::Internal("encrypt failed".into()))?);
    Ok(STANDARD.encode(out))
}

pub fn decrypt(key: &[u8; 32], stored: &str) -> AppResult<String> {
    let raw = STANDARD.decode(stored).map_err(|_| AppError::Internal("bad stored secret".into()))?;
    if raw.len() < 13 {
        return Err(AppError::Internal("bad stored secret".into()));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plain = cipher
        .decrypt(Nonce::from_slice(&raw[..12]), &raw[12..])
        .map_err(|_| AppError::Internal("could not decrypt stored secret (wrong PAYMENTS_ENCRYPTION_KEY?)".into()))?;
    String::from_utf8(plain).map_err(|_| AppError::Internal("bad stored secret".into()))
}
