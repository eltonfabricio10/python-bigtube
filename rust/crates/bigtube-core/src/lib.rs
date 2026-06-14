//! `bigtube-core` — pure logic + I/O layer for the BigTube Rust port.
//!
//! This crate is intentionally UI-free (no GTK / GLib). It mirrors the Python
//! `src/bigtube/core/` package so it can be tested headlessly and reused by the
//! GTK4 front-end. See `PORTING_RUST.md` (Fase 1) for the migration plan.

pub mod config;
pub mod converter;
pub mod converter_history;
pub mod debounce;
pub mod download_manager;
pub mod downloader;
pub mod enums;
pub mod errors;
pub mod helpers;
pub mod history;
pub mod json_store;
pub mod network_checker;
pub mod paths;
pub mod player;
pub mod process;
pub mod progress;
pub mod scheduled_downloads;
pub mod search;
pub mod search_history;
pub mod updater;
pub mod util;
pub mod validators;

pub use errors::{BigTubeError, Result};
