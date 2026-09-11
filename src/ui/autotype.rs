//! Автоввод логина и пароля в активное окно другого приложения (xdotool/wtype).
//! Последовательность задаётся строкой вида:
//!   {USERNAME}{TAB}{PASSWORD}{ENTER}
//! где {USERNAME}/{PASSWORD} — поля записи, {TAB}/{ENTER} — клавиши,
//! {DELAY мс} — пауза, остальной текст вводится как есть.

use std::cell::Cell;
use std::process::Command;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::glib;
use gtk4::prelude::*;

const COUNTDOWN_SECS: u32 = 5;

/// Один шаг последовательности автоввода.
#[derive(Clone)]
enum Step {
    /// Ввести текст (логин, пароль или произвольный литерал).
    Text(String),
    /// Нажать клавишу ("Tab", "Return" — имена xdotool/wtype).
    Key(&'static str),
    /// Пауза, миллисекунды.
    Delay(u64),
}

/// Разобрать строку последовательности в шаги.
fn parse_sequence(seq: &str, username: &str, password: &str) -> Vec<Step> {
    let seq = if seq.trim().is_empty() {
        "{USERNAME}{TAB}{PASSWORD}"
    } else {
        seq
    };
    let mut steps = Vec::new();
    let mut literal = String::new();
    let mut chars = seq.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut token = String::new();
            for t in chars.by_ref() {
                if t == '}' {
                    break;
                }
                token.push(t);
            }
            match token.trim().to_uppercase().as_str() {
                "USERNAME" => {
                    if !username.is_empty() {
                        steps.push(Step::Text(username.to_string()));
                    }
                }
                "PASSWORD" => {
                    if !password.is_empty() {
                        steps.push(Step::Text(password.to_string()));
                    }
                }
                "TAB" => steps.push(Step::Key("Tab")),
                "ENTER" | "RETURN" => steps.push(Step::Key("Return")),
                _ if token
                    .trim()
                    .to_uppercase()
                    .starts_with("DELAY") =>
                {
                    let ms = token
                        .trim()
                        .split_whitespace()
                        .nth(1)
                        .and_then(|n| n.parse::<u64>().ok())
                        .unwrap_or(150);
                    steps.push(Step::Delay(ms));
                }
                _ => {
                    // Неизвестный токен — вводим его как обычный текст.
                    literal.push('{');
                    literal.push_str(&token);
                    literal.push('}');
                }
            }
        } else {
            literal.push(c);
        }
    }
    if !literal.is_empty() {
        steps.push(Step::Text(literal));
    }
    steps
}
/// Показать окно обратного отсчёта, дать пользователю время переключиться
/// в целевое окно, затем выполнить последовательность автоввода.
pub fn run(username: &str, password: &str, sequence: &str) {
    if username.is_empty() && password.is_empty() {
        return;
    }
    // Клонируем в owned, т.к. используется внутри замыкания таймера.
    let username = username.to_string();
    let password = password.to_string();
    let sequence = sequence.to_string();

    let win = gtk::Window::builder()
        .title("Тайник — автоввод")
        .default_width(340)
        .default_height(110)
        .resizable(false)
        .decorated(false)
        .build();
    let label = gtk::Label::new(Some("Переключитесь в целевое окно…"));
    label.set_margin_top(24);
    label.set_margin_bottom(24);
    label.set_margin_start(16);
    label.set_margin_end(16);
    win.set_child(Some(&label));
    win.show();

    let remaining = Rc::new(Cell::new(COUNTDOWN_SECS));
    glib::timeout_add_seconds_local(1, move || {
        let r = remaining.get();
        if r == 0 {
            win.destroy();
            type_sequence(&parse_sequence(&sequence, &username, &password));
            return glib::ControlFlow::Break;
        }
        remaining.set(r - 1);
        label.set_text(&format!(
            "Автоввод через {r} с… Переключитесь в целевое окно"
        ));
        glib::ControlFlow::Continue
    });
}

/// Выполнить последовательность в активном окне.
/// X11 — xdotool, Wayland — wtype. Выполняется в отдельном потоке,
/// чтобы не блокировать главный цикл GTK.
fn type_sequence(steps: &[Step]) {
    let wayland_only = std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
        && std::env::var("DISPLAY")
            .map(|v| v.is_empty())
            .unwrap_or(true);
    let steps: Vec<Step> = steps.to_vec();
    std::thread::spawn(move || {
        for step in steps {
            match step {
                Step::Text(text) => {
                    let _ = if wayland_only {
                        Command::new("wtype")
                            .arg("-d")
                            .arg("60")
                            .arg(&text)
                            .status()
                    } else {
                        Command::new("xdotool")
                            .args([
                                "type",
                                "--clearmodifiers",
                                "--delay",
                                "60",
                                "--",
                                &text,
                            ])
                            .status()
                    };
                }
                Step::Key(key) => {
                    let _ = if wayland_only {
                        Command::new("wtype").arg("-k").arg(key).status()
                    } else {
                        Command::new("xdotool")
                            .args(["key", "--clearmodifiers", key])
                            .status()
                    };
                }
                Step::Delay(ms) => {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }
            }
            // Пауза между шагами, чтобы приложение успело обработать ввод.
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(steps: &[Step]) -> Vec<String> {
        steps
            .iter()
            .filter_map(|s| match s {
                Step::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn parse_default() {
        let steps = parse_sequence("", "user", "pass");
        assert_eq!(texts(&steps), vec!["user".to_string(), "pass".to_string()]);
    }

    #[test]
    fn parse_custom() {
        let steps = parse_sequence(
            "{DELAY 200}{USERNAME}{TAB}{PASSWORD}{ENTER}",
            "u",
            "p",
        );
        assert_eq!(texts(&steps), vec!["u".to_string(), "p".to_string()]);
        assert!(steps.len() >= 5);
    }
}