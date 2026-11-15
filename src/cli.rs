//! Command-line interface: scriptable subcommands plus the interactive menu of
//! the original shell tool. Missing values are prompted for on a terminal.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use dialoguer::{Confirm, Input, Select};

use pwa_manager::browser::{self, Browser};
use pwa_manager::engine;
use pwa_manager::manager::{self, Draft, IconChoice};
use pwa_manager::model::{self, Pwa, DEFAULT_PROFILE};
use pwa_manager::platform;

#[derive(Parser)]
#[command(
    name = "pwa",
    version,
    about = "Install, edit and remove Progressive Web Apps"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Interactive terminal menu
    Menu,
    /// List installed apps
    List {
        /// Print JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Install a new app (prompts for missing values on a terminal)
    Install(Fields),
    /// Change an installed app (interactive editor when no option is given)
    Edit {
        /// App id or name
        app: String,
        #[command(flatten)]
        fields: Fields,
    },
    /// Remove an installed app and its icon
    Remove {
        /// App id or name
        app: String,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Start an installed app
    Launch {
        /// App id or name
        app: String,
    },
    /// List detected browsers and their profiles
    Browsers,
    /// Manage the bundled Ungoogled Chromium engine
    Engine {
        #[command(subcommand)]
        action: EngineAction,
    },
}

#[derive(Subcommand)]
pub enum EngineAction {
    /// Show the installed and latest available version
    Status,
    /// Download (or update to) the latest release
    Install,
    /// Delete the downloaded engine
    Remove,
}

#[derive(Args, Default)]
pub struct Fields {
    /// Display name
    #[arg(long)]
    name: Option<String>,
    /// Web app address (https:// is added when missing)
    #[arg(long)]
    url: Option<String>,
    /// Icon URL or file; empty string discovers it from the site
    #[arg(long)]
    icon: Option<String>,
    /// Browser executable path or family id (chrome, chromium, brave, edge, ...)
    #[arg(long)]
    browser: Option<String>,
    /// Browser profile directory (Default, "Profile 1", ...)
    #[arg(long)]
    profile: Option<String>,
}

impl Fields {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.url.is_none()
            && self.icon.is_none()
            && self.browser.is_none()
            && self.profile.is_none()
    }
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Menu => menu(),
        Command::List { json } => list(json),
        Command::Install(fields) => install(fields),
        Command::Edit { app, fields } => edit(&find(&app)?, fields),
        Command::Remove { app, yes } => remove(&find(&app)?, yes),
        Command::Launch { app } => find(&app)?.launch(),
        Command::Browsers => browsers(),
        Command::Engine { action } => engine(action),
    }
}

fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

fn find(query: &str) -> Result<Pwa> {
    let apps = platform::list()?;
    manager::find(&apps, query)
        .cloned()
        .with_context(|| format!("no installed app matches '{query}' (see `pwa list`)"))
}

fn list(json: bool) -> Result<()> {
    let apps = platform::list()?;
    if json {
        let rows: Vec<serde_json::Value> = apps
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "name": a.name,
                    "url": a.url,
                    "browser": a.browser,
                    "profile": a.profile,
                    "icon": a.icon,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    if apps.is_empty() {
        println!("No PWAs installed.");
        return Ok(());
    }
    let name_w = apps
        .iter()
        .map(|a| a.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let profile_w = apps
        .iter()
        .map(|a| a.profile.chars().count())
        .max()
        .unwrap_or(0)
        .max(7);
    println!(
        "{:name_w$}  {:profile_w$}  {:16}  URL",
        "NAME", "PROFILE", "BROWSER"
    );
    for a in &apps {
        let browser = browser::family_of(&a.browser).label;
        println!(
            "{:name_w$}  {:profile_w$}  {browser:16}  {}",
            a.name, a.profile, a.url
        );
    }
    Ok(())
}

fn browsers() -> Result<()> {
    let found = browser::detect();
    if found.is_empty() {
        println!(
            "No Chromium-based browser found. Run `pwa engine install` to download {}.",
            engine::NAME
        );
    }
    for b in found {
        println!("{} — {}", b.label(), b.path.display());
        for p in b.profiles() {
            println!("    {}", p.label());
        }
    }
    Ok(())
}

fn engine(action: EngineAction) -> Result<()> {
    match action {
        EngineAction::Status => {
            match engine::installed() {
                Some(e) => println!(
                    "Installed: {} {} ({})",
                    engine::NAME,
                    e.version,
                    e.executable.display()
                ),
                None => println!("{} is not installed.", engine::NAME),
            }
            let latest = engine::latest()?;
            println!("Latest:    {} ({} MB)", latest.version, latest.size >> 20);
        }
        EngineAction::Install => {
            let release = engine::latest()?;
            if engine::installed().is_some_and(|e| e.version == release.version) {
                println!(
                    "{} {} is already up to date.",
                    engine::NAME,
                    release.version
                );
                return Ok(());
            }
            println!(
                "Downloading {} {} from {}",
                engine::NAME,
                release.version,
                release.url
            );
            let installed = engine::install(&release, &mut |p| match p {
                engine::Progress::Downloading { done, total } => {
                    print!("\r  {:>4} / {} MB", done >> 20, total >> 20);
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                }
                engine::Progress::Extracting => println!("\n  extracting…"),
            })?;
            println!("Installed {}", installed.executable.display());
        }
        EngineAction::Remove => {
            engine::remove()?;
            println!("{} removed.", engine::NAME);
        }
    }
    Ok(())
}

/// Resolves `--browser`: an existing path, or the first detected browser of a family.
fn resolve_browser(spec: &str) -> Result<PathBuf> {
    let path = PathBuf::from(spec);
    if path.is_file() {
        return Ok(path);
    }
    browser::detect()
        .into_iter()
        .find(|b| b.family.id.eq_ignore_ascii_case(spec))
        .map(|b| b.path)
        .with_context(|| format!("browser '{spec}' not found (see `pwa browsers`)"))
}

fn pick_browser(current: Option<&PathBuf>) -> Result<PathBuf> {
    let mut found = browser::detect();
    if found.is_empty() && interactive() {
        let download = Confirm::new()
            .with_prompt(format!(
                "No Chromium-based browser found. Download {} ({})?",
                engine::NAME,
                engine::HOMEPAGE
            ))
            .default(true)
            .interact()?;
        if download {
            self::engine(EngineAction::Install)?;
            found = browser::detect();
        }
    }
    if found.is_empty() {
        bail!("no Chromium-based browser found; run `pwa engine install` or pass --browser <path>");
    }
    if !interactive() {
        return Ok(found[0].path.clone());
    }
    let labels: Vec<String> = found
        .iter()
        .map(|b| format!("{}  ({})", b.label(), b.path.display()))
        .collect();
    let default = current
        .and_then(|c| found.iter().position(|b| &b.path == c))
        .unwrap_or(0);
    let index = Select::new()
        .with_prompt("Browser")
        .items(&labels)
        .default(default)
        .interact()?;
    Ok(found[index].path.clone())
}

fn pick_profile(browser_path: &Path, current: &str) -> Result<String> {
    if !interactive() {
        return Ok(current.to_owned());
    }
    let profiles = Browser::new(browser_path.to_path_buf()).profiles();
    let mut labels: Vec<String> = profiles.iter().map(|p| p.label()).collect();
    labels.push("Other…".to_owned());
    let default = profiles.iter().position(|p| p.dir == current).unwrap_or(0);
    let index = Select::new()
        .with_prompt("Browser profile")
        .items(&labels)
        .default(default)
        .interact()?;
    match profiles.get(index) {
        Some(p) => Ok(p.dir.clone()),
        None => Ok(Input::new()
            .with_prompt("Profile directory")
            .default(current.to_owned())
            .interact_text()?),
    }
}

fn prompt_name(default: Option<&str>) -> Result<String> {
    let mut input = Input::<String>::new().with_prompt("App name");
    if let Some(d) = default {
        input = input.default(d.to_owned());
    }
    Ok(input
        .validate_with(|v: &String| model::validate_name(v).map_err(|e| e.to_string()))
        .interact_text()?)
}

fn prompt_url(default: Option<&str>) -> Result<String> {
    let mut input = Input::<String>::new().with_prompt("App URL");
    if let Some(d) = default {
        input = input.default(d.to_owned());
    }
    Ok(input
        .validate_with(|v: &String| {
            model::validate_url(&model::normalize_url(v))
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
        .interact_text()
        .map(|v| model::normalize_url(&v))?)
}

fn prompt_icon(editing: bool) -> Result<IconChoice> {
    let hint = if editing {
        "keep current"
    } else {
        "find automatically"
    };
    let value: String = Input::new()
        .with_prompt(format!("Icon URL or file (empty: {hint})"))
        .allow_empty(true)
        .interact_text()?;
    Ok(IconChoice::from_input(&value, editing))
}

fn require<T>(value: Option<T>, flag: &str) -> Result<T> {
    value.with_context(|| format!("missing --{flag} (run on a terminal to be prompted)"))
}

fn install(fields: Fields) -> Result<()> {
    let tty = interactive();
    let name = match fields.name {
        Some(n) => n,
        None if tty => prompt_name(None)?,
        None => require(None, "name")?,
    };
    let url = match fields.url {
        Some(u) => u,
        None if tty => prompt_url(None)?,
        None => require(None, "url")?,
    };
    let browser = match &fields.browser {
        Some(spec) => resolve_browser(spec)?,
        None => pick_browser(None)?,
    };
    let profile = match fields.profile {
        Some(p) => p,
        None => pick_profile(&browser, DEFAULT_PROFILE)?,
    };
    let icon = match fields.icon {
        Some(src) => IconChoice::from_input(&src, false),
        None if tty => prompt_icon(false)?,
        None => IconChoice::Auto,
    };
    println!("Installing {name}…");
    let outcome = manager::install(Draft {
        name,
        url,
        icon,
        browser,
        profile,
    })?;
    report(&outcome);
    Ok(())
}

fn edit(app: &Pwa, fields: Fields) -> Result<()> {
    let mut draft = Draft {
        name: app.name.clone(),
        url: app.url.clone(),
        icon: IconChoice::Keep,
        browser: app.browser.clone(),
        profile: app.profile.clone(),
    };
    if fields.is_empty() {
        if !interactive() {
            bail!("nothing to change: pass --name, --url, --icon, --browser or --profile");
        }
        if !edit_interactively(&mut draft)? {
            println!("No changes saved.");
            return Ok(());
        }
    } else {
        if let Some(n) = fields.name {
            draft.name = n;
        }
        if let Some(u) = fields.url {
            draft.url = u;
        }
        if let Some(i) = fields.icon {
            draft.icon = IconChoice::from_input(&i, false);
        }
        if let Some(b) = fields.browser {
            draft.browser = resolve_browser(&b)?;
        }
        if let Some(p) = fields.profile {
            draft.profile = p;
        }
    }
    let outcome = manager::update(app, draft)?;
    report(&outcome);
    Ok(())
}

/// Field-by-field editor; returns false when the user cancels.
fn edit_interactively(draft: &mut Draft) -> Result<bool> {
    loop {
        let icon = match &draft.icon {
            IconChoice::Keep => "unchanged".to_owned(),
            IconChoice::Auto => "find automatically".to_owned(),
            IconChoice::Source(s) => s.clone(),
        };
        let items = [
            format!("Name     {}", draft.name),
            format!("URL      {}", draft.url),
            format!("Icon     {icon}"),
            format!("Browser  {}", draft.browser.display()),
            format!("Profile  {}", draft.profile),
            "Save changes".to_owned(),
            "Cancel".to_owned(),
        ];
        let choice = Select::new()
            .with_prompt("Edit")
            .items(&items)
            .default(5)
            .interact()?;
        match choice {
            0 => draft.name = prompt_name(Some(&draft.name))?,
            1 => draft.url = prompt_url(Some(&draft.url))?,
            2 => draft.icon = prompt_icon(true)?,
            3 => draft.browser = pick_browser(Some(&draft.browser))?,
            4 => draft.profile = pick_profile(&draft.browser, &draft.profile)?,
            5 => return Ok(true),
            _ => return Ok(false),
        }
    }
}

fn remove(app: &Pwa, yes: bool) -> Result<()> {
    if !yes {
        if !interactive() {
            bail!("refusing to remove without confirmation; pass --yes");
        }
        let confirmed = Confirm::new()
            .with_prompt(format!("Permanently remove '{}'?", app.name))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("Nothing removed.");
            return Ok(());
        }
    }
    platform::remove(app)?;
    println!("Removed {}.", app.name);
    Ok(())
}

fn report(outcome: &manager::Outcome) {
    let app = &outcome.app;
    println!("Saved {} ({}) → {}", app.name, app.id, app.url);
    println!(
        "  browser: {}  profile: {}",
        app.browser.display(),
        app.profile
    );
    if let Some(warning) = &outcome.warning {
        println!("  warning: {warning}");
    }
}

/// Menu-driven interface equivalent to the original `pwa` script.
fn menu() -> Result<()> {
    if !interactive() {
        bail!("the interactive menu needs a terminal");
    }
    loop {
        let choice = Select::new()
            .with_prompt("PWA Manager")
            .items([
                "Install a new PWA",
                "Edit an installed PWA",
                "Remove an installed PWA",
                "Launch an installed PWA",
                "List installed PWAs",
                "Quit",
            ])
            .default(0)
            .interact()?;
        let result = match choice {
            0 => install(Fields::default()),
            1 => select_app("Edit").and_then(|a| a.map_or(Ok(()), |a| edit(&a, Fields::default()))),
            2 => select_app("Remove").and_then(|a| a.map_or(Ok(()), |a| remove(&a, false))),
            3 => select_app("Launch").and_then(|a| a.map_or(Ok(()), |a| a.launch())),
            4 => list(false),
            _ => return Ok(()),
        };
        if let Err(e) = result {
            eprintln!("error: {e:#}");
        }
        println!();
    }
}

fn select_app(action: &str) -> Result<Option<Pwa>> {
    let apps = platform::list()?;
    if apps.is_empty() {
        println!("No PWAs installed.");
        return Ok(None);
    }
    let mut labels: Vec<String> = apps
        .iter()
        .map(|a| format!("{}  ({})", a.name, a.url))
        .collect();
    labels.push("Back".to_owned());
    let index = Select::new()
        .with_prompt(action)
        .items(&labels)
        .default(0)
        .interact()?;
    Ok(apps.get(index).cloned())
}
