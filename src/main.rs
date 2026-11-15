// Release builds on Windows are GUI-subsystem apps (no console window); the
// CLI re-attaches to the parent console below.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli;
mod gui;

use clap::Parser;

fn main() {
    let args = cli::Cli::parse_from(std::env::args_os());
    let result = match args.command {
        None if display_available() => gui::run(),
        Some(cli::Command::Gui) => gui::run(),
        None => cli::run(cli::Command::Menu),
        Some(command) => {
            attach_console();
            cli::run(command)
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// Without a graphical session (e.g. over SSH) `pwa` falls back to the menu.
fn display_available() -> bool {
    if cfg!(target_os = "linux") {
        ["DISPLAY", "WAYLAND_DISPLAY"]
            .iter()
            .any(|v| std::env::var_os(v).is_some())
    } else {
        true
    }
}

#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: plain Win32 call without pointers; failure just means no console.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}
