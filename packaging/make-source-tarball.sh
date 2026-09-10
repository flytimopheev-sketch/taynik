#!/bin/sh
# Собирает исходный tarball для RPM, включая vendored-зависимости.
# Результат: taynik-1.0.0.tar.gz (кладётся в ~/rpmbuild/SOURCES).
set -e
cd "$(dirname "$0")/.."
chmod 644 Cargo.toml Cargo.lock LICENSE README.md
find src packaging resources .cargo vendor -type f -exec chmod 644 {} +
find src packaging resources .cargo vendor -type d -exec chmod 755 {} +
tar --anchored --exclude='target' --exclude='.git' \
    --transform 's,^,taynik-1.0.0/,' \
    -czf "taynik-1.0.0.tar.gz" \
    Cargo.toml Cargo.lock LICENSE README.md \
    src packaging resources .cargo vendor
echo "OK: $(pwd)/taynik-1.0.0.tar.gz"
