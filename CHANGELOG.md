# Changelog
All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.2.1] - 2026-10-17
### Added
- Fallback download path for core update checks
### Fixed
- Core update checks no longer hang when release download redirects stall
- Clearer connection errors when an update check fails

## [0.2.0] - 2026-10-17
### Added
- portal-hotkey feature: global shortcut sessions through the xdg-desktop-portal GlobalShortcuts interface for native Wayland support
- updates feature: shared core update checks and app compatibility reporting for Synapt and SynaptClip
- core.json manifest declaring the core version and the app version ranges it supports
- Release workflow and RELEASING.md

## [0.1.0]
### Added
- Shared types crate with Peer, TrustedPeer, PeerStatus, Clip, ClipType, TransferId, TransferStatus, and CoreError
- Tray control channel (AppKind, TrayCommand) for single-icon coordination between Synapt and SynaptClip
- Apache 2.0 SPDX headers on all source files
- README and logo assets
- CONTRIBUTING.md, SECURITY.md, and GitHub issue templates
### Fixed
- Apache 2.0 copyright format corrected to 'Copyright 2026 Aatish Bagal'
- Build artifacts untracked and .gitignore added
