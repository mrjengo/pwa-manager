#!/usr/bin/env bash

set -euo pipefail

echo "================================================================="
echo "                 PWA MANAGER SYSTEM UNINSTALLER                  "
echo "================================================================="

# 1. Root Privilege Check
if [[ $EUID -ne 0 ]]; then
   echo "Error: This uninstallation requires administrator privileges."
   echo "Please run this script using: sudo ./uninstall.sh"
   exit 1
fi

# 2. Warning and Confirmation
echo "WARNING: This uninstaller ONLY removes the PWA Manager program."
echo "It DOES NOT remove the actual PWAs you have installed."
echo ""
echo "It is highly recommended to use the 'pwa' tool (Option 3: Remove)"
echo "to cleanly delete your installed PWAs BEFORE running this uninstaller."
echo "================================================================="

read -rp "Do you want to proceed with removing the PWA Manager? [y/N]: " CONFIRM

if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
    echo "Uninstallation canceled."
    exit 0
fi

# 3. System Cleanup
INSTALL_DIR="/opt/pwa"
BIN_SYMLINK="/usr/local/bin/pwa"

echo ""
echo "[+] Removing system-wide command 'pwa'..."
if [[ -L "$BIN_SYMLINK" || -e "$BIN_SYMLINK" ]]; then
    rm -f "$BIN_SYMLINK"
    echo "    -> Symlink removed."
else
    echo "    -> Symlink not found. Skipping."
fi

echo "[+] Removing installation directory at $INSTALL_DIR..."
if [[ -d "$INSTALL_DIR" ]]; then
    rm -rf "$INSTALL_DIR"
    echo "    -> Directory removed."
else
    echo "    -> Directory not found. Skipping."
fi

echo "================================================================="
echo " Uninstallation Complete!"
echo "================================================================="
