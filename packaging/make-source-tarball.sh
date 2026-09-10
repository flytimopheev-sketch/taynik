#!/bin/sh
# Собирает исходный tarball для RPM, включая vendored-зависимости.
# Результат: redpass-1.0.0.tar.gz (кладётся в ~/rpmbuild/SOURCES).
set -e
cd "$(dirname "$0")/.."
# tarball не должен содержать артефакты сборки
tar --exclude='target' --exclude='.git' \
    -czf "redpass-1.0.0.tar.gz" \
    -s '/^/redpass-1.0.0\//' \
    Cargo.toml Cargo.lock LICENSE README.md \
    src packaging resources .cargo vendor
echo "OK: $(pwd)/redpass-1.0.0.tar.gz"
