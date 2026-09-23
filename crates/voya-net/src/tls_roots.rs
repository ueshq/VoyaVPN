//! The roots every HTTPS client in this crate trusts: the bundled webpki roots
//! plus the ones the operating system trusts, so self-hosted servers behind a
//! private CA the OS already trusts work too.
//!
//! reqwest 0.13 has no `rustls-tls-native-roots` / `rustls-tls-webpki-roots`
//! features and rebuilds its default verifier per client. Both costs are
//! avoided here: one `rustls::ClientConfig` is built per trust policy and
//! handed to every client through `tls_backend_preconfigured`, so the OS store
//! is read once and reqwest never constructs its own verifier.

use std::sync::{Arc, LazyLock};

use rustls::pki_types::CertificateDer;
use rustls::RootCertStore;

/// Installs ring as the process-default crypto provider once.
///
/// reqwest's `rustls-no-provider` feature deliberately ships none; the
/// preconfigured `ClientConfig` below needs one, and `ClientConfig::builder()`
/// only works once a default is installed.
static CRYPTO_PROVIDER: LazyLock<()> = LazyLock::new(|| {
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        // Another crate already installed a provider; that one is fine too.
        tracing::debug!("a rustls crypto provider was already installed");
    }
});

/// The operating system's trusted roots, read once per process.
///
/// reqwest used to read the OS trust store again for every client it built. On
/// macOS that walks the keychains' trust settings, over 100 ms per client, and
/// the app builds clients at startup, for every Clash API client and for every
/// node a speedtest probes. A root added to the OS while the app runs is
/// trusted from the next launch.
static NATIVE_ROOTS: LazyLock<Vec<CertificateDer<'static>>> = LazyLock::new(load_native_roots);

/// Empty on a phone: the bundled webpki roots are the whole trust policy there.
///
/// See `Cargo.toml` — neither mobile OS exposes its trust store to a library,
/// and the private-CA case these roots exist for is a self-hosted desktop node.
#[cfg(any(target_os = "ios", target_os = "android"))]
fn load_native_roots() -> Vec<CertificateDer<'static>> {
    Vec::new()
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn load_native_roots() -> Vec<CertificateDer<'static>> {
    let loaded = rustls_native_certs::load_native_certs();
    for error in &loaded.errors {
        tracing::debug!(%error, "part of the OS trust store could not be read");
    }
    parseable_roots(loaded.certs)
}

/// A bare rustls store rejects a whole config build on one root it cannot
/// parse. Native stores do carry ancient or malformed roots, so they are
/// dropped here the same way reqwest's own OS-store loading drops them.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn parseable_roots(roots: Vec<CertificateDer<'static>>) -> Vec<CertificateDer<'static>> {
    let mut parseable = RootCertStore::empty();
    roots
        .into_iter()
        .filter(|root| parseable.add(root.clone()).is_ok())
        .collect()
}

/// The bundled webpki roots, plus (on desktop) the OS roots read above.
fn root_store(with_native: bool) -> RootCertStore {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if with_native {
        for root in NATIVE_ROOTS.iter() {
            // Already validated by `parseable_roots` on desktop; a bare mobile
            // list is empty so this never runs there.
            let _ = roots.add(root.clone());
        }
    }
    roots
}

fn client_config(with_native: bool) -> Arc<rustls::ClientConfig> {
    LazyLock::force(&CRYPTO_PROVIDER);
    Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store(with_native))
            .with_no_client_auth(),
    )
}

/// Bundled webpki roots only — used by the Clash REST transport, which only
/// ever talks plain HTTP on loopback and must not wait on the OS store.
static WEBPKI_ONLY_CONFIG: LazyLock<Arc<rustls::ClientConfig>> =
    LazyLock::new(|| client_config(false));

/// Bundled webpki roots plus the OS trust store (desktop) / webpki only (mobile).
static FULL_CONFIG: LazyLock<Arc<rustls::ClientConfig>> = LazyLock::new(|| client_config(true));

/// Reads the OS roots now rather than when the first client is built.
pub fn preload_tls_roots() {
    LazyLock::force(&NATIVE_ROOTS);
    LazyLock::force(&FULL_CONFIG);
}

/// `reqwest::Client::builder()` trusting the bundled webpki roots alone.
pub(crate) fn built_in_roots_only(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder.tls_backend_preconfigured(rustls::ClientConfig::clone(&WEBPKI_ONLY_CONFIG))
}

/// `reqwest::Client::builder()` with this crate's full trust policy and the OS
/// roots read once instead of per client.
pub(crate) fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder().tls_backend_preconfigured(rustls::ClientConfig::clone(&FULL_CONFIG))
}

// The OS trust store is the subject here, and mobile has none of its own.
#[cfg(all(test, not(any(target_os = "ios", target_os = "android"))))]
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
        // The full policy must be buildable with those roots (plus webpki).
        assert!(client_builder().build().is_ok());
    }

    #[test]
    fn the_webpki_only_policy_never_waits_on_the_os_store() {
        assert!(built_in_roots_only(reqwest::Client::builder())
            .build()
            .is_ok());
    }
}
