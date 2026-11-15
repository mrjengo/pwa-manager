#!/usr/bin/env bash
# Removes the PWA Manager program. Installed PWAs, their icons and the
# downloaded browser engine are left untouched.
#   ./uninstall.sh            remove the ~/.local install
#   sudo ./uninstall.sh       remove the /usr/local install
#   sudo ./uninstall.sh --legacy   also remove the old shell-script version (/opt/pwa)

set -euo pipefail

if [[ $EUID -eq 0 ]]; then
    PREFIX="${PREFIX:-/usr/local}"
else
    PREFIX="${PREFIX:-$HOME/.local}"
fi

rm -fv "$PREFIX/bin/pwa" \
    "$PREFIX/share/applications/pwa-manager.desktop" \
    "$PREFIX/share/icons/hicolor/scalable/apps/pwa-manager.svg"

if [[ "${1:-}" == "--legacy" ]]; then
    rm -rfv /opt/pwa
    if [[ -L /usr/local/bin/pwa ]]; then
        rm -fv /usr/local/bin/pwa
    fi
fi

echo "PWA Manager removed. Your installed PWAs were not touched; remove them first with 'pwa remove' if needed."
