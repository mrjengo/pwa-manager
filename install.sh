#!/usr/bin/env bash

set -euo pipefail

echo "====================================="
echo "    PWA MANAGER SYSTEM INSTALLER     "
echo "====================================="

# 1. Root Privilege Check
if [[ $EUID -ne 0 ]]; then
   echo "Error: This installation requires administrator privileges."
   echo "Please run this script using: sudo ./install.sh"
   exit 1
fi

INSTALL_DIR="/opt/pwa"
BIN_SYMLINK="/usr/local/bin/pwa"

# Ensure the script operates in the directory it is launched from
cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"

# 2. File Verification (Now includes pwa_edit)
REQUIRED_FILES=("pwa_manager" "pwa_install" "pwa_edit" "pwa_remove" "pwa_list")
for file in "${REQUIRED_FILES[@]}"; do
    if [[ ! -f "$file" ]]; then
        echo "Error: Required file '$file' not found."
        echo "Please ensure all 5 scripts are in the same folder as this installer."
        exit 1
    fi
done

# 3. Deployment
echo "[+] Creating installation directory at $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"

echo "[+] Copying scripts to $INSTALL_DIR..."
cp "${REQUIRED_FILES[@]}" "$INSTALL_DIR/"

echo "[+] Setting execution permissions for all users..."
chmod +x "$INSTALL_DIR"/*

echo "[+] Creating system-wide command 'pwa'..."
# Clean up any old symlinks if they exist before creating the new one
if [[ -e "$BIN_SYMLINK" || -L "$BIN_SYMLINK" ]]; then
    rm -f "$BIN_SYMLINK"
fi
ln -s "$INSTALL_DIR/pwa_manager" "$BIN_SYMLINK"

echo "====================================="
echo " Installation Complete!"
echo " You can now run 'pwa' from any terminal as your normal user."
echo "====================================="
