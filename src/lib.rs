// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Aatish Bagal

pub mod types;
pub mod tray;
/// Wayland global shortcuts shared by the desktop applications.
#[cfg(feature = "portal-hotkey")]
pub mod portal_hotkey;
pub use types::*;
/// Bundled core version and release compatibility checks.
#[cfg(feature = "updates")]
pub mod updates;
