//! Platform-independent description of an installed PWA.

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PROFILE: &str = "Default";

/// A web application launched by a Chromium-based browser in `--app` mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pwa {
    /// Stable identifier: the file stem of the generated launcher.
    pub id: String,
    pub name: String,
    pub url: String,
    /// Browser executable that hosts the app window.
    pub browser: PathBuf,
    /// Browser profile directory name (`Default`, `Profile 1`, ...).
    pub profile: String,
    /// PNG icon shown by the launcher, when one is installed.
    #[serde(skip)]
    pub icon: Option<PathBuf>,
}

impl Pwa {
    /// Command-line arguments passed to the browser executable.
    pub fn browser_args(&self) -> Vec<String> {
        let mut args = Vec::with_capacity(3);
        if crate::engine::owns(&self.browser) {
            if let Ok(dir) = crate::engine::user_data_dir() {
                args.push(format!("--user-data-dir={}", dir.display()));
            }
        }
        args.push(format!("--profile-directory={}", self.profile));
        args.push(format!("--app={}", self.url));
        args
    }

    /// Starts the app detached from the current process.
    pub fn launch(&self) -> Result<()> {
        std::process::Command::new(&self.browser)
            .args(self.browser_args())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("cannot start {}: {e}", self.browser.display()))?;
        Ok(())
    }

    /// Rejects values that would produce a broken or unsafe launcher.
    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        validate_url(&self.url)?;
        if self.profile.trim().is_empty()
            || self.profile.chars().any(|c| c.is_control() || c == '"')
        {
            bail!("invalid browser profile name");
        }
        if self.browser.as_os_str().is_empty() {
            bail!("no browser selected");
        }
        Ok(())
    }
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("app name cannot be empty");
    }
    if name.chars().any(char::is_control) {
        bail!("app name cannot contain control characters");
    }
    Ok(())
}

/// Accepts only absolute http(s) URLs; quotes and whitespace would break launcher quoting.
pub fn validate_url(raw: &str) -> Result<url::Url> {
    if raw.chars().any(|c| c.is_whitespace() || c == '"') {
        bail!("URL cannot contain spaces or quotes");
    }
    let parsed = url::Url::parse(raw).map_err(|e| anyhow::anyhow!("invalid URL: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        bail!("URL must start with http:// or https://");
    }
    Ok(parsed)
}

/// Normalizes a user-typed URL, adding `https://` when the scheme is missing.
pub fn normalize_url(raw: &str) -> String {
    let raw = raw.trim();
    if raw.contains("://") {
        raw.to_owned()
    } else {
        format!("https://{raw}")
    }
}

/// Lowercase ASCII identifier derived from a display name.
pub fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() || c == '-' {
            slug.push(c);
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    let slug = slug.trim_matches('_');
    if slug.is_empty() {
        "pwa".to_owned()
    } else {
        slug.to_owned()
    }
}

/// First identifier derived from `name` that is not already taken.
pub fn unique_id(name: &str, taken: impl Fn(&str) -> bool) -> String {
    let base = slugify(name);
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|id| !taken(id))
        .expect("unbounded range")
}

/// Wayland app_id / X11 WM_CLASS Chromium assigns to an `--app=<url>` window:
/// `<prefix>-<host>_<path with '/' as '_'>-<profile with ' ' as '_'>`.
pub fn wm_class(prefix: &str, app_url: &str, profile: &str) -> String {
    let (host, path) = match url::Url::parse(app_url) {
        Ok(u) => (
            u.host_str().unwrap_or_default().to_owned(),
            u.path().to_owned(),
        ),
        Err(_) => (String::new(), "/".to_owned()),
    };
    format!(
        "{prefix}-{host}_{}-{}",
        path.replace('/', "_"),
        profile.replace(' ', "_")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wm_class_matches_chromium_naming() {
        assert_eq!(
            wm_class("chrome", "https://web.whatsapp.com/", "Default"),
            "chrome-web.whatsapp.com__-Default"
        );
        assert_eq!(
            wm_class("brave", "https://discord.com/app", "Profile 1"),
            "brave-discord.com__app-Profile_1"
        );
    }

    #[test]
    fn slugs_and_unique_ids() {
        assert_eq!(slugify("My Cool App!"), "my_cool_app");
        assert_eq!(slugify("  ***  "), "pwa");
        assert_eq!(
            unique_id("Mail", |id| id == "mail" || id == "mail_2"),
            "mail_3"
        );
    }

    #[test]
    fn url_validation() {
        assert!(validate_url("https://example.com/x?y=1").is_ok());
        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("example.com").is_err());
        assert_eq!(normalize_url(" example.com "), "https://example.com");
    }
}
