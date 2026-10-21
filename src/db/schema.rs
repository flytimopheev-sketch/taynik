//! Схема базы данных. Все чувствительные поля хранятся зашифрованными BLOB.

/// Текущая версия схемы.
///
/// * v1 — базовые записи.
/// * v2 — группы, цвета, TOTP-секреты, история паролей.
/// * v3 — параметры TOTP (алгоритм/цифры/период) для импорта `otpauth://`.
pub const SCHEMA_VERSION: i64 = 3;

/// Формат шифрования полей.
/// 1 — без привязки к имени поля (legacy), 2 — с AAD.
pub const BLOB_FORMAT_LEGACY: u8 = 1;
pub const BLOB_FORMAT_AAD: u8 = 2;

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
    entry_group BLOB,
    color BLOB,
    totp BLOB,
    totp_params BLOB,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS entry_history (
    id INTEGER PRIMARY KEY,
    entry_id INTEGER NOT NULL,
    password BLOB NOT NULL,
    changed_at INTEGER NOT NULL
);
";

/// Миграция существующей базы до текущей версии схемы.
/// Выполняется после успешной расшифровки verifier (мастер-пароль верен).
pub const MIGRATE_V2: &str = "
CREATE TABLE IF NOT EXISTS entry_history (
    id INTEGER PRIMARY KEY,
    entry_id INTEGER NOT NULL,
    password BLOB NOT NULL,
    changed_at INTEGER NOT NULL
);
";

/// Добавление столбца параметров TOTP (v3).
pub const MIGRATE_V3: &str = "ALTER TABLE entries ADD COLUMN totp_params BLOB;";
