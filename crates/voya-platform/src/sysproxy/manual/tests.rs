use super::*;
use crate::test_support::RecordingRunner;

struct Observer(SystemProxyObservation);
impl SystemProxyObserver for Observer {
    fn observe(&self) -> SystemProxyObservation {
        self.0
    }
}

fn request(mode: SysProxyType) -> SystemProxyRequest {
    SystemProxyRequest {
        target_os: TargetOs::Macos,
        item: SystemProxyItem {
            sys_proxy_type: mode,
            ..SystemProxyItem::default()
        },
        force_disable: false,
        socks_port: 10808,
        script_dir: "/must/not/write".into(),
    }
}

#[test]
fn every_macos_mode_is_manual_and_never_executes_a_script() {
    let runner = Arc::new(RecordingRunner::default());
    let service = SystemProxyService::new(runner.clone())
        .with_observer(Arc::new(Observer(SystemProxyObservation::Unknown)));
    for mode in [
        SysProxyType::ForcedChange,
        SysProxyType::ForcedClear,
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
            } else {
                assert_eq!(status.proxy.as_deref(), Some("127.0.0.1:10808"));
            }
        }
    }
    assert!(runner.oneshots().is_empty());
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
