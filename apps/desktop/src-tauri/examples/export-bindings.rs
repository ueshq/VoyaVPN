use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../src/ipc/bindings.ts"));

    voyavpn_lib::export_bindings(output)
        .map_err(|error| format!("failed to export TypeScript IPC bindings: {error}").into())
}
