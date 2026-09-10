use super::*;
use crate::test_support::RecordingRunner;

struct Observer(SystemProxyObservation);
impl SystemProxyObserver for Observer {
    fn observe(&self) -> SystemProxyObservation {
        self.0
    }
}

#[derive(Default)]
struct Pac {
    fail: AtomicBool,
}
impl PacManager for Pac {
    fn start(&self, _config: PacStartConfig) -> Result<(), SystemProxyError> {
        if self.fail.load(Ordering::Relaxed) {
            Err(SystemProxyError::InvalidPort(0))
        } else {
            Ok(())
        }
    }
    fn stop(&self) {}
    fn is_supported(&self) -> bool {
        true
    }
    fn is_running(&self) -> bool {
        !self.fail.load(Ordering::Relaxed)
    }
}

fn request(mode: SysProxyType) -> SystemProxyRequest {
    SystemProxyRequest {
        target_os: TargetOs::Macos,
        item: SystemProxyItem {
            sys_proxy_type: mode,
            custom_system_proxy_script_path: Some("/must/not/execute".into()),
            ..SystemProxyItem::default()
        },
        force_disable: false,
        socks_port: 10808,
        pac_port: 10811,
        config_dir: "/unused".into(),
        script_dir: "/must/not/write".into(),
        pac_url_nonce: "first".into(),
    }
}

#[test]
fn every_macos_mode_is_manual_and_never_executes_a_script() {
    let runner = Arc::new(RecordingRunner::default());
    let service = SystemProxyService::new(runner.clone(), Arc::new(Pac::default()))
        .with_observer(Arc::new(Observer(SystemProxyObservation::Unknown)));
    for mode in [
        SysProxyType::ForcedChange,
        SysProxyType::ForcedClear,
        SysProxyType::Pac,
        SysProxyType::Unchanged,
    ] {
        let mut req = request(mode);
        for disable in [false, true] {
            req.force_disable = disable;
            let status = service.apply(&req).expect("manual operation");
            assert_eq!(status.management, SystemProxyManagement::Manual);
            assert_eq!(status.effective_type, SysProxyType::Unchanged);
            assert_eq!(status.observation, SystemProxyObservation::Unknown);
            if disable {
                assert!(status.proxy.is_none());
                assert!(status.pac_url.is_none());
            }
        }
    }
    assert!(runner.oneshots().is_empty());
}

#[test]
fn pac_address_exists_only_after_success_and_stays_stable_until_stopped() {
    let pac = Arc::new(Pac::default());
    let service = SystemProxyService::new(Arc::new(RecordingRunner::default()), pac.clone());
    let mut req = request(SysProxyType::Pac);
    assert!(service.status(&req).expect("status").pac_url.is_none());
    let url = service.apply(&req).expect("start").pac_url;
    assert!(url.is_some());
    req.pac_url_nonce = "second".into();
    assert_eq!(service.status(&req).expect("status").pac_url, url);
    assert_eq!(service.apply(&req).expect("reapply").pac_url, url);
    pac.fail.store(true, Ordering::Relaxed);
    assert!(service
        .status(&req)
        .expect("dead listener status")
        .pac_url
        .is_none());
    assert!(service.apply(&req).is_err());
    assert!(service
        .status(&req)
        .expect("failed status")
        .pac_url
        .is_none());
    service.stop_pac();
    assert!(service
        .status(&req)
        .expect("stopped status")
        .proxy
        .is_none());
}

#[test]
fn automatic_pac_status_cannot_resurrect_an_unavailable_listener() {
    let pac = Arc::new(Pac::default());
    let service = SystemProxyService::new(Arc::new(RecordingRunner::default()), pac.clone());
    let mut req = request(SysProxyType::Pac);
    req.target_os = TargetOs::Windows;
    assert!(service.status(&req).expect("running PAC").pac_url.is_some());
    pac.fail.store(true, Ordering::Relaxed);
    let status = service.status(&req).expect("failed PAC");
    assert_eq!(status.pac_url, None);
    assert_eq!(status.proxy, None);
}

#[test]
fn observations_preserve_unknown_and_detect_all_local_proxy_flavors() {
    assert_eq!(parse_observation("null"), SystemProxyObservation::Unknown);
    assert_eq!(
        parse_observation(r#"{"known":false,"servers":[],"pacURLs":[]}"#),
        SystemProxyObservation::Unknown
    );
    assert_eq!(
        parse_observation(r#"{"known":true,"servers":[],"pacURLs":[]}"#),
        SystemProxyObservation::Clear
    );
    assert_eq!(
        parse_observation(r#"{"known":true,"servers":["proxy.example"],"pacURLs":[]}"#),
        SystemProxyObservation::OtherProxy
    );
    for host in [
        "127.0.0.1",
        "127.0.0.2",
        "localhost",
        "::1",
        "127.1",
        "::ffff:127.0.0.1",
        "[::1]",
    ] {
        let json = serde_json::json!({"known":true,"servers":[host],"pacURLs":[]}).to_string();
        assert_eq!(parse_observation(&json), SystemProxyObservation::LocalProxy);
    }
    for url in [
        "http://127.0.0.1:10811/pac?t=old",
        "http://[::1]:9090/pac",
        "http://LOCALHOST/pac",
        "https://user:secret@localhost.:9090/pac",
        "http://127.1:10811/pac",
        "http://[::ffff:127.0.0.1]:10811/pac",
        "http://%6cocalhost:10811/pac",
        "http://2130706433/pac",
    ] {
        let json = serde_json::json!({"known":true,"servers":[],"pacURLs":[url]}).to_string();
        assert_eq!(parse_observation(&json), SystemProxyObservation::LocalProxy);
    }
    for url in [
        "broken-url",
        "http:///pac",
        "http://[::1/pac",
        "file:///proxy.pac",
        "http://proxy.example:invalid/pac",
        "http://localhost:99999/pac",
        "http://[not-ipv6]/pac",
    ] {
        let json = serde_json::json!({"known":true,"servers":[],"pacURLs":[url]}).to_string();
        assert_eq!(parse_observation(&json), SystemProxyObservation::Unknown);
    }
    assert_eq!(
        parse_observation(r#"{"known":false,"servers":["127.1"],"pacURLs":[]}"#),
        SystemProxyObservation::LocalProxy
    );
    assert_eq!(
        parse_observation(r#"{"known":false,"servers":["proxy.example"],"pacURLs":[]}"#),
        SystemProxyObservation::Unknown
    );
    assert_eq!(
        parse_observation(
            r#"{"known":true,"servers":[],"pacURLs":["https://proxy.example:8080/pac"]}"#
        ),
        SystemProxyObservation::OtherProxy
    );
}
