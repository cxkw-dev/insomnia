//! insomnia — keep your Mac awake, beautifully.
//!
//! - [`power`] holds the IOKit power assertions and battery status.
//! - [`config`] parses CLI flags and the optional config file.
//! - [`collect`] runs the background data sources (docker, custom cards).
//! - [`cards`] assembles and renders the dashboard cards.
//! - [`ui`] lays out the status card and cards and runs the loop.
//! - [`sky`] renders the starfield, comets, and shooting stars.
//! - [`art`] holds the title art; [`pixel`] is the half-block renderer.
//! - [`theme`] is the shared palette.

#[cfg(not(target_os = "macos"))]
compile_error!("insomnia is macOS-only — it keeps the Mac awake through IOKit power assertions");

pub mod art;
pub mod cards;
pub mod collect;
pub mod config;
pub mod dashboard;
pub mod pixel;
pub mod power;
pub mod sky;
pub mod theme;
pub mod ui;
