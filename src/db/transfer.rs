//! Зашифрованный экспорт и импорт записей (формат `.taynik`).
//!
//! В отличие от CSV, где пароли лежат открытым текстом, файл `.taynik`
//! содержит только шифротекст: ключ выводится из пароля пользователя через
//! Argon2id, данные шифруются AES-256-GCM. Такой файл можно безопасно
//! переносить на флешку или в резервную копию диска.
//!
//! ## Формат файла
//!
//! ```text
//! magic      8 байт  "TAYNKEXP"
//! version    1 байт
//! salt      32 байта
//! params    12 байт  (memory_kib u32 LE, iterations u32 LE, parallelism u32 LE)
//! nonce     12 байт
//! length     4 байта (u32 LE) — длина шифротекста
//! ciphertext  AES-256-GCM(nonce || ciphertext || tag)
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::crypto::{self, ArgonParams, CryptoKey, NONCE_LEN, SALT_LEN};
use crate::models::entry::Entry;
use crate::models::totp::TotpParams;

const MAGIC: &[u8; 8] = b"TAYNKEXP";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 8 + 1 + SALT_LEN + 12 + NONCE_LEN + 4;

/// Запись внутри экспортированного файла.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportEntry {
    pub title: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub totp_secret: String,
    #[serde(default)]
    pub totp_params: TotpParams,
}

/// Содержимое экспортированного файла (до шифрования).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payload {
    app: String,
    version: u32,
    created_at: i64,
    entries: Vec<ExportEntry>,
}

impl Entry {
    fn to_export(&self) -> ExportEntry {
        ExportEntry {
            title: self.title.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            url: self.url.clone(),
            notes: self.notes.clone(),
            tags: self.tags.clone(),
            favorite: self.favorite,
            group: self.group.clone(),
            color: self.color.clone(),
            totp_secret: self.totp_secret.clone(),
            totp_params: self.totp_params,
        }
    }

    fn from_export(e: ExportEntry) -> Entry {
        Entry {
            id: None,
            title: e.title,
            username: e.username,
            password: e.password,
            url: e.url,
            notes: e.notes,
            tags: e.tags,
            favorite: e.favorite,
            group: e.group,
            color: e.color,
            totp_secret: e.totp_secret,
            totp_params: e.totp_params,
            created_at: 0,
            updated_at: 0,
        }
    }
}

/// Экспортировать записи в зашифрованный файл.
/// Возвращает число экспортированных записей.
pub fn export(path: &Path, entries: &[Entry], passphrase: &str) -> Result<usize, String> {
    if passphrase.is_empty() {
        return Err("Пароль для шифрования файла не может быть пустым".into());
    }
    let payload = Payload {
        app: "taynik".into(),
        version: 1,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        entries: entries.iter().map(Entry::to_export).collect(),
    };
    let plain = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;

    let params = ArgonParams::default();
    let salt = crypto::generate_salt();
    let key = crypto::derive_key(passphrase, &salt, &params)?;

    let mut nonce = [0u8; NONCE_LEN];
    {
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut nonce);
    }
    let ct = crypto::encrypt_aad(&key, &plain, b"taynik.export.v1").map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(HEADER_LEN + ct.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&params.memory_kib.to_le_bytes());
    out.extend_from_slice(&params.iterations.to_le_bytes());
    out.extend_from_slice(&params.parallelism.to_le_bytes());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&(ct.len() as u32).to_le_bytes());
    out.extend_from_slice(&ct);

    std::fs::write(path, out).map_err(|e| e.to_string())?;
    Ok(entries.len())
}

/// Импортировать записи из зашифрованного файла.
pub fn import(path: &Path, passphrase: &str) -> Result<Vec<Entry>, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    if raw.len() < HEADER_LEN || &raw[..8] != MAGIC {
        return Err("файл не является экспортом Тайника (*.taynik)".into());
    }
    let version = raw[8];
    if version != VERSION {
        return Err(format!("неподдерживаемая версия файла: {version}"));
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&raw[9..9 + SALT_LEN]);
    let mut off = 9 + SALT_LEN;
    let read_u32 = |o: &mut usize| -> Result<u32, String> {
        if *o + 4 > raw.len() {
            return Err("файл повреждён: неожиданный конец".into());
        }
        let mut b = [0u8; 4];
        b.copy_from_slice(&raw[*o..*o + 4]);
        *o += 4;
        Ok(u32::from_le_bytes(b))
    };
    let memory_kib = read_u32(&mut off)?;
    let iterations = read_u32(&mut off)?;
    let parallelism = read_u32(&mut off)?;
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&raw[off..off + NONCE_LEN]);
    off += NONCE_LEN;
    let ct_len = read_u32(&mut off)? as usize;
    if raw.len() < off + ct_len {
        return Err("файл повреждён: обрезанный шифротекст".into());
    }
    let ct = &raw[off..off + ct_len];

    let params = ArgonParams {
        memory_kib,
        iterations,
        parallelism,
    };
    let key: CryptoKey = crypto::derive_key(passphrase, &salt, &params)?;
    let plain = crypto::decrypt_aad(&key, ct, b"taynik.export.v1")
        .map_err(|_| "неверный пароль файла или файл повреждён".to_string())?;
    let payload: Payload =
        serde_json::from_slice(&plain).map_err(|e| format!("не удалось разобрать файл: {e}"))?;
    if payload.app != "taynik" {
        return Err("файл создан другим приложением".into());
    }
    Ok(payload
        .entries
        .into_iter()
        .map(Entry::from_export)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Entry {
        Entry {
            title: "Сайт".into(),
            username: "user@mail".into(),
            password: "пароль, с \"кавычками\"".into(),
            url: "https://example.com".into(),
            notes: "строка1\nстрока2".into(),
            tags: vec!["работа".into(), "почта".into()],
            group: "Личное".into(),
            color: "#e74c3c".into(),
            totp_secret: "GEZDGNBVGY3TQOJQ".into(),
            totp_params: TotpParams::default(),
            favorite: true,
            ..Default::default()
        }
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("taynik_transfer_tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn roundtrip() {
        let path = temp_path("export.taynik");
        let e = sample();
        export(&path, std::slice::from_ref(&e), "пароль-файла").unwrap();
        let parsed = import(&path, "пароль-файла").unwrap();
        assert_eq!(parsed.len(), 1);
        let p = &parsed[0];
        assert_eq!(p.title, e.title);
        assert_eq!(p.username, e.username);
        assert_eq!(p.password, e.password);
        assert_eq!(p.notes, e.notes);
        assert_eq!(p.tags, e.tags);
        assert_eq!(p.group, e.group);
        assert_eq!(p.color, e.color);
        assert_eq!(p.totp_secret, e.totp_secret);
        assert!(p.favorite);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wrong_passphrase_is_rejected() {
        let path = temp_path("export_bad.taynik");
        export(&path, &[sample()], "верный").unwrap();
        assert!(import(&path, "неверный").is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_contains_no_plaintext_secrets() {
        let path = temp_path("export_plain.taynik");
        let e = sample();
        export(&path, std::slice::from_ref(&e), "пароль-файла").unwrap();
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw
            .windows(e.password.len())
            .any(|w| w == e.password.as_bytes()));
        assert!(!raw.windows(e.title.len()).any(|w| w == e.title.as_bytes()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_foreign_files() {
        let path = temp_path("not_taynik.bin");
        std::fs::write(&path, b"just some bytes, definitely not a vault").unwrap();
        assert!(import(&path, "x").is_err());
        let _ = std::fs::remove_file(&path);
    }
}
