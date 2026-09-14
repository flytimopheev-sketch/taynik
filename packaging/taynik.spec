Name:           taynik
Version:        1.1.1
Release:        1%{?dist}
Summary:        Офлайн-менеджер паролей
License:        MIT
Source0:        %{name}-%{version}.tar.gz
BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  binutils
BuildRequires:  pkgconfig
BuildRequires:  gtk4-devel >= 4.10
Requires:       gtk4 >= 4.10
%global _metainfodir %{_datadir}/metainfo

%description
Тайник — локальный GTK4-менеджер паролей. База хранится
зашифрованной (AES-256-GCM, ключ из мастер-пароля через Argon2id).
Работает полностью офлайн, без облака и телеметрии. Реализует
автоввод логина/пароля в другие приложения (xdotool / wtype).

%prep
%autosetup

# Все зависимости Rust уже в vendor/ (см. .cargo/config.toml),
# сеть при сборке не требуется.
%build
# Сборка строго через cargo-zigbuild с целевой glibc 2.17, чтобы бинарник
# работал на РЕД ОС 7/8 (glibc 2.17/2.28) и новее. Никаких "мягких" откатов:
# линковка с системной glibc раннера даёт требование GLIBC_2.39 и ломает
# установку на РЕД ОС.
ZB="$(command -v cargo-zigbuild || true)"
[ -n "$ZB" ] || ZB="$HOME/.local/bin/cargo-zigbuild"
if [ ! -x "$ZB" ]; then
    echo "ОШИБКА: cargo-zigbuild не найден — сборка с системной glibc недопустима" >&2
    exit 1
fi
# ВАЖНО: нельзя давать -L на каталог с системной glibc (/usr/lib/x86_64-linux-gnu) —
# линковщик подхватит системную libc и требование GLIBC_2.39 вернётся. Вместо этого
# собираем каталог syslibs с симлинками только на GTK-зависимости; libc при этом
# предоставляется самим zig (совместимая с glibc 2.17).
LIBDIR=$(pkg-config --variable=libdir gtk4 2>/dev/null)
if [ -z "$LIBDIR" ] && [ -d /usr/lib/x86_64-linux-gnu ]; then LIBDIR=/usr/lib/x86_64-linux-gnu; fi
if [ -z "$LIBDIR" ] && [ -d /usr/lib64 ]; then LIBDIR=/usr/lib64; fi
[ -n "$LIBDIR" ] || { echo "ОШИБКА: не найден каталог системных библиотек GTK" >&2; exit 1; }
mkdir -p syslibs
for L in gtk-4 gdk-4 gsk-4 glib-2.0 gobject-2.0 gio-2.0 cairo cairo-gobject \
         pango-1.0 pangocairo-1.0 harfbuzz gdk_pixbuf-2.0 graphene-1.0 vulkan z m; do
    if [ -e "$LIBDIR/lib$L.so" ]; then ln -sf "$LIBDIR/lib$L.so" "syslibs/lib$L.so"; fi
done
"$ZB" build --release --offline --locked --target x86_64-unknown-linux-gnu.2.17

%check
# Контроль: бинарник не должен требовать символы glibc новее 2.17.
BIN=target/x86_64-unknown-linux-gnu/release/taynik
MAXSYM=$(objdump -T "$BIN" 2>/dev/null | grep -o 'GLIBC_2\.[0-9]*' | sort -uV | tail -n1 || true)
echo "Максимальная версия GLIBC-символов: ${MAXSYM:-none}"
case "$MAXSYM" in
    GLIBC_2\.[0-9]|GLIBC_2\.1[0-7]) ;; # 2.0..2.17 — допустимо
    *) echo "ОШИБКА: бинарник требует $MAXSYM (допустимо не выше GLIBC_2.17)" >&2; exit 1 ;;
esac

%install
BIN=target/release/taynik
[ -x "$BIN" ] || BIN=target/x86_64-unknown-linux-gnu/release/taynik
install -Dm755 "$BIN" %{buildroot}%{_bindir}/taynik
install -Dm644 packaging/taynik.desktop %{buildroot}%{_datadir}/applications/taynik.desktop
install -Dm644 resources/icons/taynik.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/taynik.svg
install -Dm644 packaging/taynik.metainfo.xml %{buildroot}%{_metainfodir}/taynik.metainfo.xml

%files
%license LICENSE
%doc README.md
%{_bindir}/taynik
%{_datadir}/applications/taynik.desktop
%{_datadir}/icons/hicolor/scalable/apps/taynik.svg
%{_metainfodir}/taynik.metainfo.xml

%changelog
* Mon Sep 14 2026 flytimopheev <flytimopheev@gmail.com> - 1.1.1-1
- Исправлена сборка RPM: устранены ошибки компиляции в src/ui/main_window.rs,
  из-за которых %build падал.
  - убрана дублирующая закрывающая скобка (ранний выход из impl MainWindow,
    ошибка "unexpected closing delimiter");
  - DropDown::from_strings получал &Vec<String> вместо &[&str] (списки тегов,
    групп и цветов теперь собраны как Vec<&str>);
  - gtk::show_uri возвращает (), а код ожидал Result при открытии URL;
  - copy_btn в диалоге TOTP приводился к Widget, а не Button из-за типа
    Dialog::add_button -> Widget;
  - экспорт CSV: движением Rc<MainWindow> во внутренний move-обработчик внутри
    Fn-обработчика добавлен промежуточный clone().
* Fri Sep 11 2026 flytimopheev <flytimopheev@gmail.com> - 1.1.0-1
- Новое: настраиваемая последовательность автоввода ({USERNAME}{TAB}{PASSWORD}
  и произвольные токены) и диалог настроек.
- Новое: TOTP-коды по base32-секрету записи, история паролей с восстановлением.
- Новое: группы записей и цветовые метки, фильтр по группе.
- Новое: импорт/экспорт CSV, кнопки «Открыть URL» и «Копировать логин».
- Новое: горячие клавиши копирования логина (Ctrl+Shift+C) и пароля (Ctrl+Shift+P).
- Скруглённые поля ввода во всех окнах; в диалоге «О программе» указан
  автор flytimopheev@gmail.com, ссылка на сайт удалена.
* Fri Sep 11 2026 flytimopheev <flytimopheev@gmail.com> - 1.0.3-2
- Убраны RUSTFLAGS с -L на системную glibc (именно из-за них бинарник
  требовал GLIBC_2.39 даже при сборке через cargo-zigbuild).
- cargo-zigbuild теперь обязателен: при его отсутствии сборка падает,
  а не тихо откатывается на системный cargo.
- Добавлена проверка %check: сборка падает, если бинарник требует
  символы glibc новее 2.17.
* Fri Sep 11 2026 flytimopheev <flytimopheev@gmail.com> - 1.0.3-1
- Исправлена сборка с cargo-zigbuild (раньше она незаметно откатывалась
  на системный cargo, и требование GLIBC_2.39 оставалось).
- Из исходников убраны упоминания прежнего имени проекта.

* Fri Sep 11 2026 flytimopheev <flytimopheev@gmail.com> - 1.0.2-1
- RPM-сборка через cargo-zigbuild с целевой glibc 2.17: бинарник работает
  на РЕД ОС 7/8 и новее (раньше требовалась glibc 2.39 из ubuntu-24.04).

* Fri Sep 11 2026 flytimopheev <flytimopheev@gmail.com> - 1.0.1-1
- Сборка RPM напрямую в GitHub Actions (ubuntu-latest, без docker-контейнера).
- Исправлен MimeType в taynik.desktop (application/x-taynik).
- Исправлен URL проекта в диалоге «О программе».

* Thu Sep 10 2026 flytimopheev <flytimopheev@gmail.com> - 1.0.0-1
- Tайник: vault, generator, clipboard auto-clear, auto-lock, автоввод, темы.
