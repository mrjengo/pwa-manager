# PWA Manager

Install any website as a desktop app on **Linux, Windows and macOS**, through a native GUI or the `pwa` command line.

PWA Manager runs web apps in a Chromium-based browser's app mode (`--app=`), and gives you full control over them: name, URL, icon, browser and browser profile, all editable after installation. It is a single native binary (Rust + [Slint](https://slint.dev)): no Electron, no Tauri, no web runtime of its own.

## Features

* **Native GUI**: list, install, edit, launch and remove apps from one window. `pwa` falls back to a terminal menu when no display is available (e.g. over SSH).
* **Scriptable CLI**: `pwa install`, `edit`, `remove`, `list --json`, `launch`, `browsers`, `engine`.
* **Browser detection**: Chrome, Chromium, Brave, Edge, Vivaldi, Thorium, Opera and Yandex, including Flatpak and Snap on Linux.
* **Multi-profile**: pick the profile from the ones the browser actually has (e.g. separate personal and work accounts for the same service).
* **Automatic icons**: leave the icon empty and the best one is taken from the site's web manifest or `<link>` tags; or give an image URL or a local file (PNG, JPEG, ICO, WebP, GIF, BMP).
* **No browser? No problem**: PWA Manager can download [Ungoogled Chromium](https://ungoogled-software.github.io/) (free and open source, Chromium without Google services) just for your apps, with its own browser data kept apart from any other Chromium.
* **Correct taskbar grouping on Linux**: `StartupWMClass` matches the Wayland app_id / X11 class Chromium gives each app window, so windows group under the app's icon.

## What gets created

| Platform | Launcher | Icon |
| --- | --- | --- |
| Linux | `~/.local/share/applications/<id>.desktop` | `~/.local/share/icons/hicolor/256x256/apps/<id>.png` |
| Windows | Start Menu → `PWA Manager\<Name>.lnk` | `%LOCALAPPDATA%\pwa-manager\apps\<id>.ico` |
| macOS | `~/Applications/PWA Manager/<Name>.app` | inside the bundle (`icon.icns`) |

On Linux every desktop entry that launches a browser with `--app=` is listed, so apps created by the earlier shell-script version of this tool are picked up and can be edited. Extra flags you added to a launcher by hand are preserved on edit.

## Installation

Prebuilt binaries for Linux, Windows and macOS are attached to each [release](https://github.com/mrjengo/PWA-Manager/releases).

To build from source you need [Rust](https://rustup.rs) (1.88 or newer):

    git clone https://github.com/mrjengo/PWA-Manager.git
    cd PWA-Manager
    ./install.sh          # installs into ~/.local (no root needed)
    sudo ./install.sh     # or system-wide into /usr/local

On Linux the installer also adds a "PWA Manager" entry to your applications menu. Building on Linux requires the fontconfig and xkbcommon development headers (`libfontconfig1-dev libxkbcommon-dev` on Debian/Ubuntu, `fontconfig-devel libxkbcommon-devel` on Fedora).

Alternatively: `cargo install --path .`

## Usage

Run `pwa` to open the GUI. From a terminal:

    pwa install                                  # guided, prompts for everything
    pwa install --name GitHub --url github.com --browser brave --profile "Profile 1"
    pwa list
    pwa edit github --icon ~/Pictures/github.png
    pwa launch github
    pwa remove github
    pwa browsers                                 # detected browsers and profiles
    pwa engine install                           # download Ungoogled Chromium
    pwa menu                                     # the classic interactive menu

Apps are addressed by id (the launcher's file name) or by name. Values not passed as options are prompted for when running in a terminal. See `pwa help <command>` for details.

## Notes

* The downloaded engine lives in the per-user local data directory (`~/.local/share/pwa-manager/engine` on Linux). `pwa engine install` updates it; `pwa engine remove` deletes it but keeps its browser data.
* On Ubuntu 23.10+ the portable Ungoogled Chromium build may need unprivileged user namespaces to start its sandbox (blocked by AppArmor by default); a distro or Flatpak browser avoids this.
* On macOS the Dock shows the browser's icon for a running app; the app bundle itself, Launchpad and Spotlight use the PWA's icon.

## Uninstallation

    ./uninstall.sh            # or: sudo ./uninstall.sh

This removes only the program. Your installed PWAs and their icons are left in place; remove them first with `pwa remove` if you want them gone. Upgrading from the shell-script version? `sudo ./uninstall.sh --legacy` also removes `/opt/pwa`.

## Project layout

* `src/model.rs`, `src/manager.rs`: app record, validation and the install/edit workflow.
* `src/browser.rs`: browser and profile detection.
* `src/icon.rs`: icon download, discovery and conversion.
* `src/engine.rs`: managed Ungoogled Chromium download.
* `src/platform/`: launcher backends for Linux, Windows and macOS.
* `src/cli.rs`, `src/gui.rs`, `ui/app.slint`: the command line and the GUI.

## License

GPL-3.0. See [LICENSE](LICENSE).
