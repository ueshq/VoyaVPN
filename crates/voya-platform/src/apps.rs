//! Enumeration of candidate applications and running processes for per-app
//! proxy rules. sing-box `process_name` route rules match on the executable
//! file name, so `process_name` always carries the exact basename (including
//! `.exe` on Windows).

use std::collections::BTreeMap;

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

fn candidate_from_executable_path(
    path: &str,
    source: ProcessCandidateSource,
) -> Option<ProcessCandidate> {
    let process_name = std::path::Path::new(path)
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
                let executables_dir = bundle.join("Contents").join("MacOS");
                let executable = std::fs::read_dir(&executables_dir)
                    .ok()?
                    .flatten()
                    .map(|executable| executable.path())
                    .find(|path| path.is_file())?;
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
    use std::process::Command;

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
