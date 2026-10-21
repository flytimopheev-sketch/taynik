//! Модуль пользовательского интерфейса (GTK4).

pub mod audit_dialog;
pub mod autotype;
pub mod create_db_dialog;
pub mod main_window;
pub mod passphrase_dialog;
pub mod password_generator;
pub mod settings_dialog;
pub mod start_window;
pub mod theme;
pub mod unlock_dialog;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::prelude::*;

use crate::config::Config;
use crate::db::repository::{Database, DbError};

/// Общее состояние приложения.
pub struct AppState {
    pub db: Rc<RefCell<Option<Database>>>,
    pub config: Rc<RefCell<Config>>,
}

pub type SharedState = Rc<AppState>;

pub const APP_ID: &str = "ru.taynik.Taynik";

pub fn run() {
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        let config = Config::load();
        // Применяем сохранённую тему до создания окон.
        theme::apply(&config.theme);
        let state: SharedState = Rc::new(AppState {
            db: Rc::new(RefCell::new(None)),
            config: Rc::new(RefCell::new(config)),
        });
        start_window::show(app, state);
    });
    app.run();
}

/// Показать сообщение об ошибке.
pub fn show_error(parent: &impl IsA<gtk::Window>, message: &str) {
    let dialog = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Close,
        message,
    );
    dialog.connect_response(|d, _| d.destroy());
    dialog.show();
}

/// Показать информационное сообщение.
pub fn show_info(parent: &impl IsA<gtk::Window>, message: &str) {
    let dialog = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Info,
        gtk::ButtonsType::Close,
        message,
    );
    dialog.connect_response(|d, _| d.destroy());
    dialog.show();
}

/// Запросить мастер-пароль и открыть базу; при ошибке повторяет запрос.
pub fn prompt_unlock_and_open(parent: &impl IsA<gtk::Window>, state: &SharedState, path: PathBuf) {
    let path_clone = path.clone();
    let state = state.clone();
    unlock_dialog::show(parent, &path, move |parent, password| match Database::open(
        &path_clone,
        &password,
    ) {
        Ok(db) => {
            state.config.borrow_mut().last_database = Some(path_clone.display().to_string());
            state.config.borrow().save();
            *state.db.borrow_mut() = Some(db);
            main_window::MainWindow::new(&parent, state.clone());
        }
        Err(DbError::WrongPassword) => {
            show_error(&parent, "Неверный мастер-пароль. Попробуйте ещё раз.");
            prompt_unlock_and_open(&parent, &state, path_clone);
        }
        Err(e) => show_error(&parent, &format!("Не удалось открыть базу:\n{e}")),
    });
}

/// Диалог ввода пароля (одного или с подтверждением).
///
/// `on_ok` вызывается только если введённый пароль непустой и (при
/// `confirm`) совпадает с подтверждением.
pub fn prompt_password(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    hint: &str,
    confirm: bool,
    on_ok: impl FnOnce(String) + 'static,
) {
    let dialog = gtk::Dialog::builder()
        .title(title)
        .modal(true)
        .transient_for(parent)
        .default_width(440)
        .build();
    dialog.add_button("Отмена", gtk::ResponseType::Cancel);
    dialog.add_button("ОК", gtk::ResponseType::Ok);
    dialog.set_default_response(gtk::ResponseType::Ok);

    let content = dialog.content_area();
    let box_ = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    if !hint.is_empty() {
        let l = gtk::Label::builder()
            .label(hint)
            .wrap(true)
            .halign(gtk::Align::Start)
            .build();
        box_.append(&l);
    }
    let entry = gtk::PasswordEntry::builder()
        .show_peek_icon(true)
        .placeholder_text("Пароль")
        .activates_default(true)
        .build();
    box_.append(&gtk::Label::with_mnemonic("_Пароль:"));
    box_.append(&entry);
    let confirm_entry = gtk::PasswordEntry::builder()
        .show_peek_icon(true)
        .placeholder_text("Повторите пароль")
        .activates_default(true)
        .build();
    if confirm {
        box_.append(&gtk::Label::with_mnemonic("_Повторите:"));
        box_.append(&confirm_entry);
    }
    content.append(&box_);

    let parent_win: gtk::Window = parent.clone().upcast();
    let entry2 = entry.clone();
    let confirm_entry2 = confirm_entry.clone();
    let on_ok = RefCell::new(Some(on_ok));
    let hint_label = gtk::Label::new(None);
    hint_label.add_css_class("error");
    box_.append(&hint_label);

    // Валидация совпадения паролей — включаем кнопку только при совпадении.
    if confirm {
        let ok_btn = dialog
            .widget_for_response(gtk::ResponseType::Ok)
            .and_then(|w| w.downcast::<gtk::Button>().ok());
        let e1 = entry.clone();
        let e2 = confirm_entry.clone();
        let warn = hint_label.clone();
        let check = move || {
            let match_ok = e1.text() == e2.text() && !e1.text().is_empty();
            if let Some(b) = &ok_btn {
                b.set_sensitive(match_ok);
            }
            warn.set_text(if !match_ok && !e1.text().is_empty() {
                "Пароли не совпадают"
            } else {
                ""
            });
        };
        let c1 = check.clone();
        entry.connect_changed(move |_| c1());
        let c2 = check.clone();
        confirm_entry.connect_changed(move |_| c2());
        check();
    }

    let dlg = dialog.clone();
    entry.connect_activate(move |_| dlg.response(gtk::ResponseType::Ok));

    dialog.connect_response(move |d, resp| {
        if resp == gtk::ResponseType::Ok {
            let password = entry2.text().to_string();
            if password.is_empty() {
                show_error(&parent_win, "Пароль не может быть пустым.");
                d.destroy();
                return;
            }
            if confirm && password != confirm_entry2.text() {
                show_error(&parent_win, "Пароли не совпадают.");
                d.destroy();
                return;
            }
            d.destroy();
            if let Some(f) = on_ok.borrow_mut().take() {
                f(password);
            }
        } else {
            d.destroy();
        }
    });

    dialog.show();
    entry.grab_focus();
}
