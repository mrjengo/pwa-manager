//! Per-OS launcher backends. Each exposes `list`, `id_available`, `save` and `remove`.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{id_available, list, remove, save};

#[cfg(any(windows, target_os = "macos"))]
mod registry;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{id_available, list, remove, save};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{id_available, list, remove, save};
