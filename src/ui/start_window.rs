//! Стартовый экран: создать / открыть базу / выход.

use gtk4 as gtk;
use gtk4::glib;
use gtk4::prelude::*;

use super::{create_db_dialog, prompt_unlock_and_open, SharedState};

pub fn show(app: &gtk::Application, state: SharedState) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Тайник — Менеджер паролей")
        .default_width(420)
        .default_height(280)
        .resizable(false)
        .build();

    let box_ = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .valign(gtk::Align::Center)
        .build();

    let title = gtk::Label::builder()
        .label("Тайник — Менеджер паролей")
        .css_classes(vec!["title-1"])
        .build();
    box_.append(&title);

    let btn_create = gtk::Button::with_label("Создать новую базу");
    let btn_open = gtk::Button::with_label("Открыть существующую базу");
    let btn_quit = gtk::Button::with_label("Выход");
    box_.append(&btn_create);
    box_.append(&btn_open);
    box_.append(&btn_quit);
    window.set_child(Some(&box_));

    {
        let win = window.clone();
        let state = state.clone();
        btn_create.connect_clicked(move |_| {
            create_db_dialog::show_create(&win, state.clone());
        });
    }
    {
        let win = window.clone();
        let state = state.clone();
        btn_open.connect_clicked(move |_| {
            open_existing(&win, &state);
        });
    }
    {
        let window = window.clone();
        btn_quit.connect_clicked(move |_| window.destroy());
    }

    // Если была открыта база ранее — сразу предложить её открыть.
    let last = state.config.borrow().last_database.clone();
    if let Some(p) = last {
        if std::path::Path::new(&p).exists() {
            let win = window.clone();
            let state = state.clone();
            glib::idle_add_local_once(move || {
                prompt_unlock_and_open(&win, &state, std::path::PathBuf::from(p));
            });
        }
    }

    window.show();
}

fn open_existing(parent: &impl IsA<gtk::Window>, state: &SharedState) {
    let chooser = gtk::FileChooserNative::builder()
        .title("Выберите файл базы (.rpwm)")
        .action(gtk::FileChooserAction::Open)
        .modal(true)
        .transient_for(parent)
        .build();
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("База Тайник (*.rpwm)"));
    filter.add_pattern("*.rpwm");
    chooser.add_filter(&filter);
    let state = state.clone();
    let win = parent.clone().upcast::<gtk::Window>();
    chooser.connect_response(move |c, resp| {
        if resp == gtk::ResponseType::Accept {
            if let Some(file) = c.file() {
                if let Some(path) = file.path() {
                    prompt_unlock_and_open(&win, &state, path);
                }
            }
        }
        c.destroy();
    });
    chooser.show();
}
