#!/usr/bin/env bash
# Package the native release binary. Run on Arch Linux as an unprivileged user.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
[[ $(uname -m) == x86_64 ]] || { echo 'Only Arch x86_64 is supported' >&2; exit 1; }
node scripts/prepare-release.cjs --check-version
version=$(node -p "require('./app/src-tauri/tauri.conf.json').version")
binary=${1:-$root/app/src-tauri/target/release/lucas-translate}
[[ -x "$binary" ]] || { echo "Missing release binary: $binary" >&2; exit 1; }
out="$root/dist/arch"
mkdir -p "$out"
stage=$(mktemp -d "$out/build.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
install -Dm755 "$binary" "$stage/payload/usr/bin/lucas-translate"
install -Dm644 packaging/arch/lucas-translate.desktop "$stage/payload/usr/share/applications/lucas-translate.desktop"
install -Dm644 app/src-tauri/icons/128x128.png "$stage/payload/usr/share/icons/hicolor/128x128/apps/lucas-translate.png"
install -Dm644 LICENSE "$stage/payload/usr/share/licenses/lucas-translate/LICENSE"
install -Dm644 packaging/omarchy/lucas-translate.lua "$stage/payload/usr/share/lucas-translate/omarchy/lucas-translate.lua"
install -Dm755 packaging/omarchy/lucas-translate-setup-omarchy "$stage/payload/usr/bin/lucas-translate-setup-omarchy"
install -Dm644 docs/OMARCHY.md "$stage/payload/usr/share/doc/lucas-translate/OMARCHY.md"
tar -czf "$stage/payload.tar.gz" -C "$stage/payload" usr
checksum=$(sha256sum "$stage/payload.tar.gz")
checksum=${checksum%% *}
sed -e "s/@VERSION@/$version/g" -e "s/@SHA256@/$checksum/g" packaging/arch/PKGBUILD.in > "$stage/PKGBUILD"
cd "$stage"
PKGDEST="$out" PKGEXT='.pkg.tar.zst' makepkg --force --noconfirm
pacman -Qip "$out/lucas-translate-$version-1-x86_64.pkg.tar.zst"
