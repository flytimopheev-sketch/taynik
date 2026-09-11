//! Темы оформления: набор современных палитр поверх системной темы GTK.

use std::cell::RefCell;

use gtk4 as gtk;
use gtk4::gdk;

/// Описание одной темы.
pub struct Theme {
    pub key: &'static str,
    pub title: &'static str,
}

/// Доступные темы (ключ сохраняется в config.toml).
pub const THEMES: &[Theme] = &[
    Theme { key: "system", title: "Системная" },
    Theme { key: "light", title: "Светлая" },
    Theme { key: "dark", title: "Тёмная" },
    Theme { key: "nord", title: "Nord" },
    Theme { key: "dracula", title: "Dracula" },
    Theme { key: "solarized-dark", title: "Solarized Dark" },
    Theme { key: "solarized-light", title: "Solarized Light" },
];

thread_local! {
    static CURRENT: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Применить тему по ключу из THEMES.
pub fn apply(key: &str) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let dark = matches!(key, "dark" | "nord" | "dracula" | "solarized-dark");
    if let Some(s) = gtk::Settings::default() {
        s.set_gtk_application_prefer_dark_theme(dark);
    }
    CURRENT.with(|cell| {
        let mut cur = cell.borrow_mut();
        if let Some(old) = cur.take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }
        // Базовый CSS применяется всегда (и для системной темы тоже):
        // единые скругления полей ввода и панелей, как у главного окна.
        let base = gtk::CssProvider::new();
        base.load_from_data(BASE_CSS);
        gtk::style_context_add_provider_for_display(
            &display,
            &base,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        if let Some(css) = css_for(key) {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(css.as_str());
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            *cur = Some(provider);
        }
    });
}

/// Базовые переопределения (скругления), применяются для любой темы.
const BASE_CSS: &str = "
entry, spinbutton, textview, combobox button.combo {
    border-radius: 10px;
}
searchbar entry {
    border-radius: 10px;
}
frame > border, .card {
    border-radius: 12px;
}
";

/// CSS для ключа темы; None — использовать системную тему без переопределений.
fn css_for(key: &str) -> Option<String> {
    let (bg, bg2, fg, dim, sel, sel_fg, header) = match key {
        "dark" => (
            "#1e1e22", "#2a2a2f", "#e8e8ea", "#b9b9c0", "#3584e4", "#ffffff", "#303035",
        ),
        "nord" => (
            "#2e3440", "#3b4252", "#eceff4", "#d8dee9", "#88c0d0", "#2e3440", "#353c4a",
        ),
        "dracula" => (
            "#282a36", "#343746", "#f8f8f2", "#b0b0b8", "#bd93f9", "#282a36", "#21222c",
        ),
        "solarized-dark" => (
            "#002b36", "#073642", "#eee8d5", "#93a1a1", "#268bd2", "#fdf6e3", "#073642",
        ),
        "solarized-light" => (
            "#fdf6e3", "#eee8d5", "#073642", "#657b83", "#268bd2", "#fdf6e3", "#eee8d5",
        ),
        _ => return None,
    };
    Some(format!(
"window, dialog {{ background-color: {bg}; color: {fg}; }}
headerbar, .titlebar {{ background-color: {header}; color: {fg}; }}
searchbar {{ background-color: {bg}; }}
entry, spinbutton, textview, textview > text {{
    background-color: {bg2}; color: {fg};
}}
list, listview, list > row, listview > row {{
    background-color: {bg}; color: {fg};
}}
list > row:selected, listview > row:selected, .navigation-sidebar > row:selected {{
    background-color: {sel}; color: {sel_fg};
}}
button {{ background-color: {bg2}; color: {fg}; }}
button:hover {{ background-color: {sel}; color: {sel_fg}; }}
button.suggested-action {{ background-color: {sel}; color: {sel_fg}; }}
button.destructive-action {{ background-color: #cc3333; color: #ffffff; }}
popover, popover > contents, menu {{ background-color: {bg2}; color: {fg}; }}
.dim-label {{ color: {dim}; }}
scale trough, progressbar trough {{ background-color: {bg2}; }}
combobox button.combo {{ background-color: {bg2}; }}"
    ))
}
