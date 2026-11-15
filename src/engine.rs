//! Managed browser engine: downloads the latest Ungoogled Chromium release (FOSS,
//! Chromium without Google services) into the per-user data directory, for systems
//! with no Chromium-based browser installed.

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Ungoogled Chromium";
pub const HOMEPAGE: &str = "https://ungoogled-software.github.io/";

#[cfg(target_os = "linux")]
const REPO: &str = "ungoogled-software/ungoogled-chromium-portablelinux";
#[cfg(windows)]
const REPO: &str = "ungoogled-software/ungoogled-chromium-windows";
#[cfg(target_os = "macos")]
const REPO: &str = "ungoogled-software/ungoogled-chromium-macos";

/// Release asset suffix for this OS and CPU.
fn asset_suffix() -> Result<&'static str> {
    let suffix = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64_linux.tar.xz",
        ("linux", "aarch64") => "arm64_linux.tar.xz",
        ("windows", "x86_64") => "windows_x64.zip",
        ("windows", "aarch64") => "windows_arm64.zip",
        ("macos", "x86_64") => "x86_64-macos.dmg",
        ("macos", "aarch64") => "arm64-macos.dmg",
        (os, arch) => bail!("no {NAME} build is published for {os}/{arch}"),
    };
    Ok(suffix)
}

/// An installed engine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Engine {
    pub version: String,
    pub executable: PathBuf,
}

/// A downloadable engine release.
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub size: u64,
}

pub enum Progress {
    Downloading { done: u64, total: u64 },
    Extracting,
}

fn root() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .context("cannot determine the local data directory")?
        .join("pwa-manager")
        .join("engine"))
}

fn manifest_path() -> Result<PathBuf> {
    Ok(root()?.join("engine.json"))
}

/// The engine installed by [`install`], if its executable is still present.
pub fn installed() -> Option<Engine> {
    let text = std::fs::read(manifest_path().ok()?).ok()?;
    let engine: Engine = serde_json::from_slice(&text).ok()?;
    engine.executable.is_file().then_some(engine)
}

/// True for paths inside the managed engine directory.
pub fn owns(path: &Path) -> bool {
    root().is_ok_and(|root| path.starts_with(root))
}

/// Browser data directory of the managed engine, kept apart from any system
/// Chromium so two different versions never share (and migrate) one profile.
pub fn user_data_dir() -> Result<PathBuf> {
    Ok(root()?.join("user-data"))
}

/// Looks up the latest release for this platform on GitHub.
pub fn latest() -> Result<Release> {
    #[derive(Deserialize)]
    struct GhRelease {
        tag_name: String,
        assets: Vec<GhAsset>,
    }
    #[derive(Deserialize)]
    struct GhAsset {
        name: String,
        browser_download_url: String,
        size: u64,
    }

    let api = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let (_, text) = crate::net::get_text(&api, 4 * 1024 * 1024)?;
    let release: GhRelease =
        serde_json::from_str(&text).context("unexpected GitHub API response")?;
    let suffix = asset_suffix()?;
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name.ends_with(suffix))
        .with_context(|| format!("release {} has no *{suffix} asset", release.tag_name))?;
    Ok(Release {
        version: release.tag_name,
        url: asset.browser_download_url,
        size: asset.size,
    })
}

/// Downloads and unpacks `release`, replacing any previously installed engine.
pub fn install(release: &Release, on_progress: &mut dyn FnMut(Progress)) -> Result<Engine> {
    let root = root()?;
    let staging = root.join("staging");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;
    let result = download_and_unpack(release, &staging, on_progress).and_then(|unpacked| {
        let dest = root.join("ungoogled-chromium");
        if dest.exists() {
            std::fs::remove_dir_all(&dest)
                .with_context(|| format!("cannot replace {}", dest.display()))?;
        }
        std::fs::rename(&unpacked, &dest)?;
        let executable = find_executable(&dest)?;
        let engine = Engine {
            version: release.version.clone(),
            executable,
        };
        std::fs::write(manifest_path()?, serde_json::to_vec_pretty(&engine)?)?;
        Ok(engine)
    });
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// Deletes the managed engine but keeps its browser data (logins, settings).
/// Apps that use it stop working until reassigned or the engine is reinstalled.
pub fn remove() -> Result<()> {
    let root = root()?;
    for path in [root.join("ungoogled-chromium"), root.join("staging")] {
        if path.exists() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("cannot remove {}", path.display()))?;
        }
    }
    let manifest = manifest_path()?;
    if manifest.exists() {
        std::fs::remove_file(manifest)?;
    }
    Ok(())
}

/// Returns the directory holding the unpacked engine.
fn download_and_unpack(
    release: &Release,
    staging: &Path,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<PathBuf> {
    let archive = staging.join("archive");
    download(release, &archive, on_progress)?;
    on_progress(Progress::Extracting);
    let unpacked = staging.join("unpacked");
    std::fs::create_dir_all(&unpacked)?;
    unpack(&archive, &unpacked)?;
    Ok(unpacked)
}

fn download(release: &Release, dest: &Path, on_progress: &mut dyn FnMut(Progress)) -> Result<()> {
    let mut res = crate::net::AGENT
        .get(&release.url)
        .call()
        .with_context(|| format!("download of {} failed", release.url))?;
    let total = res.body().content_length().unwrap_or(release.size);
    let mut reader = res.body_mut().with_config().limit(4 << 30).reader();
    let mut out = BufWriter::new(File::create(dest)?);
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut reported = 0u64;
    on_progress(Progress::Downloading { done, total });
    loop {
        let n = reader.read(&mut buf).context("download interrupted")?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        done += n as u64;
        if done - reported >= 1 << 20 {
            reported = done;
            on_progress(Progress::Downloading { done, total });
        }
    }
    out.flush()?;
    if done != total {
        bail!("download incomplete: {done} of {total} bytes");
    }
    on_progress(Progress::Downloading { done, total });
    Ok(())
}

#[cfg(target_os = "linux")]
fn unpack(archive: &Path, dest: &Path) -> Result<()> {
    let decoder = liblzma::read::XzDecoder::new(std::io::BufReader::new(File::open(archive)?));
    let mut tar = tar::Archive::new(decoder);
    tar.set_preserve_permissions(true);
    tar.unpack(dest)
        .context("cannot extract the engine archive")
}

#[cfg(windows)]
fn unpack(archive: &Path, dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(File::open(archive)?))?;
    zip.extract(dest)
        .context("cannot extract the engine archive")
}

#[cfg(target_os = "macos")]
fn unpack(archive: &Path, dest: &Path) -> Result<()> {
    use std::process::Command;
    let mount = archive.with_extension("mnt");
    std::fs::create_dir_all(&mount)?;
    let status = Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-mountpoint",
        ])
        .arg(&mount)
        .arg(archive)
        .status()
        .context("cannot run hdiutil")?;
    if !status.success() {
        bail!("hdiutil could not mount the engine disk image");
    }
    let copied = (|| -> Result<()> {
        let app = std::fs::read_dir(&mount)?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .context("the disk image contains no application")?;
        let target = dest.join(app.file_name().expect("entry has a name"));
        let status = Command::new("ditto").arg(&app).arg(&target).status()?;
        if !status.success() {
            bail!("cannot copy {}", app.display());
        }
        Ok(())
    })();
    let _ = Command::new("hdiutil")
        .arg("detach")
        .arg(&mount)
        .arg("-force")
        .status();
    copied
}

/// Locates the browser executable inside an unpacked engine.
fn find_executable(dir: &Path) -> Result<PathBuf> {
    let wanted = |p: &Path| -> bool {
        if cfg!(target_os = "macos") {
            p.parent().is_some_and(|d| d.ends_with("Contents/MacOS"))
                && p.ancestors()
                    .nth(3)
                    .is_some_and(|a| a.extension().is_some_and(|e| e == "app"))
        } else if cfg!(windows) {
            p.file_name().is_some_and(|n| n == "chrome.exe")
        } else {
            p.file_name().is_some_and(|n| n == "chrome")
        }
    };
    let mut queue = vec![(dir.to_path_buf(), 0)];
    while let Some((current, depth)) = queue.pop() {
        for entry in std::fs::read_dir(&current)?.flatten() {
            let path = entry.path();
            if path.is_file() && wanted(&path) {
                return Ok(path);
            }
            if path.is_dir() && depth < 3 {
                queue.push((path, depth + 1));
            }
        }
    }
    bail!("browser executable not found in {}", dir.display())
}
