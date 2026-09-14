use super::*;

pub(super) fn linux_script_invocation(
    request: &SystemProxyRequest,
    mode: &str,
    manual: Option<(&str, i32, &str)>,
) -> ScriptInvocation {
    let executable = request.script_dir.join(LINUX_PROXY_SCRIPT_NAME);
    let generated_script = Some(GeneratedScript::new(
        request.script_dir.clone(),
        executable.clone(),
        LINUX_PROXY_SCRIPT,
        true,
    ));
    let mut arguments = vec![mode.to_string()];
    if let Some((host, port, exceptions)) = manual {
        arguments.push(host.to_string());
        arguments.push(port.to_string());
        arguments.push(exceptions.to_string());
    }

    ScriptInvocation {
        executable,
        arguments,
        generated_script,
    }
}

pub(super) fn run_script(
    runner: &dyn ProcessRunner,
    script: &ScriptInvocation,
) -> Result<(), SystemProxyError> {
    let mut spawn = ProcessSpawn::new(ProcessRole::SysProxy, &script.executable)
        .with_arguments(script.arguments.clone());
    if let Some(generated_script) = script.generated_script.clone() {
        spawn = spawn.with_generated_script(generated_script);
    }
    ensure_success(runner.run_oneshot(spawn)?, "system proxy script")
}

pub(super) fn apply_windows_clear(runner: &dyn ProcessRunner) -> Result<(), SystemProxyError> {
    apply_windows_proxy(
        runner,
        &WindowsProxySettings {
            proxy: String::new(),
            exceptions: String::new(),
            option_type: WindowsProxyOption::Direct,
        },
    )
}

pub(super) fn apply_windows_proxy(
    runner: &dyn ProcessRunner,
    settings: &WindowsProxySettings,
) -> Result<(), SystemProxyError> {
    for command in windows_registry_commands(settings) {
        ensure_success(
            runner.run_oneshot(command)?,
            "windows registry proxy command",
        )?;
    }
    refresh_windows_internet_settings();
    Ok(())
}

fn windows_registry_commands(settings: &WindowsProxySettings) -> Vec<ProcessSpawn> {
    match settings.option_type {
        WindowsProxyOption::Direct => vec![
            registry_set_dword("ProxyEnable", 0),
            registry_set_string("ProxyServer", ""),
            registry_set_string("ProxyOverride", ""),
            registry_set_string("AutoConfigURL", ""),
        ],
        WindowsProxyOption::NamedProxy => vec![
            registry_set_dword("ProxyEnable", 1),
            registry_set_string("ProxyServer", &settings.proxy),
            registry_set_string("ProxyOverride", &settings.exceptions),
            registry_set_string("AutoConfigURL", ""),
        ],
    }
}

fn registry_set_dword(name: &str, value: u32) -> ProcessSpawn {
    registry_set(name, "REG_DWORD", &value.to_string())
}

fn registry_set_string(name: &str, value: &str) -> ProcessSpawn {
    registry_set(name, "REG_SZ", value)
}

fn registry_set(name: &str, value_type: &str, value: &str) -> ProcessSpawn {
    ProcessSpawn::new(ProcessRole::SysProxy, "reg").with_arguments(reg_add_arguments(
        WINDOWS_INTERNET_SETTINGS_REG_PATH,
        name,
        value_type,
        value,
    ))
}

#[cfg(windows)]
fn refresh_windows_internet_settings() {
    use std::ffi::c_void;

    const INTERNET_OPTION_REFRESH: u32 = 37;
    const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;

    // `InternetSetOptionW` lives in wininet.dll, which is not one of the default
    // libraries the GNU/mingw linker pulls in (unlike kernel32). Without an
    // explicit link directive the test/binary link step fails with an undefined
    // reference. kernel32-only extern blocks elsewhere need no such attribute.
    #[link(name = "wininet")]
    extern "system" {
        fn InternetSetOptionW(
            internet: *mut c_void,
            option: u32,
            buffer: *mut c_void,
            buffer_length: u32,
        ) -> i32;
    }

    // SAFETY: both WinINet calls explicitly accept null handles and buffers for
    // these notification-only option values; no returned pointer is retained.
    unsafe {
        let _ = InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        let _ = InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
    }
}

#[cfg(not(windows))]
fn refresh_windows_internet_settings() {}

fn ensure_success(output: ProcessOutput, context: &'static str) -> Result<(), SystemProxyError> {
    if output.success() {
        Ok(())
    } else {
        Err(SystemProxyError::CommandFailed {
            context,
            status_code: output.status_code,
            stderr: output.stderr,
        })
    }
}

pub(super) const LINUX_PROXY_SCRIPT: &str = r#"#!/bin/sh
mode="$1"
host="$2"
port="$3"
ignore_hosts="$4"
failed=0

array_from_csv() {
  if [ -z "$1" ]; then
    printf "[]"
    return 0
  fi
  old_ifs="$IFS"
  IFS=","
  result=""
  for value in $1; do
    trimmed="$(printf "%s" "$value" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
    if [ -n "$trimmed" ]; then
      if [ -n "$result" ]; then
        result="$result,"
      fi
      result="$result'$trimmed'"
    fi
  done
  IFS="$old_ifs"
  printf "[%s]" "$result"
}

# Record failures instead of letting the last command decide the exit status:
# without this the script reports whatever the final desktop helper returned.
run_step() {
  if "$@"; then
    return 0
  fi
  echo "voya-sysproxy: command failed: $*" >&2
  failed=1
  return 0
}

set_gnome() {
  if ! command -v gsettings >/dev/null 2>&1; then
    return 0
  fi
  # A desktop can ship gsettings without the GNOME proxy schemas; treat that as
  # "not a GNOME desktop" rather than as a failure we could not have avoided.
  if ! gsettings get org.gnome.system.proxy mode >/dev/null 2>&1; then
    return 0
  fi
  run_step gsettings set org.gnome.system.proxy mode "$mode"
  if [ "$mode" = "manual" ]; then
    for proto in http https ftp socks; do
      run_step gsettings set "org.gnome.system.proxy.$proto" host "$host"
      run_step gsettings set "org.gnome.system.proxy.$proto" port "$port"
    done
    run_step gsettings set org.gnome.system.proxy ignore-hosts "$(array_from_csv "$ignore_hosts")"
  fi
  return 0
}

set_kde() {
  if command -v kwriteconfig6 >/dev/null 2>&1; then
    kwriteconfig=kwriteconfig6
  elif command -v kwriteconfig5 >/dev/null 2>&1; then
    kwriteconfig=kwriteconfig5
  else
    return 0
  fi
  if [ "$mode" = "manual" ]; then
    run_step "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key ProxyType 1
    run_step "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key httpProxy "http://$host:$port"
    run_step "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key httpsProxy "http://$host:$port"
    run_step "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key ftpProxy "http://$host:$port"
    run_step "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key socksProxy "http://$host:$port"
    run_step "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key NoProxyFor "$ignore_hosts"
  else
    run_step "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key ProxyType 0
  fi
  dbus-send --type=signal /KIO/Scheduler org.kde.KIO.Scheduler.reparseSlaveConfiguration string:"" >/dev/null 2>&1 || true
  return 0
}

if [ "$mode" != "manual" ] && [ "$mode" != "none" ]; then
  echo "Usage: $0 manual <host> <port> <ignore_hosts> | none" >&2
  exit 1
fi

set_gnome
set_kde
exit "$failed"
"#;

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
    };

    #[cfg(unix)]
    use super::LINUX_PROXY_SCRIPT;

    /// Absolute path so the tests keep working with `PATH` reduced to stubs.
    #[cfg(unix)]
    const SHELL: &str = "/bin/sh";

    #[cfg(unix)]
    #[test]
    fn sysproxy_managed_scripts_are_valid_posix_shell() {
        let root = temp_root("shell-syntax");
        {
            let (name, contents) = ("proxy_set_linux.sh", LINUX_PROXY_SCRIPT);
            let script = root.join(name);
            write_script(&script, contents);
            let output = Command::new(SHELL)
                .arg("-n")
                .arg(&script)
                .output()
                .expect("run sh -n");

            assert!(
                output.status.success(),
                "{name} is not valid POSIX shell: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn sysproxy_linux_script_succeeds_on_a_gnome_only_desktop() {
        let root = temp_root("linux-gnome-only");
        let stub_dir = root.join("bin");
        fs::create_dir_all(&stub_dir).expect("create stub directory");
        link_system_tool(&stub_dir, "sed");
        let log = root.join("gsettings.log");
        write_script(
            &stub_dir.join("gsettings"),
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
                log.display()
            ),
        );
        let script = root.join("proxy_set_linux.sh");
        write_script(&script, LINUX_PROXY_SCRIPT);

        for arguments in [
            vec!["manual", "127.0.0.1", "10808", "localhost,127.0.0.0/8"],
            vec!["none"],
        ] {
            let output = run_with_stubs(&script, &stub_dir, &arguments);

            assert!(
                output.status.success(),
                "{arguments:?} must succeed without any KDE tool installed, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let calls = fs::read_to_string(&log).expect("read gsettings calls");
        assert!(calls.contains("set org.gnome.system.proxy mode manual"));
        assert!(calls.contains("set org.gnome.system.proxy mode none"));
        assert!(calls.contains("ignore-hosts ['localhost','127.0.0.0/8']"));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn sysproxy_linux_script_reports_a_failed_gsettings_call() {
        let root = temp_root("linux-gsettings-failure");
        let stub_dir = root.join("bin");
        fs::create_dir_all(&stub_dir).expect("create stub directory");
        link_system_tool(&stub_dir, "sed");
        write_script(
            &stub_dir.join("gsettings"),
            "#!/bin/sh\nif [ \"$1\" = \"get\" ]; then exit 0; fi\nif [ \"$2\" = \"org.gnome.system.proxy.socks\" ]; then exit 1; fi\nexit 0\n",
        );
        let script = root.join("proxy_set_linux.sh");
        write_script(&script, LINUX_PROXY_SCRIPT);

        let output = run_with_stubs(
            &script,
            &stub_dir,
            &["manual", "127.0.0.1", "10808", "localhost"],
        );

        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("command failed"));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    fn run_with_stubs(script: &Path, stub_dir: &Path, arguments: &[&str]) -> Output {
        Command::new(SHELL)
            .arg(script)
            .args(arguments)
            .env("PATH", stub_dir)
            .output()
            .expect("run managed script")
    }

    /// Symlink a real system tool into the stub directory so the script can run
    /// with `PATH` restricted to stubs.
    #[cfg(unix)]
    fn link_system_tool(stub_dir: &Path, name: &str) {
        for directory in ["/usr/bin", "/bin", "/usr/local/bin"] {
            let candidate = Path::new(directory).join(name);
            if candidate.exists() {
                std::os::unix::fs::symlink(candidate, stub_dir.join(name)).expect("link tool");
                return;
            }
        }
        panic!("{name} is required to run the managed system proxy scripts");
    }

    #[cfg(unix)]
    fn write_script(path: &Path, contents: &str) {
        use std::os::unix::fs::PermissionsExt;

        fs::write(path, contents).expect("write script");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod script");
    }

    #[cfg(unix)]
    fn temp_root(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "voyavpn-sysproxy-script-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
