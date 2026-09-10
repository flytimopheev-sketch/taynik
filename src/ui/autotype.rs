//! Автоввод логина и пароля в активное окно другого приложения (xdotool/wtype).

use std::cell::Cell;
use std::process::Command;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::glib;
use gtk4::prelude::*;

const COUNTDOWN_SECS: u32 = 5;

/// Показать окно обратного отсчёта, дать пользователю время переключиться
/// в целевое окно, затем ввести логин, Tab и пароль.
pub fn run(username: &str, password: &str) {
    if username.is_empty() && password.is_empty() {
        return;
    }
    // Клонируем в owned, т.к. используется внутри замыкания таймера.
    let username = username.to_string();
    let password = password.to_string();

    let win = gtk::Window::builder()
        .title("Тайник — автоввод")
        .default_width(340)
        .default_height(90)
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
            type_sequence(username.as_str(), password.as_str());
            return glib::ControlFlow::Break;
        }
        remaining.set(r - 1);
        label.set_text(&format!(
            "Автоввод через {r} с… Переключитесь в целевое окно"
        ));
        glib::ControlFlow::Continue
    });
}

/// Ввести последовательность в активное окно.
/// X11 — xdotool, Wayland — wtype. Выполняется в отдельном потоке,
/// чтобы не блокировать главный цикл GTK.
fn type_sequence(username: &str, password: &str) {
    let wayland_only = std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
        && std::env::var("DISPLAY")
            .map(|v| v.is_empty())
            .unwrap_or(true);
    let user = username.to_string();
    let pass = password.to_string();
    std::thread::spawn(move || {
        if wayland_only {
            if !user.is_empty() {
                let _ = Command::new("wtype").arg("-d").arg("60").arg(&user).status();
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
            let _ = Command::new("wtype").arg("-k").arg("Tab").status();
            std::thread::sleep(std::time::Duration::from_millis(150));
            let _ = Command::new("wtype").arg("-d").arg("60").arg(&pass).status();
        } else {
            if !user.is_empty() {
                let _ = Command::new("xdotool")
                    .args(["type", "--clearmodifiers", "--delay", "60", "--", &user])
                    .status();
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
            let _ = Command::new("xdotool")
                .args(["key", "--clearmodifiers", "Tab"])
                .status();
            std::thread::sleep(std::time::Duration::from_millis(150));
            let _ = Command::new("xdotool")
                .args(["type", "--clearmodifiers", "--delay", "60", "--", &pass])
                .status();
        }
    });
}
