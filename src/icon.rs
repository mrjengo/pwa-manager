//! Icon acquisition (explicit URL, local file or automatic discovery) and conversion.

use std::path::Path;

use anyhow::{bail, Context, Result};
use image::{imageops::FilterType, DynamicImage, RgbaImage};

/// Edge length of the normalized square icon.
pub const SIZE: u32 = 256;
const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PAGE_BYTES: u64 = 2 * 1024 * 1024;

/// Resolves an icon source: an http(s) URL, a local file path, or (when empty)
/// the best icon advertised by `site_url` through its manifest or `<link>` tags.
pub fn obtain(source: &str, site_url: &str) -> Result<RgbaImage> {
    let source = source.trim();
    let image = if source.is_empty() {
        discover(site_url)?
    } else if source.starts_with("http://") || source.starts_with("https://") {
        decode(&crate::net::get_bytes(source, MAX_ICON_BYTES)?)?
    } else {
        let path = expand_home(source);
        decode(&std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?)?
    };
    Ok(normalize(image))
}

fn expand_home(path: &str) -> std::path::PathBuf {
    match (path.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => path.into(),
    }
}

fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    image::load_from_memory(bytes)
        .context("unsupported image format (use PNG, JPEG, ICO, WebP, GIF or BMP)")
}

/// Fits the image into a transparent `SIZE`×`SIZE` square, keeping proportions.
fn normalize(image: DynamicImage) -> RgbaImage {
    let scaled = image.resize(SIZE, SIZE, FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::new(SIZE, SIZE);
    let x = (SIZE - scaled.width()) / 2;
    let y = (SIZE - scaled.height()) / 2;
    image::imageops::overlay(&mut canvas, &scaled, x.into(), y.into());
    canvas
}

/// Downloads the largest usable icon the site advertises.
fn discover(site_url: &str) -> Result<DynamicImage> {
    let site = url::Url::parse(site_url).context("invalid app URL")?;
    let mut candidates = Vec::new();
    if let Ok((final_url, html)) = crate::net::get_text(site.as_str(), MAX_PAGE_BYTES) {
        let base = url::Url::parse(&final_url).unwrap_or_else(|_| site.clone());
        candidates = page_icons(&base, &html);
    }
    for fallback in ["/apple-touch-icon.png", "/favicon.ico"] {
        if let Ok(u) = site.join(fallback) {
            candidates.push(Candidate { url: u, size: 0 });
        }
    }

    let mut best: Option<DynamicImage> = None;
    for candidate in candidates {
        let Ok(image) =
            crate::net::get_bytes(candidate.url.as_str(), MAX_ICON_BYTES).and_then(|b| decode(&b))
        else {
            continue;
        };
        if image.width().min(image.height()) >= 128 {
            return Ok(image);
        }
        if best.as_ref().is_none_or(|b| image.width() > b.width()) {
            best = Some(image);
        }
    }
    best.with_context(|| format!("no usable icon found on {site}; provide an icon URL or file"))
}

struct Candidate {
    url: url::Url,
    /// Declared edge length; 0 when unknown.
    size: u32,
}

/// Icon candidates from the web manifest and `<link>` tags, best first.
fn page_icons(base: &url::Url, html: &str) -> Vec<Candidate> {
    let mut manifest_icons = Vec::new();
    let mut link_icons = Vec::new();
    for tag in link_tags(html) {
        let rel = attr(tag, "rel").unwrap_or_default().to_ascii_lowercase();
        let Some(href) = attr(tag, "href").and_then(|h| base.join(&h).ok()) else {
            continue;
        };
        if rel.split_whitespace().any(|r| r == "manifest") {
            if let Ok(icons) = manifest_candidates(&href) {
                manifest_icons.extend(icons);
            }
        } else if rel
            .split_whitespace()
            .any(|r| r == "icon" || r == "apple-touch-icon")
        {
            if is_svg(&href, attr(tag, "type").as_deref()) {
                continue;
            }
            let declared = attr(tag, "sizes").map(|s| largest_size(&s)).unwrap_or(0);
            // apple-touch-icon defaults to 180x180 when no size is declared.
            let size = if declared == 0 && rel.contains("apple-touch-icon") {
                180
            } else {
                declared
            };
            link_icons.push(Candidate { url: href, size });
        }
    }
    manifest_icons.sort_by_key(|c| std::cmp::Reverse(c.size));
    link_icons.sort_by_key(|c| std::cmp::Reverse(c.size));
    manifest_icons.extend(link_icons);
    manifest_icons
}

fn manifest_candidates(manifest_url: &url::Url) -> Result<Vec<Candidate>> {
    let (_, text) = crate::net::get_text(manifest_url.as_str(), MAX_PAGE_BYTES)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let Some(icons) = json.get("icons").and_then(|i| i.as_array()) else {
        bail!("manifest has no icons");
    };
    let mut out = Vec::new();
    for icon in icons {
        let field = |k: &str| icon.get(k).and_then(|v| v.as_str()).unwrap_or_default();
        let Ok(url) = manifest_url.join(field("src")) else {
            continue;
        };
        if field("src").is_empty() || is_svg(&url, Some(field("type"))) {
            continue;
        }
        // Maskable-only icons are cropped by launchers; prefer "any".
        let purpose = field("purpose");
        let mut size = largest_size(field("sizes"));
        if !purpose.is_empty() && !purpose.split_whitespace().any(|p| p == "any") {
            size /= 2;
        }
        out.push(Candidate { url, size });
    }
    Ok(out)
}

fn is_svg(url: &url::Url, mime: Option<&str>) -> bool {
    mime.is_some_and(|m| m.contains("svg")) || url.path().to_ascii_lowercase().ends_with(".svg")
}

/// Largest edge in a `sizes` attribute such as `"16x16 192x192"`.
fn largest_size(sizes: &str) -> u32 {
    sizes
        .split_whitespace()
        .filter_map(|s| s.split(['x', 'X']).next()?.parse().ok())
        .max()
        .unwrap_or(0)
}

/// Raw `<link ...>` tag bodies, in document order.
fn link_tags(html: &str) -> impl Iterator<Item = &str> {
    let lower = html.to_ascii_lowercase();
    let mut starts = Vec::new();
    let mut from = 0;
    while let Some(pos) = lower[from..].find("<link") {
        let start = from + pos + "<link".len();
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
        starts.push((start, end));
        from = end;
    }
    starts.into_iter().map(move |(s, e)| &html[s..e])
}

/// Value of an HTML attribute inside a tag body (quoted or bare).
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(name) {
        let start = from + pos;
        from = start + name.len();
        let preceded_ok = start == 0 || lower.as_bytes()[start - 1].is_ascii_whitespace();
        let rest = lower[from..].trim_start();
        if !preceded_ok || !rest.starts_with('=') {
            continue;
        }
        let value_start = tag.len() - rest.len() + 1;
        let value = tag[value_start..].trim_start();
        return Some(match value.chars().next() {
            Some(q @ ('"' | '\'')) => value[1..].split(q).next().unwrap_or_default().to_owned(),
            _ => value
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or_default()
                .to_owned(),
        });
    }
    None
}

pub fn save_png(image: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image
        .save_with_format(path, image::ImageFormat::Png)
        .with_context(|| format!("cannot write {}", path.display()))
}

/// Multi-resolution Windows icon.
#[cfg(windows)]
pub fn save_ico(image: &RgbaImage, path: &Path) -> Result<()> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    let frames = [16u32, 32, 48, 256]
        .into_iter()
        .map(|s| {
            let resized = image::imageops::resize(image, s, s, FilterType::Lanczos3);
            IcoFrame::as_png(resized.as_raw(), s, s, image::ExtendedColorType::Rgba8)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let file =
        std::fs::File::create(path).with_context(|| format!("cannot write {}", path.display()))?;
    IcoEncoder::new(std::io::BufWriter::new(file)).encode_images(&frames)?;
    Ok(())
}

/// Apple icon family with 128, 256 and 512 px representations.
#[cfg(target_os = "macos")]
pub fn save_icns(image: &RgbaImage, path: &Path) -> Result<()> {
    let mut family = icns::IconFamily::new();
    for s in [128u32, 256, 512] {
        let resized = image::imageops::resize(image, s, s, FilterType::Lanczos3);
        let icon = icns::Image::from_data(icns::PixelFormat::RGBA, s, s, resized.into_raw())?;
        family.add_icon(&icon)?;
    }
    let file =
        std::fs::File::create(path).with_context(|| format!("cannot write {}", path.display()))?;
    family.write(std::io::BufWriter::new(file))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_link_tags() {
        let base = url::Url::parse("https://example.com/app/").unwrap();
        let html = r#"<head><LINK rel="icon" href="/fav.png" sizes="32x32">
            <link href='touch.png' rel=apple-touch-icon>
            <link rel="icon" type="image/svg+xml" href="/i.svg">
            <link rel="stylesheet" href="/s.css"></head>"#;
        let urls: Vec<(String, u32)> = page_icons(&base, html)
            .into_iter()
            .map(|c| (c.url.to_string(), c.size))
            .collect();
        assert_eq!(
            urls,
            [
                ("https://example.com/app/touch.png".to_owned(), 180),
                ("https://example.com/fav.png".to_owned(), 32),
            ]
        );
    }

    #[test]
    fn normalizes_to_square() {
        let wide = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            100,
            50,
            image::Rgba([255, 0, 0, 255]),
        ));
        let out = normalize(wide);
        assert_eq!(out.dimensions(), (SIZE, SIZE));
        assert_eq!(out.get_pixel(0, 0).0[3], 0, "letterbox is transparent");
        assert_eq!(out.get_pixel(SIZE / 2, SIZE / 2).0, [255, 0, 0, 255]);
    }
}
