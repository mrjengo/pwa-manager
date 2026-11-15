//! Linux backend: freedesktop `.desktop` launchers in `$XDG_DATA_HOME/applications`.
//!
//! Any desktop entry whose `Exec` passes `--app=` to a browser is treated as a PWA,
//! so launchers created by the original shell scripts are picked up as well.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use image::RgbaImage;

use crate::model::{Pwa, DEFAULT_PROFILE};

const PROFILE_FLAG: &str = "--profile-directory=";
const APP_FLAG: &str = "--app=";

fn data_dir() -> Result<PathBuf> {
    dirs::data_dir().context("cannot determine the XDG data directory")
}

fn applications_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("applications"))
}

fn icon_theme_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("icons/hicolor"))
}

fn desktop_path(id: &str) -> Result<PathBuf> {
    Ok(applications_dir()?.join(format!("{id}.desktop")))
}

pub fn list() -> Result<Vec<Pwa>> {
    let dir = applications_dir()?;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut apps: Vec<Pwa> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "desktop"))
        .filter_map(|p| {
            let id = p.file_stem()?.to_str()?.to_owned();
            let text = std::fs::read_to_string(&p).ok()?;
            parse_pwa(id, &DesktopEntry::parse(&text))
        })
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    Ok(apps)
}

/// True when no launcher (PWA or not) uses `id` yet.
pub fn id_available(id: &str) -> bool {
    desktop_path(id).is_ok_and(|p| !p.exists())
}

pub fn save(pwa: &Pwa, icon: Option<&RgbaImage>) -> Result<Pwa> {
    let path = desktop_path(&pwa.id)?;
    let mut entry = match std::fs::read_to_string(&path) {
        Ok(text) => DesktopEntry::parse(&text),
        Err(_) => DesktopEntry::new_application(),
    };
    let previous_icon = entry.get("Icon").map(PathBuf::from);

    let mut saved = pwa.clone();
    if let Some(image) = icon {
        let icon_path = icon_theme_dir()?.join(format!("256x256/apps/{}.png", pwa.id));
        crate::icon::save_png(image, &icon_path)?;
        if let Some(old) = previous_icon.filter(|old| old != &icon_path) {
            remove_owned_icon(&old);
        }
        saved.icon = Some(icon_path);
    } else {
        saved.icon = previous_icon.filter(|p| p.is_absolute() && p.exists());
    }

    // Keep any extra browser flags the user added to an existing launcher.
    let extra_args: Vec<String> = entry
        .get("Exec")
        .map(|exec| {
            split_exec(&exec)
                .into_iter()
                .skip(1)
                .filter(|a| !a.starts_with(PROFILE_FLAG) && !a.starts_with(APP_FLAG))
                .collect()
        })
        .unwrap_or_default();
    let mut args = vec![pwa.browser.to_string_lossy().into_owned()];
    args.extend(extra_args);
    args.extend(pwa.browser_args());

    entry.set("Name", &pwa.name);
    entry.set("Exec", &join_exec(&args));
    match &saved.icon {
        Some(icon) => entry.set("Icon", &icon.to_string_lossy()),
        None => entry.remove("Icon"),
    }
    let wm_prefix = crate::browser::family_of(&pwa.browser).wm_prefix;
    entry.set(
        "StartupWMClass",
        &crate::model::wm_class(wm_prefix, &pwa.url, &pwa.profile),
    );

    std::fs::create_dir_all(path.parent().expect("desktop file has a parent"))?;
    let tmp = path.with_extension("desktop.tmp");
    std::fs::write(&tmp, entry.to_string())
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    set_executable(&tmp)?;
    std::fs::rename(&tmp, &path)?;
    refresh_caches();
    Ok(saved)
}

pub fn remove(pwa: &Pwa) -> Result<()> {
    let path = desktop_path(&pwa.id)?;
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
    }
    if let Some(icon) = &pwa.icon {
        remove_owned_icon(icon);
    }
    refresh_caches();
    Ok(())
}

/// Deletes an icon only if it lives in the user's hicolor theme (never system icons).
fn remove_owned_icon(path: &Path) {
    if icon_theme_dir().is_ok_and(|theme| path.starts_with(theme)) {
        let _ = std::fs::remove_file(path);
    }
}

fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// Best-effort refresh so menus pick up changes immediately; both tools are optional.
fn refresh_caches() {
    let quiet = |cmd: &mut Command| {
        let _ = cmd.stdout(Stdio::null()).stderr(Stdio::null()).status();
    };
    if let Ok(dir) = applications_dir() {
        quiet(Command::new("update-desktop-database").arg(dir));
    }
    if let Ok(theme) = icon_theme_dir() {
        quiet(
            Command::new("gtk-update-icon-cache")
                .args(["-f", "-t"])
                .arg(theme),
        );
    }
}

fn parse_pwa(id: String, entry: &DesktopEntry) -> Option<Pwa> {
    let args = split_exec(&entry.get("Exec")?);
    let url = args
        .iter()
        .find_map(|a| a.strip_prefix(APP_FLAG))?
        .to_owned();
    let profile = args
        .iter()
        .find_map(|a| a.strip_prefix(PROFILE_FLAG))
        .unwrap_or(DEFAULT_PROFILE)
        .to_owned();
    let icon = entry
        .get("Icon")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.exists());
    Some(Pwa {
        name: entry.get("Name").unwrap_or_else(|| id.clone()),
        id,
        url,
        browser: PathBuf::from(args.first()?),
        profile,
        icon,
    })
}

/// Line-preserving editor for the `[Desktop Entry]` group of a desktop file.
struct DesktopEntry {
    lines: Vec<String>,
}

impl DesktopEntry {
    fn new_application() -> Self {
        let text = "[Desktop Entry]\nVersion=1.0\nType=Application\nTerminal=false\nCategories=Network;WebBrowser;\n";
        Self::parse(text)
    }

    fn parse(text: &str) -> Self {
        Self {
            lines: text.lines().map(str::to_owned).collect(),
        }
    }

    /// Index range of the lines belonging to `[Desktop Entry]`.
    fn group(&self) -> Option<std::ops::Range<usize>> {
        let start = self
            .lines
            .iter()
            .position(|l| l.trim() == "[Desktop Entry]")?
            + 1;
        let end = self.lines[start..]
            .iter()
            .position(|l| l.trim_start().starts_with('['))
            .map_or(self.lines.len(), |p| start + p);
        Some(start..end)
    }

    fn key_index(&self, key: &str) -> Option<usize> {
        self.group()?.find(|&i| {
            self.lines[i]
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == key)
        })
    }

    /// Unescaped value of `key`.
    fn get(&self, key: &str) -> Option<String> {
        let line = &self.lines[self.key_index(key)?];
        Some(unescape_value(line.split_once('=')?.1.trim_start()))
    }

    fn set(&mut self, key: &str, value: &str) {
        let line = format!("{key}={}", escape_value(value));
        match self.key_index(key) {
            Some(i) => self.lines[i] = line,
            None => match self.group() {
                Some(group) => self.lines.insert(group.end, line),
                None => {
                    self.lines.insert(0, "[Desktop Entry]".to_owned());
                    self.lines.insert(1, line);
                }
            },
        }
    }

    fn remove(&mut self, key: &str) {
        if let Some(i) = self.key_index(key) {
            self.lines.remove(i);
        }
    }
}

impl std::fmt::Display for DesktopEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.lines {
            writeln!(f, "{line}")?;
        }
        Ok(())
    }
}

/// Escapes a desktop-entry string value (`\` and line breaks).
fn escape_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

fn unescape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Builds an `Exec` value (before string-value escaping) per the Desktop Entry spec.
fn join_exec(args: &[String]) -> String {
    const RESERVED: &[char] = &[
        ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(',
        ')', '`',
    ];
    args.iter()
        .map(|arg| {
            let arg = arg.replace('%', "%%");
            if arg.contains(RESERVED) {
                let mut quoted = String::from('"');
                for c in arg.chars() {
                    if matches!(c, '"' | '`' | '$' | '\\') {
                        quoted.push('\\');
                    }
                    quoted.push(c);
                }
                quoted.push('"');
                quoted
            } else {
                arg
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Splits an unescaped `Exec` value into arguments, dropping field codes such as
/// `%U`. Also accepts shell-style quoting inside an argument (`--app="..."`), as
/// written by the original shell scripts.
fn split_exec(value: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_arg = false;
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_arg {
                    args.push(std::mem::take(&mut current));
                    in_arg = false;
                }
            }
            '"' => {
                in_arg = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some(e @ ('"' | '`' | '$' | '\\')) => current.push(e),
                            Some(other) => {
                                current.push('\\');
                                current.push(other);
                            }
                            None => current.push('\\'),
                        },
                        other => current.push(other),
                    }
                }
            }
            '\'' => {
                in_arg = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    current.push(q);
                }
            }
            '\\' => {
                in_arg = true;
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            other => {
                in_arg = true;
                current.push(other);
            }
        }
    }
    if in_arg {
        args.push(current);
    }
    args.into_iter()
        .filter(|a| !(a.len() == 2 && a.starts_with('%') && a != "%%"))
        .map(|a| a.replace("%%", "%"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_launchers_written_by_the_shell_scripts() {
        let text = "[Desktop Entry]\nVersion=1.0\nName=My App\n\
            Exec=/usr/bin/brave --profile-directory=\"Profile 1\" --app=\"https://my.app.com\"\n\
            Icon=/nonexistent/icon.png\nStartupWMClass=brave-my.app.com__-Profile_1\n";
        let pwa = parse_pwa("my_app".into(), &DesktopEntry::parse(text)).unwrap();
        assert_eq!(pwa.name, "My App");
        assert_eq!(pwa.url, "https://my.app.com");
        assert_eq!(pwa.profile, "Profile 1");
        assert_eq!(pwa.browser, PathBuf::from("/usr/bin/brave"));
    }

    #[test]
    fn exec_round_trips_through_desktop_escaping() {
        let args: Vec<String> = [
            "/opt/My Browser/chrome",
            "--profile-directory=Profile 1",
            "--app=https://example.com/a%20b?x=$y",
        ]
        .map(String::from)
        .to_vec();
        let mut entry = DesktopEntry::new_application();
        entry.set("Exec", &join_exec(&args));
        let reparsed = DesktopEntry::parse(&entry.to_string());
        assert_eq!(split_exec(&reparsed.get("Exec").unwrap()), args);
    }

    #[test]
    fn field_codes_are_dropped_and_other_groups_kept() {
        let text = "[Desktop Entry]\nExec=chromium --app=https://x.org %U\n[Desktop Action new]\nExec=other\n";
        let mut entry = DesktopEntry::parse(text);
        assert_eq!(
            split_exec(&entry.get("Exec").unwrap()),
            ["chromium", "--app=https://x.org"]
        );
        entry.set("Name", "X");
        assert_eq!(
            entry.to_string(),
            "[Desktop Entry]\nExec=chromium --app=https://x.org %U\nName=X\n[Desktop Action new]\nExec=other\n"
        );
    }
}
