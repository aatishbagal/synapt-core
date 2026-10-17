// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Aatish Bagal

pub mod types;
pub mod tray;
/// Wayland global shortcuts shared by the desktop applications.
#[cfg(feature = "portal-hotkey")]
pub mod portal_hotkey;
pub use types::*;
