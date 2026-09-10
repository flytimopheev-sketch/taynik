//! Криптографический модуль: вывод ключа Argon2id и шифрование AES-256-GCM.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params as A2Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Размер соли Argon2id, байт.
pub const SALT_LEN: usize = 32;
/// Размер nonce AES-GCM, байт.
pub const NONCE_LEN: usize = 12;
/// Размер ключа, байт.
pub const KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgonParams {
    /// Память в КиБ (64 MiB = 65536).
    pub memory_kib: u32,
    /// Число итераций.
    pub iterations: u32,
    /// Степень параллелизма.
    pub parallelism: u32,
}

impl Default for ArgonParams {
    fn default() -> Self {
        Self {
            memory_kib: 65536,
            iterations: 3,
            parallelism: 4,
        }
    }
}

/// Ключ шифрования, очищается в памяти при удалении.
pub struct CryptoKey(pub [u8; KEY_LEN]);

impl CryptoKey {
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for CryptoKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Сгенерировать случайную соль.
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Вывод 32-байтового ключа из мастер-пароля через Argon2id.
pub fn derive_key(
    master_password: &str,
    salt: &[u8],
    params: &ArgonParams,
) -> Result<CryptoKey, String> {
    let a2params = A2Params::new(params.memory_kib, params.iterations, params.parallelism, None)
        .map_err(|e| format!("Некорректные параметры Argon2: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, a2params);
    let mut key = [0u8; KEY_LEN];
    argon
        .hash_password_into(master_password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Ошибка вывода ключа: {e}"))?;
    Ok(CryptoKey(key))
}

/// Шифрование: результат = nonce(12) || ciphertext || tag.
pub fn encrypt(key: &CryptoKey, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_slice()));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, Payload { msg: plaintext, aad: &[] })
        .map_err(|e| format!("Ошибка шифрования: {e}"))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Расшифровка blob вида nonce || ciphertext || tag.
pub fn decrypt(key: &CryptoKey, blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < NONCE_LEN + 16 {
        return Err("Повреждённые данные (слишком короткий blob)".into());
    }
    let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_slice()));
    cipher
        .decrypt(Nonce::from_slice(nonce_bytes), Payload { msg: ct, aad: &[] })
        .map_err(|_| "Неверный мастер-пароль или повреждённые данные".to_string())
}

/// Зашифровать строку.
pub fn encrypt_str(key: &CryptoKey, s: &str) -> Result<Vec<u8>, String> {
    encrypt(key, s.as_bytes())
}

/// Расшифровать строку.
pub fn decrypt_str(key: &CryptoKey, blob: &[u8]) -> Result<String, String> {
    let bytes = decrypt(key, blob)?;
    String::from_utf8(bytes).map_err(|_| "Некорректная кодировка данных".into())
}
