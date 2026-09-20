//! Generates the Swift and Kotlin bindings for this crate.
//!
//! uniffi reads the *built* library rather than the source, so the bindings
//! always describe the symbols the artifact beside them really exports. The
//! two `scripts/native/mobile/build-rust-*.mjs` scripts run this after the
//! build for exactly that reason.
//!
//! A bin rather than a build step: nothing in a normal `cargo build` needs it,
//! and a phone build is the only thing that does.
fn main() {
    uniffi::uniffi_bindgen_main();
}
