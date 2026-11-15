//! Operations shared by the GUI and the CLI.

use std::path::PathBuf;

use anyhow::Result;

use crate::model::{self, Pwa};
use crate::platform;

/// Where the icon of an installed or edited app comes from.
#[derive(Clone, Debug)]
pub enum IconChoice {
    /// Leave the current icon untouched (behaves like `Auto` on install).
    Keep,
    /// Discover it from the web app's manifest and `<link>` tags.
    Auto,
    /// http(s) URL or local file path.
    Source(String),
}

impl IconChoice {
    /// Empty input means "discover automatically" on install and "keep" on edit.
    pub fn from_input(input: &str, editing: bool) -> Self {
        match input.trim() {
            "" if editing => Self::Keep,
            "" => Self::Auto,
            s => Self::Source(s.to_owned()),
        }
    }
}

/// User-provided fields for a new or edited app.
#[derive(Clone, Debug)]
pub struct Draft {
    pub name: String,
    pub url: String,
    pub icon: IconChoice,
    pub browser: PathBuf,
    pub profile: String,
}

pub struct Outcome {
    pub app: Pwa,
    /// Non-fatal problem worth showing to the user (e.g. no icon found).
    pub warning: Option<String>,
}

pub fn install(draft: Draft) -> Result<Outcome> {
    let id = model::unique_id(&draft.name, |id| !platform::id_available(id));
    apply(id, draft, false)
}

pub fn update(existing: &Pwa, draft: Draft) -> Result<Outcome> {
    apply(existing.id.clone(), draft, true)
}

fn apply(id: String, draft: Draft, editing: bool) -> Result<Outcome> {
    let pwa = Pwa {
        id,
        name: draft.name.trim().to_owned(),
        url: model::normalize_url(&draft.url),
        browser: draft.browser,
        profile: match draft.profile.trim() {
            "" => model::DEFAULT_PROFILE.to_owned(),
            p => p.to_owned(),
        },
        icon: None,
    };
    pwa.validate()?;

    let mut warning = None;
    let icon = match draft.icon {
        IconChoice::Keep if editing => None,
        IconChoice::Keep | IconChoice::Auto => match crate::icon::obtain("", &pwa.url) {
            Ok(image) => Some(image),
            Err(e) => {
                warning = Some(format!("{e:#}"));
                None
            }
        },
        IconChoice::Source(source) => Some(crate::icon::obtain(&source, &pwa.url)?),
    };
    let app = platform::save(&pwa, icon.as_ref())?;
    Ok(Outcome { app, warning })
}

/// Finds an app by id, or by case-insensitive name.
pub fn find<'a>(apps: &'a [Pwa], query: &str) -> Option<&'a Pwa> {
    apps.iter()
        .find(|a| a.id == query)
        .or_else(|| apps.iter().find(|a| a.name.eq_ignore_ascii_case(query)))
}
