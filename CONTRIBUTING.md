# Contributing to synapt-core

synapt-core is the shared type library for the Synapt ecosystem. It defines data
structures used by both Synapt and SynaptClip.

## Scope

This crate intentionally has a narrow scope:

- Shared types only: `Peer`, `TrustedPeer`, `PeerStatus`, `Clip`, `ClipType`,
  `TransferId`, `TransferStatus`, and `CoreError`
- The tray coordination channel (`tray.rs`), which defines `AppKind` and `TrayCommand`
- No network code, no async, no business logic

**Open an issue before adding new types or modifying existing ones.** Changes
here affect both Synapt and SynaptClip and require coordinated updates in both
repositories. Both consume this crate through a relative path
(`../../synapt-core`), so a breaking change is felt immediately.

## Development setup

```bash
git clone https://github.com/aatishbagal/synapt-core.git
cd synapt-core
cargo build
cargo test
```

## Pull request guidelines

- Open an issue first for any type changes
- All tests must pass: `cargo test`, `cargo clippy -- -D warnings`
- No emojis in code, comments, or documentation
- Commit format: `type(scope): description`
- No `unwrap()` or `expect()` in non-test code
