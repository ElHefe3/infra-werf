//! Direct system-OpenSSH verification. No trust mutation, password transport,
//! arbitrary commands or persistent authenticated-session claim.
use crate::{operations::ExecutionContext, *};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
use ts_rs::{Config, TS};

#[derive(Clone, Serialize, Deserialize, TS)]
pub struct SshProfile {
    pub alias: String,
    pub hostname: String,
    pub user: String,
    pub port: u16,
    pub identity_files: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SshCheckState {
    Verified,
    UnknownHost,
    ChangedHost,
    AuthenticationFailed,
    Unreachable,
    UnsupportedConfiguration,
    ClientUnavailable,
    Failed,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct SshVerification {
    pub contract_version: u32,
    pub state: SshCheckState,
    pub profile: Option<SshProfile>,
    pub fingerprint: Option<String>,
    pub key_type: Option<String>,
    pub checked_at: String,
}

#[derive(Clone)]
pub struct SshOptions {
    executable: PathBuf,
    config: Option<PathBuf>,
}
impl SshOptions {
    pub fn system() -> Self {
        #[cfg(windows)]
        let executable =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
                .join("System32/OpenSSH/ssh.exe");
        #[cfg(not(windows))]
        let executable = PathBuf::from("ssh");
        Self {
            executable,
            config: std::env::var_os("ATLAS_SSH_CONFIG").map(PathBuf::from),
        }
    }
    /// Trusted backend/fixture configuration only; never a native command DTO.
    pub fn configured(executable: PathBuf, config: PathBuf) -> Self {
        Self {
            executable,
            config: Some(config),
        }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env_remove("SSH_ASKPASS")
            .env_remove("SSH_ASKPASS_REQUIRE");
        if let Some(config) = &self.config {
            command.arg("-F").arg(config);
        }
        // CLI options precede config values. No forwarding, helper command,
        // backgrounding or multiplexing may outlive this verification process.
        for option in [
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "NoHostAuthenticationForLocalhost=no",
            "FingerprintHash=sha256",
            "LogLevel=DEBUG1",
            "UpdateHostKeys=no",
            "CheckHostIP=no",
            "VerifyHostKeyDNS=no",
            "PasswordAuthentication=no",
            "KbdInteractiveAuthentication=no",
            "PreferredAuthentications=publickey",
            "NumberOfPasswordPrompts=0",
            "ControlMaster=no",
            "ControlPath=none",
            "ControlPersist=no",
            "ForkAfterAuthentication=no",
            "PermitLocalCommand=no",
            "ClearAllForwardings=yes",
            "ForwardAgent=no",
            "ForwardX11=no",
            "RequestTTY=no",
            "RemoteCommand=none",
            "SessionType=default",
            "ConnectTimeout=5",
            "ConnectionAttempts=1",
            "ServerAliveInterval=2",
            "ServerAliveCountMax=2",
        ] {
            command.arg("-o").arg(option);
        }
        command
    }
    pub fn verify(
        &self,
        alias: &str,
        context: &mut ExecutionContext,
    ) -> Result<SshVerification, AtlasError> {
        if hostname(alias)? != alias {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        let outcome = |state, profile, fingerprint: Option<(String, String)>| {
            let (key_type, fingerprint) = fingerprint
                .map(|(kind, value)| (Some(kind), Some(value)))
                .unwrap_or((None, None));
            SshVerification {
                contract_version: CONTRACT_VERSION,
                state,
                profile,
                fingerprint,
                key_type,
                checked_at: Utc::now().to_rfc3339(),
            }
        };
        let mut resolve = self.command();
        resolve.arg("-G").arg("--").arg(alias);
        let resolved = match capture(resolve, context) {
            Ok(value) => value,
            Err(ProcessFailure::Missing) => {
                return Ok(outcome(SshCheckState::ClientUnavailable, None, None));
            }
            Err(ProcessFailure::Guard(error)) => return Err(error),
        };
        if !resolved.success {
            return Ok(outcome(SshCheckState::UnsupportedConfiguration, None, None));
        }
        let Some((profile, key_alias)) = profile(alias, &resolved.stdout) else {
            return Ok(outcome(SshCheckState::UnsupportedConfiguration, None, None));
        };
        let marker = format!("ATLAS_VERIFY_{}", OperationId::generate().as_str());
        let mut check = self.command();
        check
            .arg("-v")
            .arg("-o")
            .arg("KnownHostsCommand=none")
            .arg("-o")
            .arg("ProxyCommand=none")
            .arg("-o")
            .arg("ProxyJump=none")
            .arg("-o")
            .arg(format!("Hostname={}", profile.hostname))
            .arg("-l")
            .arg(&profile.user)
            .arg("-p")
            .arg(profile.port.to_string());
        if let Some(alias) = key_alias {
            check.arg("-o").arg(format!("HostKeyAlias={alias}"));
        }
        check.arg("--").arg(alias).arg("echo").arg(&marker);
        let checked = match capture(check, context) {
            Ok(value) => value,
            Err(ProcessFailure::Missing) => {
                return Ok(outcome(
                    SshCheckState::ClientUnavailable,
                    Some(profile),
                    None,
                ));
            }
            Err(ProcessFailure::Guard(error)) => return Err(error),
        };
        let fingerprint = fingerprint(&checked.stderr);
        let state = if checked.success && checked.stdout.trim() == marker && fingerprint.is_some() {
            if publickey_authenticated(&checked.stderr) {
                SshCheckState::Verified
            } else {
                SshCheckState::AuthenticationFailed
            }
        } else {
            classify(&checked.stderr)
        };
        Ok(outcome(state, Some(profile), fingerprint))
    }
}

fn profile(alias: &str, data: &str) -> Option<(SshProfile, Option<String>)> {
    let mut fields: HashMap<&str, Vec<&str>> = HashMap::new();
    for line in data.lines() {
        let (name, value) = line.split_once(' ')?;
        fields.entry(name).or_default().push(value.trim());
    }
    let one = |name: &str| fields.get(name).and_then(|values| values.first().copied());
    // Jump/proxy/known-host helper subprocesses require their own inherited
    // trust/lifecycle proof, retained in T012. Do not run them in this slice.
    if ["proxycommand", "proxyjump", "knownhostscommand"]
        .iter()
        .any(|name| one(name).is_some_and(|value| value != "none"))
    {
        return None;
    }
    let host = one("hostname")?;
    if hostname(host).ok()?.as_str() != host {
        return None;
    }
    let user = one("user")?;
    if user.is_empty()
        || user.len() > 128
        || user.starts_with('-')
        || !user
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-.@\\$".contains(&c))
    {
        return None;
    }
    let port: u16 = one("port")?.parse().ok()?;
    if port == 0 {
        return None;
    }
    // Preserve absence for OpenSSH's port lookup and certificate principals.
    let key_alias = one("hostkeyalias").filter(|value| *value != "none");
    if key_alias.is_some_and(|value| {
        value.len() > 253
            || !value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-:[]%".contains(&c))
    }) {
        return None;
    }
    Some((
        SshProfile {
            alias: alias.into(),
            hostname: host.into(),
            user: user.into(),
            port,
            identity_files: fields
                .get("identityfile")
                .map(|v| v.iter().filter(|value| **value != "none").count() as u32)
                .unwrap_or(0),
        },
        key_alias.map(str::to_owned),
    ))
}
fn fingerprint(data: &str) -> Option<(String, String)> {
    data.lines().find_map(|line| {
        let key = line
            .strip_prefix("debug1: Server host key: ")
            .or_else(|| line.strip_prefix("debug1: Server host certificate: "))?;
        let mut words = key.split_whitespace();
        let kind = words.next()?;
        let fingerprint = words.next()?.trim_end_matches(',');
        let digest = fingerprint.strip_prefix("SHA256:")?;
        if kind.len() > 80
            || !kind
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-@._".contains(&c))
            || digest.len() != 43
            || !digest
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"+/".contains(&c))
        {
            return None;
        }
        Some((kind.into(), fingerprint.into()))
    })
}
fn publickey_authenticated(stderr: &str) -> bool {
    // OpenSSH initially attempts none even with PreferredAuthentications set.
    // Reject genuine none success anywhere, including beside spoofed remote
    // stderr/banner text that claims publickey success.
    stderr.contains("Authenticated to ")
        && stderr.contains(" using \"publickey\".")
        && !stderr.contains("using \"none\"")
}
fn classify(stderr: &str) -> SshCheckState {
    if stderr.contains("REMOTE HOST IDENTIFICATION HAS CHANGED") {
        SshCheckState::ChangedHost
    } else if stderr.contains("Host key verification failed")
        || stderr.contains("host key is known")
    {
        SshCheckState::UnknownHost
    } else if stderr.contains("Permission denied")
        || stderr.contains("No more authentication methods to try")
    {
        SshCheckState::AuthenticationFailed
    } else if [
        "Connection refused",
        "Connection timed out",
        "Could not resolve hostname",
        "No route to host",
        "Network is unreachable",
        "Connection reset",
        "Connection closed",
    ]
    .iter()
    .any(|message| stderr.contains(message))
    {
        SshCheckState::Unreachable
    } else {
        SshCheckState::Failed
    }
}

struct Captured {
    success: bool,
    stdout: String,
    stderr: String,
}
enum ProcessFailure {
    Missing,
    Guard(AtlasError),
}
impl From<AtlasError> for ProcessFailure {
    fn from(error: AtlasError) -> Self {
        Self::Guard(error)
    }
}
struct RunningChild(Child);
impl Drop for RunningChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
enum Chunk {
    Data(bool, Vec<u8>),
    Done,
    Failed,
}

fn capture(
    mut command: Command,
    context: &mut ExecutionContext,
) -> Result<Captured, ProcessFailure> {
    context.checkpoint()?;
    let child = command.spawn().map_err(|failure| {
        if failure.kind() == std::io::ErrorKind::NotFound {
            ProcessFailure::Missing
        } else {
            ProcessFailure::Guard(AtlasError::new(ErrorCode::IoFailure))
        }
    })?;
    let mut child = RunningChild(child);
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?;
    let stderr = child
        .0
        .stderr
        .take()
        .ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?;
    let (send, receive) = mpsc::sync_channel(4);
    fn read_pipe<R: Read + Send + 'static>(
        mut pipe: R,
        send: mpsc::SyncSender<Chunk>,
        stderr: bool,
    ) -> Result<std::thread::JoinHandle<()>, AtlasError> {
        std::thread::Builder::new()
            .name("atlas-ssh-output".into())
            .spawn(move || {
                let mut bytes = [0; 4096];
                loop {
                    match pipe.read(&mut bytes) {
                        Ok(0) => {
                            let _ = send.send(Chunk::Done);
                            break;
                        }
                        Ok(count) => {
                            if send
                                .send(Chunk::Data(stderr, bytes[..count].to_vec()))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(_) => {
                            let _ = send.send(Chunk::Failed);
                            break;
                        }
                    }
                }
            })
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))
    }
    let out_reader = read_pipe(stdout, send.clone(), false)?;
    let err_reader = match read_pipe(stderr, send, true) {
        Ok(value) => value,
        Err(failure) => {
            drop(receive);
            drop(child);
            let _ = out_reader.join();
            return Err(failure.into());
        }
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut finished = 0;
    let result = (|| -> Result<bool, AtlasError> {
        while finished < 2 {
            context.checkpoint()?;
            match receive.recv_timeout(Duration::from_millis(10)) {
                Ok(Chunk::Data(stderr, bytes)) => {
                    context.account_output(bytes.len())?;
                    if stderr {
                        err.extend(bytes);
                    } else {
                        out.extend(bytes);
                    }
                }
                Ok(Chunk::Done) => finished += 1,
                Ok(Chunk::Failed) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(AtlasError::new(ErrorCode::IoFailure));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        loop {
            context.checkpoint()?;
            if let Some(status) = child
                .0
                .try_wait()
                .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
            {
                return Ok(status.success());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    // Drop the receiver before joining bounded pipe writers, then kill/reap
    // the owned SSH child. The native lease lives until these joins finish.
    drop(receive);
    drop(child);
    let _ = out_reader.join();
    let _ = err_reader.join();
    Ok(Captured {
        success: result?,
        stdout: String::from_utf8_lossy(&out).into_owned(),
        stderr: String::from_utf8_lossy(&err).into_owned(),
    })
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        SshProfile::decl(config),
        SshCheckState::decl(config),
        SshVerification::decl(config),
    ]
    .into_iter()
    .map(|declaration| format!("export {declaration}\n"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_fail_closed_for_helpers_bad_users_and_ports() {
        let valid = "hostname example.invalid\nuser operator\nport 22\nidentityfile /private/reference-only\n";
        assert!(profile("alias", valid).is_some());
        assert!(
            profile("alias", &valid.replace("port 22", "port 2222"))
                .unwrap()
                .1
                .is_none()
        );
        assert_eq!(
            profile(
                "alias",
                &format!(
                    "{}hostkeyalias owner-alias\n",
                    valid.replace("port 22", "port 2222")
                )
            )
            .unwrap()
            .1,
            Some("owner-alias".into())
        );
        for suffix in [
            "proxyjump jump\n",
            "proxycommand custom %h\n",
            "knownhostscommand custom\n",
        ] {
            assert!(profile("alias", &format!("{valid}{suffix}")).is_none());
        }
        assert!(profile("alias", &valid.replace("port 22", "port 0")).is_none());
        assert!(
            profile(
                "alias",
                &valid.replace("user operator", "user -oInjected=yes")
            )
            .is_none()
        );
        assert!(
            profile(
                "alias",
                &valid.replace("example.invalid", "example;command")
            )
            .is_none()
        );
    }
    #[test]
    fn diagnostics_expose_only_valid_fingerprints_and_fixed_states() {
        let fingerprint_value = "A".repeat(43);
        let raw = format!(
            "debug1: Server host key: ssh-ed25519 SHA256:{fingerprint_value}\nsynthetic-private-banner"
        );
        assert_eq!(
            fingerprint(&raw),
            Some(("ssh-ed25519".into(), format!("SHA256:{fingerprint_value}")))
        );
        assert!(fingerprint("debug1: Server host key: bad$key SHA256:secret").is_none());
        assert!(classify("REMOTE HOST IDENTIFICATION HAS CHANGED") == SshCheckState::ChangedHost);
        assert!(classify("Host key verification failed") == SshCheckState::UnknownHost);
        assert!(classify("Permission denied (publickey)") == SshCheckState::AuthenticationFailed);
        assert!(classify("Connection refused") == SshCheckState::Unreachable);
        assert!(publickey_authenticated(
            "Authenticated to host using \"publickey\"."
        ));
        assert!(!publickey_authenticated(
            "Authenticated to host using \"none\".\nAuthenticated to spoofed using \"publickey\"."
        ));
        assert!(!publickey_authenticated(
            "banner-prefixAuthenticated to host using \"none\".\nAuthenticated to spoofed using \"publickey\"."
        ));
    }
    #[test]
    fn missing_client_returns_a_fixed_outcome_without_granting_a_session() {
        use crate::operations::{OperationBroker, ServerTarget};
        let broker = OperationBroker::default();
        let authority = broker.issue_backend_authority().unwrap();
        let target = ServerTarget::new(
            ServerId::parse("missing-client").unwrap(),
            "missing.invalid",
        )
        .unwrap();
        let ticket = broker
            .prepare_connection_check(Some(&authority), &target)
            .unwrap();
        let options = SshOptions::configured(
            std::env::temp_dir().join(format!(
                "atlas-missing-{}.exe",
                OperationId::generate().as_str()
            )),
            PathBuf::from("unused-config"),
        );
        let response = broker
            .execute_connection_check(
                Some(&authority),
                &ticket.operation_id,
                &target,
                move |_, context| {
                    let response = options.verify("missing.invalid", context)?;
                    context.write(&serde_json::to_string(&response).unwrap())
                },
            )
            .unwrap();
        let response: SshVerification = serde_json::from_str(&response).unwrap();
        assert!(response.state == SshCheckState::ClientUnavailable);
        assert!(response.profile.is_none() && response.fingerprint.is_none());
    }
}
