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
    /// A server address the application binds to, when it does not listen
    /// on loopback. Accepted only when the latest discovery for this server
    /// reported that exact address and port; absent means `localhost`.
    #[serde(default)]
    #[ts(optional)]
    pub remote_host: Option<String>,
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
        if self.remote_port == 0
            || !valid_path(&self.path)
            || self
                .remote_host
                .as_deref()
                .is_some_and(|host| destination_ip(host).is_none())
        {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        Ok(())
    }
    /// The `-L` destination: `localhost` or a bracketed/plain IP literal.
    pub fn destination(&self) -> String {
        match self.remote_host.as_deref().and_then(destination_ip) {
            Some(std::net::IpAddr::V6(ip)) => format!("[{ip}]"),
            Some(ip) => ip.to_string(),
            None => "localhost".into(),
        }
    }
}

/// A specific, usable server address (not unspecified or multicast).
pub fn destination_ip(value: &str) -> Option<std::net::IpAddr> {
    let ip: std::net::IpAddr = value.trim_matches(|c| c == '[' || c == ']').parse().ok()?;
    (!ip.is_unspecified() && !ip.is_multicast()).then_some(ip)
}

/// One listening TCP port found by read-only discovery on the server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DiscoveredApp {
    pub port: u16,
    /// Address the socket binds to as reported by `ss`.
    pub bind: String,
    /// Destination to request for this app; `None` means `localhost`.
    pub remote_host: Option<String>,
    /// HTTP status from a GET / on the server, or `None` if not HTTP.
    pub http_status: Option<u16>,
    /// HTTPS-only apps are listed but cannot be opened in the window yet.
    pub https_only: bool,
    pub title: Option<String>,
    /// Process name when visible to the SSH account.
    pub process: Option<String>,
    /// The app's own favicon as a `data:` URL, when it served a small PNG,
    /// ICO, GIF, JPEG or WebP image. The type comes from the image bytes,
    /// never from the server's claim; anything else is dropped.
    #[serde(default)]
    pub icon: Option<String>,
    /// What the page says it is built with (its `generator` meta tag), which
    /// often includes a version. Display text only.
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AppDiscovery {
    pub contract_version: u32,
    pub server_id: ServerId,
    pub discovered_at: String,
    /// False when `ss` is unavailable on the server.
    pub complete: bool,
    pub apps: Vec<DiscoveredApp>,
}

pub const MAX_DISCOVERED: usize = 64;

/// Fixed read-only discovery: listening TCP sockets from `ss`, then one GET /
/// per port from the server itself (2 s each, 64 KiB cap) for the HTTP status
/// and page title. Only those fields are returned.
pub(crate) const REMOTE_APPS_COMMAND: &str = r#####"
LC_ALL=C; LANG=C; export LC_ALL LANG
printf 'ATLAS_APPS_V1\n'
if ! command -v ss >/dev/null 2>&1; then printf 'NO_SS\nEND\n'; exit 0; fi
seen=' '; budget=40000
ss -Hltnp 2>/dev/null | awk '{print $4, $6}' | head -n 256 | while read -r local proc; do
  port=${local##*:}; addr=${local%:*}; addr=${addr#\[}; addr=${addr%\]}; addr=${addr%%%*}
  case "$port" in ''|*[!0-9]*) continue;; esac
  case "$seen" in *" $addr:$port "*) continue;; esac
  seen="$seen$addr:$port "
  name=$(printf '%s' "$proc" | sed -n 's/^users:(("\([A-Za-z0-9._-]*\)".*/\1/p' | cut -c1-64)
  case "$addr" in
    '*'|0.0.0.0|127.0.0.1|'::') host=127.0.0.1;;
    '::1') host='[::1]';;
    *:*) host="[$addr]";;
    *) host="$addr";;
  esac
  code=000; title=''; https=0; out=''; icon=''; made=''
  if command -v curl >/dev/null 2>&1; then
    out=$(curl -s -m 2 --max-filesize 65536 -H 'Accept: text/html' -w '\n__ATLAS_CODE__%{http_code}' "http://$host:$port/" 2>/dev/null | head -c 70000)
    code=${out##*__ATLAS_CODE__}; [ -n "$code" ] || code=000
    if [ "$code" = 000 ] || [ "$code" = 400 ]; then
      if curl -sk -m 2 -o /dev/null "https://$host:$port/" 2>/dev/null; then https=1; out=''; fi
    fi
    flat=$(printf '%s' "$out" | tr -d '\r' | tr '\n\t' '  ')
    title=$(printf '%s' "$flat" | sed -n 's/.*<[Tt][Ii][Tt][Ll][Ee][^>]*>\([^<]*\)<.*/\1/p' | cut -c1-160)
    [ -n "$title" ] || title=$(printf '%s' "$flat" | sed -n 's/.*<[Hh]1[^>]*>\([^<]*\)<.*/\1/p' | cut -c1-160)
    made=$(printf '%s' "$flat" | tr "'" '"' | grep -o '<[Mm][Ee][Tt][Aa][^>]*>' | grep -i 'name="generator"' | head -n 1 | sed -n 's/.*[Cc][Oo][Nn][Tt][Ee][Nn][Tt]="\([^"]*\)".*/\1/p' | cut -c1-80)
    # The app's favicon: a same-origin <link rel=icon> path, else /favicon.ico.
    # At most 6 KiB each and about 40 KB in total, so the reply stays small.
    if [ "$code" != 000 ] && [ "$https" = 0 ] && [ "$budget" -gt 0 ] && command -v base64 >/dev/null 2>&1; then
      ref=$(printf '%s' "$flat" | tr "'" '"' | grep -o '<[Ll][Ii][Nn][Kk][^>]*>' | grep -i 'rel="[^"]*icon' | head -n 1 | sed -n 's/.*[Hh][Rr][Ee][Ff]="\([^"]*\)".*/\1/p')
      case "$ref" in //*|*://*|data:*|'') ref=/favicon.ico;; /*) ;; *) ref="/$ref";; esac
      case "$ref" in *[!A-Za-z0-9._~/?=%-]*) ref=/favicon.ico;; esac
      icon=$(curl -s -m 2 --max-filesize 6144 "http://$host:$port$ref" 2>/dev/null | head -c 6145 | base64 | tr -d '\n')
      [ ${#icon} -le 8196 ] || icon=''
      budget=$((budget - ${#icon}))
      [ "$budget" -ge 0 ] || icon=''
    fi
  fi
  printf 'APP\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$addr" "$port" "$code" "$https" "$name" "$icon" "$made" "$title"
done
printf 'END\n'
"#####;

fn clean_title(raw: &str) -> Option<String> {
    let decoded = raw
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    let text = decoded
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let text: String = text.chars().take(120).collect();
    (!text.is_empty()).then_some(text)
}

/// Largest favicon accepted from discovery, in bytes.
pub const MAX_ICON_BYTES: usize = 6 * 1024;

/// A `data:` URL for a small raster image, typed by its magic bytes.
fn icon_data_url(encoded: &str) -> Option<String> {
    use base64::Engine as _;
    if encoded.is_empty() || encoded.len() > MAX_ICON_BYTES.div_ceil(3) * 4 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    if bytes.len() > MAX_ICON_BYTES {
        return None;
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        "image/x-icon"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return None;
    };
    let clean = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:{mime};base64,{clean}"))
}

/// Strictly parse discovery output; malformed lines are skipped, never echoed.
pub fn parse_discovery(output: &str) -> Option<(bool, Vec<DiscoveredApp>)> {
    let mut lines = output.lines();
    if lines.next()? != "ATLAS_APPS_V1" {
        return None;
    }
    let mut complete = true;
    let mut ended = false;
    let mut apps: Vec<DiscoveredApp> = Vec::new();
    for line in lines {
        match line {
            "END" => {
                ended = true;
                break;
            }
            "NO_SS" => complete = false,
            _ => {
                let fields: Vec<&str> = line.splitn(9, '\t').collect();
                if fields.len() != 9 || fields[0] != "APP" || apps.len() >= MAX_DISCOVERED {
                    continue;
                }
                let bind = fields[1];
                let Ok(port) = fields[2].parse::<u16>() else {
                    continue;
                };
                if port == 0 {
                    continue;
                }
                // Only these reach the app through the server's `localhost`;
                // other loopback addresses (127.0.0.2, ...) need that address.
                let loopback_or_any = matches!(bind, "*" | "0.0.0.0" | "::" | "127.0.0.1" | "::1");
                let remote_host = if loopback_or_any {
                    None
                } else {
                    match destination_ip(bind) {
                        Some(ip) => Some(ip.to_string()),
                        None => continue,
                    }
                };
                let http_status = fields[3]
                    .parse::<u16>()
                    .ok()
                    .filter(|code| (100..=599).contains(code));
                let process = Some(fields[5])
                    .filter(|name| {
                        !name.is_empty()
                            && name.len() <= 64
                            && name
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
                    })
                    .map(str::to_owned);
                let https_only = fields[4] == "1" && http_status.is_none_or(|c| c == 400);
                let app = DiscoveredApp {
                    port,
                    bind: bind.chars().take(64).collect(),
                    remote_host,
                    http_status: if https_only { None } else { http_status },
                    https_only,
                    title: clean_title(fields[8]),
                    version: clean_title(fields[7]).map(|text| text.chars().take(60).collect()),
                    process,
                    icon: if https_only {
                        None
                    } else {
                        icon_data_url(fields[6])
                    },
                };
                if !apps
                    .iter()
                    .any(|a| a.port == app.port && a.remote_host == app.remote_host)
                {
                    apps.push(app);
                }
            }
        }
    }
    if !ended {
        return None;
    }
    // A port also reachable through `localhost` needs no address-specific entry.
    let local: std::collections::HashSet<u16> = apps
        .iter()
        .filter(|a| a.remote_host.is_none())
        .map(|a| a.port)
        .collect();
    apps.retain(|a| a.remote_host.is_none() || !local.contains(&a.port));
    apps.sort_by_key(|a| (a.http_status.is_none(), a.port));
    Some((complete, apps))
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
    pub fn start(
        options: &SshOptions,
        alias: &str,
        destination: &str,
        remote_port: u16,
    ) -> Result<Self, AtlasError> {
        let local_port = ephemeral_port()?;
        let (_, mut command) =
            options.tunnel_command(alias, local_port, destination, remote_port)?;
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
    [
        TunnelRequest::decl(config),
        TunnelInfo::decl(config),
        DiscoveredApp::decl(config),
        AppDiscovery::decl(config),
    ]
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
                path: "/".into(),
                remote_host: None,
            }
            .validate()
            .is_err()
        );
        assert!(
            TunnelRequest {
                remote_port: 8080,
                path: "/".into(),
                remote_host: None,
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
        let (profile, command) = options
            .tunnel_command("lab", 40001, "localhost", 8080)
            .unwrap();
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
                    .tunnel_command("lab", 40001, "localhost", 8080)
                    .err()
                    .unwrap()
                    .code,
                ErrorCode::Denied,
                "{extra}"
            );
        }
        let (_dir, options) = options("Host lab\n  HostName 127.0.0.1\n  User fixture\n");
        assert!(options.tunnel_command("lab", 0, "localhost", 8080).is_err());
        assert!(
            options
                .tunnel_command("lab", 40001, "localhost", 0)
                .is_err()
        );
        assert!(
            options
                .tunnel_command("-oProxyCommand=x", 40001, "localhost", 80)
                .is_err()
        );
    }

    #[test]
    fn ephemeral_ports_are_loopback_and_nonzero() {
        let port = ephemeral_port().unwrap();
        assert_ne!(port, 0);
        TcpListener::bind((Ipv4Addr::LOCALHOST, port)).unwrap();
    }

    #[test]
    fn destinations_are_localhost_or_specific_ips() {
        let request = |host: Option<&str>| TunnelRequest {
            remote_port: 80,
            path: "/".into(),
            remote_host: host.map(str::to_owned),
        };
        assert_eq!(request(None).destination(), "localhost");
        assert_eq!(
            request(Some("192.168.20.12")).destination(),
            "192.168.20.12"
        );
        assert_eq!(request(Some("fd00::5")).destination(), "[fd00::5]");
        for bad in [
            "0.0.0.0",
            "::",
            "example.com",
            "224.0.0.1",
            "1.2.3.4 -oX",
            "",
        ] {
            assert!(request(Some(bad)).validate().is_err(), "{bad:?}");
        }
        assert!(request(Some("10.0.0.5")).validate().is_ok());
    }

    #[test]
    fn discovery_icons_are_typed_by_their_bytes_and_bounded() {
        use base64::Engine as _;
        let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        let png = b64(b"\x89PNG\r\n\x1a\nrest");
        let markup = b64(b"<svg onload=alert(1)>");
        let big = b64(&[0u8, 0, 1, 0].repeat(MAX_ICON_BYTES / 4 + 1));
        let output = format!(
            "ATLAS_APPS_V1\n\
APP\t127.0.0.1\t3000\t200\t0\tgrafana\t{png}\t\tGrafana\n\
APP\t127.0.0.1\t3001\t200\t0\tx\t{markup}\t\tNot an image\n\
APP\t127.0.0.1\t3002\t200\t0\tx\t{big}\t\tToo big\n\
APP\t127.0.0.1\t3003\t200\t0\tx\tnot base64!\t\tBroken\n\
APP\t*\t9443\t000\t1\t\t{png}\t\tHTTPS only\n\
END\n"
        );
        let (_, apps) = parse_discovery(&output).unwrap();
        assert_eq!(
            apps[0].icon.as_deref(),
            Some(format!("data:image/png;base64,{png}").as_str())
        );
        assert_eq!(apps.len(), 5);
        assert!(apps[1..].iter().all(|app| app.icon.is_none()));
    }

    #[test]
    fn discovery_parses_only_strict_fields() {
        let output = "ATLAS_APPS_V1\n\
APP\t127.0.0.1\t3000\t200\t0\tgrafana\t\tGrafana v11.2\t  Grafana &amp; friends \n\
APP\t0.0.0.0\t22\t000\t0\tsshd\t\t\t\n\
APP\t*\t9443\t000\t1\t\t\t\t\n\
APP\t192.168.20.12\t8080\t302\t0\tjava\t\t\t\x1b[31mEvil\x07 Title\n\
APP\t::1\t3000\t200\t0\tgrafana\t\t\tdup\n\
APP\tnot-an-ip\t81\t200\t0\tx\t\t\tbad\n\
APP\t127.0.0.1\t99999\t200\t0\tx\t\t\tbad\n\
garbage line\n\
APP\t127.0.0.1\t5000\t200\t0\tbad name!\t\t\tok\n\
APP\t192.168.20.12\t3000\t200\t0\tgrafana\t\t\tsame port on LAN\n\
END\n";
        let (complete, apps) = parse_discovery(output).unwrap();
        assert!(complete);
        let ports: Vec<u16> = apps.iter().map(|a| a.port).collect();
        assert_eq!(ports, vec![3000, 5000, 8080, 22, 9443]);
        assert_eq!(apps[0].title.as_deref(), Some("Grafana & friends"));
        assert_eq!(apps[0].process.as_deref(), Some("grafana"));
        assert_eq!(apps[0].version.as_deref(), Some("Grafana v11.2"));
        assert_eq!(apps[1].version, None);
        assert_eq!(apps[0].remote_host, None);
        assert_eq!(apps[1].process, None);
        assert_eq!(apps[2].remote_host.as_deref(), Some("192.168.20.12"));
        assert_eq!(apps[2].title.as_deref(), Some("[31mEvil Title"));
        assert_eq!(apps[3].http_status, None);
        assert!(apps[4].https_only);
        assert!(parse_discovery("ATLAS_APPS_V1\nAPP\t127.0.0.1\t1\t200\t0\tx\t\t\ty\n").is_none());
        assert!(parse_discovery("nope\nEND\n").is_none());
        assert_eq!(
            parse_discovery("ATLAS_APPS_V1\nNO_SS\nEND\n"),
            Some((false, vec![]))
        );
    }
}
