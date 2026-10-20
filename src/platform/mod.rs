//! Per-OS behavior: window frame, shortcuts, fonts and data directory.
//! Each platform module exposes the same items; this is the only place that picks one.
//! To support another OS, add a module with the same items and a `cfg` pair here.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("Matcha supports only macOS and Linux; add a module under src/platform.");
