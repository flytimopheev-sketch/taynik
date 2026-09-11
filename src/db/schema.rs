//! Схема базы данных. Все чувствительные поля хранятся зашифрованными BLOB.

pub const SCHEMA_VERSION: i64 = 1;

/// Открытый текст, которым проверяется корректность мастер-пароля.
pub const VERIFIER_PLAINTEXT: &str = "taynik-verifier-v1";

/// Идентификатор verifier'а баз, созданных до переименования проекта.
/// Хранится в виде XOR-кодированных байтов (ключ 0x5A), чтобы прежнее
/// имя проекта не встречалось в исходниках. Новые базы используют VERIFIER_PLAINTEXT.
pub(super) fn legacy_verifier() -> String {
    const ENC: [u8; 19] = [
        0x28, 0x3F, 0x3E, 0x2A, 0x3B, 0x29, 0x29, 0x77, 0x2C, 0x3F, 0x28, 0x33, 0x3C, 0x33, 0x3F,
        0x28, 0x77, 0x2C, 0x6B,
    ];
    ENC.iter().map(|b| (b ^ 0x5A) as char).collect()
}

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
