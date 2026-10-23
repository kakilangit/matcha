//! Asks whether a newer Matcha release exists.

use std::time::Duration;

use serde::Deserialize;
use ureq::tls::{RootCerts, TlsConfig};

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/kakilangit/matcha/releases/latest";
pub const LATEST_RELEASE_PAGE: &str = "https://github.com/kakilangit/matcha/releases/latest";
const TIMEOUT: Duration = Duration::from_secs(10);

/// Where the latest release comes from.
pub trait ReleaseSource: Send + Sync {
    /// The latest release's tag, e.g. `v0.1.2`. Blocks on the network.
    ///
    /// # Errors
    ///
    /// A message for the log when the release can't be fetched or parsed.
    fn latest_tag(&self) -> Result<String, String>;
}

/// The latest release published on GitHub.
pub struct GitHubReleases {
    agent: ureq::Agent,
}

impl GitHubReleases {
    #[must_use]
    pub fn new(current: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .user_agent(format!("Matcha/{current}"))
            // The OS trust store, so proxies with their own CA work.
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .into();
        Self { agent }
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

impl ReleaseSource for GitHubReleases {
    fn latest_tag(&self) -> Result<String, String> {
        let body = self
            .agent
            .get(LATEST_RELEASE_API)
            .header("Accept", "application/vnd.github+json")
            .call()
            .and_then(|mut response| response.body_mut().read_to_string())
            .map_err(|err| err.to_string())?;
        let release: Release = serde_json::from_str(&body).map_err(|err| err.to_string())?;
        Ok(release.tag_name)
    }
}

/// Returns the latest release's version if it is newer than `current`. Blocks on the network.
///
/// # Errors
///
/// The source's message when it can't tell the latest release.
pub fn newer_version(source: &dyn ReleaseSource, current: &str) -> Result<Option<String>, String> {
    let tag = source.latest_tag()?;
    let latest = tag.trim_start_matches('v');
    Ok(is_newer(latest, current).then(|| latest.to_owned()))
}

fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.').map(|part| part.parse().ok());
    let parsed = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(parsed)
}

fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(latest), Some(current)) if latest > current)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource(Result<&'static str, &'static str>);

    impl ReleaseSource for FakeSource {
        fn latest_tag(&self) -> Result<String, String> {
            self.0.map(str::to_owned).map_err(str::to_owned)
        }
    }

    #[test]
    fn reports_newer_release_without_v_prefix() {
        let found = newer_version(&FakeSource(Ok("v0.2.0")), "0.1.2").unwrap();
        assert_eq!(found.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn same_or_older_release_is_up_to_date() {
        assert_eq!(newer_version(&FakeSource(Ok("v0.1.2")), "0.1.2"), Ok(None));
        assert_eq!(newer_version(&FakeSource(Ok("0.1.1")), "0.1.2"), Ok(None));
    }

    #[test]
    fn passes_source_errors_through() {
        let err = newer_version(&FakeSource(Err("offline")), "0.1.2").unwrap_err();
        assert_eq!(err, "offline");
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.2", "0.1.2"));
        assert!(!is_newer("0.1.1", "0.1.2"));
    }

    #[test]
    fn ignores_unparsable_versions() {
        assert!(!is_newer("nightly", "0.1.2"));
        assert!(!is_newer("1.0.0-beta.1", "0.1.2"));
        assert!(!is_newer("1.0", "0.1.2"));
    }
}
