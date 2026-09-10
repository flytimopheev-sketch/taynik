#!/bin/sh
# Собирает исходный tarball для RPM, включая vendored-зависимости.
# Результат: redpass-1.0.0.tar.gz (кладётся в ~/rpmbuild/SOURCES).
set -e
cd "$(dirname "$0")/.."
# tarball не должен содержать артефакты сборки; --anchored — чтобы не
# вырезать одноимённые каталоги внутри vendor (например vendor/cc/src/target).
# Нормализуем права: на Windows/drvfs все файлы могут выглядеть executable,
# а Fedora brp-mangle-shebangs прервёт сборку RPM на «shebang»-ERROR.
chmod 644 Cargo.toml Cargo.lock LICENSE README.md
find src packaging resources .cargo vendor -type f -exec chmod 644 {} +
find src packaging resources .cargo vendor -type d -exec chmod 755 {} +
tar --anchored --exclude='target' --exclude='.git' \
    --transform 's,^,redpass-1.0.0/,' \
    -czf "redpass-1.0.0.tar.gz" \
    Cargo.toml Cargo.lock LICENSE README.md \
    src packaging resources .cargo vendor
echo "OK: $(pwd)/redpass-1.0.0.tar.gz"
