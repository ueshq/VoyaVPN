//! Enumeration of candidate applications and running processes for per-app
//! proxy rules. sing-box `process_name` route rules match on the executable
//! file name, so `process_name` always carries the exact basename (including
//! `.exe` on Windows).

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessCandidateSource {
    RunningProcess,
    InstalledApplication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessCandidate {
    /// Human-friendly name (app bundle name where known, else the process name).
    pub display_name: String,
    /// Executable basename as sing-box matches it.
    pub process_name: String,
    pub executable_path: Option<String>,
    pub source: ProcessCandidateSource,
}

/// Lists deduplicated candidate processes/apps for the current platform,
/// sorted case-insensitively by process name. Best effort: enumeration
/// failures yield an empty or partial list rather than an error.
#[must_use]
pub fn list_process_candidates() -> Vec<ProcessCandidate> {
    merge_candidates(collect_platform_candidates())
}

/// Dedupes by lowercased process name, preferring installed applications over
/// running processes and entries that know their executable path.
fn merge_candidates(candidates: Vec<ProcessCandidate>) -> Vec<ProcessCandidate> {
    let mut by_name: BTreeMap<String, ProcessCandidate> = BTreeMap::new();
    for candidate in candidates {
        let process_name = candidate.process_name.trim();
        if process_name.is_empty() {
            continue;
        }
        let key = process_name.to_lowercase();
        match by_name.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(candidate);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if candidate_rank(&candidate) > candidate_rank(entry.get()) {
                    entry.insert(candidate);
                }
            }
        }
    }

    by_name.into_values().collect()
}

fn candidate_rank(candidate: &ProcessCandidate) -> u8 {
    let source_rank = match candidate.source {
        ProcessCandidateSource::InstalledApplication => 2,
        ProcessCandidateSource::RunningProcess => 0,
    };
    source_rank + u8::from(candidate.executable_path.is_some())
}

#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn candidate_from_executable_path(
    path: &str,
    source: ProcessCandidateSource,
) -> Option<ProcessCandidate> {
    let process_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)?;

    Some(ProcessCandidate {
        display_name: process_name.clone(),
        process_name,
        executable_path: Some(path.to_string()),
        source,
    })
}

#[cfg(target_os = "macos")]
fn collect_platform_candidates() -> Vec<ProcessCandidate> {
    let mut candidates = macos::installed_applications();
    candidates.extend(macos::running_processes());
    candidates
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{candidate_from_executable_path, ProcessCandidate, ProcessCandidateSource};
    use std::process::Command;

    const NOISE_PATH_PREFIXES: [&str; 4] =
        ["/System/", "/usr/libexec/", "/usr/sbin/", "/Library/Apple/"];

    pub(super) fn running_processes() -> Vec<ProcessCandidate> {
        let Ok(output) = Command::new("/bin/ps").args(["-axo", "comm="]).output() else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| candidate_from_ps_line(line.trim()))
            .collect()
    }

    fn candidate_from_ps_line(line: &str) -> Option<ProcessCandidate> {
        if line.is_empty() {
            return None;
        }
        if !line.starts_with('/') {
            // Bare names (kernel-managed or truncated); still usable as a rule.
            return Some(ProcessCandidate {
                display_name: line.to_string(),
                process_name: line.to_string(),
                executable_path: None,
                source: ProcessCandidateSource::RunningProcess,
            });
        }
        if NOISE_PATH_PREFIXES
            .iter()
            .any(|prefix| line.starts_with(prefix))
        {
            return None;
        }

        candidate_from_executable_path(line, ProcessCandidateSource::RunningProcess)
    }

    pub(super) fn installed_applications() -> Vec<ProcessCandidate> {
        let Ok(entries) = std::fs::read_dir("/Applications") else {
            return Vec::new();
        };

        entries
            .flatten()
            .filter_map(|entry| {
                let bundle = entry.path();
                let display_name = bundle.file_stem()?.to_str()?.to_string();
                if bundle.extension().and_then(|extension| extension.to_str()) != Some("app") {
                    return None;
                }
                let executable = super::macos_bundle_executable(&bundle, plutil_bundle_executable)?;
                let process_name = executable.file_name()?.to_str()?.to_string();

                Some(ProcessCandidate {
                    display_name,
                    process_name,
                    executable_path: Some(executable.to_string_lossy().into_owned()),
                    source: ProcessCandidateSource::InstalledApplication,
                })
            })
            .collect()
    }

    /// Authoritative `CFBundleExecutable` lookup for binary `Info.plist` files,
    /// which the cheap XML scan cannot read.
    fn plutil_bundle_executable(info_plist: &std::path::Path) -> Option<String> {
        let output = Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleExecutable", "raw", "-o", "-"])
            .arg(info_plist)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!value.is_empty()).then_some(value)
    }
}

#[cfg(target_os = "linux")]
fn collect_platform_candidates() -> Vec<ProcessCandidate> {
    linux::running_processes()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{candidate_from_executable_path, ProcessCandidate, ProcessCandidateSource};

    pub(super) fn running_processes() -> Vec<ProcessCandidate> {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };

        entries
            .flatten()
            .filter_map(|entry| {
                let file_name = entry.file_name();
                let pid = file_name.to_str()?;
                if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
                    return None;
                }
                let proc_dir = entry.path();
                // Kernel threads have an empty cmdline; skip them.
                let cmdline = std::fs::read(proc_dir.join("cmdline")).unwrap_or_default();
                if cmdline.is_empty() {
                    return None;
                }

                if let Ok(executable) = std::fs::read_link(proc_dir.join("exe")) {
                    return candidate_from_executable_path(
                        &executable.to_string_lossy(),
                        ProcessCandidateSource::RunningProcess,
                    );
                }
                let comm = std::fs::read_to_string(proc_dir.join("comm")).ok()?;
                let comm = comm.trim();
                (!comm.is_empty()).then(|| ProcessCandidate {
                    display_name: comm.to_string(),
                    process_name: comm.to_string(),
                    executable_path: None,
                    source: ProcessCandidateSource::RunningProcess,
                })
            })
            .collect()
    }
}

#[cfg(windows)]
fn collect_platform_candidates() -> Vec<ProcessCandidate> {
    windows::running_processes()
}

#[cfg(windows)]
mod windows {
    use super::{parse_tasklist_csv_image_name, ProcessCandidate, ProcessCandidateSource};
    use std::{os::windows::process::CommandExt, process::Command};

    /// Console-subsystem children of a GUI-subsystem parent get their own visible
    /// console window unless this flag is set.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    const NOISE_IMAGES: [&str; 8] = [
        "system",
        "system idle process",
        "registry",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "services.exe",
        "lsass.exe",
    ];

    pub(super) fn running_processes() -> Vec<ProcessCandidate> {
        let Ok(output) = Command::new(r"C:\Windows\System32\tasklist.exe")
            .args(["/fo", "csv", "/nh"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| parse_tasklist_csv_image_name(line.trim()))
            .filter(|image| !NOISE_IMAGES.contains(&image.to_lowercase().as_str()))
            .map(|image| ProcessCandidate {
                display_name: image.clone(),
                process_name: image,
                executable_path: None,
                source: ProcessCandidateSource::RunningProcess,
            })
            .collect()
    }
}

/// Resolves the main executable of a macOS app bundle.
///
/// `Contents/MacOS` routinely holds helper binaries and CLIs (`IINA.app` ships
/// `iina-cli` and `youtube-dl`, `Bitwarden.app` ships `desktop_proxy`), and
/// `read_dir` order is unspecified, so taking the first regular file produces a
/// `process_name` sing-box never matches. The bundle's own `CFBundleExecutable`
/// is the only reliable answer. It is read from an XML `Info.plist` directly,
/// then guessed from the bundle stem (the common shape, and free), and only
/// then delegated to `read_declared_executable`, which costs a process spawn.
/// A bundle that resolves to none of those is skipped rather than guessed.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn macos_bundle_executable(
    bundle: &Path,
    read_declared_executable: impl Fn(&Path) -> Option<String>,
) -> Option<PathBuf> {
    let executables_dir = bundle.join("Contents").join("MacOS");
    let info_plist = bundle.join("Contents").join("Info.plist");

    let plist_text = std::fs::read_to_string(&info_plist).ok();
    let declared = plist_text
        .as_deref()
        .and_then(parse_xml_plist_bundle_executable);
    if let Some(executable) =
        declared.and_then(|name| bundle_executable_file(&executables_dir, &name))
    {
        return Some(executable);
    }

    if let Some(executable) = bundle
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| bundle_executable_file(&executables_dir, stem))
    {
        return Some(executable);
    }

    read_declared_executable(&info_plist)
        .and_then(|name| bundle_executable_file(&executables_dir, &name))
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn bundle_executable_file(executables_dir: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let candidate = executables_dir.join(name);
    candidate.is_file().then_some(candidate)
}

/// Reads `CFBundleExecutable` out of an XML `Info.plist` without a plist
/// parser. Binary plists (`bplist00`) yield `None` so the caller falls back.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_xml_plist_bundle_executable(plist: &str) -> Option<String> {
    const KEY: &str = "<key>CFBundleExecutable</key>";

    let rest = plist.get(plist.find(KEY)? + KEY.len()..)?.trim_start();
    let value = rest.strip_prefix("<string>")?;
    let end = value.find("</string>")?;
    let value = value.get(..end)?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Extracts the image name from one `tasklist /fo csv /nh` output line, e.g.
/// `"chrome.exe","1234","Console","1","150,000 K"` -> `chrome.exe`.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_tasklist_csv_image_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix('"')?;
    let (image, _) = rest.split_once('"')?;
    let image = image.trim();
    (!image.is_empty()).then(|| image.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn collect_platform_candidates() -> Vec<ProcessCandidate> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        process_name: &str,
        path: Option<&str>,
        source: ProcessCandidateSource,
    ) -> ProcessCandidate {
        ProcessCandidate {
            display_name: process_name.to_string(),
            process_name: process_name.to_string(),
            executable_path: path.map(str::to_string),
            source,
        }
    }

    #[test]
    fn merge_dedupes_case_insensitively_preferring_installed_apps_with_paths() {
        let merged = merge_candidates(vec![
            candidate("Firefox", None, ProcessCandidateSource::RunningProcess),
            candidate(
                "firefox",
                Some("/Applications/Firefox.app/Contents/MacOS/firefox"),
                ProcessCandidateSource::InstalledApplication,
            ),
            candidate(
                "zsh",
                Some("/bin/zsh"),
                ProcessCandidateSource::RunningProcess,
            ),
            candidate("", None, ProcessCandidateSource::RunningProcess),
        ]);

        assert_eq!(merged.len(), 2);
        assert_eq!(
            merged[0].source,
            ProcessCandidateSource::InstalledApplication
        );
        assert!(merged[0].executable_path.is_some());
        assert_eq!(merged[1].process_name, "zsh");
    }

    #[test]
    fn merge_output_is_sorted_by_lowercased_name() {
        let merged = merge_candidates(vec![
            candidate("beta", None, ProcessCandidateSource::RunningProcess),
            candidate("Alpha", None, ProcessCandidateSource::RunningProcess),
        ]);
        let names = merged
            .iter()
            .map(|candidate| candidate.process_name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, ["Alpha", "beta"]);
    }

    #[test]
    fn tasklist_csv_image_name_parses_first_quoted_field() {
        assert_eq!(
            parse_tasklist_csv_image_name(r#""chrome.exe","1234","Console","1","150,000 K""#),
            Some("chrome.exe".to_string())
        );
        assert_eq!(parse_tasklist_csv_image_name("no,quotes,here"), None);
        assert_eq!(parse_tasklist_csv_image_name(r#""""#), None);
    }

    #[test]
    fn executable_path_candidate_uses_basename_as_process_name() {
        let candidate = candidate_from_executable_path(
            "/Applications/Safari.app/Contents/MacOS/Safari",
            ProcessCandidateSource::InstalledApplication,
        )
        .expect("path with basename should produce a candidate");

        assert_eq!(candidate.process_name, "Safari");
        assert_eq!(
            candidate.executable_path.as_deref(),
            Some("/Applications/Safari.app/Contents/MacOS/Safari")
        );
    }

    fn bundle_fixture(name: &str, plist: &[u8], executables: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "voyavpn-apps-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        let bundle = root.join("IINA.app");
        let executables_dir = bundle.join("Contents").join("MacOS");
        std::fs::create_dir_all(&executables_dir).expect("create bundle fixture");
        std::fs::write(bundle.join("Contents").join("Info.plist"), plist).expect("write plist");
        for executable in executables {
            std::fs::write(executables_dir.join(executable), "#!/bin/sh\n").expect("write binary");
        }
        bundle
    }

    fn xml_plist(executable: &str) -> Vec<u8> {
        format!(
            "<?xml version=\"1.0\"?>\n<plist version=\"1.0\"><dict>\n\t<key>CFBundleName</key>\n\t<string>IINA</string>\n\t<key>CFBundleExecutable</key>\n\t<string>{executable}</string>\n</dict></plist>\n"
        )
        .into_bytes()
    }

    #[test]
    fn bundle_executable_prefers_the_declared_cf_bundle_executable() {
        let bundle = bundle_fixture(
            "declared",
            &xml_plist("IINA"),
            &["iina-cli", "IINA", "youtube-dl"],
        );

        let executable = macos_bundle_executable(&bundle, |_: &Path| None)
            .expect("declared executable resolves");

        assert_eq!(
            executable.file_name().and_then(|name| name.to_str()),
            Some("IINA")
        );

        let _ = std::fs::remove_dir_all(bundle.parent().expect("fixture root"));
    }

    #[test]
    fn bundle_executable_falls_back_to_the_bundle_stem_for_binary_plists() {
        let bundle = bundle_fixture("binary-plist", b"bplist00\x00\x01", &["helper", "IINA"]);

        let executable = macos_bundle_executable(&bundle, |_: &Path| None)
            .expect("bundle stem executable resolves");

        assert_eq!(
            executable.file_name().and_then(|name| name.to_str()),
            Some("IINA")
        );

        let _ = std::fs::remove_dir_all(bundle.parent().expect("fixture root"));
    }

    #[test]
    fn bundle_executable_asks_the_plist_reader_when_nothing_else_matches() {
        let bundle = bundle_fixture("plutil", b"bplist00\x00\x01", &["cli", "wechatdevtools"]);

        let executable =
            macos_bundle_executable(&bundle, |_: &Path| Some("wechatdevtools".to_string()))
                .expect("declared executable resolves");

        assert_eq!(
            executable.file_name().and_then(|name| name.to_str()),
            Some("wechatdevtools")
        );

        let _ = std::fs::remove_dir_all(bundle.parent().expect("fixture root"));
    }

    #[test]
    fn bundle_executable_skips_a_bundle_it_cannot_resolve() {
        let bundle = bundle_fixture("unresolved", b"bplist00\x00\x01", &["cli", "helper"]);

        assert!(macos_bundle_executable(&bundle, |_: &Path| None).is_none());
        assert!(
            macos_bundle_executable(&bundle, |_: &Path| Some("../../etc/passwd".to_string()))
                .is_none()
        );

        let _ = std::fs::remove_dir_all(bundle.parent().expect("fixture root"));
    }

    #[test]
    fn xml_plist_executable_is_only_read_from_the_key_that_declares_it() {
        assert_eq!(
            parse_xml_plist_bundle_executable(
                "<key>CFBundleExecutable</key>\n\t<string>Safari</string>"
            ),
            Some("Safari".to_string())
        );
        assert_eq!(
            parse_xml_plist_bundle_executable("<key>CFBundleName</key><string>Safari</string>"),
            None
        );
        assert_eq!(
            parse_xml_plist_bundle_executable("<key>CFBundleExecutable</key><array/>"),
            None
        );
    }

    #[test]
    fn current_platform_enumeration_returns_deduped_sorted_candidates() {
        let candidates = list_process_candidates();
        let mut names = candidates
            .iter()
            .map(|candidate| candidate.process_name.to_lowercase())
            .collect::<Vec<_>>();
        let deduped_len = names.len();
        names.dedup();
        assert_eq!(names.len(), deduped_len, "process names must be unique");
    }
}
