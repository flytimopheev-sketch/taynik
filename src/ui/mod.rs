//! Модуль пользовательского интерфейса (GTK4).

pub mod autotype;
pub mod create_db_dialog;
pub mod main_window;
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
    unlock_dialog::show(parent, &path, move |parent, password| {
        match Database::open(&path_clone, &password) {
            Ok(db) => {
                state.config.borrow_mut().last_database =
                    Some(path_clone.display().to_string());
                state.config.borrow().save();
                *state.db.borrow_mut() = Some(db);
                main_window::MainWindow::new(&parent, state.clone());
            }
            Err(DbError::WrongPassword) => {
                show_error(&parent, "Неверный мастер-пароль. Попробуйте ещё раз.");
                prompt_unlock_and_open(&parent, &state, path_clone);
            }
            Err(e) => show_error(&parent, &format!("Не удалось открыть базу:\n{e}")),
        }
    });
}
