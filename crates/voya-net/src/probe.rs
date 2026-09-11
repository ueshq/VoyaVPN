use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use thiserror::Error;
use tokio::{net::TcpStream, time};

const LOOPBACK_ADDR: &str = "127.0.0.1";

mod country;
pub use country::{IpLookupResult, DEFAULT_IP_LOOKUP_URL};

pub type CancellationFlag = Arc<AtomicBool>;
pub type Result<T> = std::result::Result<T, NetworkProbeError>;

#[derive(Debug, Error)]
pub enum NetworkProbeError {
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("network probe was cancelled")]
    Cancelled,
}

#[derive(Clone)]
pub struct SocksHttpProbe {
    client: reqwest::Client,
}

impl SocksHttpProbe {
    pub fn new(socks_port: u16) -> Result<Self> {
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::all(format!(
                "socks5h://{LOOPBACK_ADDR}:{socks_port}"
            ))?)
            .build()?;
        Ok(Self { client })
    }

    pub async fn best_latency(
        &self,
        url: &str,
        timeout: Duration,
        attempts: usize,
        cancel: &CancellationFlag,
    ) -> Result<i32> {
        let mut best_delay = None;
        let mut last_error = None;
        for attempt in 0..attempts.max(1) {
            check_cancelled(cancel)?;
            let started = Instant::now();
            match self.client.get(url).timeout(timeout).send().await {
                Ok(response) => match response.error_for_status() {
                    Ok(_) => {
                        let delay = millis_i32(started.elapsed());
                        best_delay =
                            Some(best_delay.map_or(delay, |current: i32| current.min(delay)));
                    }
                    Err(error) => last_error = Some(error),
                },
                Err(error) => last_error = Some(error),
            }
            if attempt + 1 < attempts.max(1) {
                time::sleep(Duration::from_millis(100)).await;
            }
        }
        check_cancelled(cancel)?;
        match best_delay {
            Some(delay) => Ok(delay),
            None => Err(last_error.map_or(NetworkProbeError::Cancelled, NetworkProbeError::Http)),
        }
    }

    pub async fn optional_text(&self, url: &str, timeout: Duration) -> Option<String> {
        self.client
            .get(url)
            .timeout(timeout)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .text()
            .await
            .ok()
            .filter(|value| !value.trim().is_empty())
    }
}

pub async fn tcp_port_is_open(host: &str, port: u16) -> bool {
    TcpStream::connect((host, port)).await.is_ok()
}

fn check_cancelled(cancel: &CancellationFlag) -> Result<()> {
    if is_cancelled(cancel) {
        Err(NetworkProbeError::Cancelled)
    } else {
        Ok(())
    }
}

fn is_cancelled(cancel: &CancellationFlag) -> bool {
    cancel.load(Ordering::SeqCst)
}

fn millis_i32(duration: Duration) -> i32 {
    i32::try_from(duration.as_millis()).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, net::Ipv4Addr, sync::Arc};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };

    use super::*;
    use crate::download::test_support::{
        spawn_http_fixture, spawn_raw_http_fixture, RawFixtureResponse,
    };

    const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

    fn not_cancelled() -> CancellationFlag {
        Arc::new(AtomicBool::new(false))
    }

    fn cancelled() -> CancellationFlag {
        Arc::new(AtomicBool::new(true))
    }

    /// Minimal SOCKS5 CONNECT relay so the probes exercise the real proxied client instead of a
    /// hand-built one. It answers no-auth, dials the requested target and copies both ways.
    async fn spawn_socks5_relay() -> u16 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("SOCKS5 relay should bind");
        let port = listener.local_addr().expect("SOCKS5 relay address").port();

        tokio::spawn(async move {
            while let Ok((mut client, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut greeting = [0; 2];
                    if client.read_exact(&mut greeting).await.is_err() {
                        return;
                    }
                    let mut methods = vec![0; usize::from(greeting[1])];
                    if client.read_exact(&mut methods).await.is_err() {
                        return;
                    }
                    if client.write_all(&[0x05, 0x00]).await.is_err() {
                        return;
                    }

                    let mut header = [0; 4];
                    if client.read_exact(&mut header).await.is_err() {
                        return;
                    }
                    let Some(target) = read_socks5_target(&mut client, header[3]).await else {
                        return;
                    };
                    let Ok(mut upstream) = TcpStream::connect(target.as_str()).await else {
                        let _ = client
                            .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                            .await;
                        return;
                    };
                    if client
                        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                        .await
                        .is_err()
                    {
                        return;
                    }
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
                });
            }
        });

        port
    }

    async fn read_socks5_target(client: &mut TcpStream, address_type: u8) -> Option<String> {
        let host = match address_type {
            0x01 => {
                let mut address = [0; 4];
                client.read_exact(&mut address).await.ok()?;
                Ipv4Addr::from(address).to_string()
            }
            0x03 => {
                let mut length = [0; 1];
                client.read_exact(&mut length).await.ok()?;
                let mut domain = vec![0; usize::from(length[0])];
                client.read_exact(&mut domain).await.ok()?;
                String::from_utf8(domain).ok()?
            }
            _ => return None,
        };
        let mut port = [0; 2];
        client.read_exact(&mut port).await.ok()?;

        Some(format!("{host}:{}", u16::from_be_bytes(port)))
    }

    #[tokio::test]
    async fn tcp_port_is_open_follows_the_listener() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("probe listener should bind");
        let port = listener.local_addr().expect("listener address").port();
        assert!(tcp_port_is_open("127.0.0.1", port).await);

        drop(listener);
        assert!(!tcp_port_is_open("127.0.0.1", port).await);
    }

    #[tokio::test]
    async fn best_latency_keeps_the_fastest_attempt_and_surfaces_http_failures() {
        let socks_port = spawn_socks5_relay().await;
        let base = spawn_http_fixture(
            HashMap::from([("/ping".to_string(), "pong".to_string())]),
            2,
            Arc::new(Mutex::new(Vec::new())),
        )
        .await;
        let probe = SocksHttpProbe::new(socks_port).expect("probe client should build");

        let delay = probe
            .best_latency(&format!("{base}/ping"), PROBE_TIMEOUT, 2, &not_cancelled())
            .await
            .expect("two healthy attempts should produce a delay");
        assert!(delay >= 0, "{delay}");

        let failing = spawn_raw_http_fixture(
            HashMap::from([(
                "/ping".to_string(),
                RawFixtureResponse {
                    status: "500 Internal Server Error".to_string(),
                    content_length: Some(5),
                    extra_headers: Vec::new(),
                    body: b"boom!".to_vec(),
                },
            )]),
            2,
        )
        .await;
        let error = probe
            .best_latency(
                &format!("{failing}/ping"),
                PROBE_TIMEOUT,
                2,
                &not_cancelled(),
            )
            .await
            .expect_err("a 500 on every attempt should fail");
        assert!(matches!(error, NetworkProbeError::Http(_)), "{error:?}");

        let error = probe
            .best_latency(&format!("{base}/ping"), PROBE_TIMEOUT, 2, &cancelled())
            .await
            .expect_err("a cancelled probe should not request anything");
        assert!(matches!(error, NetworkProbeError::Cancelled), "{error:?}");
    }

    #[tokio::test]
    async fn optional_text_returns_none_for_error_statuses() {
        let socks_port = spawn_socks5_relay().await;
        let base = spawn_http_fixture(
            HashMap::from([("/ip".to_string(), "203.0.113.7".to_string())]),
            2,
            Arc::new(Mutex::new(Vec::new())),
        )
        .await;
        let probe = SocksHttpProbe::new(socks_port).expect("probe client should build");

        assert_eq!(
            probe
                .optional_text(&format!("{base}/ip"), PROBE_TIMEOUT)
                .await,
            Some("203.0.113.7".to_string())
        );
        assert_eq!(
            probe
                .optional_text(&format!("{base}/missing"), PROBE_TIMEOUT)
                .await,
            None
        );
    }

    // A SOCKS server that answers for a deliberately unresolvable host. Each
    // instance represents a different exit; a direct/local-DNS request fails.
    async fn country_exit(
        body: &'static str,
        status: &'static str,
        stall: bool,
    ) -> (u16, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind exit");
        let port = listener.local_addr().expect("exit address").port();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept SOCKS");
            let mut greeting = [0; 2];
            socket.read_exact(&mut greeting).await.expect("greeting");
            let mut methods = vec![0; usize::from(greeting[1])];
            socket.read_exact(&mut methods).await.expect("methods");
            socket.write_all(&[5, 0]).await.expect("no auth");
            let mut header = [0; 4];
            socket.read_exact(&mut header).await.expect("connect");
            assert_eq!(header[3], 3, "hostname must be resolved by the proxy");
            let target = read_socks5_target(&mut socket, header[3])
                .await
                .expect("target");
            socket
                .write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
                .await
                .expect("connected");
            let mut request = [0; 4096];
            let bytes_read = socket.read(&mut request).await.expect("HTTP request");
            assert!(bytes_read > 0, "the proxy must receive an HTTP request");
            if stall {
                // Wait for cancellation/timeout to close the client socket.
                let mut rest = Vec::new();
                socket.read_to_end(&mut rest).await.expect("client closed");
            } else {
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("response");
            }
            target
        });
        (port, task)
    }

    #[tokio::test]
    async fn country_lookup_uses_each_exit_and_remote_dns() {
        for (body, expected) in [
            (r#"{"success":true,"country_code":"US"}"#, "US"),
            ("JP", "JP"),
        ] {
            let (port, server) = country_exit(body, "200 OK", false).await;
            let result = SocksHttpProbe::new(port)
                .expect("probe")
                .lookup_country(
                    "http://geo.invalid/location",
                    PROBE_TIMEOUT,
                    &not_cancelled(),
                )
                .await
                .expect("lookup");
            assert_eq!(result.country_code.as_deref(), Some(expected));
            assert_eq!(result.text, body, "keep custom IP information intact");
            assert_eq!(server.await.expect("server"), "geo.invalid:80");
        }
    }

    #[tokio::test]
    async fn country_lookup_handles_rate_limits_timeouts_and_cancellation() {
        let (port, server) = country_exit("limited", "429 Too Many Requests", false).await;
        assert!(SocksHttpProbe::new(port)
            .expect("probe")
            .lookup_country("http://geo.invalid/", PROBE_TIMEOUT, &not_cancelled())
            .await
            .is_none());
        server.await.expect("one request, no retry");

        let (port, server) = country_exit("", "200 OK", true).await;
        assert!(SocksHttpProbe::new(port)
            .expect("probe")
            .lookup_country(
                "http://geo.invalid/",
                Duration::from_millis(50),
                &not_cancelled()
            )
            .await
            .is_none());
        time::timeout(Duration::from_secs(1), server)
            .await
            .expect("timeout closes socket")
            .expect("server");

        let (port, server) = country_exit("", "200 OK", true).await;
        let cancel = not_cancelled();
        let signal = cancel.clone();
        let cancel_task = tokio::spawn(async move {
            time::sleep(Duration::from_millis(100)).await;
            signal.store(true, Ordering::SeqCst);
        });
        let probe = SocksHttpProbe::new(port).expect("probe");
        assert!(time::timeout(
            Duration::from_secs(1),
            probe.lookup_country("http://geo.invalid/", PROBE_TIMEOUT, &cancel)
        )
        .await
        .expect("cancel promptly")
        .is_none());
        cancel_task.await.expect("cancel task");
        server.await.expect("cancel closes socket");
        assert!(probe
            .lookup_country("", PROBE_TIMEOUT, &cancel)
            .await
            .is_none());
    }
}
