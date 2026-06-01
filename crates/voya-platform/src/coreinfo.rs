use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;
use voya_core::CoreType;

use crate::paths::AppPaths;

pub const GITHUB_URL: &str = "https://github.com";
pub const GITHUB_API_URL: &str = "https://api.github.com/repos";
pub const V2RAY_LOCAL_ASSET_ENV: &str = "V2RAY_LOCATION_ASSET";
pub const XRAY_LOCAL_ASSET_ENV: &str = "XRAY_LOCATION_ASSET";
pub const XRAY_LOCAL_CERT_ENV: &str = "XRAY_LOCATION_CERT";
pub const MIERU_CONFIG_ENV: &str = "MIERU_CONFIG_JSON_FILE";

const V2RAY_EXES: &[&str] = &["v2ray"];
const XRAY_EXES: &[&str] = &["xray"];
const HYSTERIA_EXES: &[&str] = &["hysteria"];
const NAIVE_EXES: &[&str] = &["naive", "naiveproxy"];
const TUIC_EXES: &[&str] = &["tuic-client", "tuic"];
const SING_BOX_EXES: &[&str] = &["sing-box-client", "sing-box"];
const JUICITY_EXES: &[&str] = &["juicity-client", "juicity"];
const HYSTERIA2_EXES: &[&str] = &["hysteria-windows-amd64", "hysteria-linux-amd64", "hysteria"];
const BROOK_EXES: &[&str] = &["brook_windows_amd64", "brook_linux_amd64", "brook"];
const OVERTLS_EXES: &[&str] = &["overtls-bin", "overtls"];
const SHADOWQUIC_EXES: &[&str] = &["shadowquic"];
const MIERU_EXES: &[&str] = &["mieru"];
const EMPTY_EXES: &[&str] = &[];

const MIHOMO_WINDOWS_EXES: &[&str] = &[
    "mihomo-windows-amd64-v1",
    "mihomo-windows-amd64-compatible",
    "mihomo-windows-amd64",
    "mihomo-windows-arm64",
    "clash",
    "mihomo",
];
const MIHOMO_LINUX_EXES: &[&str] = &[
    "mihomo-linux-amd64-v1",
    "mihomo-linux-amd64",
    "mihomo-linux-arm64",
    "mihomo-linux-riscv64",
    "clash",
    "mihomo",
];
const MIHOMO_MACOS_EXES: &[&str] = &[
    "mihomo-darwin-amd64-v1",
    "mihomo-darwin-amd64",
    "mihomo-darwin-arm64",
    "clash",
    "mihomo",
];
const MIHOMO_FALLBACK_EXES: &[&str] = &["clash", "mihomo"];

const EMPTY_ENV: &[CoreEnvTemplate] = &[];
const V2FLY_ENV: &[CoreEnvTemplate] = &[CoreEnvTemplate {
    key: V2RAY_LOCAL_ASSET_ENV,
    value: EnvValueTemplate::BinDir,
}];
const XRAY_ENV: &[CoreEnvTemplate] = &[
    CoreEnvTemplate {
        key: XRAY_LOCAL_ASSET_ENV,
        value: EnvValueTemplate::BinDir,
    },
    CoreEnvTemplate {
        key: XRAY_LOCAL_CERT_ENV,
        value: EnvValueTemplate::BinDir,
    },
];
const MIERU_ENV: &[CoreEnvTemplate] = &[CoreEnvTemplate {
    key: MIERU_CONFIG_ENV,
    value: EnvValueTemplate::ConfigArgument,
}];

const CORE_INFOS: &[CoreInfo] = &[
    CoreInfo {
        core_type: CoreType::v2rayN,
        executables: CoreExecutables::Static(EMPTY_EXES),
        arguments: CoreArguments::Static(""),
        url: "https://github.com/2dust/v2rayN/releases",
        release_api_url: Some("https://api.github.com/repos/2dust/v2rayN/releases"),
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::v2fly,
        executables: CoreExecutables::Static(V2RAY_EXES),
        arguments: CoreArguments::Static("{0}"),
        url: "https://github.com/v2fly/v2ray-core/releases",
        release_api_url: None,
        match_keyword: Some("V2Ray"),
        version_arg: Some("-version"),
        absolute_path: false,
        environment: V2FLY_ENV,
    },
    CoreInfo {
        core_type: CoreType::v2fly_v5,
        executables: CoreExecutables::Static(V2RAY_EXES),
        arguments: CoreArguments::Static("run -c {0} -format jsonv5"),
        url: "https://github.com/v2fly/v2ray-core/releases",
        release_api_url: None,
        match_keyword: Some("V2Ray"),
        version_arg: Some("version"),
        absolute_path: false,
        environment: V2FLY_ENV,
    },
    CoreInfo {
        core_type: CoreType::Xray,
        executables: CoreExecutables::Static(XRAY_EXES),
        arguments: CoreArguments::Static("run -c {0}"),
        url: "https://github.com/XTLS/Xray-core/releases",
        release_api_url: Some("https://api.github.com/repos/XTLS/Xray-core/releases"),
        match_keyword: Some("Xray"),
        version_arg: Some("-version"),
        absolute_path: false,
        environment: XRAY_ENV,
    },
    CoreInfo {
        core_type: CoreType::mihomo,
        executables: CoreExecutables::Mihomo,
        arguments: CoreArguments::MihomoPortableDataDir,
        url: "https://github.com/MetaCubeX/mihomo/releases",
        release_api_url: Some("https://api.github.com/repos/MetaCubeX/mihomo/releases"),
        match_keyword: Some("Mihomo"),
        version_arg: Some("-v"),
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::hysteria,
        executables: CoreExecutables::Static(HYSTERIA_EXES),
        arguments: CoreArguments::Static(""),
        url: "https://github.com/apernet/hysteria/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::naiveproxy,
        executables: CoreExecutables::Static(NAIVE_EXES),
        arguments: CoreArguments::Static("{0}"),
        url: "https://github.com/klzgrad/naiveproxy/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::tuic,
        executables: CoreExecutables::Static(TUIC_EXES),
        arguments: CoreArguments::Static("-c {0}"),
        url: "https://github.com/EAimTY/tuic/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::sing_box,
        executables: CoreExecutables::Static(SING_BOX_EXES),
        arguments: CoreArguments::Static("run -c {0} --disable-color"),
        url: "https://github.com/SagerNet/sing-box/releases",
        release_api_url: Some("https://api.github.com/repos/SagerNet/sing-box/releases"),
        match_keyword: Some("sing-box"),
        version_arg: Some("version"),
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::juicity,
        executables: CoreExecutables::Static(JUICITY_EXES),
        arguments: CoreArguments::Static("run -c {0}"),
        url: "https://github.com/juicity/juicity/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::hysteria2,
        executables: CoreExecutables::Static(HYSTERIA2_EXES),
        arguments: CoreArguments::Static(""),
        url: "https://github.com/apernet/hysteria/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::brook,
        executables: CoreExecutables::Static(BROOK_EXES),
        arguments: CoreArguments::Static(" {0}"),
        url: "https://github.com/txthinking/brook/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: true,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::overtls,
        executables: CoreExecutables::Static(OVERTLS_EXES),
        arguments: CoreArguments::Static("-r client -c {0}"),
        url: "https://github.com/ShadowsocksR-Live/overtls/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::shadowquic,
        executables: CoreExecutables::Static(SHADOWQUIC_EXES),
        arguments: CoreArguments::Static("-c {0}"),
        url: "https://github.com/spongebob888/shadowquic/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: EMPTY_ENV,
    },
    CoreInfo {
        core_type: CoreType::mieru,
        executables: CoreExecutables::Static(MIERU_EXES),
        arguments: CoreArguments::Static("run"),
        url: "https://github.com/enfein/mieru/releases",
        release_api_url: None,
        match_keyword: None,
        version_arg: None,
        absolute_path: false,
        environment: MIERU_ENV,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreInfo {
    pub core_type: CoreType,
    pub executables: CoreExecutables,
    pub arguments: CoreArguments,
    pub url: &'static str,
    pub release_api_url: Option<&'static str>,
    pub match_keyword: Option<&'static str>,
    pub version_arg: Option<&'static str>,
    pub absolute_path: bool,
    pub environment: &'static [CoreEnvTemplate],
}

impl CoreInfo {
    #[must_use]
    pub fn executable_names(&self) -> &'static [&'static str] {
        self.executable_names_for_os(TargetOs::current())
    }

    #[must_use]
    pub fn executable_names_for_os(&self, os: TargetOs) -> &'static [&'static str] {
        match self.executables {
            CoreExecutables::Static(names) => names,
            CoreExecutables::Mihomo => mihomo_executable_names_for_os(os),
        }
    }

    #[must_use]
    pub fn argument_template(&self, paths: &AppPaths) -> String {
        match self.arguments {
            CoreArguments::Static(template) => template.to_string(),
            CoreArguments::MihomoPortableDataDir => {
                format!("-f {{0}} -d {}", quote_path(paths.bin_dir()))
            }
        }
    }

    #[must_use]
    pub fn config_argument(&self, paths: &AppPaths, config_file: impl AsRef<Path>) -> String {
        if self.absolute_path {
            quote_path(paths.bin_config_file(config_file))
        } else {
            config_file.as_ref().to_string_lossy().into_owned()
        }
    }

    #[must_use]
    pub fn resolve_arguments(&self, paths: &AppPaths, config_file: impl AsRef<Path>) -> String {
        let config_argument = self.config_argument(paths, config_file);
        self.argument_template(paths)
            .replace("{0}", &config_argument)
    }

    #[must_use]
    pub fn resolve_environment(
        &self,
        paths: &AppPaths,
        config_file: impl AsRef<Path>,
    ) -> BTreeMap<String, String> {
        let config_argument = self.config_argument(paths, config_file);
        self.environment
            .iter()
            .map(|template| {
                let value = match template.value {
                    EnvValueTemplate::BinDir => paths.bin_dir().to_string_lossy().into_owned(),
                    EnvValueTemplate::ConfigArgument => config_argument.clone(),
                };
                (template.key.to_string(), value)
            })
            .collect()
    }

    #[must_use]
    pub fn resolve_launch(
        &self,
        executable: impl Into<PathBuf>,
        paths: &AppPaths,
        config_file: impl AsRef<Path>,
    ) -> CoreLaunch {
        CoreLaunch {
            executable: executable.into(),
            arguments: self.resolve_arguments(paths, config_file.as_ref()),
            working_dir: paths.bin_config_dir().to_path_buf(),
            environment: self.resolve_environment(paths, config_file),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreExecutables {
    Static(&'static [&'static str]),
    Mihomo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreArguments {
    Static(&'static str),
    MihomoPortableDataDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreEnvTemplate {
    pub key: &'static str,
    pub value: EnvValueTemplate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvValueTemplate {
    BinDir,
    ConfigArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreLaunch {
    pub executable: PathBuf,
    pub arguments: String,
    pub working_dir: PathBuf,
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    Linux,
    Macos,
    Other,
}

impl TargetOs {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Error)]
pub enum CoreInfoError {
    #[error("no core info entry for {0:?}")]
    MissingCoreInfo(CoreType),
    #[error("core {core_type:?} executable not found in {search_dir}; expected one of: {candidates}; download: {url}")]
    ExecutableNotFound {
        core_type: CoreType,
        search_dir: PathDisplay,
        candidates: String,
        url: &'static str,
    },
    #[error("failed to create core bin directory {path}: {source}")]
    CreateCoreBinDir { path: PathBuf, source: io::Error },
    #[error("failed to inspect executable {path}: {source}")]
    InspectExecutable { path: PathBuf, source: io::Error },
    #[error("failed to update executable permissions for {path}: {source}")]
    ChmodExecutable { path: PathBuf, source: io::Error },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathDisplay(PathBuf);

impl std::fmt::Display for PathDisplay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0.display())
    }
}

#[must_use]
pub fn all_core_infos() -> &'static [CoreInfo] {
    CORE_INFOS
}

#[must_use]
pub fn get_core_info(core_type: CoreType) -> Option<&'static CoreInfo> {
    CORE_INFOS
        .iter()
        .find(|core_info| core_info.core_type == core_type)
}

#[must_use]
pub const fn core_type_name(core_type: CoreType) -> &'static str {
    match core_type {
        CoreType::v2fly => "v2fly",
        CoreType::Xray => "Xray",
        CoreType::v2fly_v5 => "v2fly_v5",
        CoreType::mihomo => "mihomo",
        CoreType::hysteria => "hysteria",
        CoreType::naiveproxy => "naiveproxy",
        CoreType::tuic => "tuic",
        CoreType::sing_box => "sing_box",
        CoreType::juicity => "juicity",
        CoreType::hysteria2 => "hysteria2",
        CoreType::brook => "brook",
        CoreType::overtls => "overtls",
        CoreType::shadowquic => "shadowquic",
        CoreType::mieru => "mieru",
        CoreType::v2rayN => "v2rayN",
    }
}

#[must_use]
pub const fn core_type_dir_name(core_type: CoreType) -> &'static str {
    match core_type {
        CoreType::Xray => "xray",
        CoreType::v2rayN => "v2rayn",
        _ => core_type_name(core_type),
    }
}

#[must_use]
pub const fn mihomo_executable_names_for_os(os: TargetOs) -> &'static [&'static str] {
    match os {
        TargetOs::Windows => MIHOMO_WINDOWS_EXES,
        TargetOs::Linux => MIHOMO_LINUX_EXES,
        TargetOs::Macos => MIHOMO_MACOS_EXES,
        TargetOs::Other => MIHOMO_FALLBACK_EXES,
    }
}

#[must_use]
pub fn executable_name_for_os(name: &str, os: TargetOs) -> String {
    if os == TargetOs::Windows && !name.to_ascii_lowercase().ends_with(".exe") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

#[must_use]
pub fn executable_name_for_current_os(name: &str) -> String {
    executable_name_for_os(name, TargetOs::current())
}

pub fn discover_executable(
    paths: &AppPaths,
    core_info: &CoreInfo,
) -> Result<PathBuf, CoreInfoError> {
    let search_dir = paths.core_bin_dir(core_type_dir_name(core_info.core_type));
    fs::create_dir_all(&search_dir).map_err(|source| CoreInfoError::CreateCoreBinDir {
        path: search_dir.clone(),
        source,
    })?;

    for name in core_info.executable_names() {
        let executable_name = executable_name_for_current_os(name);
        let candidate = search_dir.join(executable_name);
        match candidate.try_exists() {
            Ok(true) if candidate.is_file() => {
                ensure_executable_permission(&candidate)?;
                return Ok(candidate);
            }
            Ok(_) => {}
            Err(source) => {
                return Err(CoreInfoError::InspectExecutable {
                    path: candidate,
                    source,
                });
            }
        }
    }

    let candidates = core_info
        .executable_names()
        .iter()
        .map(|name| executable_name_for_current_os(name))
        .collect::<Vec<_>>()
        .join(", ");
    Err(CoreInfoError::ExecutableNotFound {
        core_type: core_info.core_type,
        search_dir: PathDisplay(search_dir),
        candidates,
        url: core_info.url,
    })
}

pub fn chmod_existing_core_executables(paths: &AppPaths) -> Result<Vec<PathBuf>, CoreInfoError> {
    let mut updated = Vec::new();
    for core_info in all_core_infos() {
        if core_info.core_type == CoreType::v2rayN {
            continue;
        }
        let search_dir = paths.core_bin_dir(core_type_dir_name(core_info.core_type));
        for name in core_info.executable_names() {
            let executable_name = executable_name_for_current_os(name);
            let candidate = search_dir.join(executable_name);
            match candidate.try_exists() {
                Ok(true) if candidate.is_file() => {
                    ensure_executable_permission(&candidate)?;
                    updated.push(candidate);
                }
                Ok(_) => {}
                Err(source) => {
                    return Err(CoreInfoError::InspectExecutable {
                        path: candidate,
                        source,
                    });
                }
            }
        }
    }
    Ok(updated)
}

pub fn ensure_executable_permission(path: impl AsRef<Path>) -> Result<(), CoreInfoError> {
    ensure_executable_permission_inner(path.as_ref())
}

#[cfg(unix)]
fn ensure_executable_permission_inner(path: &Path) -> Result<(), CoreInfoError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| CoreInfoError::InspectExecutable {
        path: path.to_path_buf(),
        source,
    })?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    let executable_mode = mode | 0o111;
    if executable_mode != mode {
        permissions.set_mode(executable_mode);
        fs::set_permissions(path, permissions).map_err(|source| {
            CoreInfoError::ChmodExecutable {
                path: path.to_path_buf(),
                source,
            }
        })?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_executable_permission_inner(path: &Path) -> Result<(), CoreInfoError> {
    let _ = fs::metadata(path).map_err(|source| CoreInfoError::InspectExecutable {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

#[must_use]
pub fn quote_path(path: impl AsRef<Path>) -> String {
    let path = path.as_ref().to_string_lossy();
    if path.is_empty() {
        String::new()
    } else {
        format!("\"{path}\"")
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use crate::paths::{AppPaths, StorageMode};

    use super::*;

    #[test]
    fn coreinfo_table_covers_all_reference_core_types() {
        let core_types = all_core_infos()
            .iter()
            .map(|core_info| core_info.core_type)
            .collect::<Vec<_>>();

        assert_eq!(core_types.len(), 15);
        assert_eq!(
            core_types,
            vec![
                CoreType::v2rayN,
                CoreType::v2fly,
                CoreType::v2fly_v5,
                CoreType::Xray,
                CoreType::mihomo,
                CoreType::hysteria,
                CoreType::naiveproxy,
                CoreType::tuic,
                CoreType::sing_box,
                CoreType::juicity,
                CoreType::hysteria2,
                CoreType::brook,
                CoreType::overtls,
                CoreType::shadowquic,
                CoreType::mieru,
            ]
        );
    }

    #[test]
    fn coreinfo_mihomo_probe_order_matches_reference_per_os() {
        assert_eq!(
            mihomo_executable_names_for_os(TargetOs::Windows),
            [
                "mihomo-windows-amd64-v1",
                "mihomo-windows-amd64-compatible",
                "mihomo-windows-amd64",
                "mihomo-windows-arm64",
                "clash",
                "mihomo",
            ]
        );
        assert_eq!(
            mihomo_executable_names_for_os(TargetOs::Linux),
            [
                "mihomo-linux-amd64-v1",
                "mihomo-linux-amd64",
                "mihomo-linux-arm64",
                "mihomo-linux-riscv64",
                "clash",
                "mihomo",
            ]
        );
        assert_eq!(
            mihomo_executable_names_for_os(TargetOs::Macos),
            [
                "mihomo-darwin-amd64-v1",
                "mihomo-darwin-amd64",
                "mihomo-darwin-arm64",
                "clash",
                "mihomo",
            ]
        );
    }

    #[test]
    fn coreinfo_arguments_and_env_match_reference_templates() {
        let paths = AppPaths::new("/tmp/VoyaVPN", StorageMode::Portable);

        let xray = get_core_info(CoreType::Xray).expect("xray core info");
        assert_eq!(
            xray.resolve_arguments(&paths, "config.json"),
            "run -c config.json"
        );
        assert_eq!(
            xray.resolve_environment(&paths, "config.json"),
            BTreeMap::from([
                (
                    XRAY_LOCAL_ASSET_ENV.to_string(),
                    "/tmp/VoyaVPN/bin".to_string()
                ),
                (
                    XRAY_LOCAL_CERT_ENV.to_string(),
                    "/tmp/VoyaVPN/bin".to_string()
                ),
            ])
        );

        let v2fly_v5 = get_core_info(CoreType::v2fly_v5).expect("v2fly v5 core info");
        assert_eq!(
            v2fly_v5.resolve_arguments(&paths, "config.json"),
            "run -c config.json -format jsonv5"
        );
        assert_eq!(
            v2fly_v5.resolve_environment(&paths, "config.json"),
            BTreeMap::from([(
                V2RAY_LOCAL_ASSET_ENV.to_string(),
                "/tmp/VoyaVPN/bin".to_string()
            )])
        );

        let mieru = get_core_info(CoreType::mieru).expect("mieru core info");
        assert_eq!(mieru.resolve_arguments(&paths, "config.json"), "run");
        assert_eq!(
            mieru.resolve_environment(&paths, "config.json"),
            BTreeMap::from([(MIERU_CONFIG_ENV.to_string(), "config.json".to_string())])
        );

        let mihomo = get_core_info(CoreType::mihomo).expect("mihomo core info");
        assert_eq!(
            mihomo.resolve_arguments(&paths, "config.json"),
            "-f config.json -d \"/tmp/VoyaVPN/bin\""
        );

        let brook = get_core_info(CoreType::brook).expect("brook core info");
        assert_eq!(
            brook.resolve_arguments(&paths, "config.json"),
            " \"/tmp/VoyaVPN/binConfigs/config.json\""
        );

        let hysteria = get_core_info(CoreType::hysteria).expect("hysteria core info");
        let hysteria2 = get_core_info(CoreType::hysteria2).expect("hysteria2 core info");
        assert_eq!(hysteria.resolve_arguments(&paths, "config.json"), "");
        assert_eq!(hysteria2.resolve_arguments(&paths, "config.json"), "");
    }

    #[test]
    fn coreinfo_executable_discovery_uses_core_subdir_and_probe_order() {
        let root = unique_temp_root("discover");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        let xray = get_core_info(CoreType::Xray).expect("xray core info");
        let exe = paths.core_bin_file(
            core_type_dir_name(CoreType::Xray),
            executable_name_for_current_os("xray"),
        );
        fs::create_dir_all(exe.parent().expect("xray exe parent")).expect("create xray dir");
        fs::write(&exe, b"").expect("write xray exe");

        let discovered = discover_executable(&paths, xray).expect("discover xray");
        assert_eq!(discovered, exe);

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn coreinfo_discovery_chmods_unix_executables() {
        let root = unique_temp_root("chmod");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        let sing_box = get_core_info(CoreType::sing_box).expect("sing-box core info");
        let exe = paths.core_bin_file(core_type_dir_name(CoreType::sing_box), "sing-box-client");
        fs::create_dir_all(exe.parent().expect("sing-box exe parent"))
            .expect("create sing-box dir");
        fs::write(&exe, b"").expect("write sing-box exe");
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o600)).expect("set initial mode");

        let discovered = discover_executable(&paths, sing_box).expect("discover sing-box");
        let mode = fs::metadata(&discovered)
            .expect("stat discovered exe")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coreinfo_windows_exe_suffix_is_added_only_for_windows() {
        assert_eq!(
            executable_name_for_os("xray", TargetOs::Windows),
            "xray.exe"
        );
        assert_eq!(
            executable_name_for_os("xray.exe", TargetOs::Windows),
            "xray.exe"
        );
        assert_eq!(
            executable_name_for_os("xray.ExE", TargetOs::Windows),
            "xray.ExE"
        );
        assert_eq!(executable_name_for_os("xray", TargetOs::Linux), "xray");
        assert_eq!(executable_name_for_os("xray", TargetOs::Macos), "xray");
    }

    fn unique_temp_root(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "voyavpn-coreinfo-{name}-{}-{}",
            std::process::id(),
            monotonic_nanos()
        ))
    }

    fn monotonic_nanos() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    }
}
