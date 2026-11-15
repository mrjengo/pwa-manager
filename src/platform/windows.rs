//! Windows backend: Start Menu shortcuts (`.lnk`) with a multi-size `.ico`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use image::RgbaImage;

use super::registry;
use crate::model::Pwa;

fn shortcut_dir() -> Result<PathBuf> {
    Ok(dirs::data_dir()
        .context("cannot determine %APPDATA%")?
        .join("Microsoft\\Windows\\Start Menu\\Programs\\PWA Manager"))
}

fn shortcut_path(pwa: &Pwa) -> Result<PathBuf> {
    Ok(shortcut_dir()?.join(format!("{}.lnk", registry::file_name(&pwa.name))))
}

pub fn list() -> Result<Vec<Pwa>> {
    registry::list()
}

pub fn id_available(id: &str) -> bool {
    registry::id_available(id)
}

/// Quotes one argument following the MSVC command-line parsing rules.
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    let mut out = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            c => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

pub fn save(pwa: &Pwa, icon: Option<&RgbaImage>) -> Result<Pwa> {
    let previous = registry::load(&pwa.id);
    let mut saved = pwa.clone();
    let ico = registry::dir()?.join(format!("{}.ico", pwa.id));
    if let Some(image) = icon {
        let png = registry::icon_png(&pwa.id)?;
        crate::icon::save_png(image, &png)?;
        crate::icon::save_ico(image, &ico)?;
        saved.icon = Some(png);
    } else {
        saved.icon = previous.as_ref().and_then(|p| p.icon.clone());
    }

    if let Some(old) = &previous {
        let old_link = shortcut_path(old)?;
        if old_link != shortcut_path(pwa)? && old_link.exists() {
            std::fs::remove_file(old_link)?;
        }
    }

    let link_path = shortcut_path(pwa)?;
    std::fs::create_dir_all(shortcut_dir()?)?;
    let mut link = mslnk::ShellLink::new(&pwa.browser).map_err(|e| {
        anyhow::anyhow!("cannot create shortcut to {}: {e:?}", pwa.browser.display())
    })?;
    let args: Vec<String> = pwa.browser_args().iter().map(|a| quote_arg(a)).collect();
    link.set_arguments(Some(args.join(" ")));
    link.set_name(Some(pwa.name.clone()));
    if ico.exists() {
        link.set_icon_location(Some(ico.to_string_lossy().into_owned()));
    }
    link.create_lnk(&link_path)
        .map_err(|e| anyhow::anyhow!("cannot write {}: {e:?}", link_path.display()))?;

    registry::store(&saved)?;
    Ok(saved)
}

pub fn remove(pwa: &Pwa) -> Result<()> {
    let link = shortcut_path(pwa)?;
    if link.exists() {
        std::fs::remove_file(&link).with_context(|| format!("cannot remove {}", link.display()))?;
    }
    registry::delete(&pwa.id)
}
