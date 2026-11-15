#!/usr/bin/env bash
# Builds PWA Manager and installs the `pwa` command plus a desktop launcher.
# As a normal user it installs into ~/.local; run with sudo for a system-wide
# install into /usr/local.

set -euo pipefail

cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"

if [[ $EUID -eq 0 ]]; then
    PREFIX="${PREFIX:-/usr/local}"
else
    PREFIX="${PREFIX:-$HOME/.local}"
fi

if ! command -v cargo >/dev/null; then
    echo "Error: Rust is required to build PWA Manager. Install it from https://rustup.rs" >&2
    exit 1
fi

echo "[+] Building PWA Manager (release)..."
cargo build --release --locked

echo "[+] Installing into $PREFIX..."
install -Dm755 target/release/pwa "$PREFIX/bin/pwa"

if [[ "$(uname -s)" == "Linux" ]]; then
    install -Dm644 assets/pwa-manager.svg "$PREFIX/share/icons/hicolor/scalable/apps/pwa-manager.svg"
    install -Dm644 assets/pwa-manager.desktop "$PREFIX/share/applications/pwa-manager.desktop"
    sed -i "s|^Exec=.*|Exec=$PREFIX/bin/pwa gui|" "$PREFIX/share/applications/pwa-manager.desktop"
    update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
    gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

# The shell-script version lived in /opt/pwa with a /usr/local/bin/pwa symlink.
if [[ -d /opt/pwa ]]; then
    echo "Note: the old shell-script version is still in /opt/pwa; run 'sudo ./uninstall.sh --legacy' to remove it."
fi

echo "Done. Run 'pwa' (or open PWA Manager from your applications menu)."
case ":$PATH:" in
    *":$PREFIX/bin:"*) ;;
    *) echo "Note: $PREFIX/bin is not in your PATH." ;;
esac
