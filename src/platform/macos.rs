//! macOS backend: minimal `.app` bundles in `~/Applications/PWA Manager` whose
//! executable is a shell script that starts the browser in app mode.

use std::path::PathBuf;

use anyhow::{Context, Result};
use image::RgbaImage;

use super::registry;
use crate::model::Pwa;

fn apps_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("cannot determine the home directory")?
        .join("Applications/PWA Manager"))
}

fn bundle_path(pwa: &Pwa) -> Result<PathBuf> {
    Ok(apps_dir()?.join(format!("{}.app", registry::file_name(&pwa.name))))
}

pub fn list() -> Result<Vec<Pwa>> {
    registry::list()
}

pub fn id_available(id: &str) -> bool {
    registry::id_available(id)
}

fn shell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn info_plist(pwa: &Pwa) -> String {
    let name = xml_escape(&pwa.name);
    let id = xml_escape(&pwa.id);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key><string>{name}</string>
	<key>CFBundleDisplayName</key><string>{name}</string>
	<key>CFBundleIdentifier</key><string>io.github.pwa-manager.{id}</string>
	<key>CFBundleExecutable</key><string>launcher</string>
	<key>CFBundleIconFile</key><string>icon</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundleShortVersionString</key><string>1.0</string>
	<key>LSMinimumSystemVersion</key><string>10.13</string>
	<key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#
    )
}

pub fn save(pwa: &Pwa, icon: Option<&RgbaImage>) -> Result<Pwa> {
    let previous = registry::load(&pwa.id);
    let mut saved = pwa.clone();
    if let Some(image) = icon {
        let png = registry::icon_png(&pwa.id)?;
        crate::icon::save_png(image, &png)?;
        saved.icon = Some(png);
    } else {
        saved.icon = previous.as_ref().and_then(|p| p.icon.clone());
    }

    let bundle = bundle_path(pwa)?;
    if let Some(old) = &previous {
        let old_bundle = bundle_path(old)?;
        if old_bundle != bundle && old_bundle.exists() {
            std::fs::remove_dir_all(old_bundle)?;
        }
    }

    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    std::fs::create_dir_all(&macos)?;
    std::fs::create_dir_all(&resources)?;
    std::fs::write(contents.join("Info.plist"), info_plist(pwa))?;

    let mut script = format!(
        "#!/bin/sh\nexec {}",
        shell_quote(&pwa.browser.to_string_lossy())
    );
    for arg in pwa.browser_args() {
        script.push(' ');
        script.push_str(&shell_quote(&arg));
    }
    script.push('\n');
    let launcher = macos.join("launcher");
    std::fs::write(&launcher, script)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755))?;
    }

    if let Some(png) = &saved.icon {
        let image = image::open(png)?.to_rgba8();
        crate::icon::save_icns(&image, &resources.join("icon.icns"))?;
    }
    // Ask Finder/Launch Services to pick up the new icon and metadata.
    let _ = std::process::Command::new("touch").arg(&bundle).status();

    registry::store(&saved)?;
    Ok(saved)
}

pub fn remove(pwa: &Pwa) -> Result<()> {
    let bundle = bundle_path(pwa)?;
    if bundle.exists() {
        std::fs::remove_dir_all(&bundle)
            .with_context(|| format!("cannot remove {}", bundle.display()))?;
    }
    registry::delete(&pwa.id)
}
