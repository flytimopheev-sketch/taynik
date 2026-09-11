//! Импорт/экспорт записей в CSV.
//! ВНИМАНИЕ: CSV хранит пароли в открытом виде — UI обязан предупреждать.

use std::io::Read;
use std::path::Path;

use crate::models::entry::Entry;

/// Экранировать поле CSV (RFC 4180).
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Экспортировать записи в CSV-файл. Возвращает число записей.
pub fn export_csv(path: &Path, entries: &[Entry]) -> Result<usize, String> {
    let mut out = String::from(
        "title,username,password,url,notes,tags,group,color,totp_secret,favorite\r\n",
    );
    for e in entries {
        let tags = e.tags.join(";");
        let fav = if e.favorite { "1" } else { "0" };
        let row = [
            e.title.as_str(),
            e.username.as_str(),
            e.password.as_str(),
            e.url.as_str(),
            e.notes.as_str(),
            tags.as_str(),
            e.group.as_str(),
            e.color.as_str(),
            e.totp_secret.as_str(),
            fav,
        ];
        out.push_str(
            &row.iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("\r\n");
    }
    std::fs::write(path, out).map_err(|e| e.to_string())?;
    Ok(entries.len())
}

/// Импортировать записи из CSV-файла.
pub fn import_csv(path: &Path) -> Result<Vec<Entry>, String> {
    let mut s = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    parse_csv(&s)
}
/// Простой CSV-парсер (RFC 4180): кавычки, запятые, переводы строк в полях.
pub fn parse_csv(s: &str) -> Result<Vec<Entry>, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut field = String::new();
    let mut row: Vec<String> = Vec::new();
    let mut in_quotes = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                '\r' => {}
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(std::mem::take(&mut field));
        rows.push(std::mem::take(&mut row));
    }
    let mut iter = rows.into_iter();
    let header = iter.next().ok_or("CSV-файл пуст")?;
    if header.first().map(|h| h.trim().to_lowercase()).as_deref() != Some("title") {
        return Err("неожиданный формат CSV: ожидается заголовок title,username,password,…".into());
    }
    let idx = |name: &str| {
        header
            .iter()
            .position(|h| h.trim().to_lowercase() == name)
    };
    let mut out = Vec::new();
    for r in iter {
        if r.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        let g = |name: &str| idx(name).and_then(|i| r.get(i)).cloned().unwrap_or_default();
        let tags = g("tags");
        out.push(Entry {
            id: None,
            title: g("title"),
            username: g("username"),
            password: g("password"),
            url: g("url"),
            notes: g("notes"),
            tags: tags
                .split(';')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
            group: g("group"),
            color: g("color"),
            totp_secret: g("totp_secret"),
            favorite: g("favorite").trim() == "1",
            created_at: 0,
            updated_at: 0,
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Entry {
        Entry {
            id: Some(1),
            title: "Сайт".into(),
            username: "user@mail".into(),
            password: "pass, с \"кавычками\"".into(),
            url: "https://example.com".into(),
            notes: "строка1\nстрока2".into(),
            tags: vec!["работа".into(), "почта".into()],
            group: "Личное".into(),
            color: "#e74c3c".into(),
            totp_secret: "GEZDGNBVGY3TQOJQ".into(),
            favorite: true,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn csv_roundtrip() {
        let dir = std::env::temp_dir().join("taynik_io_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.csv");
        let e = sample();
        export_csv(&path, &[e.clone()]).unwrap();
        let parsed = import_csv(&path).unwrap();
        assert_eq!(parsed.len(), 1);
        let p = &parsed[0];
        assert_eq!(p.title, e.title);
        assert_eq!(p.password, e.password);
        assert_eq!(p.notes, e.notes);
        assert_eq!(p.tags, e.tags);
        assert_eq!(p.group, e.group);
        assert_eq!(p.totp_secret, e.totp_secret);
        assert!(p.favorite);
        let _ = std::fs::remove_file(&path);
    }
}