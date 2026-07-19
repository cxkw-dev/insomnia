//! insomnia — keep your Mac awake, beautifully.
//!
//! - [`power`] holds the IOKit power assertions and battery status.
//! - [`config`] parses CLI flags and the optional config file.
//! - [`collect`] runs the background data sources (docker, custom cards).
//! - [`ui`] lays out the status and dashboard cards and runs the loop.
//! - [`sky`] renders the starfield and the registry of pixel-art flyers.
//! - [`art`] and [`pixel`] hold the sprites and the half-block renderer.
//! - [`theme`] is the shared palette.

#[cfg(not(target_os = "macos"))]
compile_error!("insomnia is macOS-only — it keeps the Mac awake through IOKit power assertions");

pub mod art;
pub mod collect;
pub mod config;
pub mod pixel;
pub mod power;
pub mod sky;
pub mod theme;
pub mod ui;
