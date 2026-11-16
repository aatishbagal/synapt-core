# SynaptCore releases

SynaptCore is compiled into Synapt and SynaptClip. It is never downloaded as
executable code independently of an app update. Both apps resolve the latest
published stable release through GitHub's API and read `core.json` from that
exact tag. The release asset is a fallback when the API or tagged file cannot
be reached. Installation continues through their signed Tauri app updater.

The 0.2.0 release adds Wayland portal hotkeys and core update reporting. The
0.2.1 patch fixes checks that stall on release-download redirects and exposes
connection failure causes. Synapt and SynaptClip 0.5.4 include 0.2.1. The
0.2.2 release drops the upper bound on supported app versions. Their version
numbers are independent.

## Version policy

- Bump the patch version for compatible bug fixes.
- Bump the minor version for significant features or incompatible changes while
  core is below 1.0. After 1.0, use a major bump for incompatible changes.
- Keep `Cargo.toml` and `core.json` versions equal.
- Each side owns one bound. `core.json` declares only the minimum app version
  each app needs (`>=X.Y.Z`); raise it only when a core release depends on newer
  app behaviour. Never add an upper bound: core cannot know what a future app
  release will need, and the app's own requirement already expresses that.
- Apps depend on core with a caret range (`^0.2.2`), which accepts every
  compatible patch. A breaking core change ships as a minor bump below 1.0, and
  apps opt in by moving their requirement (for example to `^0.3.0`).
- App releases still build against one exact core: the version resolved in the
  app's `Cargo.lock`. Commit the refreshed lockfile to adopt a new core patch.
  Existing installations receive it with a new app release.

CI verifies release metadata, shared features, and compatibility behavior.
Merging core to main publishes a versioned release with `core.json`. Code changes
under an already published version fail the release job instead of replacing
that version. Documentation-only merges do not create another release.

## Coordinated rollout

Development app CI checks out core/dev and refreshes only the `synapt-core`
entry in the app lockfile, so a new core patch on dev never breaks app CI. App
release jobs read the exact core version from the app's `Cargo.lock`, check that
it satisfies the manifest's caret requirement, and wait up to 20 minutes for that
core release to be published. They then check out its tag, not a moving branch. This allows
all three main merges to start together without packaging the previous core.
If core validation fails or publication times out, the app release stops.

A newer core may be published before a matching app is available. Installed
apps show the newer core as available and explain that it arrives through an
app update. An offline, missing, or malformed feed is shown as a failed check,
never as proof that the installation is up to date. App update checks remain
independent, so a core-feed failure cannot hide an available app update.

## Security rollout: core 0.2.3

1. Commit and push core/dev first, then wait for its verification workflow.
2. Commit and push the app dev branches with both core requirements set to
   `^0.2.3` and the core lock entry at `0.2.3`. The core lockfile does not control
   app dependencies; each app lockfile must also contain the patched TLS crates.
3. Merge the core PR to main and wait for stable release `v0.2.3` with its
   `core.json` asset. Keep the minimum supported app versions at `>=0.5.4`: this
   patch does not require new app behavior.
4. Merge and release Synapt 0.6.2 and SynaptClip 0.6.1. These two app releases
   can run in either order once core is published. Their release workflows
   select `v0.2.3` from the app lockfile and stop if it is unavailable.

Installed apps receive the patched core through their signed app update. A
newer core feed is informational and must not block the app updater; it is
normal for it to appear before both app releases are available.
