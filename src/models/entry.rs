/// Запись менеджера паролей (в памяти — всегда расшифрованная).
use gtk4::glib;

#[derive(Debug, Clone, Default)]
pub struct Entry {
    pub id: Option<i64>,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Entry {
    /// Форматированная дата изменения (локальное время).
    pub fn updated_at_str(&self) -> String {
        glib::DateTime::from_unix_local(self.updated_at)
            .map(|dt| {
                dt.format("%Y-%m-%d %H:%M")
                    .unwrap_or_else(|_| "—".into())
                    .to_string()
            })
            .unwrap_or_else(|_| "—".into())
    }
}
