//! Диалог генератора паролей-фраз (diceware).

use gtk4 as gtk;
use gtk4::prelude::*;

use crate::models::passphrase::{self, PassphraseOptions};

/// Показать генератор парольных фраз. `on_use` вызывается при нажатии
/// «Использовать».
pub fn show_generator(parent: &impl IsA<gtk::Window>, on_use: impl Fn(&str) + 'static) {
    let dialog = gtk::Dialog::builder()
        .title("Генератор парольных фраз")
        .modal(true)
        .transient_for(parent)
        .default_width(520)
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

    // Количество слов.
    let words = gtk::SpinButton::with_range(3.0, 12.0, 1.0);
    words.set_value(6.0);
    {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        row.append(&gtk::Label::with_mnemonic("_Количество слов:"));
        row.append(&words);
        box_.append(&row);
    }

    // Разделитель.
    let sep_dd = gtk::DropDown::from_strings(&["-", ".", "_", "Пробел", "Без разделителя"]);
    sep_dd.set_selected(0);
    {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        row.append(&gtk::Label::with_mnemonic("_Разделитель:"));
        row.append(&sep_dd);
        box_.append(&row);
    }

    let cb_capitalize = gtk::CheckButton::with_label("Капитализировать слова");
    let cb_number = gtk::CheckButton::with_label("Добавить число в конце");
    box_.append(&cb_capitalize);
    box_.append(&cb_number);

    let suffix_e = gtk::Entry::builder()
        .placeholder_text("Символ в конце (необязательно)")
        .build();
    box_.append(&suffix_e);

    let preview = gtk::Entry::builder().editable(false).hexpand(true).build();
    let strength_label = gtk::Label::new(None);
    box_.append(&preview);
    box_.append(&strength_label);

    let info = gtk::Label::builder()
        .label(format!(
            "Словарь: {} слов (10 бит энтропии на слово)",
            passphrase::WORDLIST_SIZE
        ))
        .halign(gtk::Align::Start)
        .css_classes(vec!["dim-label"])
        .build();
    box_.append(&info);

    let btn_gen = gtk::Button::with_label("Сгенерировать");
    let btn_copy = gtk::Button::with_label("Скопировать");
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
        let words = words.clone();
        let sep_dd = sep_dd.clone();
        let cb_capitalize = cb_capitalize.clone();
        let cb_number = cb_number.clone();
        let suffix_e = suffix_e.clone();
        move || -> PassphraseOptions {
            let separators = ["-", ".", "_", " ", ""];
            let idx = sep_dd.selected() as usize;
            PassphraseOptions {
                words: words.value() as usize,
                separator: separators.get(idx).copied().unwrap_or("-").to_string(),
                suffix: suffix_e.text().to_string(),
                append_number: cb_number.is_active(),
                capitalize: cb_capitalize.is_active(),
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
                super::show_info(&win, "Фраза скопирована в буфер обмена");
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
    opts: &PassphraseOptions,
    preview: &gtk::Entry,
    strength_label: &gtk::Label,
) -> Result<(), String> {
    let phrase = passphrase::generate(opts)?;
    let bits = passphrase::entropy_bits(opts);
    let s = passphrase::strength(opts);
    strength_label.set_text(&format!(
        "Надёжность: {} (≈{:.0} бит, {} слов)",
        s.label(),
        bits,
        opts.words
    ));
    strength_label.set_css_classes(&[s.css_class()]);
    preview.set_text(&phrase);
    Ok(())
}
