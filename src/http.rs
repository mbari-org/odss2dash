//! The HTTP client shared by the outgoing requests to ODSS and to TethysDash.
//!
//! A single `reqwest::Client` is used for all of them so that connections
//! (and their TLS handshakes) are reused across requests.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Timeout for the complete request/response cycle of an outgoing request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// The shared client. Must be called from within the runtime (see [`crate::rt`]).
pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .use_preconfigured_tls(tls_config())
            .build()
            .expect("HTTP client should be created")
    })
}

/// Timeout for the healthcheck request, comfortably inside the one the container
/// healthcheck itself is given.
const HEALTHCHECK_TIMEOUT: Duration = Duration::from_secs(3);

/// The body of a successful response from the given health endpoint, or why there was
/// none. For the `healthcheck` command; nothing else asks the service about itself.
pub async fn get_health(url: &str) -> Result<String, String> {
    let response = client()
        .get(url)
        .timeout(HEALTHCHECK_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("{e}"))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status.is_success() {
        Ok(body)
    } else {
        Err(format!("{status}: {body}"))
    }
}

/// rustls with the ring provider and the bundled webpki roots, so the service
/// does not depend on a CA store being present in the (distroless) image.
fn tls_config() -> rustls::ClientConfig {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("TLS protocol versions should be supported")
        .with_root_certificates(roots)
        .with_no_client_auth()
}
