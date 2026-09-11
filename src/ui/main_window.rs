//! Главное окно: список записей, поиск, фильтр по тегам, детали записи.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::gio;
use gtk4::gio::prelude::*;
use gtk4::glib;
use gtk4::prelude::*;

use super::password_generator;
use super::{prompt_unlock_and_open, show_error, SharedState};
use crate::models::entry::Entry;
use crate::models::generator;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Title,
    Updated,
}

pub struct MainWindow {
    window: gtk::Window,
    state: SharedState,
    entries: RefCell<Vec<Entry>>,
    edit_id: Cell<Option<i64>>,
    sort_mode: Cell<SortMode>,
    search: gtk::SearchEntry,
    tag_dd: gtk::DropDown,
    list: gtk::ListBox,
    toast: gtk::Revealer,
    toast_label: gtk::Label,
    row_ids: RefCell<Vec<i64>>,
    title_e: gtk::Entry,
    username_e: gtk::Entry,
    password_e: gtk::PasswordEntry,
    url_e: gtk::Entry,
    tags_e: gtk::Entry,
    notes_buf: gtk::TextBuffer,
    fav_cb: gtk::CheckButton,
    strength_l: gtk::Label,
    updated_l: gtk::Label,
    last_activity: Rc<Cell<i64>>,
    /// Защита от повторной блокировки (иначе открывается второе окно).
    locked: Cell<bool>,
}

impl MainWindow {
    pub fn new(parent: &gtk::Window, state: SharedState) -> Rc<MainWindow> {
        let app = parent.application();

        let window = gtk::Window::builder()
            .title("Тайник")
            .default_width(1000)
            .default_height(640)
            .build();
        if let Some(app) = &app {
            window.set_application(Some(app));
        }

        let m = Rc::new(MainWindow {
            window: window.clone(),
            state: state.clone(),
            entries: RefCell::new(Vec::new()),
            edit_id: Cell::new(None),
            sort_mode: Cell::new(SortMode::Updated),
            search: gtk::SearchEntry::new(),
            tag_dd: gtk::DropDown::from_strings(&["Все теги"]),
            list: gtk::ListBox::new(),
            toast: gtk::Revealer::builder()
                .transition_type(gtk::RevealerTransitionType::SlideDown)
                .reveal_child(false)
                .valign(gtk::Align::Start)
                .build(),
            toast_label: gtk::Label::new(None),
            row_ids: RefCell::new(Vec::new()),
            title_e: gtk::Entry::new(),
            username_e: gtk::Entry::new(),
            password_e: gtk::PasswordEntry::builder().show_peek_icon(true).build(),
            url_e: gtk::Entry::new(),
            tags_e: gtk::Entry::new(),
            notes_buf: gtk::TextBuffer::new(None),
            fav_cb: gtk::CheckButton::with_label("Избранное"),
            strength_l: gtk::Label::new(None),
            updated_l: gtk::Label::new(None),
            last_activity: Rc::new(Cell::new(glib::monotonic_time())),
            locked: Cell::new(false),
        });

        m.setup_header();
        m.setup_ui();
        m.setup_actions(&app);
        m.setup_auto_lock();
        m.refresh();

        window.connect_close_request({
            let m = m.clone();
            move |_| {
                *m.state.db.borrow_mut() = None;
                glib::Propagation::Proceed
            }
        });
        window.show();
        // Нельзя уничтожать родителя синхронно: мы, скорее всего, находимся
        // внутри его обработчика сигнала (кнопка/диалог). Откладываем до
        // следующей итерации главного цикла GTK.
        glib::idle_add_local_once({
            let parent = parent.clone();
            move || parent.destroy()
        });
        m
    }

    fn setup_header(self: &Rc<Self>) {
        let header = gtk::HeaderBar::new();
        let mb = gtk::PopoverMenuBar::from_model(Some(&self.build_menu()));
        header.set_title_widget(Some(&mb));

        let btn_new = gtk::Button::with_label("＋ Запись");
        btn_new.set_tooltip_text(Some("Новая запись (Ctrl+N)"));
        btn_new.add_css_class("suggested-action");
        {
            let m = self.clone();
            btn_new.connect_clicked(move |_| m.new_entry());
        }
        header.pack_end(&btn_new);
        self.window.set_titlebar(Some(&header));
    }

    fn build_menu(&self) -> gio::Menu {
        let file_menu = gio::Menu::new();
        file_menu.append(Some("Создать новую базу…"), Some("win.create-db"));
        file_menu.append(Some("Открыть базу…"), Some("win.open-db"));
        file_menu.append(Some("Сменить мастер-пароль…"), Some("win.change-password"));
        file_menu.append(Some("Создать резервную копию…"), Some("win.backup"));
        file_menu.append(Some("Заблокировать"), Some("win.lock"));
        file_menu.append(Some("Выход"), Some("win.quit"));

        let edit_menu = gio::Menu::new();
        edit_menu.append(Some("Новая запись"), Some("win.new-entry"));
        edit_menu.append(Some("Удалить запись"), Some("win.delete-entry"));

        let view_menu = gio::Menu::new();
        view_menu.append(Some("Сортировать по названию"), Some("win.sort-title"));
        view_menu.append(Some("Сортировать по дате изменения"), Some("win.sort-updated"));

        let theme_menu = gio::Menu::new();
        for t in super::theme::THEMES {
            theme_menu.append(Some(t.title), Some(&format!("win.theme::{}", t.key)));
        }
        view_menu.append_submenu(Some("Тема оформления"), &theme_menu);

        let help_menu = gio::Menu::new();
        help_menu.append(Some("О программе"), Some("win.about"));

        let menubar = gio::Menu::new();
        menubar.append_submenu(Some("Файл"), &file_menu);
        menubar.append_submenu(Some("Правка"), &edit_menu);
        menubar.append_submenu(Some("Вид"), &view_menu);
        menubar.append_submenu(Some("Помощь"), &help_menu);
        menubar
    }
}

impl MainWindow {
    fn setup_actions(self: &Rc<Self>, app: &Option<gtk::Application>) {
        let group = gio::SimpleActionGroup::new();
        let m = self.clone();
        let add = |name: &str, f: Box<dyn Fn(&Rc<MainWindow>)>| {
            let action = gio::SimpleAction::new(name, None);
            let mm = m.clone();
            action.connect_activate(move |_, _| f(&mm));
            group.add_action(&action);
        };
        add("new-entry", Box::new(|m| m.new_entry()));
        add("delete-entry", Box::new(|m| m.delete_entry()));
        add("lock", Box::new(|m| m.lock()));
        add("autotype", Box::new(|m| m.autotype_selected()));
        add("quit", Box::new(|m| m.window.destroy()));
        add("create-db", Box::new(|m| {
            super::create_db_dialog::show_create(&m.window, m.state.clone());
        }));
        add("open-db", Box::new(|m| {
            let chooser = gtk::FileChooserNative::builder()
                .title("Открыть базу")
                .action(gtk::FileChooserAction::Open)
                .modal(true)
                .transient_for(&m.window)
                .build();
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("База Тайник (*.rpwm)"));
            filter.add_pattern("*.rpwm");
            chooser.add_filter(&filter);
            let win = m.window.clone();
            let state = m.state.clone();
            chooser.connect_response(move |c, resp| {
                if resp == gtk::ResponseType::Accept {
                    if let Some(path) = c.file().and_then(|f| f.path()) {
                        prompt_unlock_and_open(&win, &state, path);
                    }
                }
                c.destroy();
            });
            chooser.show();
        }));
        add("change-password", Box::new(|m| {
            m.show_change_password();
        }));
        add("backup", Box::new(|m| m.backup()));
        add("sort-title", Box::new(|m| {
            m.sort_mode.set(SortMode::Title);
            m.apply_filter();
        }));
        add("sort-updated", Box::new(|m| {
            m.sort_mode.set(SortMode::Updated);
            m.apply_filter();
        }));
        add("about", Box::new(|m| {
            gtk::AboutDialog::builder()
                .title("О программе")
                .program_name("Тайник")
                .version(env!("CARGO_PKG_VERSION"))
                .comments("Офлайн-менеджер паролей с автовводом и темами оформления")
                .website("https://github.com/flytimopheev-sketch/taynik")
                .copyright("© 2026 Taynik authors")
                .authors(vec!["Taynik authors".to_string()])
                .license_type(gtk::License::MitX11)
                .modal(true)
                .transient_for(&m.window)
                .build()
                .show();
        }));
        add("focus-search", Box::new(|m| {
            m.search.grab_focus();
        }));

        // Переключение темы оформления (Вид → Тема оформления).
        let theme = self.state.config.borrow().theme.clone();
        let theme_action = gio::SimpleAction::new_stateful(
            "theme",
            Some(&glib::VariantTy::STRING),
            &glib::Variant::from(theme.as_str()),
        );
        {
            let m = self.clone();
            theme_action.connect_activate(move |a, param| {
                let key = param
                    .and_then(|v| v.str().map(|s| s.to_string()))
                    .unwrap_or_default();
                super::theme::apply(&key);
                a.set_state(&glib::Variant::from(key.as_str()));
                m.state.config.borrow_mut().theme = key;
                m.state.config.borrow().save();
            });
        }
        group.add_action(&theme_action);

        self.window.insert_action_group("win", Some(&group));

        if let Some(app) = app {
            app.set_accels_for_action("win.new-entry", &["<Ctrl>n"]);
            app.set_accels_for_action("win.focus-search", &["<Ctrl>f"]);
            app.set_accels_for_action("win.quit", &["<Ctrl>q"]);
            app.set_accels_for_action("win.lock", &["<Ctrl>l"]);
            app.set_accels_for_action("win.autotype", &["<Ctrl><Shift>v"]);
        }
    }
}

impl MainWindow {
    fn setup_ui(self: &Rc<Self>) {
        let overlay = gtk::Overlay::new();
        self.window.set_child(Some(&overlay));

        // --- Панель поиска и тегов ---
        let search_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        search_row.append(&gtk::Label::with_mnemonic("Поиск:"));
        self.search.set_hexpand(true);
        self.search.set_placeholder_text(Some("Название, логин, URL, теги…"));
        search_row.append(&self.search);
        search_row.append(&gtk::Label::with_mnemonic("Теги:"));
        self.tag_dd.set_tooltip_text(Some("Фильтр по тегу"));
        search_row.append(&self.tag_dd);

        // --- Левая часть: список записей ---
        let left_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        self.list.set_selection_mode(gtk::SelectionMode::Single);
        self.list.add_css_class("navigation-sidebar");
        left_scroll.set_child(Some(&self.list));

        // --- Правая часть: детали записи ---
        let form = gtk::Grid::builder()
            .row_spacing(8)
            .column_spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        for w in [&self.title_e, &self.username_e, &self.url_e, &self.tags_e] {
            w.set_hexpand(true);
        }
        self.password_e.set_hexpand(true);
        Self::add_form_row(&form, 0, "Название:", &self.title_e);
        Self::add_form_row(&form, 1, "Логин:", &self.username_e);
        Self::add_form_row(&form, 2, "Пароль:", &self.password_e);
        Self::add_form_row(&form, 3, "URL:", &self.url_e);
        Self::add_form_row(&form, 4, "Теги:", &self.tags_e);
        self.tags_e.set_tooltip_text(Some("Разделяйте теги запятыми"));

        form.attach(&gtk::Label::new(Some("Заметки:")), 0, 5, 1, 1);
        let notes = gtk::TextView::with_buffer(&self.notes_buf);
        notes.set_hexpand(true);
        notes.set_vexpand(true);
        notes.set_wrap_mode(gtk::WrapMode::WordChar);
        let notes_scroll = gtk::ScrolledWindow::builder()
            .child(&notes)
            .height_request(80)
            .build();
        form.attach(&notes_scroll, 1, 5, 1, 1);

        form.attach(&gtk::Label::new(Some("Изменена:")), 0, 6, 1, 1);
        self.updated_l.set_halign(gtk::Align::Start);
        self.updated_l.set_text("—");
        form.attach(&self.updated_l, 1, 6, 1, 1);
        form.attach(&self.fav_cb, 1, 7, 1, 1);

        // --- Кнопки ---
        // FlowBox вместо горизонтального Box: кнопки переносятся на новую
        // строку при узкой панели, поэтому «Удалить» никогда не обрезается.
        let btns = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .activate_on_single_click(false)
            .min_children_per_line(2)
            .max_children_per_line(3)
            .homogeneous(true)
            .column_spacing(8)
            .row_spacing(8)
            .margin_top(8)
            .hexpand(true)
            .build();
        let btn_gen = gtk::Button::with_label("Сгенерировать");
        let btn_copy = gtk::Button::with_label("Копировать пароль");
        let btn_autotype = gtk::Button::with_label("Автоввод");
        let btn_save = gtk::Button::with_label("Сохранить");
        let btn_cancel = gtk::Button::with_label("Отмена");
        let btn_delete = gtk::Button::with_label("Удалить");
        btn_save.add_css_class("suggested-action");
        btn_delete.add_css_class("destructive-action");
        for b in [&btn_gen, &btn_copy, &btn_autotype, &btn_save, &btn_cancel, &btn_delete] {
            b.set_hexpand(true);
        }
        btns.insert(&btn_gen, -1);
        btns.insert(&btn_copy, -1);
        btns.insert(&btn_autotype, -1);
        btns.insert(&btn_save, -1);
        btns.insert(&btn_cancel, -1);
        btns.insert(&btn_delete, -1);
        // Индикатор стойкости пароля (обновляется в update_strength).
        self.strength_l.set_halign(gtk::Align::Start);
        form.attach(&self.strength_l, 1, 8, 1, 1);
        form.attach(&btns, 1, 9, 1, 1);

        // --- Компоновка панелей ---
        let details_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&form)
            .build();
        let paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .start_child(&self.wrap_frame(&left_scroll))
            .end_child(&self.wrap_frame(&details_scroll))
            .position(320)
            .vexpand(true)
            .build();

        let root = gtk::Box::builder().orientation(gtk::Orientation::Vertical).build();
        root.append(&search_row);

        // Встраиваемое уведомление (замена gtk::ToastOverlay для GTK < 4.10).
        {
            let notif = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .build();
            self.toast_label.set_halign(gtk::Align::Center);
            self.toast_label.add_css_class("osd");
            self.toast_label.set_margin_top(4);
            self.toast_label.set_margin_bottom(4);
            notif.append(&self.toast_label);
            self.toast.set_child(Some(&notif));
        }

        root.append(&paned);

        overlay.set_child(Some(&root));
        overlay.add_overlay(&self.toast);
        self.toast.set_halign(gtk::Align::Fill);
        self.toast.set_valign(gtk::Align::Start);

        self.connect_signals(btn_gen, btn_copy, btn_save, btn_cancel, btn_delete, btn_autotype);
    }

    fn wrap_frame(self: &Rc<Self>, w: &impl IsA<gtk::Widget>) -> gtk::Frame {
        let f = gtk::Frame::new(None);
        f.set_margin_top(4);
        f.set_margin_bottom(8);
        f.set_margin_start(8);
        f.set_margin_end(8);
        f.set_child(Some(w));
        f
    }

    fn connect_signals(
        self: &Rc<Self>,
        btn_gen: gtk::Button,
        btn_copy: gtk::Button,
        btn_save: gtk::Button,
        btn_cancel: gtk::Button,
        btn_delete: gtk::Button,
        btn_autotype: gtk::Button,
    ) {
        {
            let m = self.clone();
            self.search.connect_search_changed(move |_| m.apply_filter());
        }
        {
            let m = self.clone();
            self.tag_dd.connect_selected_notify(move |_| m.apply_filter());
        }
        {
            let m = self.clone();
            self.list.connect_row_activated(move |_, row| {
                let idx = row.index();
                if let Some(id) = m.row_ids.borrow().get(idx as usize).copied() {
                    m.load_entry(id);
                }
            });
        }
        {
            let m = self.clone();
            self.password_e.connect_changed(move |_| m.update_strength());
        }
        {
            let m = self.clone();
            btn_gen.connect_clicked(move |_| {
                let pass_e = m.password_e.clone();
                password_generator::show_generator(&m.window, move |pass| {
                    pass_e.set_text(pass);
                });
            });
        }
        {
            let m = self.clone();
            btn_copy.connect_clicked(move |_| m.copy_password());
        }
        {
            let m = self.clone();
            btn_autotype.connect_clicked(move |_| m.autotype_selected());
        }
        {
            let m = self.clone();
            btn_save.connect_clicked(move |_| m.save_current());
        }
        {
            let m = self.clone();
            btn_cancel.connect_clicked(move |_| m.clear_details());
        }
        {
            let m = self.clone();
            btn_delete.connect_clicked(move |_| m.delete_entry());
        }

        // Учёт активности для автоблокировки.
        let last = self.last_activity.clone();
        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, _, _, _| {
            last.set(glib::monotonic_time());
            glib::Propagation::Proceed
        });
        self.window.add_controller(key_ctrl);
        let last = self.last_activity.clone();
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(move |_, _, _| last.set(glib::monotonic_time()));
        self.window.add_controller(motion);
    }

    fn setup_auto_lock(self: &Rc<Self>) {
        // Периодическая проверка бездействия (каждые 30 секунд).
        {
            let m = self.clone();
            glib::timeout_add_seconds_local(30, move || {
                let minutes = m.state.config.borrow().auto_lock_minutes;
                if minutes > 0 && m.state.db.borrow().is_some() {
                    let idle_secs = (glib::monotonic_time() - m.last_activity.get()) / 1_000_000;
                    if idle_secs >= i64::from(minutes) * 60 {
                        m.lock();
                        return glib::ControlFlow::Break;
                    }
                }
                glib::ControlFlow::Continue
            });
        }

        // Блокировка при сворачивании (опция).
        {
            let m = self.clone();
            self.window.connect_is_active_notify(move |_| {
                if m.state.config.borrow().lock_on_minimize
                    && !m.window.is_active()
                    && m.state.db.borrow().is_some()
                {
                    m.lock();
                }
            });
        }
    }

    fn add_form_row(grid: &gtk::Grid, row: i32, label: &str, w: &impl IsA<gtk::Widget>) {
        let l = gtk::Label::with_mnemonic(label);
        l.set_halign(gtk::Align::Start);
        l.set_mnemonic_widget(Some(w));
        grid.attach(&l, 0, row, 1, 1);
        grid.attach(w, 1, row, 1, 1);
    }
}

impl MainWindow {
    /// Загрузить записи из базы и обновить список.
    pub fn refresh(self: &Rc<Self>) {
        let entries = match self.state.db.borrow().as_ref() {
            Some(db) => db.list_entries().unwrap_or_default(),
            None => Vec::new(),
        };
        self.entries.replace(entries);
        self.rebuild_tag_filter();
        self.apply_filter();
        self.clear_details();
    }

    fn rebuild_tag_filter(self: &Rc<Self>) {
        let mut tags: Vec<String> = Vec::new();
        for e in self.entries.borrow().iter() {
            for t in &e.tags {
                if !tags.contains(t) {
                    tags.push(t.clone());
                }
            }
        }
        tags.sort();
        let mut items = vec!["Все теги".to_string()];
        items.extend(tags);
        let store = gio::ListStore::new::<gtk::StringObject>();
        for s in &items {
            store.append(&gtk::StringObject::new(s));
        }
        self.tag_dd.set_model(Some(&store));
    }

    fn apply_filter(self: &Rc<Self>) {
        let query = self.search.text().to_lowercase();
        let tag = self
            .tag_dd
            .selected_item()
            .and_then(|o| o.downcast::<gtk::StringObject>().ok())
            .map(|s| s.string().to_string())
            .unwrap_or_else(|| "Все теги".into());

        let mut filtered: Vec<Entry> = self
            .entries
            .borrow()
            .iter()
            .filter(|e| {
                let tag_ok = tag == "Все теги" || e.tags.iter().any(|t| *t == tag);
                let q = &query;
                let text_ok = q.is_empty()
                    || e.title.to_lowercase().contains(q.as_str())
                    || e.username.to_lowercase().contains(q.as_str())
                    || e.url.to_lowercase().contains(q.as_str())
                    || e.tags.iter().any(|t| t.to_lowercase().contains(q.as_str()));
                tag_ok && text_ok
            })
            .cloned()
            .collect();

        match self.sort_mode.get() {
            SortMode::Title => filtered.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
            SortMode::Updated => filtered.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
        }

        while let Some(row) = self.list.row_at_index(0) {
            self.list.remove(&row);
        }
        // Очищаем соответствие строк и id записей перед повторным заполнением,
        // иначе клик по строке после фильтрации откроет чужую запись.
        self.row_ids.borrow_mut().clear();
        for e in &filtered {
            let star = if e.favorite { "★ " } else { "" };
            let row = gtk::ListBoxRow::new();
            let label = gtk::Label::builder()
                .label(format!("{star}{}", e.title))
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            row.set_child(Some(&label));
            self.row_ids.borrow_mut().push(e.id.unwrap_or(-1));
            self.list.append(&row);
        }
    }

    fn load_entry(self: &Rc<Self>, id: i64) {
        let entry = self.entries.borrow().iter().find(|e| e.id == Some(id)).cloned();
        if let Some(e) = entry {
            self.edit_id.set(e.id);
            self.title_e.set_text(&e.title);
            self.username_e.set_text(&e.username);
            self.password_e.set_text(&e.password);
            self.url_e.set_text(&e.url);
            self.tags_e.set_text(&e.tags.join(", "));
            self.notes_buf.set_text(&e.notes);
            self.fav_cb.set_active(e.favorite);
            self.updated_l.set_text(&e.updated_at_str());
        }
    }

    pub fn new_entry(self: &Rc<Self>) {
        self.clear_details();
        self.title_e.grab_focus();
    }

    fn clear_details(self: &Rc<Self>) {
        self.edit_id.set(None);
        self.title_e.set_text("");
        self.username_e.set_text("");
        self.password_e.set_text("");
        self.url_e.set_text("");
        self.tags_e.set_text("");
        self.notes_buf.set_text("");
        self.fav_cb.set_active(false);
        self.updated_l.set_text("—");
    }

    fn save_current(self: &Rc<Self>) {
        let title = self.title_e.text().to_string();
        let password = self.password_e.text().to_string();
        if title.is_empty() || password.is_empty() {
            show_error(&self.window, "Название и пароль не могут быть пустыми.");
            return;
        }
        let tags: Vec<String> = self
            .tags_e
            .text()
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        let mut entry = Entry {
            id: self.edit_id.get(),
            title,
            username: self.username_e.text().to_string(),
            password,
            url: self.url_e.text().to_string(),
            notes: self
                .notes_buf
                .text(&self.notes_buf.start_iter(), &self.notes_buf.end_iter(), true)
                .to_string(),
            tags,
            favorite: self.fav_cb.is_active(),
            created_at: 0,
            updated_at: 0,
        };
        // Если база не открыта (например, после блокировки) — не сообщаем
        // об успехе, а честно показываем ошибку.
        let save_result = match self.state.db.borrow().as_ref() {
            Some(db) => db.save_entry(&mut entry),
            None => {
                show_error(&self.window, "База не открыта — запись не сохранена.");
                return;
            }
        };
        if let Err(e) = save_result {
            show_error(&self.window, &format!("Не удалось сохранить запись:\n{e}"));
            return;
        }
        self.toast_toast("Запись сохранена");
        self.refresh();
    }

    fn delete_entry(self: &Rc<Self>) {
        let id = match self.edit_id.get() {
            Some(id) => id,
            None => {
                show_error(&self.window, "Сначала выберите запись в списке.");
                return;
            }
        };
        let dialog = gtk::MessageDialog::new(
            Some(&self.window),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Question,
            gtk::ButtonsType::YesNo,
            "Удалить выбранную запись?",
        );
        let m = self.clone();
        dialog.connect_response(move |d, resp| {
            if resp == gtk::ResponseType::Yes {
                if let Some(db) = m.state.db.borrow().as_ref() {
                    let _ = db.delete_entry(id);
                }
                m.refresh();
            }
            d.destroy();
        });
        dialog.show();
    }

    /// Автоввод логина и пароля выбранной записи в активное окно.
    fn autotype_selected(self: &Rc<Self>) {
        let username = self.username_e.text().to_string();
        let password = self.password_e.text().to_string();
        if password.is_empty() {
            show_error(&self.window, "Сначала выберите запись — пароль пуст.");
            return;
        }
        // Автоввод считается активностью: не даём сработать автоблокировке.
        self.last_activity.set(glib::monotonic_time());
        super::autotype::run(&username, &password);
    }

    fn copy_password(self: &Rc<Self>) {
        let password = self.password_e.text().to_string();
        if password.is_empty() {
            show_error(&self.window, "Пароль пуст — нечего копировать.");
            return;
        }
        let clipboard = self.window.clipboard();
        clipboard.set_text(&password);
        let secs = self.state.config.borrow().clipboard_clear_seconds;
        self.toast_toast(&format!(
            "Пароль скопирован, будет очищен через {secs} секунд"
        ));
        let win = self.window.downgrade();
        glib::timeout_add_seconds_local(secs as u32, move || {
            if let Some(w) = win.upgrade() {
                w.clipboard().set_text("");
            }
            glib::ControlFlow::Break
        });
    }

    fn update_strength(self: &Rc<Self>) {
        let pass = self.password_e.text().to_string();
        if pass.is_empty() {
            self.strength_l.set_text("");
            self.strength_l.set_css_classes(&[]);
            return;
        }
        let s = generator::strength(&pass);
        self.strength_l.set_text(s.label());
        self.strength_l.set_css_classes(&match s {
            generator::Strength::Weak => vec!["error"],
            generator::Strength::Medium => vec!["warning"],
            generator::Strength::Strong => vec!["success"],
        });
    }

    fn toast_toast(self: &Rc<Self>, text: &str) {
        self.toast_label.set_text(text);
        self.toast.set_reveal_child(true);
        let m = self.clone();
        glib::timeout_add_seconds_local(3, move || {
            m.toast.set_reveal_child(false);
            glib::ControlFlow::Break
        });
    }

    /// Заблокировать: сбросить ключ и потребовать мастер-пароль заново.
    pub fn lock(self: &Rc<Self>) {
        // Защита от повторной блокировки: потеря фокуса окна (например, при
        // открытии диалога мастер-пароля) или таймер могли вызвать lock()
        // несколько раз — открывалось второе окно программы.
        if self.locked.get() {
            return;
        }
        self.locked.set(true);
        let path = self.state.db.borrow().as_ref().map(|db| db.path.clone());
        *self.state.db.borrow_mut() = None;
        self.clear_details();
        if let Some(path) = path {
            let win = self.window.clone();
            let state = self.state.clone();
            glib::idle_add_local_once(move || {
                prompt_unlock_and_open(&win, &state, path);
            });
        }
    }

    fn backup(self: &Rc<Self>) {
        let chooser = gtk::FileChooserNative::builder()
            .title("Каталог для резервной копии")
            .action(gtk::FileChooserAction::SelectFolder)
            .modal(true)
            .transient_for(&self.window)
            .build();
        let m = self.clone();
        chooser.connect_response(move |c, resp| {
            if resp == gtk::ResponseType::Accept {
                if let Some(dir) = c.file().and_then(|f| f.path()) {
                    if let Some(db) = m.state.db.borrow().as_ref() {
                        match db.backup(&dir) {
                            Ok(dest) => m.toast_toast(&format!(
                                "Резервная копия создана: {}",
                                dest.display()
                            )),
                            Err(e) => show_error(&m.window, &format!("Ошибка копирования:\n{e}")),
                        }
                    }
                }
            }
            c.destroy();
        });
        chooser.show();
    }

    fn show_change_password(self: &Rc<Self>) {
        let dialog = gtk::Dialog::builder()
            .title("Сменить мастер-пароль")
            .modal(true)
            .transient_for(&self.window)
            .default_width(440)
            .build();
        dialog.add_button("Отмена", gtk::ResponseType::Cancel);
        dialog.add_button("Сменить", gtk::ResponseType::Ok);

        let content = dialog.content_area();
        let box_ = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        let old = gtk::PasswordEntry::builder().show_peek_icon(true).build();
        let new = gtk::PasswordEntry::builder().show_peek_icon(true).build();
        let confirm = gtk::PasswordEntry::builder().show_peek_icon(true).build();
        box_.append(&gtk::Label::with_mnemonic("Старый пароль:"));
        box_.append(&old);
        box_.append(&gtk::Label::with_mnemonic("Новый пароль:"));
        box_.append(&new);
        box_.append(&gtk::Label::with_mnemonic("Подтверждение:"));
        box_.append(&confirm);
        content.append(&box_);

        let m = self.clone();
        dialog.connect_response(move |d, resp| {
            if resp == gtk::ResponseType::Ok {
                let o = old.text().to_string();
                let n = new.text().to_string();
                if n != confirm.text() {
                    show_error(&m.window, "Новый пароль и подтверждение не совпадают.");
                    return;
                }
                if n.len() < 8 {
                    show_error(&m.window, "Новый пароль слишком короткий (минимум 8 символов).");
                    return;
                }
                let res = m
                    .state
                    .db
                    .borrow_mut()
                    .as_mut()
                    .map(|db| db.change_master_password(&o, &n));
                match res {
                    Some(Ok(())) => {
                        m.toast_toast("Мастер-пароль изменён");
                        d.destroy();
                    }
                    Some(Err(e)) => {
                        show_error(&m.window, &format!("Не удалось сменить пароль:\n{e}"));
                    }
                    None => {
                        show_error(&m.window, "База не открыта.");
                    }
                }
            } else {
                d.destroy();
            }
        });
        dialog.show();
    }
}
