Name:           taynik
Version:        1.0.3
Release:        1%{?dist}
Summary:        Офлайн-менеджер паролей
License:        MIT
URL:            https://github.com/flytimopheev-sketch/taynik
Source0:        %{name}-%{version}.tar.gz
BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  gcc
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
# В CI установлен cargo-zigbuild — собираем с целевой glibc 2.17, чтобы
# бинарник работал на РЕД ОС 7/8 (glibc 2.17/2.28) и новее.
ZB=$(command -v cargo-zigbuild || true)
[ -n "$ZB" ] || ZB="$HOME/.local/bin/cargo-zigbuild"
if [ -x "$ZB" ]; then
    "$ZB" build --release --offline --locked --target x86_64-unknown-linux-gnu.2.17
else
    echo "cargo-zigbuild не найден — сборка с системной glibc" >&2
    cargo build --release --offline --locked
fi

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
* Fri Sep 11 2026 Taynik Maintainer <maintainer@local> - 1.0.3-1
- Исправлена сборка с cargo-zigbuild (раньше она незаметно откатывалась
  на системный cargo, и требование GLIBC_2.39 оставалось).
- Из исходников убраны упоминания прежнего имени проекта.

* Fri Sep 11 2026 Taynik Maintainer <maintainer@local> - 1.0.2-1
- RPM-сборка через cargo-zigbuild с целевой glibc 2.17: бинарник работает
  на РЕД ОС 7/8 и новее (раньше требовалась glibc 2.39 из ubuntu-24.04).

* Fri Sep 11 2026 Taynik Maintainer <maintainer@local> - 1.0.1-1
- Сборка RPM напрямую в GitHub Actions (ubuntu-latest, без docker-контейнера).
- Исправлен MimeType в taynik.desktop (application/x-taynik).
- Исправлен URL проекта в диалоге «О программе».

* Thu Sep 10 2026 Taynik Maintainer <maintainer@local> - 1.0.0-1
- Tайник: vault, generator, clipboard auto-clear, auto-lock, автоввод, темы.
