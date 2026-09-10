//! Диалог ввода мастер-пароля.

use std::path::Path;

use gtk4 as gtk;
use gtk4::prelude::*;

/// Показать диалог ввода мастер-пароля; `on_ok` вызывается с введённым паролем.
pub fn show(parent: &impl IsA<gtk::Window>, _path: &Path, on_ok: impl FnOnce(gtk::Window, String) + 'static) {
    let dialog = gtk::Dialog::builder()
        .title("Введите мастер-пароль")
        .modal(true)
        .transient_for(parent)
        .default_width(440)
        .build();
    dialog.add_button("Отмена", gtk::ResponseType::Cancel);
    dialog.add_button("OK", gtk::ResponseType::Ok);
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

    let entry = gtk::PasswordEntry::builder()
        .show_peek_icon(true)
        .placeholder_text("Пароль")
        .activates_default(true)
        .build();
    box_.append(&gtk::Label::with_mnemonic("_Пароль:"));
    box_.append(&entry);
    content.append(&box_);

    let dlg = dialog.clone();
    let parent_win: gtk::Window = parent.clone().upcast();
    entry.connect_activate(move |_| dlg.response(gtk::ResponseType::Ok));

    let parent_win2 = parent_win.clone();
    let entry_cb = entry.clone();
    let on_ok = std::cell::RefCell::new(Some(on_ok));
    dialog.connect_response(move |d, resp| {
        if resp == gtk::ResponseType::Ok {
            let password = entry_cb.text().to_string();
            d.destroy();
            // Пустой пароль обрабатывается обычным путём: Database::open
            // вернёт WrongPassword, и пользователю снова покажут диалог.
            if let Some(f) = on_ok.borrow_mut().take() {
                f(parent_win2.clone(), password);
            }
        } else {
            d.destroy();
        }
    });

    dialog.show();
    entry.grab_focus();
}
