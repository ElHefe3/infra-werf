//! Explicit, bounded network observations. These checks are independent of
//! SSH credentials and never persist their results.
use crate::{
    AtlasError, ErrorCode, Health, HostReachability, Reachability, ServerId, hostname,
    validation::text,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use ts_rs::TS;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const READ_POLL_INTERVAL: Duration = Duration::from_millis(100);
const MAX_INTERVAL_SECONDS: u16 = 3600;
const MAX_BACKOFF_SECONDS: u64 = 300;

fn failure(code: ErrorCode) -> AtlasError {
    AtlasError::new(code)
}
fn now() -> String {
    Utc::now().to_rfc3339()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProbeKind {
    Tcp,
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct HealthProbeConfig {
    pub server_id: ServerId,
    pub host: String,
    pub host_port: u16,
    pub service_port: u16,
    pub service_kind: ProbeKind,
    pub path: Option<String>,
    pub interval_seconds: u16,
}
impl HealthProbeConfig {
    pub fn validate(&self) -> Result<(), AtlasError> {
        let host = hostname(&self.host)?;
        if host != self.host
            || self.host_port == 0
            || self.service_port == 0
            || !(15..=MAX_INTERVAL_SECONDS).contains(&self.interval_seconds)
        {
            return Err(failure(ErrorCode::InvalidInput));
        }
        match self.service_kind {
            ProbeKind::Tcp if self.path.is_none() => Ok(()),
            ProbeKind::Http => {
                let path = self
                    .path
                    .as_deref()
                    .ok_or_else(|| failure(ErrorCode::InvalidInput))?;
                let path = text(path, 512, false)?;
                if !path.starts_with('/')
                    || path != self.path.as_deref().unwrap_or_default()
                    || !path.is_ascii()
                    || path
                        .bytes()
                        .any(|byte| byte <= b' ' || byte == 0x7f || byte == b'#')
                {
                    return Err(failure(ErrorCode::InvalidInput));
                }
                Ok(())
            }
            ProbeKind::Tcp => Err(failure(ErrorCode::InvalidInput)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ServiceHealth {
    pub state: Health,
    pub last_checked_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
}
impl Default for ServiceHealth {
    fn default() -> Self {
        Self {
            state: Health::Unknown,
            last_checked_at: None,
            last_success_at: None,
            last_failure_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct HealthSnapshot {
    pub contract_version: u32,
    pub server_id: ServerId,
    pub host: String,
    pub host_reachability: HostReachability,
    pub service: ServiceHealth,
    pub observed_at: Option<String>,
}
impl HealthSnapshot {
    pub fn unknown(config: &HealthProbeConfig) -> Self {
        Self {
            contract_version: crate::CONTRACT_VERSION,
            server_id: config.server_id.clone(),
            host: config.host.clone(),
            host_reachability: HostReachability::default(),
            service: ServiceHealth::default(),
            observed_at: None,
        }
    }
    pub fn is_fresh(&self, at: chrono::DateTime<Utc>) -> bool {
        self.observed_at.as_deref().is_some_and(|value| {
            crate::observation_time(value, at).is_ok_and(|time| {
                at.signed_duration_since(time).num_seconds() <= crate::STALE_AFTER_SECONDS
            })
        })
    }
}

fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, AtlasError> {
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|_| failure(ErrorCode::IoFailure))?
        .take(8)
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        Err(failure(ErrorCode::IoFailure))
    } else {
        Ok(addresses)
    }
}
fn connect(host: &str, port: u16, cancelled: &AtomicBool) -> Result<TcpStream, AtlasError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(failure(ErrorCode::Cancelled));
    }
    let addresses = resolve(host, port)?;
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let mut last_error = None;
    for address in addresses {
        if cancelled.load(Ordering::Acquire) {
            return Err(failure(ErrorCode::Cancelled));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match TcpStream::connect_timeout(&address, remaining) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    Err(failure(
        if last_error.is_some_and(|error| error.kind() == std::io::ErrorKind::TimedOut) {
            ErrorCode::TimedOut
        } else {
            ErrorCode::IoFailure
        },
    ))
}
fn http_health(
    host: &str,
    port: u16,
    path: &str,
    cancelled: &AtomicBool,
) -> Result<Health, AtlasError> {
    let mut stream = connect(host, port, cancelled)?;
    stream
        .set_write_timeout(Some(READ_TIMEOUT))
        .and_then(|_| stream.set_read_timeout(Some(READ_TIMEOUT)))
        .map_err(|_| failure(ErrorCode::IoFailure))?;
    let host_header = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_header}\r\nConnection: close\r\nUser-Agent: Atlas-Health/1\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|_| failure(ErrorCode::IoFailure))?;
    let mut line = Vec::with_capacity(128);
    let mut byte = [0u8; 1];
    let deadline = Instant::now() + READ_TIMEOUT;
    while line.len() < 256 {
        if cancelled.load(Ordering::Acquire) {
            return Err(failure(ErrorCode::Cancelled));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(failure(ErrorCode::TimedOut));
        }
        stream
            .set_read_timeout(Some(remaining.min(READ_POLL_INTERVAL)))
            .map_err(|_| failure(ErrorCode::IoFailure))?;
        let count = match stream.read(&mut byte) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(_) => return Err(failure(ErrorCode::IoFailure)),
        };
        if count == 0 || byte[0] == b'\n' {
            break;
        }
        if byte[0] != b'\r' {
            line.push(byte[0]);
        }
    }
    let line = std::str::from_utf8(&line).map_err(|_| failure(ErrorCode::IoFailure))?;
    let mut parts = line.split_ascii_whitespace();
    if !matches!(parts.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
        return Err(failure(ErrorCode::IoFailure));
    }
    let status: u16 = parts
        .next()
        .and_then(|value| value.parse().ok())
        .filter(|value| (100..=599).contains(value))
        .ok_or_else(|| failure(ErrorCode::IoFailure))?;
    Ok(if (200..300).contains(&status) {
        Health::Healthy
    } else if (300..500).contains(&status) {
        Health::Degraded
    } else {
        Health::Unhealthy
    })
}

pub fn backoff_seconds(interval_seconds: u16, consecutive_failures: u8) -> u64 {
    let multiplier = 1u64 << consecutive_failures.min(5);
    (u64::from(interval_seconds) * multiplier).min(MAX_BACKOFF_SECONDS)
}

/// Run one host TCP and one configured service check. Failed observations are
/// explicit data; cancellation never manufactures a failure result.
pub fn probe_once(
    config: &HealthProbeConfig,
    cancelled: &AtomicBool,
) -> Result<HealthSnapshot, AtlasError> {
    config.validate()?;
    let checked_at = now();
    let host_success = connect(&config.host, config.host_port, cancelled).is_ok();
    if cancelled.load(Ordering::Acquire) {
        return Err(failure(ErrorCode::Cancelled));
    }
    let service_result = match config.service_kind {
        ProbeKind::Tcp => {
            connect(&config.host, config.service_port, cancelled).map(|_| Health::Healthy)
        }
        ProbeKind::Http => http_health(
            &config.host,
            config.service_port,
            config.path.as_deref().unwrap_or("/"),
            cancelled,
        ),
    };
    if cancelled.load(Ordering::Acquire) {
        return Err(failure(ErrorCode::Cancelled));
    }
    let finished_at = now();
    let service_state = service_result.unwrap_or(Health::Unhealthy);
    let service_ok = service_state == Health::Healthy;
    Ok(HealthSnapshot {
        contract_version: crate::CONTRACT_VERSION,
        server_id: config.server_id.clone(),
        host: config.host.clone(),
        host_reachability: HostReachability {
            state: if host_success {
                Reachability::Reachable
            } else {
                Reachability::Unreachable
            },
            last_checked_at: Some(checked_at.clone()),
            last_success_at: host_success.then(|| finished_at.clone()),
            last_failure_at: (!host_success).then(|| finished_at.clone()),
        },
        service: ServiceHealth {
            state: service_state,
            last_checked_at: Some(checked_at),
            last_success_at: service_ok.then(|| finished_at.clone()),
            last_failure_at: (!service_ok).then(|| finished_at.clone()),
        },
        observed_at: Some(finished_at),
    })
}

#[derive(Clone)]
pub struct SharedSnapshot(Arc<Mutex<HealthSnapshot>>);
impl SharedSnapshot {
    pub fn new(config: &HealthProbeConfig) -> Self {
        Self(Arc::new(Mutex::new(HealthSnapshot::unknown(config))))
    }
    pub fn get(&self) -> Result<HealthSnapshot, AtlasError> {
        self.0
            .lock()
            .map(|value| value.clone())
            .map_err(|_| failure(ErrorCode::IoFailure))
    }
    pub fn set(&self, value: HealthSnapshot) -> Result<(), AtlasError> {
        *self.0.lock().map_err(|_| failure(ErrorCode::IoFailure))? = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, sync::atomic::AtomicBool, thread};

    fn config(
        host_port: u16,
        service_port: u16,
        kind: ProbeKind,
        path: Option<&str>,
    ) -> HealthProbeConfig {
        HealthProbeConfig {
            server_id: ServerId::parse("fixture").unwrap(),
            host: "127.0.0.1".into(),
            host_port,
            service_port,
            service_kind: kind,
            path: path.map(str::to_owned),
            interval_seconds: 60,
        }
    }

    #[test]
    fn reachable_host_and_failed_service_stay_distinct() {
        let host = TcpListener::bind("127.0.0.1:0").unwrap();
        let service = TcpListener::bind("127.0.0.1:0").unwrap();
        let host_port = host.local_addr().unwrap().port();
        let service_port = service.local_addr().unwrap().port();
        let host_task = thread::spawn(move || drop(host.accept().unwrap()));
        let service_task = thread::spawn(move || {
            let (mut stream, _) = service.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 256];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });
        let result = probe_once(
            &config(host_port, service_port, ProbeKind::Http, Some("/health")),
            &AtomicBool::new(false),
        )
        .unwrap();
        host_task.join().unwrap();
        service_task.join().unwrap();
        assert_eq!(result.host_reachability.state, Reachability::Reachable);
        assert_eq!(result.service.state, Health::Unhealthy);
        assert!(result.host_reachability.last_success_at.is_some());
        assert!(result.service.last_failure_at.is_some());
    }

    #[test]
    fn unreachable_endpoint_is_not_unknown_or_a_success() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let result = probe_once(
            &config(port, port, ProbeKind::Tcp, None),
            &AtomicBool::new(false),
        )
        .unwrap();

        assert_eq!(result.host_reachability.state, Reachability::Unreachable);
        assert_eq!(result.service.state, Health::Unhealthy);
        assert!(result.host_reachability.last_checked_at.is_some());
        assert!(result.host_reachability.last_failure_at.is_some());
        assert!(result.host_reachability.last_success_at.is_none());
        assert!(result.service.last_failure_at.is_some());
        assert!(result.service.last_success_at.is_none());
    }

    #[test]
    fn successful_http_response_reports_healthy_service() {
        let host = TcpListener::bind("127.0.0.1:0").unwrap();
        let service = TcpListener::bind("127.0.0.1:0").unwrap();
        let host_port = host.local_addr().unwrap().port();
        let service_port = service.local_addr().unwrap().port();
        let host_task = thread::spawn(move || drop(host.accept().unwrap()));
        let service_task = thread::spawn(move || -> String {
            let (mut stream, _) = service.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 256];
            while request.len() < 2048 {
                let count = stream.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });

        let result = probe_once(
            &config(host_port, service_port, ProbeKind::Http, Some("/health")),
            &AtomicBool::new(false),
        )
        .unwrap();

        host_task.join().unwrap();
        let request = service_task.join().unwrap();
        assert_eq!(
            result.service.state,
            Health::Healthy,
            "request: {request:?}"
        );
        assert!(request.starts_with("GET /health HTTP/1.1\r\n"));
        assert!(request.contains(&format!("Host: 127.0.0.1:{service_port}\r\n")));
        assert!(result.service.last_success_at.is_some());
        assert!(result.service.last_failure_at.is_none());
    }

    #[test]
    fn cancellation_interrupts_a_stalled_http_status_read() {
        let service = TcpListener::bind("127.0.0.1:0").unwrap();
        let service_port = service.local_addr().unwrap().port();
        let (received, ready) = std::sync::mpsc::channel();
        let service_task = thread::spawn(move || {
            let (mut stream, _) = service.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 256];
            while request.len() < 2048 {
                let count = stream.read(&mut chunk).unwrap();
                if count == 0 {
                    return;
                }
                request.extend_from_slice(&chunk[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            received.send(()).unwrap();
            let mut byte = [0; 1];
            let _ = stream.read(&mut byte);
        });
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancelled.clone();
        let worker = thread::spawn(move || {
            http_health("127.0.0.1", service_port, "/health", &worker_cancel)
        });
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        cancelled.store(true, Ordering::Release);
        assert_eq!(
            worker.join().unwrap().unwrap_err().code,
            ErrorCode::Cancelled
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        service_task.join().unwrap();
    }

    #[test]
    fn old_observations_are_not_reported_as_fresh() {
        let mut report = HealthSnapshot::unknown(&config(22, 80, ProbeKind::Tcp, None));
        report.observed_at = Some((Utc::now() - chrono::Duration::seconds(901)).to_rfc3339());
        assert!(!report.is_fresh(Utc::now()));
        assert!(
            !HealthSnapshot::unknown(&config(22, 80, ProbeKind::Tcp, None)).is_fresh(Utc::now())
        );
    }

    #[test]
    fn validation_cancellation_and_backoff_are_bounded() {
        let mut bad = config(0, 80, ProbeKind::Tcp, None);
        assert_eq!(bad.validate().unwrap_err().code, ErrorCode::InvalidInput);
        bad = config(22, 80, ProbeKind::Http, Some("/bad\r\nHost: attacker"));
        assert_eq!(bad.validate().unwrap_err().code, ErrorCode::InvalidInput);
        let cancelled = AtomicBool::new(true);
        assert_eq!(
            probe_once(&config(22, 80, ProbeKind::Tcp, None), &cancelled)
                .unwrap_err()
                .code,
            ErrorCode::Cancelled
        );
        assert_eq!(backoff_seconds(60, 0), 60);
        assert_eq!(backoff_seconds(60, 2), 240);
        assert_eq!(backoff_seconds(60, 5), 300);
    }
}
