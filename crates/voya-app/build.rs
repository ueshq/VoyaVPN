//! Extracts the startup failure dialog's strings from the renderer's locale
//! files, so the binary embeds those few keys instead of every locale whole.

use std::{env, error::Error, fs, path::PathBuf};

const LOCALES: [&str; 3] = ["en", "zh-Hans", "zh-Hant"];
const NAMESPACE: &str = "startupFailure";

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let locales_dir = manifest_dir.join("../../packages/i18n/src/locales");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    for locale in LOCALES {
        let source = locales_dir.join(format!("{locale}.json"));
        println!("cargo:rerun-if-changed={}", source.display());

        let strings: serde_json::Value = serde_json::from_str(&fs::read_to_string(&source)?)?;
        let subtree = strings
            .get(NAMESPACE)
            .ok_or_else(|| format!("{} has no `{NAMESPACE}` object", source.display()))?;
        let extracted = serde_json::json!({ NAMESPACE: subtree });
        fs::write(
            out_dir.join(format!("startup-failure.{locale}.json")),
            serde_json::to_string(&extracted)?,
        )?;
    }
    Ok(())
}
