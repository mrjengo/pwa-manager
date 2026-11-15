//! Native desktop window built with Slint. Slow work (network, disk) runs on
//! worker threads and reports back through the Slint event loop.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::Result;
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};

use pwa_manager::browser::{self, Browser, Profile};
use pwa_manager::engine;
use pwa_manager::manager::{self, Draft, IconChoice};
use pwa_manager::model::{Pwa, DEFAULT_PROFILE};
use pwa_manager::platform;

slint::include_modules!();

#[derive(Default)]
struct State {
    apps: Vec<Pwa>,
    browsers: Vec<Browser>,
    /// Profiles of the selected browser; the combo box adds a final "Other…".
    profiles: Vec<Profile>,
}

type Shared = Rc<RefCell<State>>;

thread_local! {
    /// UI-thread state, reachable from callbacks posted back by worker threads.
    static STATE: Shared = Rc::default();
}

fn ui_state() -> Shared {
    STATE.with(Rc::clone)
}

pub fn run() -> Result<()> {
    let ui = AppWindow::new()?;
    let state = ui_state();
    ui.set_engine_name(engine::NAME.into());

    reload(&ui, &state, None);
    clear_form(&ui, &state);

    ui.on_new_app({
        let (ui, state) = (ui.as_weak(), state.clone());
        move || clear_form(&ui.unwrap(), &state)
    });
    ui.on_app_selected({
        let (ui, state) = (ui.as_weak(), state.clone());
        move |index| load_form(&ui.unwrap(), &state, index as usize)
    });
    ui.on_browser_changed({
        let (ui, state) = (ui.as_weak(), state.clone());
        move |index| {
            let ui = ui.unwrap();
            let current = selected_profile(&ui, &state);
            set_profiles(&ui, &state, index as usize, &current);
        }
    });
    ui.on_browse_icon({
        let ui = ui.as_weak();
        move || {
            let file = rfd::FileDialog::new()
                .set_title("Choose an icon")
                .add_filter(
                    "Images",
                    &["png", "jpg", "jpeg", "ico", "webp", "gif", "bmp"],
                )
                .pick_file();
            if let Some(path) = file {
                let ui = ui.unwrap();
                ui.set_form_icon(path.to_string_lossy().as_ref().into());
                if let Ok(image) = Image::load_from_path(&path) {
                    ui.set_form_icon_preview(image);
                    ui.set_form_has_icon(true);
                }
            }
        }
    });
    ui.on_launch({
        let (ui, state) = (ui.as_weak(), state.clone());
        move || {
            let ui = ui.unwrap();
            let Some(app) = selected_app(&ui, &state) else {
                return;
            };
            match app.launch() {
                Ok(()) => set_status(&ui, &format!("Started {}.", app.name), false),
                Err(e) => set_status(&ui, &format!("{e:#}"), true),
            }
        }
    });
    ui.on_remove({
        let (ui, state) = (ui.as_weak(), state.clone());
        move || {
            let ui = ui.unwrap();
            let Some(app) = selected_app(&ui, &state) else {
                return;
            };
            match platform::remove(&app) {
                Ok(()) => {
                    reload(&ui, &state, None);
                    clear_form(&ui, &state);
                    set_status(&ui, &format!("Removed {}.", app.name), false);
                }
                Err(e) => set_status(&ui, &format!("{e:#}"), true),
            }
        }
    });
    ui.on_save({
        let (ui, state) = (ui.as_weak(), state.clone());
        move || save(&ui.unwrap(), &state)
    });
    ui.on_download_engine({
        let ui = ui.as_weak();
        move || download_engine(&ui.unwrap())
    });

    ui.run()?;
    Ok(())
}

/// Reloads apps and browsers; keeps the app with `select_id` selected.
fn reload(ui: &AppWindow, state: &Shared, select_id: Option<&str>) {
    let apps = platform::list().unwrap_or_else(|e| {
        set_status(ui, &format!("Cannot list apps: {e:#}"), true);
        Vec::new()
    });
    let items: Vec<AppItem> = apps
        .iter()
        .map(|a| {
            let icon = a.icon.as_ref().and_then(|p| Image::load_from_path(p).ok());
            AppItem {
                name: a.name.as_str().into(),
                url: a.url.as_str().into(),
                has_icon: icon.is_some(),
                icon: icon.unwrap_or_default(),
            }
        })
        .collect();
    ui.set_apps(ModelRc::new(VecModel::from(items)));

    let browsers = browser::detect();
    let labels: Vec<SharedString> = browsers.iter().map(|b| b.label().into()).collect();
    ui.set_browser_labels(ModelRc::new(VecModel::from(labels)));
    ui.set_engine_installed(engine::installed().is_some());

    let selected = select_id.and_then(|id| apps.iter().position(|a| a.id == id));
    {
        let mut s = state.borrow_mut();
        s.apps = apps;
        s.browsers = browsers;
    }
    match selected {
        Some(index) => load_form(ui, state, index),
        None => ui.set_selected_index(-1),
    }
}

fn clear_form(ui: &AppWindow, state: &Shared) {
    ui.set_selected_index(-1);
    ui.set_form_name("".into());
    ui.set_form_url("".into());
    ui.set_form_icon("".into());
    ui.set_form_has_icon(false);
    ui.set_custom_profile("".into());
    ui.set_browser_index(0);
    set_profiles(ui, state, 0, DEFAULT_PROFILE);
}

fn load_form(ui: &AppWindow, state: &Shared, index: usize) {
    let Some(app) = state.borrow().apps.get(index).cloned() else {
        return;
    };
    ui.set_selected_index(index as i32);
    ui.set_form_name(app.name.as_str().into());
    ui.set_form_url(app.url.as_str().into());
    ui.set_form_icon("".into());
    match app
        .icon
        .as_ref()
        .and_then(|p| Image::load_from_path(p).ok())
    {
        Some(image) => {
            ui.set_form_icon_preview(image);
            ui.set_form_has_icon(true);
        }
        None => ui.set_form_has_icon(false),
    }

    // The app may use a browser that detection did not find (e.g. a custom path).
    let browser_index = {
        let mut s = state.borrow_mut();
        match s.browsers.iter().position(|b| b.path == app.browser) {
            Some(i) => i,
            None => {
                s.browsers.push(Browser::new(app.browser.clone()));
                s.browsers.len() - 1
            }
        }
    };
    let labels: Vec<SharedString> = state
        .borrow()
        .browsers
        .iter()
        .map(|b| b.label().into())
        .collect();
    ui.set_browser_labels(ModelRc::new(VecModel::from(labels)));
    ui.set_browser_index(browser_index as i32);
    set_profiles(ui, state, browser_index, &app.profile);
    set_status(ui, "", false);
}

/// Fills the profile combo box for a browser and selects `current`.
fn set_profiles(ui: &AppWindow, state: &Shared, browser_index: usize, current: &str) {
    let mut profiles = state
        .borrow()
        .browsers
        .get(browser_index)
        .map(Browser::profiles)
        .unwrap_or_default();
    if profiles.is_empty() {
        profiles.push(Profile {
            dir: DEFAULT_PROFILE.to_owned(),
            name: String::new(),
        });
    }
    let mut labels: Vec<SharedString> = profiles.iter().map(|p| p.label().into()).collect();
    labels.push("Other…".into());
    let index = profiles.iter().position(|p| p.dir == current);
    ui.set_profile_labels(ModelRc::new(VecModel::from(labels)));
    match index {
        Some(i) => ui.set_profile_index(i as i32),
        None => {
            ui.set_profile_index(profiles.len() as i32);
            ui.set_custom_profile(current.into());
        }
    }
    state.borrow_mut().profiles = profiles;
}

fn selected_profile(ui: &AppWindow, state: &Shared) -> String {
    let index = ui.get_profile_index();
    state
        .borrow()
        .profiles
        .get(index.max(0) as usize)
        .map(|p| p.dir.clone())
        .unwrap_or_else(|| ui.get_custom_profile().trim().to_owned())
}

fn selected_app(ui: &AppWindow, state: &Shared) -> Option<Pwa> {
    let index = ui.get_selected_index();
    (index >= 0)
        .then(|| state.borrow().apps.get(index as usize).cloned())
        .flatten()
}

fn set_status(ui: &AppWindow, text: &str, error: bool) {
    ui.set_status(text.into());
    ui.set_status_is_error(error);
}

fn save(ui: &AppWindow, state: &Shared) {
    let existing = selected_app(ui, state);
    let browser: Option<PathBuf> = state
        .borrow()
        .browsers
        .get(ui.get_browser_index().max(0) as usize)
        .map(|b| b.path.clone());
    let Some(browser) = browser else {
        set_status(ui, "Select a browser first.", true);
        return;
    };
    let draft = Draft {
        name: ui.get_form_name().to_string(),
        url: ui.get_form_url().to_string(),
        icon: IconChoice::from_input(&ui.get_form_icon(), existing.is_some()),
        browser,
        profile: selected_profile(ui, state),
    };

    ui.set_busy(true);
    set_status(
        ui,
        if existing.is_some() {
            "Saving…"
        } else {
            "Installing…"
        },
        false,
    );
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let result = match &existing {
            Some(app) => manager::update(app, draft),
            None => manager::install(draft),
        };
        let message = match &result {
            Ok(outcome) => Ok((
                outcome.app.id.clone(),
                outcome.app.name.clone(),
                outcome.warning.clone(),
            )),
            Err(e) => Err(format!("{e:#}")),
        };
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_busy(false);
            match message {
                Ok((id, name, warning)) => {
                    reload(&ui, &ui_state(), Some(&id));
                    match warning {
                        Some(w) => {
                            set_status(&ui, &format!("Saved {name} without an icon: {w}"), true)
                        }
                        None => set_status(
                            &ui,
                            &format!("Saved {name}. Find it in your applications menu."),
                            false,
                        ),
                    }
                }
                Err(e) => set_status(&ui, &e, true),
            }
        });
    });
}

fn download_engine(ui: &AppWindow) {
    ui.set_engine_busy(true);
    ui.set_engine_progress(-1.0);
    set_status(ui, "", false);
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let progress_ui = weak.clone();
        let result = engine::latest().and_then(|release| {
            engine::install(&release, &mut |p| {
                let value = match p {
                    engine::Progress::Downloading { done, total } if total > 0 => {
                        done as f32 / total as f32
                    }
                    _ => -1.0,
                };
                let _ = progress_ui.upgrade_in_event_loop(move |ui| ui.set_engine_progress(value));
            })
        });
        let message = result.map(|e| e.version).map_err(|e| format!("{e:#}"));
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_engine_busy(false);
            match message {
                Ok(version) => {
                    let state = ui_state();
                    let selected = selected_app(&ui, &state).map(|a| a.id);
                    reload(&ui, &state, selected.as_deref());
                    if selected.is_none() {
                        clear_form(&ui, &state);
                    }
                    set_status(&ui, &format!("{} {version} is ready.", engine::NAME), false);
                }
                Err(e) => set_status(&ui, &format!("Download failed: {e}"), true),
            }
        });
    });
}
