//! Core is bundled into each app. This feed reports availability only; signed app
//! updates remain the sole installation path for executable code.

use std::collections::BTreeMap;
use std::time::Duration;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

/// Version of the core compiled into the running application.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_URL: &str =
    "https://github.com/aatishbagal/synapt-core/releases/latest/download/core.json";
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

/// Check the stable core release feed with a bounded request and response size.
pub async fn check_for_update(app: &str, app_version: &str, required: &str) -> CoreUpdateStatus {
    check_at(
        app,
        app_version,
        required,
        RELEASE_URL,
        Duration::from_secs(15),
    )
    .await
}

async fn check_at(
    app: &str,
    app_version: &str,
    required: &str,
    url: &str,
    timeout: Duration,
) -> CoreUpdateStatus {
    let mut status = bundled_status(app, app_version, required);
    if let Err(error) = fetch_release(&mut status, app, app_version, url, timeout).await {
        status.check_error = Some(error);
    }
    status
}

async fn fetch_release(
    status: &mut CoreUpdateStatus,
    app: &str,
    app_version: &str,
    url: &str,
    timeout: Duration,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(format!("{app}/{app_version}"))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(url)
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > 64 * 1024 {
            return Err("SynaptCore release information is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    let release: Release = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    apply_release(status, &release, app, app_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn respond_once(
        code: u16,
        body: String,
        delay: Duration,
    ) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 4096];
            socket.read(&mut request).await.unwrap();
            tokio::time::sleep(delay).await;
            let response = format!(
                "HTTP/1.1 {code} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });
        (format!("http://{address}/core.json"), task)
    }

    #[tokio::test]
    async fn http_check_reads_published_core_metadata() {
        let body = BUNDLED_MANIFEST.replace("0.2.0", "0.2.1");
        let (url, server) = respond_once(200, body, Duration::ZERO).await;
        let status = check_at("synapt", "0.5.4", "=0.2.0", &url, Duration::from_secs(2)).await;
        server.await.unwrap();
        assert_eq!(status.latest_version.as_deref(), Some("0.2.1"));
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
            let status = check_at("synapt", "0.5.3", "=0.2.0", &url, Duration::from_secs(2)).await;
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
            "=0.2.0",
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
            let status = bundled_status(app, "0.5.4", "=0.2.0");
            assert!(status.compatible, "{:?}", status.compatibility_warning);
            assert!(status.latest_version.is_none());
        }
    }

    #[test]
    fn rejects_old_core_or_incompatible_app_without_network() {
        assert!(!bundled_status("synapt", "0.5.4", ">=0.3.0").compatible);
        assert!(!bundled_status("synapt-clip", "0.5.3", "=0.2.0").compatible);
        assert!(!bundled_status("unknown", "0.5.4", "=0.2.0").compatible);
    }

    #[test]
    fn newer_patch_is_available_even_if_current_app_is_compatible() {
        let mut status = bundled_status("synapt", "0.5.4", "=0.2.0");
        apply_release(&mut status, &release("0.2.1", ">=0.5.4"), "synapt", "0.5.4").unwrap();
        assert!(status.update_available);
        assert!(status.compatible);
        assert_eq!(status.latest_app_compatible, Some(true));
    }

    #[test]
    fn newer_core_can_require_a_newer_app() {
        let mut status = bundled_status("synapt-clip", "0.5.4", "=0.2.0");
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
        for version in ["0.2.0", "0.1.0"] {
            let mut status = bundled_status("synapt", "0.5.4", "=0.2.0");
            apply_release(&mut status, &release(version, ">=0.5.4"), "synapt", "0.5.4").unwrap();
            assert!(!status.update_available);
        }
    }

    #[test]
    fn malformed_or_prerelease_feed_never_claims_up_to_date() {
        for version in ["invalid", "0.3.0-beta.1"] {
            let mut status = bundled_status("synapt", "0.5.4", "=0.2.0");
            assert!(
                apply_release(&mut status, &release(version, ">=0.5.4"), "synapt", "0.5.4")
                    .is_err()
            );
            assert!(status.latest_version.is_none());
        }
        let mut status = bundled_status("synapt", "0.5.4", "=0.2.0");
        let mut invalid = release("0.3.0", ">=0.5.4");
        invalid.schema_version = 2;
        assert!(apply_release(&mut status, &invalid, "synapt", "0.5.4").is_err());
        invalid.schema_version = 1;
        invalid.apps.remove("synapt-clip");
        assert!(apply_release(&mut status, &invalid, "synapt", "0.5.4").is_err());
        assert!(status.latest_version.is_none());
    }
}
