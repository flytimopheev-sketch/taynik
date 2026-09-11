//! Диалог настроек: автоблокировка, очистка буфера обмена,
//! блокировка при сворачивании, последовательность автоввода.

use gtk4 as gtk;
use gtk4::prelude::*;

use super::SharedState;

pub fn show(parent: &impl IsA<gtk::Window>, state: SharedState) {
    let dialog = gtk::Dialog::builder()
        .title("Настройки")
        .modal(true)
        .transient_for(parent)
        .default_width(480)
        .build();
    dialog.add_button("Отмена", gtk::ResponseType::Cancel);
    dialog.add_button("Сохранить", gtk::ResponseType::Ok);
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

    let cfg = state.config.borrow();

    let auto_lock = gtk::SpinButton::with_range(0.0, 240.0, 1.0);
    auto_lock.set_value(f64::from(cfg.auto_lock_minutes));
    auto_lock.set_tooltip_text(Some(
        "Автоблокировка базы через N минут бездействия (0 — отключить)",
    ));
    {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        row.append(&gtk::Label::with_mnemonic("Автоблокировка (мин):"));
        row.append(&auto_lock);
        box_.append(&row);
    }

    let clipboard = gtk::SpinButton::with_range(0.0, 600.0, 5.0);
    clipboard.set_value(cfg.clipboard_clear_seconds as f64);
    clipboard.set_tooltip_text(Some(
        "Очистка буфера обмена через N секунд после копирования пароля (0 — отключить)",
    ));
    {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        row.append(&gtk::Label::with_mnemonic("Очистка буфера (сек):"));
        row.append(&clipboard);
        box_.append(&row);
    }

    let lock_minimize =
        gtk::CheckButton::with_label("Блокировать при сворачивании окна");
    lock_minimize.set_active(cfg.lock_on_minimize);
    box_.append(&lock_minimize);

    let seq = gtk::Entry::builder()
        .text(cfg.autotype_sequence.as_str())
        .hexpand(true)
        .build();
    seq.set_tooltip_text(Some(
        "Последовательность автоввода. Токены: {USERNAME}, {PASSWORD}, \
         {TAB}, {ENTER}, {DELAY мс}; остальной текст вводится как есть",
    ));
    {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .build();
        row.append(&gtk::Label::with_mnemonic("Последовательность автоввода:"));
        row.append(&seq);
        let hint = gtk::Label::builder()
            .label("Пример: {USERNAME}{TAB}{PASSWORD}{ENTER}")
            .halign(gtk::Align::Start)
            .css_classes(vec!["dim-label"])
            .build();
        row.append(&hint);
        box_.append(&row);
    }
    drop(cfg);
    content.append(&box_);

    let state_clone = state.clone();
    dialog.connect_response(move |d, resp| {
        if resp == gtk::ResponseType::Ok {
            {
                let mut cfg = state_clone.config.borrow_mut();
                cfg.auto_lock_minutes = auto_lock.value() as u32;
                cfg.clipboard_clear_seconds = clipboard.value() as u64;
                cfg.lock_on_minimize = lock_minimize.is_active();
                let s = seq.text().to_string();
                cfg.autotype_sequence = if s.trim().is_empty() {
                    "{USERNAME}{TAB}{PASSWORD}".to_string()
                } else {
                    s
                };
                cfg.save();
            }
            d.destroy();
        } else {
            d.destroy();
        }
    });

    dialog.show();
}