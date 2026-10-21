//! Аудит безопасности: поиск слабых, повторяющихся и устаревших паролей.

use std::collections::HashMap;

use super::entry::Entry;
use super::generator::{self, Strength};

/// Тип найденной проблемы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    /// Пароль слабый.
    Weak,
    /// Такой же пароль используется в нескольких записях.
    Duplicate,
    /// Пароль не менялся дольше указанного срока.
    Old,
}

impl IssueKind {
    pub fn label(self) -> &'static str {
        match self {
            IssueKind::Weak => "Слабый пароль",
            IssueKind::Duplicate => "Повторяющийся пароль",
            IssueKind::Old => "Пароль устарел",
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            IssueKind::Weak => "error",
            IssueKind::Duplicate => "warning",
            IssueKind::Old => "warning",
        }
    }
}

/// Одна найденная проблема.
#[derive(Debug, Clone)]
pub struct Issue {
    /// Идентификатор записи (None — если запись ещё не сохранена).
    pub entry_id: Option<i64>,
    /// Название записи.
    pub title: String,
    /// Тип проблемы.
    pub kind: IssueKind,
    /// Пояснение для пользователя.
    pub detail: String,
}

/// Провести аудит базы.
///
/// `old_days` — срок, после которого пароль считается устаревшим.
/// `min_strength` — минимально допустимая стойкость пароля.
pub fn audit(entries: &[Entry], old_days: u64, min_strength: Strength) -> Vec<Issue> {
    let mut issues: Vec<Issue> = Vec::new();

    // Считаем, сколько записей используют каждый пароль.
    let mut usage: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if e.password.is_empty() {
            continue;
        }
        usage.entry(e.password.as_str()).or_default().push(i);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let old_threshold = now.saturating_sub((old_days as i64) * 86_400);

    for (i, e) in entries.iter().enumerate() {
        // Слабый пароль.
        if !e.password.is_empty() {
            let s = generator::strength(&e.password);
            if s == Strength::Weak || (min_strength == Strength::Strong && s == Strength::Medium) {
                issues.push(Issue {
                    entry_id: e.id,
                    title: e.title.clone(),
                    kind: IssueKind::Weak,
                    detail: format!("Стойкость: {}", s.label().to_lowercase()),
                });
            }
        }
        // Повторяющийся пароль.
        if let Some(list) = usage.get(e.password.as_str()) {
            if list.len() > 1 {
                let others: Vec<&str> = list
                    .iter()
                    .filter(|&&j| j != i)
                    .map(|&j| entries[j].title.as_str())
                    .collect();
                issues.push(Issue {
                    entry_id: e.id,
                    title: e.title.clone(),
                    kind: IssueKind::Duplicate,
                    detail: format!("Такой же пароль у записей: {}", others.join(", ")),
                });
            }
        }
        // Устаревший пароль.
        if old_days > 0 && e.updated_at > 0 && e.updated_at < old_threshold {
            let days = (now - e.updated_at) / 86_400;
            issues.push(Issue {
                entry_id: e.id,
                title: e.title.clone(),
                kind: IssueKind::Old,
                detail: format!("Не менялся {days} дн."),
            });
        }
    }

    issues
}

/// Сводка по количеству проблем.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuditSummary {
    pub weak: usize,
    pub duplicate: usize,
    pub old: usize,
}

impl AuditSummary {
    pub fn total(&self) -> usize {
        self.weak + self.duplicate + self.old
    }
}

/// Подсчитать сводку по списку проблем.
pub fn summarize(issues: &[Issue]) -> AuditSummary {
    let mut s = AuditSummary::default();
    for i in issues {
        match i.kind {
            IssueKind::Weak => s.weak += 1,
            IssueKind::Duplicate => s.duplicate += 1,
            IssueKind::Old => s.old += 1,
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::generator::Strength;

    fn entry(id: i64, title: &str, password: &str, updated_at: i64) -> Entry {
        Entry {
            id: Some(id),
            title: title.into(),
            password: password.into(),
            updated_at,
            ..Default::default()
        }
    }

    #[test]
    fn finds_weak_passwords() {
        let entries = vec![entry(1, "Слабый", "123456", 0)];
        let issues = audit(&entries, 0, Strength::Medium);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, IssueKind::Weak);
        assert_eq!(issues[0].entry_id, Some(1));
    }

    #[test]
    fn finds_duplicates() {
        let strong = "Xk9$mQ2#vLp7@rTn4%wYz8&";
        let entries = vec![entry(1, "Первый", strong, 0), entry(2, "Второй", strong, 0)];
        let issues = audit(&entries, 0, Strength::Medium);
        let dups: Vec<_> = issues
            .iter()
            .filter(|i| i.kind == IssueKind::Duplicate)
            .collect();
        assert_eq!(dups.len(), 2);
        assert!(dups[0].detail.contains("Второй") || dups[0].detail.contains("Первый"));
    }

    #[test]
    fn finds_old_passwords() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let old = now - 400 * 86_400;
        let entries = vec![entry(1, "Старая", "Xk9$mQ2#vLp7@rTn4%wYz8&", old)];
        let issues = audit(&entries, 365, Strength::Medium);
        let olds: Vec<_> = issues.iter().filter(|i| i.kind == IssueKind::Old).collect();
        assert_eq!(olds.len(), 1);
        assert!(olds[0].detail.contains("дн."));
    }

    #[test]
    fn clean_vault_has_no_issues() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let entries = vec![
            entry(1, "Первая", "Xk9$mQ2#vLp7@rTn4%wYz8&", now),
            entry(2, "Вторая", "Tr8&yU3#kQs9@mNq6!pLx2%", now),
        ];
        let issues = audit(&entries, 365, Strength::Medium);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(summarize(&issues).total(), 0);
    }

    #[test]
    fn summary_counts_by_kind() {
        let issues = vec![
            Issue {
                entry_id: Some(1),
                title: "a".into(),
                kind: IssueKind::Weak,
                detail: String::new(),
            },
            Issue {
                entry_id: Some(2),
                title: "b".into(),
                kind: IssueKind::Duplicate,
                detail: String::new(),
            },
            Issue {
                entry_id: Some(3),
                title: "c".into(),
                kind: IssueKind::Duplicate,
                detail: String::new(),
            },
        ];
        let s = summarize(&issues);
        assert_eq!(s.weak, 1);
        assert_eq!(s.duplicate, 2);
        assert_eq!(s.total(), 3);
    }
}
