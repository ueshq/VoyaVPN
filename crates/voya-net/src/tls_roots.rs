//! The roots every HTTPS client in this crate trusts: the bundled webpki roots
//! plus the ones the operating system trusts, so self-hosted servers behind a
//! private CA the OS already trusts work too.

use std::sync::LazyLock;

/// The operating system's trusted roots, read once per process.
///
/// reqwest reads the OS trust store again for every client it builds. On macOS
/// that walks the keychains' trust settings, over 100 ms per client, and the
/// app builds clients at startup, for every Clash API client and for every
/// node a speedtest probes. A root added to the OS while the app runs is
/// trusted from the next launch.
static NATIVE_ROOTS: LazyLock<Vec<reqwest::Certificate>> = LazyLock::new(load_native_roots);

fn load_native_roots() -> Vec<reqwest::Certificate> {
    let loaded = rustls_native_certs::load_native_certs();
    for error in &loaded.errors {
        tracing::debug!(%error, "part of the OS trust store could not be read");
    }
    // reqwest fails a whole client build on one root it cannot parse, while its
    // own OS-store loading skips such roots. Native stores do carry ancient or
    // malformed roots, so they are dropped here the same way.
    let mut parseable = rustls::RootCertStore::empty();
    loaded
        .certs
        .into_iter()
        .filter(|root| parseable.add(root.clone()).is_ok())
        .filter_map(|root| reqwest::Certificate::from_der(root.as_ref()).ok())
        .collect()
}

/// Reads the OS roots now rather than when the first client is built.
pub fn preload_tls_roots() {
    LazyLock::force(&NATIVE_ROOTS);
}

/// `reqwest::Client::builder()` with this crate's trust policy and the OS roots
/// read once instead of per client.
pub(crate) fn client_builder() -> reqwest::ClientBuilder {
    NATIVE_ROOTS.iter().cloned().fold(
        reqwest::Client::builder()
            .tls_built_in_webpki_certs(true)
            .tls_built_in_native_certs(false),
        reqwest::ClientBuilder::add_root_certificate,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cached_os_root_is_accepted_by_a_client_build() {
        // One unparsable root would fail every client this crate builds.
        assert!(client_builder().build().is_ok());
        assert!(client_builder().build().is_ok());
    }
}
