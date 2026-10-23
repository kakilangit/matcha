//! Matcha, a regular expression workbench.
//!
//! The public modules are the core, without UI: regex engines, explanations,
//! code generation, the pattern library, the update check, and the workbench
//! that ties them together. The benchmarks in `benches/` build on them. The UI
//! modules are private; `main.rs` only calls [`run`].

// Lets modules name this crate `matcha::` instead of `matcha::`.
extern crate self as matcha;

pub mod codegen;
pub mod engine;
pub mod explain;
pub mod json_store;
pub mod library;
pub mod update;
pub mod workbench;

mod about;
mod app;
mod editor;
mod platform;
mod startup;
mod theme;

pub use startup::run;
