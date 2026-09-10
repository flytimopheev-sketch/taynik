%global app_id ru.redos.RedPass

Name:           redpass
Version:        1.0.0
Release:        1%{?dist}
Summary:        Офлайн-менеджер паролей для РЕД ОС
License:        GPL-3.0-or-later
URL:            https://example.local/redpass
Source0:        %{name}-%{version}.tar.gz
BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  pkgconfig
BuildRequires:  gtk4-devel >= 4.10
Requires:       gtk4 >= 4.10

%description
RedPass — локальный GTK4-менеджер паролей для РЕД ОС. База хранится
зашифрованной (AES-256-GCM, ключ из мастер-пароля через Argon2id).
Работает полностью офлайн, без облака и телеметрии.

%prep
%autosetup

# Все зависимости Rust уже в vendor/ (см. .cargo/config.toml),
# сеть при сборке не требуется.
%build
cargo build --release --offline --locked

%install
install -Dm755 target/release/redpass %{buildroot}%{_bindir}/redpass
install -Dm644 packaging/redpass.desktop %{buildroot}%{_datadir}/applications/redpass.desktop
install -Dm644 resources/icons/redpass.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/redpass.svg
install -Dm644 packaging/redpass.metainfo.xml %{buildroot}%{_metainfodir}/redpass.metainfo.xml

%files
%license LICENSE
%doc README.md
%{_bindir}/redpass
%{_datadir}/applications/redpass.desktop
%{_datadir}/icons/hicolor/scalable/apps/redpass.svg
%{_metainfodir}/redpass.metainfo.xml

%changelog
* Wed Sep 02 2026 RedPass Maintainer <maintainer@local> - 1.0.0-1
- Initial package: encrypted vault, generator, clipboard auto-clear, auto-lock.
