//! Asks GitHub whether a newer Matcha release exists.

use std::time::Duration;

use serde::Deserialize;
use ureq::tls::{RootCerts, TlsConfig};

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/kakilangit/matcha/releases/latest";
pub const LATEST_RELEASE_PAGE: &str = "https://github.com/kakilangit/matcha/releases/latest";
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// Returns the latest release's version if it is newer than `current`. Blocks on the network.
pub fn newer_version(current: &str) -> Result<Option<String>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
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
    let body = agent
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .map_err(|err| err.to_string())?;
    let release: Release = serde_json::from_str(&body).map_err(|err| err.to_string())?;
    let latest = release.tag_name.trim_start_matches('v');
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
    use super::is_newer;

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
