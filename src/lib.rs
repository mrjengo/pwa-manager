//! Core of PWA Manager: installs Chromium-based Progressive Web Apps as native
//! launchers (Linux desktop entries, Windows Start Menu shortcuts, macOS app bundles).

pub mod browser;
pub mod icon;
pub mod manager;
pub mod model;
mod net;
pub mod platform;
