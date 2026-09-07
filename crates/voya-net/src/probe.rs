use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use thiserror::Error;
use tokio::{
    net::{lookup_host, TcpStream},
    time,
};

const LOOPBACK_ADDR: &str = "127.0.0.1";

pub type CancellationFlag = Arc<AtomicBool>;
pub type Result<T> = std::result::Result<T, NetworkProbeError>;

#[derive(Debug, Error)]
pub enum NetworkProbeError {
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("network probe was cancelled")]
    Cancelled,
    #[error("network probe port {0} is outside the valid range")]
    InvalidPort(i32),
    #[error("network probe request timed out")]
    Timeout,
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

    /// Measures throughput in bytes per second: the best full-second window, or the overall
    /// average when the body finishes before a window completes.
    ///
    /// A body still arriving when `timeout` expires counts as a normal completion and reports the
    /// bytes measured so far — that is the point of a fixed-duration speed test. Only a response
    /// whose *headers* never arrive in time yields [`NetworkProbeError::Timeout`], which is why
    /// this differs from [`tcp_connect_delay`], where a timeout is `Ok(-1)`.
    pub async fn download_speed(
        &self,
        url: &str,
        timeout: Duration,
        cancel: &CancellationFlag,
    ) -> Result<f64> {
        check_cancelled(cancel)?;
        let started = Instant::now();
        let mut response = time::timeout(timeout, self.client.get(url).send())
            .await
            .map_err(|_| NetworkProbeError::Timeout)??
            .error_for_status()?;
        let mut total_bytes = 0_u64;
        let mut window_bytes = 0_u64;
        let mut window_started = Instant::now();
        let mut max_speed = 0.0_f64;
        let deadline = started + timeout;

        loop {
            check_cancelled(cancel)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let chunk = match time::timeout(remaining, response.chunk()).await {
                Ok(Ok(Some(chunk))) => chunk,
                Ok(Ok(None)) | Err(_) => break,
                Ok(Err(error)) => return Err(error.into()),
            };
            let len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
            total_bytes = total_bytes.saturating_add(len);
            window_bytes = window_bytes.saturating_add(len);
            let elapsed = window_started.elapsed().as_secs_f64();
            if elapsed >= 1.0 {
                max_speed = max_speed.max(window_bytes as f64 / elapsed);
                window_started = Instant::now();
                window_bytes = 0;
            }
        }
        check_cancelled(cancel)?;
        Ok(max_speed.max(total_bytes as f64 / started.elapsed().as_secs_f64().max(0.001)))
    }
}

/// Measures how long a TCP handshake to `host:port` takes, in milliseconds.
///
/// The result deliberately distinguishes three outcomes, because the speed-test table renders
/// them differently: a completed handshake yields the elapsed milliseconds, an *unreachable but
/// answering* endpoint (`Ok(-1)`) covers the cases where the peer never replies at all — a host
/// that resolves to nothing, and a handshake that outlives `timeout` — and a peer that actively
/// refuses or errors yields `Err`, which carries the reason to the user. Unlike
/// [`SocksHttpProbe::download_speed`], a timeout here is *not* an error.
pub async fn tcp_connect_delay(
    host: &str,
    port: i32,
    timeout: Duration,
    cancel: &CancellationFlag,
) -> Result<i32> {
    check_cancelled(cancel)?;
    let port = u16::try_from(port).map_err(|_| NetworkProbeError::InvalidPort(port))?;
    let mut addresses = lookup_host((host, port)).await?;
    let Some(address) = addresses.next() else {
        return Ok(-1);
    };
    connect_address_delay(address, timeout, cancel).await
}

pub async fn tcp_port_is_open(host: &str, port: u16) -> bool {
    TcpStream::connect((host, port)).await.is_ok()
}

/// Measure a TCP handshake, mapping "did not answer in time" to `-1`.
///
/// A budget that elapses is `Ok(-1)`, not an error: the speed-test table renders
/// an unreachable endpoint as -1, while a refused or malformed target is a real
/// error the caller surfaces. That timeout branch has no unit test on purpose —
/// reaching it needs a connect that stays pending, which cannot be arranged
/// deterministically here: loopback completes before the deadline is checked,
/// paused time does not advance while the IO driver is parked, and with a
/// tunnel up even RFC 5737 documentation space is routed and answered.
async fn connect_address_delay(
    address: SocketAddr,
    timeout: Duration,
    cancel: &CancellationFlag,
) -> Result<i32> {
    let started = Instant::now();
    let result = time::timeout(timeout, TcpStream::connect(address)).await;
    check_cancelled(cancel)?;
    match result {
        Ok(Ok(_)) => Ok(millis_i32(started.elapsed())),
        Ok(Err(error)) => Err(error.into()),
        Err(_) => Ok(-1),
    }
}

fn check_cancelled(cancel: &CancellationFlag) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(NetworkProbeError::Cancelled)
    } else {
        Ok(())
    }
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
        spawn_dripping_http_fixture, spawn_http_fixture, spawn_raw_http_fixture, RawFixtureResponse,
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
    async fn tcp_connect_delay_separates_reachable_refused_and_invalid_ports() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("probe listener should bind");
        let open_port = listener.local_addr().expect("listener address").port();

        let delay = tcp_connect_delay(
            &Ipv4Addr::LOCALHOST.to_string(),
            i32::from(open_port),
            PROBE_TIMEOUT,
            &not_cancelled(),
        )
        .await
        .expect("an open port should measure a delay");
        assert!(delay >= 0, "{delay}");

        let closed = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("probe listener should bind");
        let closed_port = closed.local_addr().expect("listener address").port();
        drop(closed);
        let error = tcp_connect_delay(
            &Ipv4Addr::LOCALHOST.to_string(),
            i32::from(closed_port),
            PROBE_TIMEOUT,
            &not_cancelled(),
        )
        .await
        .expect_err("a refused connection should be reported, not folded into -1");
        assert!(matches!(error, NetworkProbeError::Io(_)), "{error:?}");

        let error = tcp_connect_delay("127.0.0.1", 70_000, PROBE_TIMEOUT, &not_cancelled())
            .await
            .expect_err("a port above the valid range should fail");
        assert!(
            matches!(error, NetworkProbeError::InvalidPort(70_000)),
            "{error:?}"
        );

        let error = tcp_connect_delay("127.0.0.1", 80, PROBE_TIMEOUT, &cancelled())
            .await
            .expect_err("a cancelled probe should not connect");
        assert!(matches!(error, NetworkProbeError::Cancelled), "{error:?}");
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

    #[tokio::test]
    async fn download_speed_reports_the_average_for_a_short_body() {
        let socks_port = spawn_socks5_relay().await;
        let base = spawn_http_fixture(
            HashMap::from([("/payload".to_string(), "x".repeat(64 * 1024))]),
            1,
            Arc::new(Mutex::new(Vec::new())),
        )
        .await;
        let probe = SocksHttpProbe::new(socks_port).expect("probe client should build");

        let speed = probe
            .download_speed(&format!("{base}/payload"), PROBE_TIMEOUT, &not_cancelled())
            .await
            .expect("a completed body should report a speed");

        assert!(speed > 0.0, "{speed}");
    }

    /// A body still arriving when the budget expires is a normal end to a fixed-duration speed
    /// test, so the bytes already measured are reported instead of an error.
    #[tokio::test]
    async fn download_speed_reports_partial_progress_when_the_budget_expires() {
        let socks_port = spawn_socks5_relay().await;
        let base =
            spawn_dripping_http_fixture(200, 200, &[b'x'; 1024], Duration::from_millis(10)).await;
        let probe = SocksHttpProbe::new(socks_port).expect("probe client should build");

        let speed = probe
            .download_speed(
                &format!("{base}/payload"),
                Duration::from_millis(500),
                &not_cancelled(),
            )
            .await
            .expect("an unfinished transfer should still report throughput");

        assert!(speed > 0.0, "{speed}");
    }

    /// Only a response whose headers never arrive is a timeout, and it is the one download case
    /// that is an error rather than a measurement.
    #[tokio::test]
    async fn download_speed_maps_a_silent_server_to_a_timeout() {
        let socks_port = spawn_socks5_relay().await;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("silent server should bind");
        let address = listener.local_addr().expect("silent server address");
        tokio::spawn(async move {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            time::sleep(Duration::from_secs(30)).await;
            drop(socket);
        });
        let probe = SocksHttpProbe::new(socks_port).expect("probe client should build");

        let error = probe
            .download_speed(
                &format!("http://{address}/payload"),
                Duration::from_millis(200),
                &not_cancelled(),
            )
            .await
            .expect_err("a server that never answers should time out");

        assert!(matches!(error, NetworkProbeError::Timeout), "{error:?}");
    }
}
