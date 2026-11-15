//! Shared blocking HTTP client.

use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{Context, Result};
use ureq::ResponseExt;

const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (compatible; pwa-manager/",
    env!("CARGO_PKG_VERSION"),
    ")"
);

pub static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(Duration::from_secs(60)))
        .build()
        .into()
});

/// Downloads at most `limit` bytes from `url`.
pub fn get_bytes(url: &str, limit: u64) -> Result<Vec<u8>> {
    AGENT
        .get(url)
        .call()
        .and_then(|mut res| res.body_mut().with_config().limit(limit).read_to_vec())
        .with_context(|| format!("download of {url} failed"))
}

/// Downloads `url` as text, returning the final URL after redirects.
pub fn get_text(url: &str, limit: u64) -> Result<(String, String)> {
    let mut res = AGENT
        .get(url)
        .call()
        .with_context(|| format!("request to {url} failed"))?;
    let final_url = res.get_uri().to_string();
    let text = res
        .body_mut()
        .with_config()
        .limit(limit)
        .lossy_utf8(true)
        .read_to_string()
        .with_context(|| format!("reading {url} failed"))?;
    Ok((final_url, text))
}
