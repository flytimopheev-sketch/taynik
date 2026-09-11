//! Схема базы данных. Все чувствительные поля хранятся зашифрованными BLOB.

pub const SCHEMA_VERSION: i64 = 1;

/// Открытый текст, которым проверяется корректность мастер-пароля.
pub const VERIFIER_PLAINTEXT: &str = "taynik-verifier-v1";

/// Verifier из старых баз (до переименования проекта) — для совместимости
/// при открытии существующих баз. Новые базы создаются с VERIFIER_PLAINTEXT.
pub const LEGACY_VERIFIER_PLAINTEXT: &str = "redpass-verifier-v1";

pub const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS entries (
    id INTEGER PRIMARY KEY,
    title BLOB NOT NULL,
    username BLOB,
    password BLOB NOT NULL,
    url BLOB,
    notes BLOB,
    tags BLOB,
    favorite INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
";
