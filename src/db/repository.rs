//! Репозиторий: открытие/создание зашифрованной базы и операции с записями.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use gtk4::glib;

use super::crypto::{self, ArgonParams, CryptoKey};
use super::schema::{
    CREATE_SCHEMA, LEGACY_VERIFIER_PLAINTEXT, SCHEMA_VERSION, VERIFIER_PLAINTEXT,
};
use crate::models::entry::Entry;

#[derive(Debug)]
pub enum DbError {
    WrongPassword,
    Corrupted(String),
    Io(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::WrongPassword => write!(f, "Неверный мастер-пароль"),
            DbError::Corrupted(m) => write!(f, "Файл базы повреждён: {m}"),
            DbError::Io(m) => write!(f, "Ошибка ввода-вывода: {m}"),
        }
    }
}

pub struct Database {
    conn: Connection,
    key: CryptoKey,
    pub path: PathBuf,
}

fn meta_params(conn: &Connection) -> Result<(Vec<u8>, ArgonParams), DbError> {
    let salt: Vec<u8> = conn
        .query_row("SELECT value FROM meta WHERE key='argon2_salt'", [], |r| r.get(0))
        .optional()
        .map_err(|e| DbError::Corrupted(e.to_string()))?
        .ok_or_else(|| DbError::Corrupted("отсутствует salt в meta".into()))?;
    let raw: Vec<u8> = conn
        .query_row("SELECT value FROM meta WHERE key='argon2_params'", [], |r| r.get(0))
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
    let p: ArgonParams =
        serde_json::from_slice(&raw).map_err(|e| DbError::Corrupted(e.to_string()))?;
    Ok((salt, p))
}

impl Database {
    /// Создать новую зашифрованную базу.
    pub fn create(path: &Path, master_password: &str) -> Result<Self, DbError> {
        if path.exists() {
            return Err(DbError::Io(format!("файл {} уже существует", path.display())));
        }
        let conn = Connection::open(path).map_err(|e| DbError::Io(e.to_string()))?;
        conn.execute_batch(CREATE_SCHEMA)
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        let params_ = ArgonParams::default();
        let salt = crypto::generate_salt();
        let key =
            crypto::derive_key(master_password, &salt, &params_).map_err(DbError::Corrupted)?;
        let verifier =
            crypto::encrypt(&key, VERIFIER_PLAINTEXT.as_bytes()).map_err(DbError::Corrupted)?;

        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('argon2_salt', ?1)",
            params![salt.to_vec()],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('argon2_params', ?1)",
            params![serde_json::to_vec(&params_).unwrap()],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('encryption_key_params', ?1)",
            params![json!({"algorithm": "AES-256-GCM", "nonce_size": 12}).to_string()],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('verifier', ?1)",
            params![verifier],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_le_bytes().to_vec()],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;

        Ok(Self { conn, key, path: path.to_path_buf() })
    }

    /// Открыть существующую базу с проверкой мастер-пароля.
    pub fn open(path: &Path, master_password: &str) -> Result<Self, DbError> {
        let conn = Connection::open(path).map_err(|e| DbError::Io(e.to_string()))?;
        let (salt, p) = meta_params(&conn)?;
        let key = crypto::derive_key(master_password, &salt, &p).map_err(DbError::Corrupted)?;
        let blob: Vec<u8> = conn
            .query_row("SELECT value FROM meta WHERE key='verifier'", [], |r| r.get(0))
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        match crypto::decrypt(&key, &blob) {
            Ok(pt) if verifier_matches(&pt) => {}
            _ => return Err(DbError::WrongPassword),
        }
        Ok(Self { conn, key, path: path.to_path_buf() })
    }
}

/// Verifier корректен, если это текущий или legacy-идентификатор (старые базы).
fn verifier_matches(pt: &[u8]) -> bool {
    pt == VERIFIER_PLAINTEXT.as_bytes() || pt == LEGACY_VERIFIER_PLAINTEXT.as_bytes()
}

impl Database {
    /// Смена мастер-пароля: выводится новый ключ и создаётся новый verifier.
    pub fn change_master_password(&mut self, old: &str, new: &str) -> Result<(), DbError> {
        let (salt, p) = meta_params(&self.conn)?;
        let check = crypto::derive_key(old, &salt, &p).map_err(DbError::Corrupted)?;
        let blob: Vec<u8> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key='verifier'", [], |r| r.get(0))
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        match crypto::decrypt(&check, &blob) {
            Ok(pt) if verifier_matches(&pt) => {}
            _ => return Err(DbError::WrongPassword),
        }

        let new_salt = crypto::generate_salt();
        let new_key = crypto::derive_key(new, &new_salt, &p).map_err(DbError::Corrupted)?;
        let verifier =
            crypto::encrypt(&new_key, VERIFIER_PLAINTEXT.as_bytes()).map_err(DbError::Corrupted)?;
        let tx = self
            .conn
            .transaction()
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.execute(
            "UPDATE meta SET value=?1 WHERE key='argon2_salt'",
            params![new_salt.to_vec()],
        )
        .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.execute("UPDATE meta SET value=?1 WHERE key='verifier'", params![verifier])
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        tx.commit().map_err(|e| DbError::Corrupted(e.to_string()))?;
        self.key = new_key;
        Ok(())
    }

    /// Создать резервную копию базы в указанном каталоге.
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
        std::fs::copy(&self.path, &dest).map_err(|e| DbError::Io(e.to_string()))?;
        Ok(dest)
    }

    fn enc(&self, s: &str) -> Vec<u8> {
        crypto::encrypt_str(&self.key, s).unwrap_or_default()
    }

    fn dec_opt(&self, blob: Option<Vec<u8>>) -> Option<String> {
        let blob = blob?;
        crypto::decrypt_str(&self.key, &blob).ok()
    }
}

/// Реализация в отдельном impl-блоке: работа с записями.
impl Database {
    /// Все записи (расшифрованные).
    pub fn list_entries(&self) -> Result<Vec<Entry>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, username, password, url, notes, tags, favorite, created_at, updated_at
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
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
            .map_err(|e| DbError::Corrupted(e.to_string()))?;

        let mut out = Vec::new();
        for row in rows {
            let (id, title, username, password, url, notes, tags, favorite, created_at, updated_at) =
                row.map_err(|e| DbError::Corrupted(e.to_string()))?;
            let dec = |b: Vec<u8>| -> String { self.dec_opt(Some(b)).unwrap_or_default() };
            let tags: Vec<String> = tags
                .and_then(|t| self.dec_opt(Some(t)))
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            out.push(Entry {
                id: Some(id),
                title: dec(title),
                username: username.and_then(|u| self.dec_opt(Some(u))).unwrap_or_default(),
                password: dec(password),
                url: url.and_then(|u| self.dec_opt(Some(u))).unwrap_or_default(),
                notes: notes.and_then(|n| self.dec_opt(Some(n))).unwrap_or_default(),
                tags,
                favorite: favorite != 0,
                created_at,
                updated_at,
            });
        }
        Ok(out)
    }

    /// Сохранить (вставить или обновить) запись.
    pub fn save_entry(&self, entry: &mut Entry) -> Result<(), DbError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        entry.updated_at = now;
        let tags_json = serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".into());
        if let Some(id) = entry.id {
            self.conn
                .execute(
                    "UPDATE entries SET title=?1, username=?2, password=?3, url=?4, notes=?5,
                     tags=?6, favorite=?7, updated_at=?8 WHERE id=?9",
                    params![
                        self.enc(&entry.title),
                        self.enc(&entry.username),
                        self.enc(&entry.password),
                        self.enc(&entry.url),
                        self.enc(&entry.notes),
                        self.enc(&tags_json),
                        entry.favorite as i64,
                        entry.updated_at,
                        id
                    ],
                )
                .map_err(|e| DbError::Corrupted(e.to_string()))?;
        } else {
            entry.created_at = now;
            self.conn
                .execute(
                    "INSERT INTO entries (title, username, password, url, notes, tags, favorite, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        self.enc(&entry.title),
                        self.enc(&entry.username),
                        self.enc(&entry.password),
                        self.enc(&entry.url),
                        self.enc(&entry.notes),
                        self.enc(&tags_json),
                        entry.favorite as i64,
                        entry.created_at,
                        entry.updated_at
                    ],
                )
                .map_err(|e| DbError::Corrupted(e.to_string()))?;
            entry.id = Some(self.conn.last_insert_rowid());
        }
        Ok(())
    }

    pub fn delete_entry(&self, id: i64) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM entries WHERE id=?1", params![id])
            .map_err(|e| DbError::Corrupted(e.to_string()))?;
        Ok(())
    }
}
