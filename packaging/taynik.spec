! 1.0
Name:           taynik
Version:        1.0.0
Release:        1%{?dist}
Summary:        Офлайн-менеджер паролей
License:        MIT
URL:            https://example.local/taynik
Source0:        %{name}-%{version}.tar.gz
BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  pkgconfig
BuildRequires:  gtk4-devel >= 4.10
Requires:       gtk4 >= 4.10

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
cargo build --release --offline --locked

%install
install -Dm755 target/release/taynik %{buildroot}%{_bindir}/taynik
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
* Thu Sep 10 2026 Taynik Maintainer <maintainer@local> - 1.0.0-1
- Tайник: vault, generator, clipboard auto-clear, auto-lock, автоввод, темы.
