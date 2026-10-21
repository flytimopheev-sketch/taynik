//! Криптографический модуль: вывод ключа Argon2id и шифрование AES-256-GCM.
//!
//! Формат зашифрованного значения: `nonce(12) || ciphertext || tag(16)`.
//! С версии формата 2 к каждому полю привязывается AAD — имя поля.
//! Это закрывает атаку подмены: злоумышленник, получивший доступ к файлу
//! базы, не может переставить шифротексты между столбцами (например, подставить
//! шифротекст пароля в столбец «название»). Старые базы (формат 1) читаются
//! без AAD и мигрируются при первом успешном открытии.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params as A2Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

/// Размер соли Argon2, байт.
pub const SALT_LEN: usize = 32;
/// Размер nonce AES-GCM, байт.
pub const NONCE_LEN: usize = 12;
/// Размер ключа, байт.
pub const KEY_LEN: usize = 32;
/// Размер GCM-тега, байт.
pub const TAG_LEN: usize = 16;

/// AAD, привязывающий шифротекст к конкретному полю таблицы `entries`.
pub mod aad {
    pub const TITLE: &str = "title";
    pub const USERNAME: &str = "username";
    pub const PASSWORD: &str = "password";
    pub const URL: &str = "url";
    pub const NOTES: &str = "notes";
    pub const TAGS: &str = "tags";
    pub const GROUP: &str = "group";
    pub const COLOR: &str = "color";
    pub const TOTP: &str = "totp";
    pub const TOTP_PARAMS: &str = "totp_params";
    pub const HISTORY: &str = "entry_history.password";
    pub const VERIFIER: &str = "meta.verifier";
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Ключ шифрования, очищается из памяти при удалении.
///
/// Поле намеренно приватное: иначе ключ можно случайно вывести в лог
/// или показать в диалоге.
pub struct CryptoKey([u8; KEY_LEN]);

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

/// Вывести 32-байтовый ключ из мастер-пароля через Argon2id.
pub fn derive_key(
    master_password: &str,
    salt: &[u8],
    params: &ArgonParams,
) -> Result<CryptoKey, String> {
    let a2params = A2Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        None,
    )
    .map_err(|e| format!("Некорректные параметры Argon2: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, a2params);
    let mut key = [0u8; KEY_LEN];
    // Копия пароля обнуляется сразу после вывода ключа.
    let secret = Zeroizing::new(master_password.as_bytes().to_vec());
    argon
        .hash_password_into(&secret, salt, &mut key)
        .map_err(|e| format!("Ошибка вывода ключа: {e}"))?;
    Ok(CryptoKey(key))
}

/// Шифрование: результат = nonce(12) || ciphertext || tag.
pub fn encrypt(key: &CryptoKey, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    encrypt_aad(key, plaintext, b"")
}

/// Шифрование с привязкой к контексту (AAD).
pub fn encrypt_aad(key: &CryptoKey, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_slice()));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|e| format!("Ошибка шифрования: {e}"))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Расшифровка blob вида nonce || ciphertext || tag.
pub fn decrypt(key: &CryptoKey, blob: &[u8]) -> Result<Vec<u8>, String> {
    decrypt_aad(key, blob, b"")
}

/// Расшифровка blob с проверкой AAD.
pub fn decrypt_aad(key: &CryptoKey, blob: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < NONCE_LEN + TAG_LEN {
        return Err("Повреждённые данные (слишком короткий blob)".into());
    }
    let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_slice()));
    cipher
        .decrypt(Nonce::from_slice(nonce_bytes), Payload { msg: ct, aad })
        .map_err(|_| "Неверный мастер-пароль или повреждённые данные".to_string())
}

/// Зашифровать строку.
pub fn encrypt_str(key: &CryptoKey, s: &str) -> Result<Vec<u8>, String> {
    encrypt(key, s.as_bytes())
}

/// Зашифровать строку с привязкой к контексту.
pub fn encrypt_str_aad(key: &CryptoKey, s: &str, aad: &str) -> Result<Vec<u8>, String> {
    encrypt_aad(key, s.as_bytes(), aad.as_bytes())
}

/// Расшифровать строку.
pub fn decrypt_str(key: &CryptoKey, blob: &[u8]) -> Result<String, String> {
    let bytes = decrypt(key, blob)?;
    String::from_utf8(bytes).map_err(|_| "Нескорректная кодировка данных".into())
}

/// Расшифровать строку с проверкой AAD.
pub fn decrypt_str_aad(key: &CryptoKey, blob: &[u8], aad: &str) -> Result<String, String> {
    let bytes = decrypt_aad(key, blob, aad.as_bytes())?;
    String::from_utf8(bytes).map_err(|_| "Нескорректная кодировка данных".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> CryptoKey {
        CryptoKey([7u8; KEY_LEN])
    }

    #[test]
    fn roundtrip_with_aad() {
        let k = key();
        let blob = encrypt_str_aad(&k, "секрет", aad::PASSWORD).unwrap();
        assert_eq!(decrypt_str_aad(&k, &blob, aad::PASSWORD).unwrap(), "секрет");
    }

    #[test]
    fn wrong_aad_is_rejected() {
        let k = key();
        let blob = encrypt_str_aad(&k, "секрет", aad::PASSWORD).unwrap();
        // Тот же blob, но объявленный как другое поле — расшифровка обязана провалиться.
        assert!(decrypt_str_aad(&k, &blob, aad::TITLE).is_err());
    }

    #[test]
    fn nonce_is_unique_per_field() {
        let k = key();
        let a = encrypt_str_aad(&k, "одинаковые данные", aad::PASSWORD).unwrap();
        let b = encrypt_str_aad(&k, "одинаковые данные", aad::PASSWORD).unwrap();
        assert_ne!(a, b, "nonce должен быть уникальным для каждого шифрования");
    }

    #[test]
    fn short_blob_is_rejected() {
        let k = key();
        assert!(decrypt_aad(&k, &[0u8; 8], b"").is_err());
    }

    #[test]
    fn derive_key_is_deterministic() {
        let salt = [1u8; SALT_LEN];
        let p = ArgonParams::default();
        let a = derive_key("мастер-пароль", &salt, &p).unwrap();
        let b = derive_key("мастер-пароль", &salt, &p).unwrap();
        let c = derive_key("другой", &salt, &p).unwrap();
        assert_eq!(a.as_slice(), b.as_slice());
        assert_ne!(a.as_slice(), c.as_slice());
    }
}
