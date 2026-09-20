//! The roots every HTTPS client in this crate trusts: the bundled webpki roots
//! plus the ones the operating system trusts, so self-hosted servers behind a
//! private CA the OS already trusts work too.

use std::sync::LazyLock;

use rustls::pki_types::CertificateDer;

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
    parseable_roots(loaded.certs)
}

/// reqwest fails a whole client build on one root it cannot parse, while its
/// own OS-store loading skips such roots. Native stores do carry ancient or
/// malformed roots, so they are dropped here the same way.
fn parseable_roots(roots: Vec<CertificateDer<'static>>) -> Vec<reqwest::Certificate> {
    let mut parseable = rustls::RootCertStore::empty();
    roots
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
        // A store with no roots at all (a bare CI container) would make the
        // build below pass without checking anything.
        if !rustls_native_certs::load_native_certs().certs.is_empty() {
            assert!(!NATIVE_ROOTS.is_empty(), "every OS root was dropped");
        }
        // One unparsable root would fail every client this crate builds.
        assert!(client_builder().build().is_ok());
    }

    #[test]
    fn an_unparseable_root_is_dropped_and_the_others_kept() {
        let os_roots = rustls_native_certs::load_native_certs().certs;
        let expected = parseable_roots(os_roots.clone()).len();
        let mut roots = os_roots;
        // A DER SEQUENCE holding one INTEGER: well-formed DER, not a certificate.
        roots.push(CertificateDer::from(vec![0x30, 0x03, 0x02, 0x01, 0x00]));

        let kept = parseable_roots(roots);

        assert_eq!(kept.len(), expected);
        assert!(kept
            .into_iter()
            .fold(
                reqwest::Client::builder(),
                reqwest::ClientBuilder::add_root_certificate
            )
            .build()
            .is_ok());
    }
}
