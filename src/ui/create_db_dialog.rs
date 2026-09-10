//! Диалог создания новой базы: выбор файла, мастер-пароль + подтверждение.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::prelude::*;

use super::main_window::MainWindow;
use super::SharedState;
use crate::db::repository::Database;

pub fn show_create(parent: &impl IsA<gtk::Window>, state: SharedState) {
    let dialog = gtk::Dialog::builder()
        .title("Создать новую базу")
        .modal(true)
        .transient_for(parent)
        .default_width(480)
        .build();
    dialog.add_button("Отмена", gtk::ResponseType::Cancel);
    let ok_btn = dialog.add_button("Создать", gtk::ResponseType::Ok);
    dialog.set_default_response(gtk::ResponseType::Ok);
    ok_btn.set_sensitive(false);

    let content = dialog.content_area();
    let box_ = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let file_btn = gtk::Button::with_label("Выбрать файл базы…");
    let file_label = gtk::Label::builder()
        .label("Файл: не выбран")
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();
    box_.append(&file_btn);
    box_.append(&file_label);

    let pass = gtk::PasswordEntry::builder().show_peek_icon(true).build();
    let confirm = gtk::PasswordEntry::builder().show_peek_icon(true).build();
    box_.append(&labeled("_Пароль:", &pass));
    box_.append(&labeled("_Подтверждение:", &confirm));

    let warn = gtk::Label::builder().css_classes(vec!["warning"]).build();
    box_.append(&warn);
    content.append(&box_);

    // Состояние: выбранный путь.
    let selected: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

    {
        let dialog = dialog.clone();
        let selected = selected.clone();
        let file_label = file_label.clone();
        file_btn.connect_clicked(move |_| {
            let chooser = gtk::FileChooserNative::builder()
                .title("Сохранить базу как")
                .action(gtk::FileChooserAction::Save)
                .modal(true)
                .transient_for(&dialog)
                .build();
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("База RedPass (*.rpwm)"));
            filter.add_pattern("*.rpwm");
            chooser.add_filter(&filter);
            chooser.set_current_name("passwords.rpwm");
            let selected = selected.clone();
            let file_label = file_label.clone();
            chooser.connect_response(move |c, resp| {
                if resp == gtk::ResponseType::Accept {
                    if let Some(path) = c.file().and_then(|f| f.path()) {
                        let mut p = path;
                        if p.extension().map(|e| e != "rpwm").unwrap_or(true) {
                            p.set_extension("rpwm");
                        }
                        file_label.set_text(&format!("Файл: {}", p.display()));
                        *selected.borrow_mut() = Some(p);
                    }
                }
                c.destroy();
            });
            chooser.show();
        });
    }

    // Валидация ввода. Замыкание клонируется для нескольких обработчиков
    // (все захваты Clone-типы).
    {
        let validate = {
            let dialog = dialog.clone();
            let selected = selected.clone();
            let pass = pass.clone();
            let confirm = confirm.clone();
            let warn = warn.clone();
            move || {
                let path_ok = selected.borrow().is_some();
                let p = pass.text().to_string();
                let c = confirm.text().to_string();
                let match_ok = !p.is_empty() && p == c;
                dialog
                    .widget_for_response(gtk::ResponseType::Ok)
                    .map(|w| w.set_sensitive(path_ok && match_ok));
                if !p.is_empty() && !match_ok {
                    warn.set_text("Пароли не совпадают");
                } else {
                    warn.set_text("");
                }
            }
        };
        let v1 = validate.clone();
        pass.connect_changed(move |_| v1());
        let v2 = validate.clone();
        confirm.connect_changed(move |_| v2());
        let v3 = validate.clone();
        file_btn.connect_clicked(move |_| v3());
    }

    let dlg = dialog.clone();
    dialog.connect_response(move |d, resp| {
        if resp == gtk::ResponseType::Ok {
            let path = selected.borrow().clone();
            if let Some(path) = path {
                let password = pass.text().to_string();
                match Database::create(&path, &password) {
                    Ok(db) => {
                        state.config.borrow_mut().last_database =
                            Some(path.display().to_string());
                        state.config.borrow().save();
                        *state.db.borrow_mut() = Some(db);
                        let win = d.clone().upcast::<gtk::Window>();
                        // Главное окно само отложенно закроет диалог (см. MainWindow::new).
                        MainWindow::new(&win, state.clone());
                        // Стартовое окно (владелец диалога) больше не нужно:
                        // закрываем его, чтобы оно не осталось за главным окном.
                        if let Some(owner) = d.transient_for() {
                            owner.destroy();
                        }
                    }
                    Err(e) => {
                        super::show_error(&dlg, &format!("Не удалось создать базу:\n{e}"));
                    }
                }
            }
        } else {
            d.destroy();
        }
    });

    dialog.show();
}

fn labeled(label: &str, w: &impl IsA<gtk::Widget>) -> gtk::Box {
    let b = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    let l = gtk::Label::with_mnemonic(label);
    l.set_mnemonic_widget(Some(w));
    b.append(&l);
    b.append(w);
    b
}
