# Linux PWA Manager

A system-wide, interactive Bash utility for installing, managing, and editing Progressive Web Apps (PWAs) on Linux. 

Unlike native browser "Create Shortcut" features, this tool gives you absolute control over your PWAs. It features advanced support for Wayland window grouping and multi-profile browser routing, allowing you to seamlessly run different PWAs under different browser accounts.

## Features

* Interactive Menu System: Easy-to-use terminal interface to manage your web apps.
* Smart Browser Detection: Automatically scans your system and Flatpak/Snap paths for Chromium-based browsers (Chrome, Chromium, Brave, Edge, Vivaldi, Thorium, Opera, Yandex).
* Multi-Profile Support: Tie specific PWAs to different browser profiles (e.g., separate your personal and work accounts for the same web service).
* Wayland Compatibility: Automatically calculates and injects the correct StartupWMClass based on your selected browser and profile, ensuring high-res icons group properly in your taskbar/panel on Wayland compositors (KDE, GNOME, etc.).
* On-the-Fly Editing: Change a PWA's Name, URL, Icon, underlying Browser, or assigned Profile without having to delete and recreate it.
* Safe Uninstallation: Cleanly removes .desktop files and custom icons when purging an app.

## Installation

Clone the repository and run the included installer script with administrator privileges.

    git clone https://github.com/YOUR_USERNAME/YOUR_REPO_NAME.git
    cd YOUR_REPO_NAME
    chmod +x install.sh
    sudo ./install.sh

The installer will:
1. Create a dedicated directory at /opt/pwa.
2. Copy the suite of scripts to this directory.
3. Create a system-wide symlink so you can run the tool from anywhere.

## Usage

Once installed, simply open a terminal as your normal user and run:

    pwa

This will launch the main manager menu:
1. Install a new PWA: Prompts you for a Name, URL, Icon URL, and Browser Profile, then generates the .desktop file in your ~/.local/share/applications folder.
2. Edit an existing PWA: Allows you to dynamically change the properties of an installed PWA. Rebuilds the Wayland window class automatically if you change the URL, browser, or profile.
3. Remove an existing PWA: Interactively safely deletes the .desktop entry and associated icon payload.
4. List installed PWAs: Prints a clean table showing all managed PWAs, their target URLs, and their active browser profiles.
5. Quit: Exits the manager.

## Uninstallation

To remove the PWA Manager from your system, run the provided uninstaller script.

    sudo ./uninstall.sh

Note: The uninstaller only removes the manager tool itself. It intentionally leaves your installed PWAs and icons intact in your home directory so you don't lose your apps.

## File Structure

* pwa_manager - The main interactive loop and menu router.
* pwa_install - Handles detection, Wayland class generation, and .desktop creation.
* pwa_edit - Persistent editor for modifying existing PWA configurations.
* pwa_remove - Interactive purge tool.
* pwa_list - Outputs a formatted table of installed apps.
* install.sh / uninstall.sh - System deployment scripts.
