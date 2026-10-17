# SynaptCore releases

SynaptCore is compiled into Synapt and SynaptClip. It is never downloaded as
executable code independently of an app update. Both apps resolve the latest
published stable release through GitHub's API and read `core.json` from that
exact tag. The release asset is a fallback when the API or tagged file cannot
be reached. Installation continues through their signed Tauri app updater.

The 0.2.0 release adds Wayland portal hotkeys and core update reporting. The
0.2.1 patch fixes checks that stall on release-download redirects and exposes
connection failure causes. Synapt and SynaptClip 0.5.4 include 0.2.1. Their
version numbers are independent.

## Version policy

- Bump the patch version for compatible bug fixes.
- Bump the minor version for significant features or incompatible changes while
  core is below 1.0. After 1.0, use a major bump for incompatible changes.
- Keep `Cargo.toml` and `core.json` versions equal. Update the supported app
  ranges in `core.json` when compatibility changes.
- Update each app's exact `synapt-core` dependency and lockfile to adopt a core
  release. Existing installations receive it with a new app release.

CI verifies release metadata, shared features, and compatibility behavior.
Merging core to main publishes a versioned release with `core.json`. Code changes
under an already published version fail the release job instead of replacing
that version. Documentation-only merges do not create another release.

## Coordinated rollout

Development app CI checks out core/dev. App release jobs read the exact core
version from their Cargo manifest and wait up to 20 minutes for that core release
to be published. They then check out its tag, not a moving branch. This allows
all three main merges to start together without packaging the previous core.
If core validation fails or publication times out, the app release stops.

A newer core may be published before a matching app is available. Installed
apps show the newer core as available and explain that it arrives through an
app update. An offline, missing, or malformed feed is shown as a failed check,
never as proof that the installation is up to date. App update checks remain
independent, so a core-feed failure cannot hide an available app update.
