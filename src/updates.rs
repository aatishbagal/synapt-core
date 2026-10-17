//! Core is bundled into each app. This feed reports availability only; signed app
//! updates remain the sole installation path for executable code.

use std::collections::BTreeMap;
use std::error::Error;
use std::time::Duration;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

/// Version of the core compiled into the running application.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_URL: &str =
    "https://github.com/aatishbagal/synapt-core/releases/latest/download/core.json";
const RELEASE_API: &str = "https://api.github.com/repos/aatishbagal/synapt-core/releases/latest";
const TAG_CONTENT_BASE: &str = "https://raw.githubusercontent.com/aatishbagal/synapt-core/";
const BUNDLED_MANIFEST: &str = include_str!("../core.json");

#[derive(Deserialize)]
struct Release {
    schema_version: u32,
    version: String,
    apps: BTreeMap<String, String>,
}

impl Release {
    fn validate(&self) -> Result<Version, String> {
        if self.schema_version != 1 {
            return Err("Unsupported SynaptCore release information format".into());
        }
        let version = Version::parse(&self.version).map_err(|e| e.to_string())?;
        if !version.pre.is_empty() {
            return Err("The stable core feed returned a prerelease".into());
        }
        for app in ["synapt", "synapt-clip"] {
            let requirement = self
                .apps
                .get(app)
                .ok_or("Missing app compatibility information")?;
            VersionReq::parse(requirement).map_err(|e| e.to_string())?;
        }
        Ok(version)
    }

    fn supports(&self, app: &str, version: &str) -> Result<bool, String> {
        let requirement = self.apps.get(app).ok_or("Unknown Synapt application")?;
        Ok(VersionReq::parse(requirement)
            .map_err(|e| e.to_string())?
            .matches(&Version::parse(version).map_err(|e| e.to_string())?))
    }
}

/// Core version and compatibility state, independent of app-update availability.
#[derive(Clone, Debug, Serialize)]
pub struct CoreUpdateStatus {
    /// Core version compiled into this app.
    pub bundled_version: String,
    /// Core requirement declared in this app's Cargo manifest.
    pub required_version: String,
    /// Latest published stable core version, when successfully checked.
    pub latest_version: Option<String>,
    /// Whether the bundled core and this app satisfy both compatibility rules.
    pub compatible: bool,
    /// Whether the published core is newer than the bundled copy.
    pub update_available: bool,
    /// Whether the latest core supports the currently running app version.
    pub latest_app_compatible: Option<bool>,
    /// Explanation when the installed app and core are incompatible.
    pub compatibility_warning: Option<String>,
    /// A failed remote check must never be represented as up to date.
    pub check_error: Option<String>,
}

/// Inspect bundled compatibility without making a network request.
pub fn bundled_status(app: &str, app_version: &str, required: &str) -> CoreUpdateStatus {
    let mut status = CoreUpdateStatus {
        bundled_version: VERSION.into(),
        required_version: required.into(),
        latest_version: None,
        compatible: false,
        update_available: false,
        latest_app_compatible: None,
        compatibility_warning: None,
        check_error: None,
    };
    let result = (|| {
        let manifest: Release =
            serde_json::from_str(BUNDLED_MANIFEST).map_err(|e| e.to_string())?;
        let bundled = manifest.validate()?;
        if bundled.to_string() != VERSION {
            return Err("Bundled core release information does not match its version".into());
        }
        let requirement = VersionReq::parse(required).map_err(|e| e.to_string())?;
        if !requirement.matches(&bundled) || !manifest.supports(app, app_version)? {
            return Err(format!(
                "This app and SynaptCore {VERSION} are out of sync. Install a compatible app update."
            ));
        }
        Ok(())
    })();
    match result {
        Ok(()) => status.compatible = true,
        Err(error) => status.compatibility_warning = Some(error),
    }
    status
}

fn apply_release(
    status: &mut CoreUpdateStatus,
    release: &Release,
    app: &str,
    app_version: &str,
) -> Result<(), String> {
    let latest = release.validate()?;
    let bundled = Version::parse(&status.bundled_version).map_err(|e| e.to_string())?;
    let supports_app = release.supports(app, app_version)?;
    status.update_available = latest > bundled;
    status.latest_app_compatible = Some(supports_app);
    status.latest_version = Some(release.version.clone());
    Ok(())
}

#[derive(Deserialize)]
struct PublishedRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
}

impl PublishedRelease {
    fn version(&self) -> Result<Version, String> {
        if self.draft
            || self.prerelease
            || !self.assets.iter().any(|asset| asset.name == "core.json")
        {
            return Err("No published stable SynaptCore metadata is available".into());
        }
        let version = self
            .tag_name
            .strip_prefix('v')
            .ok_or("Invalid core release tag")?;
        let version = Version::parse(version).map_err(|e| e.to_string())?;
        if !version.pre.is_empty() || self.tag_name != format!("v{version}") {
            return Err("Invalid stable core release tag".into());
        }
        Ok(version)
    }
}

struct ReleaseSources<'a> {
    api: &'a str,
    tags: &'a str,
    asset: &'a str,
}

/// Check published core metadata, with the release asset as an independent fallback.
pub async fn check_for_update(app: &str, app_version: &str, required: &str) -> CoreUpdateStatus {
    check_sources(
        app,
        app_version,
        required,
        ReleaseSources {
            api: RELEASE_API,
            tags: TAG_CONTENT_BASE,
            asset: RELEASE_URL,
        },
        Duration::from_secs(15),
    )
    .await
}

async fn check_sources(
    app: &str,
    app_version: &str,
    required: &str,
    sources: ReleaseSources<'_>,
    timeout: Duration,
) -> CoreUpdateStatus {
    let mut status = bundled_status(app, app_version, required);
    let result = async {
        let client = build_client(app, app_version, timeout)?;
        // Release-asset redirects can stall even when GitHub itself is reachable.
        // Read only the tag selected by a published stable release, never main.
        let release = match fetch_published_release(&client, sources.api, sources.tags).await {
            Ok(release) => release,
            Err(primary_error) => {
                fetch_asset_release(&client, sources.asset)
                    .await
                    .map_err(|fallback_error| {
                        format!(
                            "Release lookup: {primary_error}. Download fallback: {fallback_error}"
                        )
                    })?
            }
        };
        apply_release(&mut status, &release, app, app_version)
    }
    .await;
    if let Err(error) = result {
        status.check_error = Some(error);
    }
    status
}

fn network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        return "The update request timed out".into();
    }
    // Omit signed redirect URLs while retaining DNS, TLS, and connection causes.
    let error = error.without_url();
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let detail = cause.to_string();
        if !message.contains(&detail) {
            message.push_str(": ");
            message.push_str(&detail);
        }
        source = cause.source();
    }
    message
}

fn build_client(
    app: &str,
    app_version: &str,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(timeout.min(Duration::from_secs(5)))
        .timeout(timeout)
        .user_agent(format!("{app}/{app_version}"))
        .build()
        .map_err(network_error)
}

async fn fetch_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .await
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if body.len() + chunk.len() > 64 * 1024 {
            return Err("SynaptCore release information is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn fetch_asset_release(client: &reqwest::Client, url: &str) -> Result<Release, String> {
    let body = fetch_bytes(client, url).await?;
    let release: Release = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    release.validate()?;
    Ok(release)
}

async fn fetch_published_release(
    client: &reqwest::Client,
    api: &str,
    tag_base: &str,
) -> Result<Release, String> {
    let body = fetch_bytes(client, api).await?;
    let published: PublishedRelease = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let expected = published.version()?;
    let url = format!("{tag_base}{}/core.json", published.tag_name);
    let release = fetch_asset_release(client, &url).await?;
    if release.validate()? != expected {
        return Err("Core metadata does not match the published release tag".into());
    }
    Ok(release)
}

#[cfg(test)]
async fn check_at(
    app: &str,
    app_version: &str,
    required: &str,
    url: &str,
    timeout: Duration,
) -> CoreUpdateStatus {
    let mut status = bundled_status(app, app_version, required);
    let result = async {
        let client = build_client(app, app_version, timeout)?;
        let release = fetch_asset_release(&client, url).await?;
        apply_release(&mut status, &release, app, app_version)
    }
    .await;
    if let Err(error) = result {
        status.check_error = Some(error);
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn respond_once(
        code: u16,
        body: String,
        delay: Duration,
    ) -> (String, tokio::task::JoinHandle<()>) {
        respond_once_at(code, body, delay, None).await
    }

    async fn respond_once_at(
        code: u16,
        body: String,
        delay: Duration,
        path: Option<&'static str>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 4096];
            let read = socket.read(&mut request).await.unwrap();
            if let Some(path) = path {
                assert!(
                    String::from_utf8_lossy(&request[..read]).starts_with(&format!("GET {path} "))
                );
            }
            tokio::time::sleep(delay).await;
            let response = format!(
                "HTTP/1.1 {code} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });
        (format!("http://{address}/core.json"), task)
    }

    fn published_json() -> String {
        serde_json::json!({
            "tag_name": "v0.2.2", "draft": false, "prerelease": false,
            "assets": [{"name": "core.json"}]
        })
        .to_string()
    }

    #[tokio::test]
    async fn published_tag_works_without_reaching_release_asset_host() {
        let (api, api_server) = respond_once(200, published_json(), Duration::ZERO).await;
        let (raw, raw_server) = respond_once_at(
            200,
            BUNDLED_MANIFEST.replace("0.2.1", "0.2.2"),
            Duration::ZERO,
            Some("/v0.2.2/core.json"),
        )
        .await;
        let status = check_sources(
            "synapt",
            "0.5.4",
            "=0.2.1",
            ReleaseSources {
                api: &api,
                tags: raw.trim_end_matches("core.json"),
                asset: "http://127.0.0.1:1/unreachable",
            },
            Duration::from_secs(2),
        )
        .await;
        api_server.await.unwrap();
        raw_server.await.unwrap();
        assert_eq!(status.latest_version.as_deref(), Some("0.2.2"));
        assert!(status.update_available);
        assert!(status.check_error.is_none());
    }

    #[tokio::test]
    async fn api_rate_limits_or_failures_use_the_asset_fallback() {
        for code in [403, 429, 503] {
            let (api, api_server) = respond_once(code, "unavailable".into(), Duration::ZERO).await;
            let (asset, asset_server) =
                respond_once(200, BUNDLED_MANIFEST.into(), Duration::ZERO).await;
            let status = check_sources(
                "synapt-clip",
                "0.5.4",
                "=0.2.1",
                ReleaseSources {
                    api: &api,
                    tags: "http://127.0.0.1:1/",
                    asset: &asset,
                },
                Duration::from_secs(2),
            )
            .await;
            api_server.await.unwrap();
            asset_server.await.unwrap();
            assert_eq!(status.latest_version.as_deref(), Some(VERSION));
            assert!(status.check_error.is_none());
        }
    }

    #[tokio::test]
    async fn metadata_must_match_the_published_release_tag() {
        let (api, api_server) = respond_once(200, published_json(), Duration::ZERO).await;
        let (raw, raw_server) = respond_once(200, BUNDLED_MANIFEST.into(), Duration::ZERO).await;
        let (asset, asset_server) = respond_once(503, "unavailable".into(), Duration::ZERO).await;
        let status = check_sources(
            "synapt",
            "0.5.4",
            "=0.2.1",
            ReleaseSources {
                api: &api,
                tags: raw.trim_end_matches("core.json"),
                asset: &asset,
            },
            Duration::from_secs(2),
        )
        .await;
        api_server.await.unwrap();
        raw_server.await.unwrap();
        asset_server.await.unwrap();
        assert!(status.latest_version.is_none());
        let error = status.check_error.unwrap();
        assert!(error.contains("does not match"));
        assert!(error.contains("503"));
    }

    #[tokio::test]
    async fn stalled_release_lookup_still_allows_asset_fallback() {
        let (api, api_server) = respond_once(200, published_json(), Duration::from_secs(5)).await;
        let (asset, asset_server) =
            respond_once(200, BUNDLED_MANIFEST.into(), Duration::ZERO).await;
        let status = check_sources(
            "synapt",
            "0.5.4",
            "=0.2.1",
            ReleaseSources {
                api: &api,
                tags: "http://127.0.0.1:1/",
                asset: &asset,
            },
            Duration::from_millis(100),
        )
        .await;
        api_server.abort();
        asset_server.await.unwrap();
        assert!(status.check_error.is_none());
        assert_eq!(status.latest_version.as_deref(), Some(VERSION));
    }

    #[test]
    fn unpublished_or_nonversion_tags_cannot_select_metadata() {
        for (key, value) in [
            ("draft", serde_json::json!(true)),
            ("prerelease", serde_json::json!(true)),
            ("assets", serde_json::json!([])),
            ("tag_name", serde_json::json!("main")),
            ("tag_name", serde_json::json!("../../main")),
            ("tag_name", serde_json::json!("v0.3.0-beta.1")),
        ] {
            let mut json: serde_json::Value = serde_json::from_str(&published_json()).unwrap();
            json[key] = value;
            let release: PublishedRelease = serde_json::from_value(json).unwrap();
            assert!(release.version().is_err());
        }
    }

    #[tokio::test]
    async fn http_check_reads_published_core_metadata() {
        let body = BUNDLED_MANIFEST.replace("0.2.1", "0.2.2");
        let (url, server) = respond_once(200, body, Duration::ZERO).await;
        let status = check_at("synapt", "0.5.4", "=0.2.1", &url, Duration::from_secs(2)).await;
        server.await.unwrap();
        assert_eq!(status.latest_version.as_deref(), Some("0.2.2"));
        assert!(status.update_available);
        assert!(status.check_error.is_none());
    }

    #[tokio::test]
    async fn failed_or_oversized_feed_preserves_local_compatibility_warning() {
        for (code, body) in [
            (404, "missing".into()),
            (503, "unavailable".into()),
            (200, "not json".into()),
            (200, "x".repeat(65537)),
        ] {
            let (url, server) = respond_once(code, body, Duration::ZERO).await;
            let status = check_at("synapt", "0.5.3", "=0.2.1", &url, Duration::from_secs(2)).await;
            server.await.unwrap();
            assert!(status.check_error.is_some());
            assert!(status.latest_version.is_none());
            assert!(!status.compatible);
            assert!(status.compatibility_warning.is_some());
        }
    }

    #[tokio::test]
    async fn stalled_feed_times_out_instead_of_hanging_updates() {
        let (url, server) =
            respond_once(200, BUNDLED_MANIFEST.into(), Duration::from_secs(5)).await;
        let status = check_at(
            "synapt",
            "0.5.4",
            "=0.2.1",
            &url,
            Duration::from_millis(100),
        )
        .await;
        server.abort();
        assert!(status.check_error.is_some());
        assert!(status.latest_version.is_none());
        assert!(status.compatible);
    }

    fn release(version: &str, app_requirement: &str) -> Release {
        Release {
            schema_version: 1,
            version: version.into(),
            apps: ["synapt", "synapt-clip"]
                .into_iter()
                .map(|app| (app.into(), app_requirement.into()))
                .collect(),
        }
    }

    #[test]
    fn bundled_manifest_matches_version_and_both_apps() {
        for app in ["synapt", "synapt-clip"] {
            let status = bundled_status(app, "0.5.4", "=0.2.1");
            assert!(status.compatible, "{:?}", status.compatibility_warning);
            assert!(status.latest_version.is_none());
        }
    }

    #[test]
    fn rejects_old_core_or_incompatible_app_without_network() {
        assert!(!bundled_status("synapt", "0.5.4", ">=0.3.0").compatible);
        assert!(!bundled_status("synapt-clip", "0.5.3", "=0.2.1").compatible);
        assert!(!bundled_status("unknown", "0.5.4", "=0.2.1").compatible);
    }

    #[test]
    fn newer_patch_is_available_even_if_current_app_is_compatible() {
        let mut status = bundled_status("synapt", "0.5.4", "=0.2.1");
        apply_release(&mut status, &release("0.2.2", ">=0.5.4"), "synapt", "0.5.4").unwrap();
        assert!(status.update_available);
        assert!(status.compatible);
        assert_eq!(status.latest_app_compatible, Some(true));
    }

    #[test]
    fn newer_core_can_require_a_newer_app() {
        let mut status = bundled_status("synapt-clip", "0.5.4", "=0.2.1");
        apply_release(
            &mut status,
            &release("0.3.0", ">=0.6.0"),
            "synapt-clip",
            "0.5.4",
        )
        .unwrap();
        assert!(status.update_available);
        assert_eq!(status.latest_app_compatible, Some(false));
        assert!(status.compatible);
    }

    #[test]
    fn equal_or_older_release_does_not_offer_a_downgrade() {
        for version in ["0.2.1", "0.1.0"] {
            let mut status = bundled_status("synapt", "0.5.4", "=0.2.1");
            apply_release(&mut status, &release(version, ">=0.5.4"), "synapt", "0.5.4").unwrap();
            assert!(!status.update_available);
        }
    }

    #[test]
    fn malformed_or_prerelease_feed_never_claims_up_to_date() {
        for version in ["invalid", "0.3.0-beta.1"] {
            let mut status = bundled_status("synapt", "0.5.4", "=0.2.1");
            assert!(
                apply_release(&mut status, &release(version, ">=0.5.4"), "synapt", "0.5.4")
                    .is_err()
            );
            assert!(status.latest_version.is_none());
        }
        let mut status = bundled_status("synapt", "0.5.4", "=0.2.1");
        let mut invalid = release("0.3.0", ">=0.5.4");
        invalid.schema_version = 2;
        assert!(apply_release(&mut status, &invalid, "synapt", "0.5.4").is_err());
        invalid.schema_version = 1;
        invalid.apps.remove("synapt-clip");
        assert!(apply_release(&mut status, &invalid, "synapt", "0.5.4").is_err());
        assert!(status.latest_version.is_none());
    }
}
