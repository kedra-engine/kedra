//! Bounded public HTTPS requests. Redirects are checked before contacting each host.
use super::{
    Config,
    release::{self, Kind, Release},
};
use std::time::{Duration, Instant};
use url::Url;

pub(super) fn allowed(url: &Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some(
                "api.github.com"
                    | "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn get_with_policy(
    url: &Url,
    timeout: Duration,
    policy: impl Fn(&Url) -> bool,
) -> Result<ureq::http::Response<ureq::Body>, String> {
    let started = Instant::now();
    let mut url = url.clone();
    for _ in 0..=5 {
        if !policy(&url) {
            return Err("Update URL origin is not allowed".into());
        }
        let remaining = timeout
            .checked_sub(started.elapsed())
            .ok_or("Update request timed out")?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .timeout_global(Some(remaining))
            .timeout_connect(Some(remaining.min(Duration::from_secs(10))))
            .build()
            .into();
        let response = agent
            .get(url.as_str())
            .header("User-Agent", "Kedra update client")
            .header(
                "Accept",
                if url.host_str() == Some("api.github.com") {
                    "application/vnd.github+json"
                } else {
                    "application/octet-stream"
                },
            )
            .call()
            .map_err(|e| format!("Update request failed: {e}"))?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get("Location")
                .and_then(|h| h.to_str().ok())
                .ok_or("Update redirect has no location")?;
            url = url.join(location).map_err(|e| e.to_string())?;
            continue;
        }
        if status.as_u16() == 403 || status.as_u16() == 429 {
            return Err(
                "GitHub update check unavailable or rate limited; try on the next launch".into(),
            );
        }
        if status.as_u16() != 200 {
            return Err(format!("Update server returned {status}"));
        }
        return Ok(response);
    }
    Err("Update request exceeded five redirects".into())
}

pub(super) fn get(
    url: &Url,
    timeout: Duration,
) -> Result<ureq::http::Response<ureq::Body>, String> {
    get_with_policy(url, timeout, allowed)
}

/// Performs one anonymous startup discovery, bounded to 30 seconds and fixed JSON
/// sizes. Returns no candidate or validated metadata; network, quota and policy
/// failures return errors. No scheduler, retries, ambient credentials or exit.
pub fn discover(config: &Config, kind: Kind) -> Result<Option<Release>, String> {
    let started = Instant::now();
    let timeout = Duration::from_secs(30);
    let url = Url::parse(&format!(
        "https://api.github.com/repos/{}/releases?per_page=5",
        config.repository
    ))
    .map_err(|e| e.to_string())?;
    let bytes = get(&url, timeout)?
        .body_mut()
        .with_config()
        .limit(release::MAX_LISTING as u64)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    let Some(candidate) = release::candidate(config, &bytes)? else {
        return Ok(None);
    };
    let inventory = release::asset(config, &candidate, "artifact-inventory.json")?;
    if inventory.size > release::MAX_INVENTORY as u64 {
        return Err("Release inventory is too large".into());
    }
    let remaining = timeout
        .checked_sub(started.elapsed())
        .ok_or("Update discovery timed out")?;
    let bytes = get(&inventory.browser_download_url, remaining)?
        .body_mut()
        .with_config()
        .limit(release::MAX_INVENTORY as u64)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    release::validate(config, &candidate, &bytes, kind).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn only_exact_public_https_origins_are_allowed() -> TestResult {
        for origin in [
            "https://github.com",
            "https://api.github.com",
            "https://release-assets.githubusercontent.com",
        ] {
            assert!(allowed(&Url::parse(origin)?), "{origin}");
        }
        for origin in [
            "http://github.com",
            "https://github.com.evil.example",
            "https://user:secret@github.com",
            "https://github.com:8443",
            "https://127.0.0.1",
        ] {
            assert!(!allowed(&Url::parse(origin)?), "{origin}");
        }
        Ok(())
    }
    #[test]
    fn real_redirect_is_rejected_before_contacting_unexpected_origin() -> TestResult {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let thread = std::thread::spawn(move || -> std::io::Result<()> {
            let (mut connection, _) = listener.accept()?;
            let mut buffer = [0; 4096];
            let received = connection.read(&mut buffer)?;
            assert!(
                received > 0,
                "HTTP fixture requires a request before redirecting"
            );
            connection.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        });
        let url = Url::parse(&format!("http://{address}/"))?;
        let result = get_with_policy(&url, Duration::from_secs(2), |u| u == &url);
        assert!(result.is_err_and(|e| e.contains("origin")));
        thread.join().map_err(|_| "HTTP fixture panicked")??;
        Ok(())
    }
}
