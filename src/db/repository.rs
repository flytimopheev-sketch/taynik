//! Репозиторий: открытие/создание зашифрованной базы и операции с записями.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use gtk4::glib;

use super::crypto::{self, aad, ArgonParams, CryptoKey};
use super::schema::{
    BLOB_FORMAT_AAD, BLOB_FORMAT_LEGACY, CREATE_SCHEMA, MIGRATE_V2, MIGRATE_V3, SCHEMA_VERSION,
    VERIFIER_PLAINTEXT,
};
use crate::models::entry::Entry;

/// Строка таблицы `entries` в виде зашифрованных блобов.
type EntryRow = (
    i64,             // id
    Vec<u8>,         // title
    Option<Vec<u8>>, // username
    Vec<u8>,         // password
    Option<Vec<u8>>, // url
    Option<Vec<u8>>, // notes
    Option<Vec<u8>>, // tags
    Option<Vec<u8>>, // entry_group
    Option<Vec<u8>>, // color
    Option<Vec<u8>>, // totp
    Option<Vec<u8>>, // totp_params
);

#[derive(Debug)]
pub enum DbError {
    WrongPassword,
    Corrupted(String),
    Crypto(String),
    Io(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::WrongPassword => write!(f, "Неверный мастер-пароль"),
            DbError::Corrupted(m) => write!(f, "Файл базы повреждён: {m}"),
            DbError::Crypto(m) => write!(f, "Ошибка шифрования: {m}"),
            DbError::Io(m) => write!(f, "Ошибка ввода-вывода: {m}"),
        }
    }
}

impl std::error::Error for DbError {}

/// Зашифровать значение поля с учётом текущего формата.
fn enc_with(key: &CryptoKey, format: u8, field: &str, value: &str) -> Result<Vec<u8>, DbError> {
    let r = if format == BLOB_FORMAT_AAD {
        crypto::encrypt_str_aad(key, value, field)
    } else {
        crypto::encrypt_str(key, value)
    };
    r.map_err(DbError::Crypto)
}

/// Расшифровать значение поля с учётом текущего формата.
fn dec_with(key: &CryptoKey, format: u8, field: &str, blob: &[u8]) -> Option<String> {
    if format == BLOB_FORMAT_AAD {
        crypto::decrypt_str_aad(key, blob, field).ok()
    } else {
        crypto::decrypt_str(key, blob).ok()
    }
}

/// Перешифровать одно поле: прочитать старым ключом и записать новым.
/// Используется при смене мастер-пароля и при переводе базы на формат с AAD.
fn re_field(
    old_key: &CryptoKey,
    old_format: u8,
    new_key: &CryptoKey,
    new_format: u8,
    field: &str,
    blob: Option<&[u8]>,
) -> Result<Vec<u8>, DbError> {
    let plain = blob
        .and_then(|b| dec_with(old_key, old_format, field, b))
        .unwrap_or_default();
    enc_with(new_key, new_format, field, &plain)
}

/// Прочитать все записи и историю паролей из транзакции.
fn read_rows(tx: &rusqlite::Transaction) -> Result<Vec<EntryRow>, DbError> {
    let mut stmt = tx
        .prepare(
            "SELECT id, title, username, password, url, notes, tags,
                    entry_group, color, totp, totp_params
             FROM entries",
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
    let mapped = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Option<Vec<u8>>>(4)?,
                row.get::<_, Option<Vec<u8>>>(5)?,
                row.get::<_, Option<Vec<u8>>>(6)?,
                row.get::<_, Option<Vec<u8>>>(7)?,
                row.get::<_, Option<Vec<u8>>>(8)?,
                row.get::<_, Option<Vec<u8>>>(9)?,
                row.get::<_, Option<Vec<u8>>>(10)?,
            ))
        })
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
    let mut out = Vec::new();
    for r in mapped {
        out.push(r.map_err(|e| DbError::Corrupted(e.to_string()))?);
    }
    Ok(out)
}

fn read_history(tx: &rusqlite::Transaction) -> Result<Vec<(i64, Vec<u8>)>, DbError> {
    let mut stmt = tx
        .prepare("SELECT id, password FROM entry_history")
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
    let mapped = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
    let mut out = Vec::new();
    for r in mapped {
        out.push(r.map_err(|e| DbError::Corrupted(e.to_string()))?);
    }
    Ok(out)
}

/// Перешифровать все записи и историю паролей одним ключом.
/// Вызывается внутри открытой транзакции: при сбое всё откатывается.
fn reencrypt_all(
    tx: &rusqlite::Transaction,
    old_key: &CryptoKey,
    old_format: u8,
    new_key: &CryptoKey,
    new_format: u8,
) -> Result<(), DbError> {
    for (id, title, username, password, url, notes, tags, group, color, totp, totp_params) in
        read_rows(tx)?
    {
        let enc_title = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::TITLE,
            Some(&title),
        )?;
        let enc_username = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::USERNAME,
            username.as_deref(),
        )?;
        let enc_password = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::PASSWORD,
            Some(&password),
        )?;
        let enc_url = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::URL,
            url.as_deref(),
        )?;
        let enc_notes = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::NOTES,
            notes.as_deref(),
        )?;
        let enc_tags = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::TAGS,
            tags.as_deref(),
        )?;
        let enc_group = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::GROUP,
            group.as_deref(),
        )?;
        let enc_color = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::COLOR,
            color.as_deref(),
        )?;
        let enc_totp = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::TOTP,
            totp.as_deref(),
        )?;
        let enc_totp_params = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::TOTP_PARAMS,
            totp_params.as_deref(),
        )?;
        tx.execute(
            "UPDATE entries SET title=?1, username=?2, password=?3, url=?4, notes=?5,
                    tags=?6, entry_group=?7, color=?8, totp=?9, totp_params=?10
             WHERE id=?11",
            params![
                enc_title,
                enc_username,
                enc_password,
                enc_url,
                enc_notes,
                enc_tags,
                enc_group,
                enc_color,
                enc_totp,
                enc_totp_params,
                id
            ],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
    }

    for (id, blob) in read_history(tx)? {
        let enc = re_field(
            old_key,
            old_format,
            new_key,
            new_format,
            aad::HISTORY,
            Some(&blob),
        )?;
        tx.execute(
            "UPDATE entry_history SET password=?1 WHERE id=?2",
            params![enc, id],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
    }
    Ok(())
}

pub struct Database {
    conn: Connection,
    key: CryptoKey,
    pub path: PathBuf,
    /// Формат шифрования полей: 1 — legacy (без AAD), 2 — с привязкой к полю.
    blob_format: u8,
}

fn meta_params(conn: &Connection) -> Result<(Vec<u8>, ArgonParams), DbError> {
    let salt: Vec<u8> = conn
        .query_row("SELECT value FROM meta WHERE key='argon2_salt'", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| DbError::Corrupted(e.to_string()))?
        .ok_or_else(|| DbError::Corrupted("отсутствует salt в meta".into()))?;
    let raw: Vec<u8> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='argon2_params'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
    let p: ArgonParams =
        serde_json::from_slice(&raw).map_err(|e| DbError::Corrupted(e.to_string()))?;
    Ok((salt, p))
}

impl Database {
    /// Создать новую зашифрованную базу.
    pub fn create(path: &Path, master_password: &str) -> Result<Self, DbError> {
        Self::create_with_params(path, master_password, &ArgonParams::default())
    }

    /// Создать новую базу с заданными параметрами Argon2id.
    ///
    /// Параметры по умолчанию (64 МиБ памяти, 3 итерации) рассчитаны на
    /// реальное использование. Облегчённые параметры нужны только тестам:
    /// полный Argon2id занимает секунды и делает набор тестов медленным.
    pub fn create_with_params(
        path: &Path,
        master_password: &str,
        params_: &ArgonParams,
    ) -> Result<Self, DbError> {
        if path.exists() {
            return Err(DbError::Io(format!(
                "файл {} уже существует",
                path.display()
            )));
        }
        let conn = Connection::open(path).map_err(|e| DbError::Io(e.to_string()))?;
        conn.execute_batch(CREATE_SCHEMA)
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        let salt = crypto::generate_salt();
        let key = crypto::derive_key(master_password, &salt, params_).map_err(DbError::Crypto)?;
        let verifier = crypto::encrypt_str_aad(&key, VERIFIER_PLAINTEXT, aad::VERIFIER)
            .map_err(DbError::Crypto)?;

        let db = Self {
            conn,
            key,
            path: path.to_path_buf(),
            blob_format: BLOB_FORMAT_AAD,
        };

        db.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('argon2_salt', ?1)",
                params![salt.to_vec()],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        db.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('argon2_params', ?1)",
                params![serde_json::to_vec(&params_).unwrap()],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        db.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('encryption_key_params', ?1)",
                params![
                    json!({"algorithm": "AES-256-GCM", "nonce_size": 12, "aad": true}).to_string()
                ],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        db.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('verifier', ?1)",
                params![verifier],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        db.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)",
                params![SCHEMA_VERSION.to_le_bytes().to_vec()],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        db.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('blob_format', ?1)",
                params![vec![BLOB_FORMAT_AAD]],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        Ok(db)
    }

    /// Открыть существующую базу с проверкой мастер-пароля.
    pub fn open(path: &Path, master_password: &str) -> Result<Self, DbError> {
        let conn = Connection::open(path).map_err(|e| DbError::Io(e.to_string()))?;
        let (salt, p) = meta_params(&conn)?;
        let key = crypto::derive_key(master_password, &salt, &p).map_err(DbError::Crypto)?;
        let blob: Vec<u8> = conn
            .query_row("SELECT value FROM meta WHERE key='verifier'", [], |r| {
                r.get(0)
            })
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        // Новые базы привязывают verifier к имени поля (AAD), старые — нет.
        // Пробуем оба варианта: формат станет известен после успеха.
        let (blob_format, verifier_ok) = match crypto::decrypt_str_aad(&key, &blob, aad::VERIFIER) {
            Ok(pt) if verifier_matches(pt.as_bytes()) => (BLOB_FORMAT_AAD, true),
            _ => match crypto::decrypt_str(&key, &blob) {
                Ok(pt) if verifier_matches(pt.as_bytes()) => (BLOB_FORMAT_LEGACY, true),
                _ => (BLOB_FORMAT_LEGACY, false),
            },
        };
        if !verifier_ok {
            return Err(DbError::WrongPassword);
        }

        let mut db = Self {
            conn,
            key,
            path: path.to_path_buf(),
            blob_format,
        };
        db.migrate_schema()?;
        db.migrate_blob_format()?;
        Ok(db)
    }

    /// Миграция существующей базы до текущей версии схемы.
    /// v2: группы, цвета, TOTP-секреты, история паролей.
    /// v3: параметры TOTP.
    fn migrate_schema(&self) -> Result<(), DbError> {
        self.conn
            .execute_batch(MIGRATE_V2)
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        let cols: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("PRAGMA table_info(entries)")
                .map_err(|e| DbError::Corrupted(e.to_string()))?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .map_err(|e| DbError::Corrupted(e.to_string()))?;
            rows.filter_map(|r| r.ok()).collect()
        };
        for (name, ddl) in [
            (
                "entry_group",
                "ALTER TABLE entries ADD COLUMN entry_group BLOB",
            ),
            ("color", "ALTER TABLE entries ADD COLUMN color BLOB"),
            ("totp", "ALTER TABLE entries ADD COLUMN totp BLOB"),
        ] {
            if !cols.iter().any(|c| c == name) {
                self.conn
                    .execute(ddl, [])
                    .map_err(|e| DbError::Corrupted(e.to_string()))?;
            }
        }
        // Параметры TOTP добавлены в v3.
        if !cols.iter().any(|c| c == "totp_params") {
            self.conn
                .execute_batch(MIGRATE_V3)
                .map_err(|e| DbError::Corrupted(e.to_string()))?;
        }
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![SCHEMA_VERSION.to_le_bytes().to_vec()],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        Ok(())
    }

    /// Перевести базу с устаревшего формата шифрования (без AAD) на новый:
    /// все поля перешифровываются с привязкой к имени столбца.
    /// Выполняется в транзакции — при сбое база остаётся в прежнем формате.
    fn migrate_blob_format(&mut self) -> Result<(), DbError> {
        if self.blob_format == BLOB_FORMAT_AAD {
            return Ok(());
        }
        let old_key = &self.key;
        let old_format = self.blob_format;

        let tx = self
            .conn
            .transaction()
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        reencrypt_all(&tx, old_key, old_format, old_key, BLOB_FORMAT_AAD)?;

        tx.execute(
            "INSERT INTO meta (key, value) VALUES ('blob_format', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![vec![BLOB_FORMAT_AAD]],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.commit().map_err(|e| DbError::Corrupted(e.to_string()))?;

        self.blob_format = BLOB_FORMAT_AAD;
        Ok(())
    }
}

/// Verifier корректен, если это текущий или legacy-идентификатор (старые базы).
fn verifier_matches(pt: &[u8]) -> bool {
    pt == VERIFIER_PLAINTEXT.as_bytes() || pt == super::schema::legacy_verifier().as_bytes()
}

impl Database {
    /// Смена мастер-пароля: выводится новый ключ, **все данные перешифровываются**
    /// новым ключом, и только потом обновляются соль и verifier.
    ///
    /// Прежняя реализация меняла соль и verifier, но не трогала записи:
    /// после смены пароля база оставалась зашифрованной старым ключом и
    /// все пароли становились нечитаемыми — данные терялись безвозвратно.
    pub fn change_master_password(&mut self, old: &str, new: &str) -> Result<(), DbError> {
        if new.is_empty() {
            return Err(DbError::Io(
                "Новый мастер-пароль не может быть пустым".into(),
            ));
        }
        let (salt, p) = meta_params(&self.conn)?;
        let check = crypto::derive_key(old, &salt, &p).map_err(DbError::Crypto)?;
        let blob: Vec<u8> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key='verifier'", [], |r| {
                r.get(0)
            })
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        let verified = if self.blob_format == BLOB_FORMAT_AAD {
            crypto::decrypt_str_aad(&check, &blob, aad::VERIFIER)
                .map(|pt| verifier_matches(pt.as_bytes()))
                .unwrap_or(false)
        } else {
            crypto::decrypt_str(&check, &blob)
                .map(|pt| verifier_matches(pt.as_bytes()))
                .unwrap_or(false)
        };
        if !verified {
            return Err(DbError::WrongPassword);
        }

        let new_salt = crypto::generate_salt();
        let new_key = crypto::derive_key(new, &new_salt, &p).map_err(DbError::Crypto)?;
        let new_verifier = if self.blob_format == BLOB_FORMAT_AAD {
            crypto::encrypt_str_aad(&new_key, VERIFIER_PLAINTEXT, aad::VERIFIER)
        } else {
            crypto::encrypt_str(&new_key, VERIFIER_PLAINTEXT)
        }
        .map_err(DbError::Crypto)?;

        let old_key = &self.key;
        let format = self.blob_format;

        let tx = self
            .conn
            .transaction()
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        // Перешифровка всех данных новым ключом — иначе после смены пароля
        // база осталась бы зашифрованной старым ключом.
        reencrypt_all(&tx, old_key, format, &new_key, format)?;

        tx.execute(
            "UPDATE meta SET value=?1 WHERE key='argon2_salt'",
            params![new_salt.to_vec()],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.execute(
            "UPDATE meta SET value=?1 WHERE key='verifier'",
            params![new_verifier],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.commit().map_err(|e| DbError::Corrupted(e.to_string()))?;

        self.key = new_key;
        Ok(())
    }

    /// Создать резервную копию базы в указанном каталоге.
    ///
    /// Используется `VACUUM INTO`: он создаёт согласованный снимок базы,
    /// в отличие от простого копирования файла, которое может поймать базу
    /// в середине записи.
    pub fn backup(&self, dir: &Path) -> Result<PathBuf, DbError> {
        let ts = glib::DateTime::now_local()
            .map_err(|e| DbError::Io(e.to_string()))?
            .format("%Y%m%d-%H%M%S")
            .map_err(|e| DbError::Io(e.to_string()))?;
        let mut name = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("passwords")
            .to_string();
        name.push_str(&format!("-backup-{ts}.rpwm"));
        let dest = dir.join(name);
        // VACUUM INTO требует, чтобы целевого файла не существовало.
        let _ = std::fs::remove_file(&dest);
        self.conn
            .execute("VACUUM INTO ?1", params![dest.to_string_lossy()])
            .map_err(|e| DbError::Io(format!("не удалось создать резервную копию: {e}")))?;
        Ok(dest)
    }

    fn enc(&self, field: &str, s: &str) -> Result<Vec<u8>, DbError> {
        enc_with(&self.key, self.blob_format, field, s)
    }

    fn dec_opt(&self, field: &str, blob: Option<Vec<u8>>) -> Option<String> {
        let blob = blob?;
        dec_with(&self.key, self.blob_format, field, &blob)
    }
}

/// Реализация в отдельном impl-блоке: работа с записями.
impl Database {
    /// Все записи (расшифрованные).
    pub fn list_entries(&self) -> Result<Vec<Entry>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, username, password, url, notes, tags, favorite,
                        entry_group, color, totp, totp_params, created_at, updated_at
                 FROM entries ORDER BY updated_at DESC",
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Option<Vec<u8>>>(4)?,
                    row.get::<_, Option<Vec<u8>>>(5)?,
                    row.get::<_, Option<Vec<u8>>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<Vec<u8>>>(8)?,
                    row.get::<_, Option<Vec<u8>>>(9)?,
                    row.get::<_, Option<Vec<u8>>>(10)?,
                    row.get::<_, Option<Vec<u8>>>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, i64>(13)?,
                ))
            })
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        let mut out = Vec::new();
        for row in rows {
            let (
                id,
                title,
                username,
                password,
                url,
                notes,
                tags,
                favorite,
                group,
                color,
                totp,
                totp_params,
                created_at,
                updated_at,
            ) = row.map_err(|e| DbError::Corrupted(e.to_string()))?;
            let tags: Vec<String> = self
                .dec_opt(aad::TAGS, tags)
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let totp_params = self
                .dec_opt(aad::TOTP_PARAMS, totp_params)
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            out.push(Entry {
                id: Some(id),
                title: self.dec_opt(aad::TITLE, Some(title)).unwrap_or_default(),
                username: self.dec_opt(aad::USERNAME, username).unwrap_or_default(),
                password: self
                    .dec_opt(aad::PASSWORD, Some(password))
                    .unwrap_or_default(),
                url: self.dec_opt(aad::URL, url).unwrap_or_default(),
                notes: self.dec_opt(aad::NOTES, notes).unwrap_or_default(),
                tags,
                favorite: favorite != 0,
                group: self.dec_opt(aad::GROUP, group).unwrap_or_default(),
                color: self.dec_opt(aad::COLOR, color).unwrap_or_default(),
                totp_secret: self.dec_opt(aad::TOTP, totp).unwrap_or_default(),
                totp_params,
                created_at,
                updated_at,
            });
        }
        Ok(out)
    }

    /// Сохранить (вставить или обновить) запись.
    pub fn save_entry(&mut self, entry: &mut Entry) -> Result<(), DbError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        entry.updated_at = now;
        let tags_json = serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".into());
        let totp_params_json =
            serde_json::to_string(&entry.totp_params).unwrap_or_else(|_| "{}".into());

        // Ошибка шифрования не должна приводить к записи пустого блоба:
        // раньше здесь стоял unwrap_or_default(), и ошибка тихо превращалась
        // в потерю данных. Теперь ошибка возвращается вызывающему.
        let enc_title = self.enc(aad::TITLE, &entry.title)?;
        let enc_username = self.enc(aad::USERNAME, &entry.username)?;
        let enc_password = self.enc(aad::PASSWORD, &entry.password)?;
        let enc_url = self.enc(aad::URL, &entry.url)?;
        let enc_notes = self.enc(aad::NOTES, &entry.notes)?;
        let enc_tags = self.enc(aad::TAGS, &tags_json)?;
        let enc_group = self.enc(aad::GROUP, &entry.group)?;
        let enc_color = self.enc(aad::COLOR, &entry.color)?;
        let enc_totp = self.enc(aad::TOTP, &entry.totp_secret)?;
        let enc_totp_params = self.enc(aad::TOTP_PARAMS, &totp_params_json)?;

        // Прежний пароль читаем ДО открытия транзакции: `transaction()`
        // занимает `&mut self.conn`, а расшифровка требует `&self`.
        let old_password: Option<String> = match entry.id {
            Some(id) => {
                let old_blob: Option<Option<Vec<u8>>> = self
                    .conn
                    .query_row(
                        "SELECT password FROM entries WHERE id=?1",
                        params![id],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|e| DbError::Corrupted(e.to_string()))?;
                old_blob
                    .and_then(|b| self.dec_opt(aad::PASSWORD, b))
                    .filter(|p| !p.is_empty() && p != &entry.password)
            }
            None => None,
        };
        // История: если пароль меняется, прежний сохраняется в entry_history.
        let enc_old = match &old_password {
            Some(old) => Some(self.enc(aad::HISTORY, old)?),
            None => None,
        };

        let tx = self
            .conn
            .transaction()
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        if let Some(id) = entry.id {
            if let Some(enc_old) = enc_old {
                tx.execute(
                    "INSERT INTO entry_history (entry_id, password, changed_at)
                     VALUES (?1, ?2, ?3)",
                    params![id, enc_old, now],
                )
                .map_err(|e| DbError::Corrupted(e.to_string()))?;
            }
            tx.execute(
                "UPDATE entries SET title=?1, username=?2, password=?3, url=?4, notes=?5,
                 tags=?6, favorite=?7, entry_group=?8, color=?9, totp=?10, totp_params=?11,
                 updated_at=?12 WHERE id=?13",
                params![
                    enc_title,
                    enc_username,
                    enc_password,
                    enc_url,
                    enc_notes,
                    enc_tags,
                    entry.favorite as i64,
                    enc_group,
                    enc_color,
                    enc_totp,
                    enc_totp_params,
                    entry.updated_at,
                    id
                ],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        } else {
            entry.created_at = now;
            tx.execute(
                "INSERT INTO entries (title, username, password, url, notes, tags, favorite,
                 entry_group, color, totp, totp_params, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    enc_title,
                    enc_username,
                    enc_password,
                    enc_url,
                    enc_notes,
                    enc_tags,
                    entry.favorite as i64,
                    enc_group,
                    enc_color,
                    enc_totp,
                    enc_totp_params,
                    entry.created_at,
                    entry.updated_at
                ],
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
            entry.id = Some(tx.last_insert_rowid());
        }
        tx.commit().map_err(|e| DbError::Corrupted(e.to_string()))?;
        Ok(())
    }

    pub fn delete_entry(&mut self, id: i64) -> Result<(), DbError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.execute("DELETE FROM entries WHERE id=?1", params![id])
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.execute("DELETE FROM entry_history WHERE entry_id=?1", params![id])
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.commit().map_err(|e| DbError::Corrupted(e.to_string()))?;
        Ok(())
    }

    /// История паролей записи: (changed_at, пароль), новые сверху.
    pub fn password_history(&self, entry_id: i64) -> Result<Vec<(i64, String)>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT changed_at, password FROM entry_history
                 WHERE entry_id=?1 ORDER BY id DESC",
            )
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        let rows = stmt
            .query_map(params![entry_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            let (ts, blob) = row.map_err(|e| DbError::Corrupted(e.to_string()))?;
            let pass = self.dec_opt(aad::HISTORY, Some(blob)).unwrap_or_default();
            out.push((ts, pass));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::totp::TotpParams;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("taynik_db_tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    /// Облегчённые параметры Argon2id — тесты не должны ждать полный KDF.
    fn test_params() -> ArgonParams {
        ArgonParams {
            memory_kib: 8 * 1024,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn create_test_db(path: &Path, password: &str) -> Database {
        let _ = std::fs::remove_file(path);
        Database::create_with_params(path, password, &test_params()).unwrap()
    }

    fn sample_entry(title: &str, password: &str) -> Entry {
        Entry {
            title: title.into(),
            username: "user@example.com".into(),
            password: password.into(),
            url: "https://example.com".into(),
            notes: "заметка".into(),
            tags: vec!["тег".into()],
            favorite: true,
            group: "Группа".into(),
            color: "#e74c3c".into(),
            totp_secret: "GEZDGNBVGY3TQOJQ".into(),
            totp_params: TotpParams::default(),
            ..Default::default()
        }
    }

    #[test]
    fn create_open_and_roundtrip() {
        let path = temp_path("roundtrip.rpwm");
        let _ = std::fs::remove_file(&path);
        let mut db = create_test_db(&path, "мастер-пароль");
        let mut e = sample_entry("Запись", "пароль-1");
        db.save_entry(&mut e).unwrap();
        assert!(e.id.is_some());
        drop(db);

        let db = Database::open(&path, "мастер-пароль").unwrap();
        let list = db.list_entries().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Запись");
        assert_eq!(list[0].password, "пароль-1");
        assert_eq!(list[0].tags, vec!["тег".to_string()]);
        assert_eq!(list[0].group, "Группа");
        assert_eq!(list[0].totp_secret, "GEZDGNBVGY3TQOJQ");
        assert!(list[0].favorite);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wrong_password_is_rejected() {
        let path = temp_path("wrongpw.rpwm");
        let _ = std::fs::remove_file(&path);
        let db = create_test_db(&path, "правильный");
        drop(db);
        let err = Database::open(&path, "неправильный")
            .err()
            .expect("должна быть ошибка");
        assert!(matches!(err, DbError::WrongPassword));
        let _ = std::fs::remove_file(&path);
    }

    /// Регрессия: смена мастер-пароля обязана сохранять данные.
    #[test]
    fn change_master_password_keeps_data_readable() {
        let path = temp_path("changepw.rpwm");
        let _ = std::fs::remove_file(&path);
        let mut db = create_test_db(&path, "старый-пароль");
        let mut e = sample_entry("Запись", "пароль-1");
        db.save_entry(&mut e).unwrap();
        // Второй пароль — чтобы в истории появилась запись.
        e.password = "пароль-2".into();
        db.save_entry(&mut e).unwrap();
        assert_eq!(db.password_history(e.id.unwrap()).unwrap().len(), 1);

        db.change_master_password("старый-пароль", "новый-пароль")
            .unwrap();

        // База должна открываться новым паролем и содержать те же данные.
        drop(db);
        let db = Database::open(&path, "новый-пароль").unwrap();
        let list = db.list_entries().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Запись");
        assert_eq!(list[0].password, "пароль-2");
        assert_eq!(list[0].username, "user@example.com");
        assert_eq!(list[0].notes, "заметка");
        assert_eq!(list[0].totp_secret, "GEZDGNBVGY3TQOJQ");
        // История паролей тоже должна остаться читаемой.
        let hist = db.password_history(list[0].id.unwrap()).unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].1, "пароль-1");

        // Старый пароль больше не подходит.
        assert!(matches!(
            Database::open(&path, "старый-пароль")
                .err()
                .expect("старый пароль не должен подходить"),
            DbError::WrongPassword
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn change_master_password_rejects_wrong_old_password() {
        let path = temp_path("changepw_bad.rpwm");
        let _ = std::fs::remove_file(&path);
        let mut db = create_test_db(&path, "старый-пароль");
        let err = db
            .change_master_password("неверный", "новый-пароль")
            .unwrap_err();
        assert!(matches!(err, DbError::WrongPassword));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn aad_prevents_field_substitution() {
        let path = temp_path("aad.rpwm");
        let _ = std::fs::remove_file(&path);
        let mut db = create_test_db(&path, "мастер-пароль");
        let mut e = sample_entry("Запись", "пароль-1");
        db.save_entry(&mut e).unwrap();

        // Подменяем шифротекст пароля на шифротекст заголовка.
        let title_blob: Vec<u8> = db
            .conn
            .query_row(
                "SELECT title FROM entries WHERE id=?1",
                params![e.id.unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        db.conn
            .execute(
                "UPDATE entries SET password=?1 WHERE id=?2",
                params![title_blob, e.id.unwrap()],
            )
            .unwrap();

        // Расшифровка пароля обязана провалиться: AAD не совпадает.
        let list = db.list_entries().unwrap();
        assert!(list[0].password.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn backup_creates_readable_copy() {
        let path = temp_path("backup.rpwm");
        let _ = std::fs::remove_file(&path);
        let mut db = create_test_db(&path, "мастер-пароль");
        let mut e = sample_entry("Запись", "пароль-1");
        db.save_entry(&mut e).unwrap();

        let dir = temp_path("backup_dir");
        std::fs::create_dir_all(&dir).unwrap();
        let dest = db.backup(&dir).unwrap();
        assert!(dest.exists());

        let copy = Database::open(&dest, "мастер-пароль").unwrap();
        assert_eq!(copy.list_entries().unwrap().len(), 1);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&dest);
    }

    #[test]
    fn delete_entry_removes_history() {
        let path = temp_path("delete.rpwm");
        let _ = std::fs::remove_file(&path);
        let mut db = create_test_db(&path, "мастер-пароль");
        let mut e = sample_entry("Запись", "пароль-1");
        db.save_entry(&mut e).unwrap();
        e.password = "пароль-2".into();
        db.save_entry(&mut e).unwrap();
        let id = e.id.unwrap();
        assert_eq!(db.password_history(id).unwrap().len(), 1);
        db.delete_entry(id).unwrap();
        assert!(db.list_entries().unwrap().is_empty());
        assert!(db.password_history(id).unwrap().is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
