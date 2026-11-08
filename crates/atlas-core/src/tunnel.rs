//! Owner-started remote application tunnels (F17/T035, D051).
//!
//! One system-OpenSSH child per tunnel forwards `127.0.0.1:<ephemeral>` on
//! this PC to `localhost:<port>` on the selected server. Nothing listens on
//! the LAN, nothing is installed remotely, and the child dies with InfraWerf
//! (Windows job object) as well as on explicit close, expiry or Lock.
use crate::{ssh::SshOptions, *};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    process::{Child, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use ts_rs::{Config, TS};

/// Simultaneous tunnels per workspace.
pub const MAX_TUNNELS: usize = 8;
/// Absolute lifetime; reopening is one click.
pub const TUNNEL_LIFETIME: Duration = Duration::from_secs(4 * 60 * 60);
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const STDERR_LIMIT: usize = 8 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TunnelRequest {
    pub remote_port: u16,
    /// Initial path such as `/` or `/admin`; never a scheme or host.
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TunnelInfo {
    pub contract_version: u32,
    pub tunnel_id: String,
    pub server_id: ServerId,
    pub server_name: String,
    pub remote_port: u16,
    pub local_port: u16,
    /// Loopback URL the isolated window opens.
    pub url: String,
    pub opened_at: String,
    pub expires_at: String,
}

impl TunnelRequest {
    pub fn validate(&self) -> Result<(), AtlasError> {
        if self.remote_port == 0 || !valid_path(&self.path) {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        Ok(())
    }
}

/// A relative URL path with a conservative character set: no scheme, host,
/// backslash, whitespace or control characters, and at most 512 bytes.
pub fn valid_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && path.len() <= 512
        && path
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/-._~%!$&'()*+,;=:@?#".contains(&c))
}

/// Reserve an ephemeral loopback port. OpenSSH rebinds it immediately;
/// `ExitOnForwardFailure` turns a lost race into a visible failure.
fn ephemeral_port() -> Result<u16, AtlasError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))
}

pub struct TunnelProcess {
    child: Child,
    local_port: u16,
    stderr: Arc<Mutex<Vec<u8>>>,
    started: Instant,
}

impl TunnelProcess {
    /// Start the forward and wait until its loopback listener accepts.
    /// Callers verify trusted SSH access first; this never alters trust.
    pub fn start(options: &SshOptions, alias: &str, remote_port: u16) -> Result<Self, AtlasError> {
        let local_port = ephemeral_port()?;
        let (_, mut command) = options.tunnel_command(alias, local_port, remote_port)?;
        command.stdin(Stdio::null()).stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
        job::assign(&child);
        let stderr = Arc::new(Mutex::new(Vec::new()));
        if let Some(mut pipe) = child.stderr.take() {
            let sink = stderr.clone();
            std::thread::spawn(move || {
                let mut buffer = [0u8; 1024];
                while let Ok(read) = pipe.read(&mut buffer) {
                    if read == 0 {
                        break;
                    }
                    if let Ok(mut kept) = sink.lock() {
                        kept.extend_from_slice(&buffer[..read]);
                        let excess = kept.len().saturating_sub(STDERR_LIMIT);
                        kept.drain(..excess);
                    }
                }
            });
        }
        let mut process = Self {
            child,
            local_port,
            stderr,
            started: Instant::now(),
        };
        process.wait_ready()?;
        Ok(process)
    }

    fn wait_ready(&mut self) -> Result<(), AtlasError> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, self.local_port));
        let deadline = Instant::now() + READY_TIMEOUT;
        while Instant::now() < deadline {
            if !self.alive() {
                return Err(self.failure());
            }
            if TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok() {
                // A listener can only be OpenSSH's: the port was ours a moment
                // ago and the child is still alive with ExitOnForwardFailure.
                if self.alive() {
                    return Ok(());
                }
                return Err(self.failure());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(AtlasError::new(ErrorCode::TimedOut))
    }

    fn failure(&self) -> AtlasError {
        let stderr = self
            .stderr
            .lock()
            .map(|kept| String::from_utf8_lossy(&kept).into_owned())
            .unwrap_or_default();
        AtlasError::new(match crate::ssh::classify(&stderr) {
            crate::ssh::SshCheckState::UnknownHost | crate::ssh::SshCheckState::ChangedHost => {
                ErrorCode::HostTrustFailure
            }
            crate::ssh::SshCheckState::AuthenticationFailed => ErrorCode::Unauthenticated,
            crate::ssh::SshCheckState::Unreachable => ErrorCode::IoFailure,
            // A verified host whose forward fails: the local port was taken
            // or the server refused port forwarding.
            _ => ErrorCode::RemoteRejected,
        })
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn age(&self) -> Duration {
        self.started.elapsed()
    }
}

impl Drop for TunnelProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
mod job {
    //! A kill-on-close job object: if InfraWerf exits by any route, Windows
    //! terminates every tunnel child with it.
    use std::{os::windows::io::AsRawHandle, process::Child, sync::OnceLock};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    struct Job(usize);
    static JOB: OnceLock<Option<Job>> = OnceLock::new();
    fn job() -> Option<usize> {
        JOB.get_or_init(|| unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return None;
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            (ok != 0).then_some(Job(handle as usize))
        })
        .as_ref()
        .map(|job| job.0)
    }
    pub fn assign(child: &Child) {
        if let Some(handle) = job() {
            unsafe {
                AssignProcessToJobObject(handle as _, child.as_raw_handle() as _);
            }
        }
    }
}
#[cfg(not(windows))]
mod job {
    pub fn assign(_: &std::process::Child) {}
}

pub fn typescript_contracts(config: &Config) -> String {
    [TunnelRequest::decl(config), TunnelInfo::decl(config)]
        .into_iter()
        .map(|d| format!("export {d}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(config: &str) -> (tempfile::TempDir, SshOptions) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config");
        std::fs::write(&path, config).unwrap();
        (
            directory,
            SshOptions::configured(SshOptions::system_executable(), path),
        )
    }
    fn args(command: &std::process::Command) -> Vec<String> {
        command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }
    fn has_client() -> bool {
        SshOptions::system_executable().exists()
            || std::process::Command::new("ssh").arg("-V").output().is_ok()
    }

    #[test]
    fn paths_are_relative_and_conservative() {
        for good in ["/", "/admin", "/a/b?x=1&y=2#top", "/%20space"] {
            assert!(valid_path(good), "{good}");
        }
        for bad in [
            "",
            "admin",
            "//evil.example/",
            "/a b",
            "/a\\b",
            "/\n",
            "http://x/",
        ] {
            assert!(!valid_path(bad), "{bad:?}");
        }
        assert!(valid_path(&format!("/{}", "a".repeat(511))));
        assert!(!valid_path(&format!("/{}", "a".repeat(512))));
        assert!(
            TunnelRequest {
                remote_port: 0,
                path: "/".into()
            }
            .validate()
            .is_err()
        );
        assert!(
            TunnelRequest {
                remote_port: 8080,
                path: "/".into()
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn forward_is_loopback_only_and_keeps_strict_options() {
        if !has_client() {
            return;
        }
        let (_dir, options) =
            options("Host lab\n  HostName 127.0.0.1\n  Port 2222\n  User fixture\n");
        let (profile, command) = options.tunnel_command("lab", 40001, 8080).unwrap();
        assert_eq!(profile.port, 2222);
        let args = args(&command);
        let forward = args.iter().position(|a| a == "-L").unwrap();
        assert_eq!(args[forward + 1], "127.0.0.1:40001:localhost:8080");
        assert_eq!(args.iter().filter(|a| *a == "-L").count(), 1);
        for required in [
            "-N",
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "ExitOnForwardFailure=yes",
            "GatewayPorts=no",
            "ForwardAgent=no",
            "ForwardX11=no",
            "PasswordAuthentication=no",
            "ControlMaster=no",
        ] {
            assert!(args.iter().any(|a| a == required), "missing {required}");
        }
        // ClearAllForwardings would also clear the reviewed -L forward.
        assert!(!args.iter().any(|a| a.starts_with("ClearAllForwardings")));
        assert!(!args.iter().any(|a| a == "-R" || a == "-D" || a == "-g"));
        assert_eq!(args.last().unwrap(), "lab");
    }

    #[test]
    fn configured_forwards_and_bad_inputs_are_refused() {
        if !has_client() {
            return;
        }
        for extra in [
            "  LocalForward 9000 localhost:9000\n",
            "  RemoteForward 9001 localhost:22\n",
            "  DynamicForward 1080\n",
        ] {
            let (_dir, options) = options(&format!(
                "Host lab\n  HostName 127.0.0.1\n  User fixture\n{extra}"
            ));
            assert_eq!(
                options
                    .tunnel_command("lab", 40001, 8080)
                    .err()
                    .unwrap()
                    .code,
                ErrorCode::Denied,
                "{extra}"
            );
        }
        let (_dir, options) = options("Host lab\n  HostName 127.0.0.1\n  User fixture\n");
        assert!(options.tunnel_command("lab", 0, 8080).is_err());
        assert!(options.tunnel_command("lab", 40001, 0).is_err());
        assert!(
            options
                .tunnel_command("-oProxyCommand=x", 40001, 80)
                .is_err()
        );
    }

    #[test]
    fn ephemeral_ports_are_loopback_and_nonzero() {
        let port = ephemeral_port().unwrap();
        assert_ne!(port, 0);
        TcpListener::bind((Ipv4Addr::LOCALHOST, port)).unwrap();
    }
}
