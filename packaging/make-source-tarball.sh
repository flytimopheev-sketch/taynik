#!/bin/sh
# Собирает исходный tarball для RPM, включая vendored-зависимости.
# Версия берётся из Cargo.toml. Результат: taynik-<версия>.tar.gz (кладётся в ~/rpmbuild/SOURCES).
set -e
cd "$(dirname "$0")/.."
VER=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -n1)
[ -n "$VER" ] || { echo "Не удалось определить версию из Cargo.toml" >&2; exit 1; }
chmod 644 Cargo.toml Cargo.lock LICENSE README.md
find src packaging resources .cargo vendor -type f -exec chmod 644 {} +
find src packaging resources .cargo vendor -type d -exec chmod 755 {} +
tar --anchored --exclude='target' --exclude='.git' \
    --transform "s,^,taynik-$VER/," \
    -czf "taynik-$VER.tar.gz" \
    Cargo.toml Cargo.lock LICENSE README.md \
    src packaging resources .cargo vendor
echo "OK: $(pwd)/taynik-$VER.tar.gz"