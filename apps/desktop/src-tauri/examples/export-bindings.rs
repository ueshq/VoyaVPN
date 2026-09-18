use std::path::PathBuf;

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../src/ipc/bindings.ts"));

    voyavpn_lib::export_bindings(output).expect("failed to export TypeScript IPC bindings");
}
