//! Диалог генератора паролей.

use gtk4 as gtk;
use gtk4::prelude::*;

use crate::models::generator::{self, GeneratorOptions, Strength};

/// Показать генератор. `on_use` вызывается при нажатии «Использовать».
pub fn show_generator(
    parent: &impl IsA<gtk::Window>,
    on_use: impl Fn(&str) + 'static,
) {
    let dialog = gtk::Dialog::builder()
        .title("Генератор паролей")
        .modal(true)
        .transient_for(parent)
        .default_width(460)
        .build();
    dialog.add_button("Закрыть", gtk::ResponseType::Close);

    let content = dialog.content_area();
    let box_ = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    // Длина.
    let length = gtk::SpinButton::with_range(8.0, 64.0, 1.0);
    length.set_value(20.0);
    {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        row.append(&gtk::Label::with_mnemonic("Длина (8–64):"));
        row.append(&length);
        box_.append(&row);
    }

    // Наборы символов.
    let cb_lower = gtk::CheckButton::with_label("Строчные (a–z)");
    let cb_upper = gtk::CheckButton::with_label("Заглавные (A–Z)");
    let cb_digits = gtk::CheckButton::with_label("Цифры (0–9)");
    let cb_symbols = gtk::CheckButton::with_label("Спецсимволы (!@#$…)");
    let cb_nosimilar = gtk::CheckButton::with_label("Исключить похожие (l, 1, I, O, 0)");
    cb_lower.set_active(true);
    cb_upper.set_active(true);
    cb_digits.set_active(true);
    cb_symbols.set_active(true);
    cb_nosimilar.set_active(true);
    for cb in [&cb_lower, &cb_upper, &cb_digits, &cb_symbols, &cb_nosimilar] {
        box_.append(cb);
    }

    // Минимумы.
    let min_digits = gtk::SpinButton::with_range(0.0, 16.0, 1.0);
    min_digits.set_value(2.0);
    let min_symbols = gtk::SpinButton::with_range(0.0, 16.0, 1.0);
    min_symbols.set_value(2.0);
    {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        row.append(&gtk::Label::with_mnemonic("Мин. цифр:"));
        row.append(&min_digits);
        row.append(&gtk::Label::with_mnemonic("Мин. спецсимволов:"));
        row.append(&min_symbols);
        box_.append(&row);
    }

    // Предпросмотр.
    let preview = gtk::Entry::builder().editable(false).hexpand(true).build();
    let strength_label = gtk::Label::new(None);
    box_.append(&preview);
    box_.append(&strength_label);

    // Кнопки.
    let btn_gen = gtk::Button::with_label("Сгенерировать");
    let btn_copy = gtk::Button::with_label("Копировать");
    let btn_use = gtk::Button::with_label("Использовать");
    let btns = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    btns.append(&btn_gen);
    btns.append(&btn_copy);
    btns.append(&btn_use);
    box_.append(&btns);
    content.append(&box_);

    let read_opts = {
        let length = length.clone();
        let cb_lower = cb_lower.clone();
        let cb_upper = cb_upper.clone();
        let cb_digits = cb_digits.clone();
        let cb_symbols = cb_symbols.clone();
        let cb_nosimilar = cb_nosimilar.clone();
        let min_digits = min_digits.clone();
        let min_symbols = min_symbols.clone();
        move || -> GeneratorOptions {
            GeneratorOptions {
                length: length.value() as usize,
                lowercase: cb_lower.is_active(),
                uppercase: cb_upper.is_active(),
                digits: cb_digits.is_active(),
                symbols: cb_symbols.is_active(),
                exclude_similar: cb_nosimilar.is_active(),
                min_digits: min_digits.value() as usize,
                min_symbols: min_symbols.value() as usize,
            }
        }
    };

    let dlg_err = dialog.clone();
    let preview_gen = preview.clone();
    let strength_gen = strength_label.clone();
    let do_generate = move || match generate_and_show(&read_opts(), &preview_gen, &strength_gen) {
        Ok(()) => {}
        Err(e) => super::show_error(&dlg_err, &e),
    };

    {
        let do_generate = do_generate.clone();
        btn_gen.connect_clicked(move |_| do_generate());
    }
    {
        let preview = preview.clone();
        let win = dialog.clone();
        btn_copy.connect_clicked(move |_| {
            let text = preview.text().to_string();
            if !text.is_empty() {
                win.clipboard().set_text(&text);
                super::show_info(&win, "Пароль скопирован в буфер обмена");
            }
        });
    }
    {
        let preview = preview.clone();
        let dialog = dialog.clone();
        btn_use.connect_clicked(move |_| {
            let text = preview.text().to_string();
            if !text.is_empty() {
                on_use(&text);
                dialog.destroy();
            }
        });
    }

    dialog.connect_response(|d, _| d.destroy());
    dialog.show();
    do_generate();
}

fn generate_and_show(
    opts: &GeneratorOptions,
    preview: &gtk::Entry,
    strength_label: &gtk::Label,
) -> Result<(), String> {
    let pass = generator::generate(opts)?;
    let s = generator::strength(&pass);
    strength_label.set_text(&format!(
        "Надёжность: {} ({} символов)",
        s.label(),
        pass.chars().count()
    ));
    strength_label.set_css_classes(&match s {
        Strength::Weak => vec!["error"],
        Strength::Medium => vec!["warning"],
        Strength::Strong => vec!["success"],
    });
    preview.set_text(&pass);
    Ok(())
}
