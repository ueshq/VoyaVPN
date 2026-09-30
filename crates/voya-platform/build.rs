use std::env;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_macos_native_bridges();
    }
}

fn build_macos_native_bridges() {
    let source = "native/macos_packet_tunnel_bridge.m";

    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed=native/macos_tunnel_wait.h");
    println!("cargo:rerun-if-changed=native/macos_window_chrome.m");
    println!("cargo:rerun-if-changed=native/macos_login_item.m");
    println!("cargo:rerun-if-changed=native/macos_login_item.h");

    cc::Build::new()
        .file(source)
        .file("native/macos_window_chrome.m")
        .file("native/macos_login_item.m")
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .flag("-mmacosx-version-min=10.15")
        .compile("voya_macos_packet_tunnel_bridge");

    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=NetworkExtension");
    println!("cargo:rustc-link-lib=framework=SystemExtensions");
    println!("cargo:rustc-link-lib=framework=ServiceManagement");
}
