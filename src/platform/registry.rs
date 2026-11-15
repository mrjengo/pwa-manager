//! App registry for platforms whose launchers cannot carry metadata
//! (Windows shortcuts, macOS app bundles): one JSON file plus a PNG icon per app
//! in the per-user data directory.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::model::Pwa;

pub fn dir() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .context("cannot determine the local data directory")?
        .join("pwa-manager")
        .join("apps"))
}

pub fn icon_png(id: &str) -> Result<PathBuf> {
    Ok(dir()?.join(format!("{id}.png")))
}

pub fn load(id: &str) -> Option<Pwa> {
    let path = dir().ok()?.join(format!("{id}.json"));
    let mut pwa: Pwa = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    pwa.icon = icon_png(&pwa.id).ok().filter(|p| p.exists());
    Some(pwa)
}

pub fn list() -> Result<Vec<Pwa>> {
    let Ok(entries) = std::fs::read_dir(dir()?) else {
        return Ok(Vec::new());
    };
    let mut apps: Vec<Pwa> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| load(p.file_stem()?.to_str()?))
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    Ok(apps)
}

pub fn id_available(id: &str) -> bool {
    dir().is_ok_and(|d| !d.join(format!("{id}.json")).exists())
}

pub fn store(pwa: &Pwa) -> Result<()> {
    let dir = dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", pwa.id));
    std::fs::write(&path, serde_json::to_vec_pretty(pwa)?)
        .with_context(|| format!("cannot write {}", path.display()))
}

/// Removes the record and every file named after the app id (icons included).
pub fn delete(id: &str) -> Result<()> {
    let dir = dir()?;
    for ext in ["json", "png", "ico"] {
        let path = dir.join(format!("{id}.{ext}"));
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("cannot remove {}", path.display()))?;
        }
    }
    Ok(())
}

/// File name derived from the display name, safe on Windows and macOS.
pub fn file_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_end_matches(['.', ' ']);
    if cleaned.is_empty() {
        "PWA".to_owned()
    } else {
        cleaned.to_owned()
    }
}
