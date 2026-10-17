//! insomnia — keep your Mac awake, beautifully.
//!
//! - [`power`] holds the IOKit power assertions and battery status.
//! - [`config`] parses CLI flags and the optional config file;
//!   [`duration`] parses and formats durations.
//! - [`collect`] runs the background data sources (docker, custom cards)
//!   and parses the card row protocol.
//! - [`cards`] assembles cards and draws their rows; [`dashboard`] lays
//!   them out as the combined, scrollable frame.
//! - [`ui`] runs the loop and draws both layouts and the session.
//! - [`sky`] renders the starfield, comets, and shooting stars.
//! - [`art`] is the wordmark, [`pixel`] the half-block renderer, [`theme`]
//!   the shared palette, and [`text`] the display-width text helpers.

#[cfg(not(target_os = "macos"))]
compile_error!("insomnia is macOS-only — it keeps the Mac awake through IOKit power assertions");

pub mod art;
pub mod cards;
pub mod collect;
pub mod config;
pub mod dashboard;
pub mod duration;
pub mod pixel;
pub mod power;
pub mod sky;
pub mod text;
pub mod theme;
pub mod ui;
