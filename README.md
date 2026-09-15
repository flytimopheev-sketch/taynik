# Тайник — офлайн-менеджер паролей

GTK4-приложение на Rust для локального хранения, генерации и использования
паролей. Работает полностью офлайн: без облака, сети и телеметрии.

## Безопасность

- Устройство `.rpwm` — SQLite-файл; все чувствительные поля (название, логин,
  пароль, URL, заметки, теги, группа, цвет, TOTP-секрет) хранятся зашифрованными BLOB (AES-256-GCM,
  уникальный 12-байтовый nonce на поле, формат `nonce || ciphertext || tag`).
- Ключ шифрования выводится из мастер-пароля через **Argon2id**:
  память 64 МиБ, 3 итерации, parallelism 4, 32 байта.
- В базе хранятся только соль и параметры Argon2 (таблица `meta`); вместо
  мастер-пароля хранится зашифрованный «verifier» для проверки пароля.
- Ключ очищается из памяти (zeroize); автоматическая блокировка по таймеру
  бездействия и (опционально) при сворачивании окна.
- Буфер обмена автоматически очищается через N секунд (по умолчанию 45).

## Сборка

Требуется Rust (stable) и GTK4 ≥ 4.10:

```bash
cargo build --release
```

## Запуск

```bash
./target/release/taynik
```

## Горячие клавиши

| Комбинация | Действие |
|------------|----------|
| Ctrl+N | Новая запись |
| Ctrl+F | Фокус на поиск |
| Ctrl+Q | Выход |
| Ctrl+L | Заблокировать (закрыть базу) |
| Ctrl+Shift+V | Автоввод логина и пароля в активное окно |
| Ctrl+Shift+C | Копировать логин |
| Ctrl+Shift+P | Копировать пароль |

## Функции

- Создание/открытие базы, смена мастер-пароля, резервные копии
  (`-backup-YYYYMMDD-HHMMSS.rpwm`).
- CRUD записей, поиск по названию/логину/URL/тегам/группам, фильтры
  по тегам и группам, сортировка по названию или дате изменения,
  избранное (★), цветовые метки записей.
- Генератор паролей: длина 8–64, наборы символов, исключение похожих
  символов, минимум цифр/спецсимволов, индикатор стойкости.
- Автоввод в другие приложения (`xdotool`/`wtype`) по настраиваемой
  последовательности: {USERNAME}, {PASSWORD}, {TAB}, {ENTER}, {DELAY мс}.
- TOTP-коды (двухфакторная аутентификация) по base32-секрету записи.
- История паролей записи: старые пароли сохраняются при смене,
  их можно скопировать или восстановить.
- Импорт/экспорт CSV (экспорт — с предупреждением об открытом виде).
- Открытие URL записи в браузере, копирование логина/пароля,
  копирование TOTP-кода.
- Диалог настроек: автоблокировка, таймаут очистки буфера обмена,
  блокировка при сворачивании, последовательность автоввода.
- Темы оформления: система, светлая, тёмная, Nord, Dracula, Solarized.

## Установка (RPM для РЕД ОС 7/8)

Готовый RPM собирается автоматически в GitHub Actions (без docker,
прямо на ubuntu-раннере) и публикуется в разделе
[Releases](https://github.com/flytimopheev-sketch/taynik/releases) на тегах `v*`.

```bash
sudo dnf install ./taynik-1.1.2-1.*.rpm
```

### Сборка RPM вручную

```bash
rpmbuild -bb packaging/taynik.spec
sudo dnf install ~/rpmbuild/RPMS/x86_64/taynik-1.1.2-1.*.rpm
```

### Установка на машину без интернета

Все Rust-зависимости вендорены в каталог `vendor/` (подключены через
`.cargo/config.toml`), поэтому сборка не требует доступа к crates.io.
Нужен только пакет `gtk4-devel` (и `rust`, `cargo`, `gcc`, `pkgconfig`) —
они берутся из локального репозитория РЕД ОС / установочного носителя.

На машине с интернетом (один раз):

```bash
./packaging/make-source-tarball.sh
mkdir -p ~/rpmbuild/SOURCES
cp taynik-1.1.2.tar.gz ~/rpmbuild/SOURCES/
```

Перенести каталог `rpmbuild` на офлайн-машину, затем:

```bash
rpmbuild -bb packaging/taynik.spec
sudo dnf install ~/rpmbuild/RPMS/x86_64/taynik-1.1.2-1.*.rpm
```

При необходимости обновить вендор: `cargo vendor vendor` (на машине
с интернетом), следуя выводимой инструкции для `.cargo/config.toml`.

Устанавливается: `/usr/bin/taynik`, `/usr/share/applications/taynik.desktop`,
`/usr/share/icons/hicolor/scalable/apps/taynik.svg`,
`/usr/share/metainfo/taynik.metainfo.xml`.

## Структура

```
src/
  main.rs, app.rs, config.rs
  db/      crypto.rs (Argon2id + AES-256-GCM), repository.rs, schema.rs, io.rs (CSV)
  models/  entry.rs, generator.rs, totp.rs
  ui/      start_window, unlock_dialog, create_db_dialog, main_window,
           password_generator, settings_dialog, autotype
packaging/ taynik.spec (RPM), desktop/metainfo, make-source-tarball.sh
.github/   workflows/build-rpm.yml (сборка RPM в GitHub Actions)
vendor/    вендоренные Rust-зависимости (сборка без интернета)
```
