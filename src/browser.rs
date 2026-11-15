//! Discovery of installed Chromium-based browsers and their profiles.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::model::DEFAULT_PROFILE;

/// A Chromium-based browser family and where it lives on each platform.
#[derive(Debug, PartialEq, Eq)]
pub struct Family {
    pub id: &'static str,
    pub label: &'static str,
    /// Prefix Chromium uses for app window classes on Linux. Chromium itself
    /// (distro and ungoogled builds) uses `chrome`, like Google Chrome.
    pub wm_prefix: &'static str,
    /// Lowercase substrings identifying the executable or Flatpak/Snap id.
    needles: &'static [&'static str],
    /// User data directory under `$XDG_CONFIG_HOME`.
    linux_config: &'static str,
    /// Executable paths relative to Program Files / %LOCALAPPDATA%.
    windows_exe: &'static [&'static str],
    /// User data directory relative to %LOCALAPPDATA%.
    windows_data: &'static str,
    /// Bundle name in /Applications and the executable inside `Contents/MacOS`.
    macos_app: (&'static str, &'static str),
    /// User data directory under `~/Library/Application Support`.
    macos_data: &'static str,
}

pub static FAMILIES: &[Family] = &[
    Family {
        id: "ungoogled-chromium",
        label: "Ungoogled Chromium",
        wm_prefix: "chrome",
        needles: &["ungoogled"],
        linux_config: "chromium",
        windows_exe: &[],
        windows_data: "Chromium\\User Data",
        macos_app: ("", ""),
        macos_data: "Chromium",
    },
    Family {
        id: "chromium",
        label: "Chromium",
        wm_prefix: "chrome",
        needles: &["chromium"],
        linux_config: "chromium",
        windows_exe: &["Chromium\\Application\\chrome.exe"],
        windows_data: "Chromium\\User Data",
        macos_app: ("Chromium.app", "Chromium"),
        macos_data: "Chromium",
    },
    Family {
        id: "brave",
        label: "Brave",
        wm_prefix: "brave",
        needles: &["brave"],
        linux_config: "BraveSoftware/Brave-Browser",
        windows_exe: &["BraveSoftware\\Brave-Browser\\Application\\brave.exe"],
        windows_data: "BraveSoftware\\Brave-Browser\\User Data",
        macos_app: ("Brave Browser.app", "Brave Browser"),
        macos_data: "BraveSoftware/Brave-Browser",
    },
    Family {
        id: "edge",
        label: "Microsoft Edge",
        wm_prefix: "msedge",
        needles: &[
            "microsoft-edge",
            "msedge",
            "com.microsoft.edge",
            "microsoft edge",
        ],
        linux_config: "microsoft-edge",
        windows_exe: &["Microsoft\\Edge\\Application\\msedge.exe"],
        windows_data: "Microsoft\\Edge\\User Data",
        macos_app: ("Microsoft Edge.app", "Microsoft Edge"),
        macos_data: "Microsoft Edge",
    },
    Family {
        id: "vivaldi",
        label: "Vivaldi",
        wm_prefix: "vivaldi",
        needles: &["vivaldi"],
        linux_config: "vivaldi",
        windows_exe: &["Vivaldi\\Application\\vivaldi.exe"],
        windows_data: "Vivaldi\\User Data",
        macos_app: ("Vivaldi.app", "Vivaldi"),
        macos_data: "Vivaldi",
    },
    Family {
        id: "thorium",
        label: "Thorium",
        wm_prefix: "thorium",
        needles: &["thorium"],
        linux_config: "thorium",
        windows_exe: &["Thorium\\Application\\thorium.exe"],
        windows_data: "Thorium\\User Data",
        macos_app: ("Thorium.app", "Thorium"),
        macos_data: "Thorium",
    },
    Family {
        id: "opera",
        label: "Opera",
        wm_prefix: "opera",
        needles: &["opera"],
        linux_config: "opera",
        windows_exe: &["Programs\\Opera\\opera.exe"],
        windows_data: "",
        macos_app: ("Opera.app", "Opera"),
        macos_data: "",
    },
    Family {
        id: "yandex",
        label: "Yandex Browser",
        wm_prefix: "yandex",
        needles: &["yandex"],
        linux_config: "yandex-browser",
        windows_exe: &["Yandex\\YandexBrowser\\Application\\browser.exe"],
        windows_data: "Yandex\\YandexBrowser\\User Data",
        macos_app: ("Yandex.app", "Yandex"),
        macos_data: "Yandex/YandexBrowser",
    },
    // Last: the fallback for every executable not matched above.
    Family {
        id: "chrome",
        label: "Google Chrome",
        wm_prefix: "chrome",
        needles: &[],
        linux_config: "google-chrome",
        windows_exe: &["Google\\Chrome\\Application\\chrome.exe"],
        windows_data: "Google\\Chrome\\User Data",
        macos_app: ("Google Chrome.app", "Google Chrome"),
        macos_data: "Google/Chrome",
    },
];

/// Family used for executables that match no known needle.
fn fallback_family() -> &'static Family {
    FAMILIES.last().expect("non-empty family table")
}

/// Classifies a browser executable by its path: the file name first, then the
/// whole path (Windows and portable builds name every executable `chrome`).
pub fn family_of(path: &Path) -> &'static Family {
    let lower = path.to_string_lossy().to_lowercase();
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let specific = &FAMILIES[..FAMILIES.len() - 1];
    specific
        .iter()
        .find(|f| f.needles.iter().any(|n| file.contains(n)))
        .or_else(|| {
            specific
                .iter()
                .find(|f| f.needles.iter().any(|n| lower.contains(n)))
        })
        .unwrap_or_else(fallback_family)
}

/// An installed browser executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Browser {
    pub family: &'static Family,
    pub path: PathBuf,
}

impl Browser {
    pub fn new(path: PathBuf) -> Self {
        Self {
            family: family_of(&path),
            path,
        }
    }

    /// Human readable label, including the packaging format when relevant.
    pub fn label(&self) -> String {
        let p = self.path.to_string_lossy();
        let kind = if crate::engine::owns(&self.path) {
            " (managed)"
        } else if p.contains("/flatpak/") {
            " (Flatpak)"
        } else if p.starts_with("/snap/") {
            " (Snap)"
        } else {
            ""
        };
        format!("{}{kind}", self.family.label)
    }

    /// Profiles found in the browser's `Local State`; always contains `Default`.
    pub fn profiles(&self) -> Vec<Profile> {
        let mut profiles: Vec<Profile> = self
            .user_data_dirs()
            .iter()
            .find_map(|dir| read_profiles(&dir.join("Local State")))
            .unwrap_or_default();
        if !profiles.iter().any(|p| p.dir == DEFAULT_PROFILE) {
            profiles.insert(
                0,
                Profile {
                    dir: DEFAULT_PROFILE.to_owned(),
                    name: String::new(),
                },
            );
        }
        profiles
    }

    fn user_data_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if crate::engine::owns(&self.path) {
            dirs.extend(crate::engine::user_data_dir());
        } else if cfg!(target_os = "linux") {
            let rel = self.family.linux_config;
            if let (Some(home), Some(file)) = (dirs::home_dir(), self.path.file_name()) {
                let file = file.to_string_lossy();
                if self
                    .path
                    .to_string_lossy()
                    .contains("/flatpak/exports/bin/")
                {
                    dirs.push(home.join(".var/app").join(&*file).join("config").join(rel));
                } else if self.path.starts_with("/snap/bin") {
                    dirs.push(home.join("snap").join(&*file).join("common").join(rel));
                    dirs.push(
                        home.join("snap")
                            .join(&*file)
                            .join("current/.config")
                            .join(rel),
                    );
                }
            }
            if let Some(config) = dirs::config_dir() {
                dirs.push(config.join(rel));
            }
        } else if cfg!(windows) {
            if let (Some(local), false) =
                (dirs::data_local_dir(), self.family.windows_data.is_empty())
            {
                dirs.push(local.join(self.family.windows_data));
            }
        } else if cfg!(target_os = "macos") {
            if let (Some(support), false) = (dirs::data_dir(), self.family.macos_data.is_empty()) {
                dirs.push(support.join(self.family.macos_data));
            }
        }
        dirs
    }
}

/// A browser profile: its directory name and the name the user gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub dir: String,
    pub name: String,
}

impl Profile {
    pub fn label(&self) -> String {
        if self.name.is_empty() || self.name == self.dir {
            self.dir.clone()
        } else {
            format!("{} ({})", self.name, self.dir)
        }
    }
}

fn read_profiles(local_state: &Path) -> Option<Vec<Profile>> {
    let text = std::fs::read_to_string(local_state).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let cache = json.get("profile")?.get("info_cache")?.as_object()?;
    let mut profiles: Vec<Profile> = cache
        .iter()
        .map(|(dir, info)| Profile {
            dir: dir.clone(),
            name: info
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or_default()
                .to_owned(),
        })
        .collect();
    // Default first, then "Profile N" in numeric order.
    profiles.sort_by_key(|p| {
        let n = p
            .dir
            .strip_prefix("Profile ")
            .and_then(|n| n.parse::<u32>().ok());
        (
            p.dir != DEFAULT_PROFILE,
            n.unwrap_or(u32::MAX),
            p.dir.clone(),
        )
    });
    Some(profiles)
}

/// Every Chromium-based browser installed on this system, deduplicated; the
/// managed engine (if downloaded) comes first.
pub fn detect() -> Vec<Browser> {
    let mut seen = HashSet::new();
    crate::engine::installed()
        .map(|e| e.executable)
        .into_iter()
        .chain(candidates())
        .filter(|p| is_executable(p))
        .filter(|p| seen.insert(std::fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
        .map(Browser::new)
        .collect()
}

#[cfg(target_os = "linux")]
fn candidates() -> Vec<PathBuf> {
    // Executable name prefixes (lowercase), including Flatpak application ids.
    const PREFIXES: &[&str] = &[
        "google-chrome",
        "chromium",
        "microsoft-edge",
        "brave",
        "vivaldi",
        "thorium",
        "opera",
        "yandex",
        "com.google.chrome",
        "org.chromium.chromium",
        "io.github.ungoogled_software.ungoogled_chromium",
        "com.brave.browser",
        "com.microsoft.edge",
        "com.vivaldi.vivaldi",
        "com.opera.opera",
        "ru.yandex.browser",
    ];
    const EXCLUDED: &[&str] = &[
        "sandbox",
        "driver",
        "wrapper",
        "crashpad",
        "gnome-shell",
        "flags",
    ];
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".local/share/flatpak/exports/bin"));
    }
    dirs.push("/var/lib/flatpak/exports/bin".into());
    dirs.push("/snap/bin".into());

    let mut found = Vec::new();
    let mut seen_dirs = HashSet::new();
    for dir in dirs {
        if !seen_dirs.insert(dir.clone()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                PREFIXES.iter().any(|x| name.starts_with(x))
                    && !EXCLUDED.iter().any(|x| name.contains(x))
            })
            .collect();
        names.sort();
        found.extend(names);
    }
    found
}

#[cfg(windows)]
fn candidates() -> Vec<PathBuf> {
    let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect();
    FAMILIES
        .iter()
        .flat_map(|f| f.windows_exe.iter())
        .flat_map(|rel| roots.iter().map(move |root| root.join(rel)))
        .collect()
}

#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }
    FAMILIES
        .iter()
        .filter(|f| !f.macos_app.0.is_empty())
        .flat_map(|f| {
            roots.iter().map(move |r| {
                r.join(f.macos_app.0)
                    .join("Contents/MacOS")
                    .join(f.macos_app.1)
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
fn candidates() -> Vec<PathBuf> {
    Vec::new()
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_executables() {
        let id = |p: &str| family_of(Path::new(p)).id;
        assert_eq!(id("/usr/bin/google-chrome-stable"), "chrome");
        assert_eq!(id("/usr/bin/chromium-browser"), "chromium");
        assert_eq!(
            id("/var/lib/flatpak/exports/bin/com.brave.Browser"),
            "brave"
        );
        assert_eq!(
            id("/var/lib/flatpak/exports/bin/org.chromium.Chromium"),
            "chromium"
        );
        assert_eq!(id("/usr/bin/microsoft-edge-stable"), "edge");
        assert_eq!(
            id("/var/lib/flatpak/exports/bin/io.github.ungoogled_software.ungoogled_chromium"),
            "ungoogled-chromium"
        );
        assert_eq!(
            id("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"),
            "chrome"
        );
        assert_eq!(
            id("C:\\Program Files\\Chromium\\Application\\chrome.exe"),
            "chromium"
        );
        assert_eq!(
            id("/data/pwa-manager/engine/ungoogled-chromium/ungoogled-chromium-1-x86_64_linux/chrome"),
            "ungoogled-chromium"
        );
        assert_eq!(id("/opt/knowledge-base/bin/chrome"), "chrome");
        assert_eq!(
            id("C:\\Program Files\\Chromium\\Application\\chrome.exe"),
            "chromium"
        );
        assert_eq!(
            id("/data/pwa-manager/engine/ungoogled-chromium/ungoogled-chromium-1-x86_64_linux/chrome"),
            "ungoogled-chromium"
        );
        assert_eq!(id("/opt/knowledge-base/bin/chrome"), "chrome");
        assert_eq!(
            id("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"),
            "brave"
        );
    }

    #[test]
    fn reads_profiles_from_local_state() {
        let dir = std::env::temp_dir().join(format!("pwa-local-state-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Local State");
        std::fs::write(
            &file,
            r#"{"profile":{"info_cache":{"Profile 10":{"name":"Ten"},"Profile 2":{"name":"Work"},"Default":{"name":"Me"}}}}"#,
        )
        .unwrap();
        let dirs: Vec<String> = read_profiles(&file)
            .unwrap()
            .into_iter()
            .map(|p| p.dir)
            .collect();
        assert_eq!(dirs, ["Default", "Profile 2", "Profile 10"]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
