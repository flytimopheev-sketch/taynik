# RedPass — офлайн-менеджер паролей для РЕД ОС

GTK4-приложение на Rust для локального хранения, генерации и использования
паролей. Работает полностью офлайн: без облака, сети и телеметрии.

## Безопасность

- База `.rpwm` — SQLite-файл; все чувствительные поля (название, логин,
  пароль, URL, заметки, теги) хранятся зашифрованными BLOB (AES-256-GCM,
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
./target/release/redpass
```

## Горячие клавиши

| Комбинация | Действие |
|------------|----------|
| Ctrl+N | Новая запись |
| Ctrl+F | Фокус на поиск |
| Ctrl+Q | Выход |
| Ctrl+L | Заблокировать (закрыть базу) |

## Функции

- Создание/открытие базы, смена мастер-пароля, резервные копии
  (`-backup-YYYYMMDD-HHMMSS.rpwm`).
- CRUD записей, поиск по названию/логину/URL/тегам, фильтр по тегам,
  сортировка по названию или дате изменения, избранное (★).
- Генератор паролей: длина 8–64, наборы символов, исключение похожих
  символов, минимум цифр/спецсимволов, индикатор стойкости.

## Установка (RPM для РЕД ОС 7/8)

```bash
rpmbuild -bb packaging/redpass.spec
sudo dnf install ~/rpmbuild/RPMS/x86_64/redpass-1.0.0-1.*.rpm
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
cp redpass-1.0.0.tar.gz ~/rpmbuild/SOURCES/
```

Перенести каталог `rpmbuild` на офлайн-машину, затем:

```bash
rpmbuild -bb packaging/redpass.spec
sudo dnf install ~/rpmbuild/RPMS/x86_64/redpass-1.0.0-1.*.rpm
```

При необходимости обновить вендор: `cargo vendor vendor` (на машине
с интернетом), следуя выводимой инструкции для `.cargo/config.toml`.

Устанавливается: `/usr/bin/redpass`, `/usr/share/applications/redpass.desktop`,
`/usr/share/icons/hicolor/scalable/apps/redpass.svg`,
`/usr/share/metainfo/redpass.metainfo.xml`.

## Структура

```
src/
  main.rs, app.rs, config.rs
  db/      crypto.rs (Argon2id + AES-256-GCM), repository.rs, schema.rs
  models/  entry.rs, generator.rs
  ui/      start_window, unlock_dialog, create_db_dialog, main_window,
           password_generator
packaging/ redpass.spec (RPM), desktop/metainfo, make-source-tarball.sh
.github/   workflows/build-rpm.yml (сборка RPM в GitHub Actions)
vendor/    вендоренные Rust-зависимости (сборка без интернета)
```
