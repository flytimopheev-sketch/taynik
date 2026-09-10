//! Инициализация приложения и глобальные действия.

use gtk4::prelude::*;
use gtk4::{self as gtk, gio};

/// Идентификатор приложения (совпадает с .desktop и AppStream).
pub const APP_ID: &str = "ru.redos.RedPass";

/// Создать gtk::Application с настройками (тёмная тема подхватывается
/// автоматически через схему GTK color-scheme).
pub fn build_application() -> gtk::Application {
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_startup(|_app| {
        // Предпочитаем тёмную тему, если пользователь её выбрал в системе.
        let settings = gtk::Settings::default();
        if let Some(s) = settings {
            s.set_gtk_application_prefer_dark_theme(true);
        }
    });

    app
}
