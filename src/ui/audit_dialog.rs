//! Диалог аудита безопасности: слабые, повторяющиеся и устаревшие пароли.

use gtk4 as gtk;
use gtk4::prelude::*;

use crate::models::audit::{self, Issue};
use crate::models::entry::Entry;
use crate::models::generator::Strength;

/// Показать результаты аудита.
///
/// `on_open` вызывается при выборе записи — чтобы открыть её в главном окне.
pub fn show(
    parent: &impl IsA<gtk::Window>,
    entries: &[Entry],
    old_days: u64,
    min_strength: Strength,
    on_open: std::rc::Rc<dyn Fn(i64)>,
) {
    let issues = audit::audit(entries, old_days, min_strength);
    let summary = audit::summarize(&issues);

    let dialog = gtk::Dialog::builder()
        .title("Аудит безопасности")
        .modal(true)
        .transient_for(parent)
        .default_width(640)
        .default_height(480)
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

    let header = if issues.is_empty() {
        "Проблем не найдено — все пароли выглядят надёжно.".to_string()
    } else {
        format!(
            "Найдено проблем: всего {}, слабых {}, повторяющихся {}, устаревших {}.",
            summary.total(),
            summary.weak,
            summary.duplicate,
            summary.old
        )
    };
    let header_l = gtk::Label::builder()
        .label(header)
        .wrap(true)
        .halign(gtk::Align::Start)
        .build();
    if issues.is_empty() {
        header_l.add_css_class("success");
    } else {
        header_l.add_css_class("warning");
    }
    box_.append(&header_l);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");

    if issues.is_empty() {
        list.append(
            &gtk::ListBoxRow::builder()
                .child(&gtk::Label::new(Some(
                    "Все пароли стойкие, не повторяются и менялись недавно.",
                )))
                .build(),
        );
    }

    for issue in &issues {
        list.append(&issue_row(issue, &on_open));
    }
    scroll.set_child(Some(&list));
    box_.append(&scroll);
    content.append(&box_);

    dialog.connect_response(|d, _| d.destroy());
    dialog.show();
}

/// Строка списка: тип проблемы, название записи и пояснение.
fn issue_row(issue: &Issue, on_open: &std::rc::Rc<dyn Fn(i64)>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let box_ = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(8)
        .margin_end(8)
        .build();

    let kind_l = gtk::Label::new(Some(issue.kind.label()));
    kind_l.add_css_class(issue.kind.css_class());
    kind_l.set_halign(gtk::Align::Start);
    kind_l.set_size_request(160, -1);
    box_.append(&kind_l);

    let title_l = gtk::Label::builder()
        .label(issue.title.as_str())
        .halign(gtk::Align::Start)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();
    box_.append(&title_l);

    let detail_l = gtk::Label::new(Some(&issue.detail));
    detail_l.add_css_class("dim-label");
    detail_l.set_halign(gtk::Align::End);
    box_.append(&detail_l);

    if let Some(id) = issue.entry_id {
        let btn = gtk::Button::with_label("Открыть");
        let on_open = on_open.clone();
        btn.connect_clicked(move |_| on_open(id));
        box_.append(&btn);
    }

    row.set_child(Some(&box_));
    row
}
