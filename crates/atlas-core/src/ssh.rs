//! Direct system-OpenSSH verification. No trust mutation, password transport,
//! arbitrary commands or persistent authenticated-session claim.
use crate::{operations::ExecutionContext, *};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
use ts_rs::{Config, TS};
pub(crate) mod conversation;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
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
    /// Discover only literal aliases in the user's primary config. No helper,
    /// Include expansion, network scan or authentication occurs here.
    pub fn configured_aliases(&self) -> Result<Vec<String>, AtlasError> {
        let path = self.config.clone().or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|p| PathBuf::from(p).join(".ssh/config"))
        });
        let Some(path) = path else {
            return Ok(Vec::new());
        };
        if !path.exists() {
            return Ok(Vec::new());
        }
        if std::fs::metadata(&path)
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
            .len()
            > 256 * 1024
        {
            return Err(AtlasError::new(ErrorCode::OutputLimit));
        }
        let text =
            std::fs::read_to_string(path).map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
        Ok(literal_aliases(&text))
    }
    /// The Windows system OpenSSH client, or `ssh` from PATH elsewhere.
    pub fn system_executable() -> PathBuf {
        #[cfg(windows)]
        return PathBuf::from(
            std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
        )
        .join("System32/OpenSSH/ssh.exe");
        #[cfg(not(windows))]
        return PathBuf::from("ssh");
    }
    pub fn system() -> Self {
        Self {
            executable: Self::system_executable(),
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
        self.base_command(false)
    }
    /// `forwarding` omits only ClearAllForwardings so one explicit, reviewed
    /// `-L` loopback forward can be added (F17/T035, D051). Every other
    /// restriction is identical.
    fn base_command(&self, forwarding: bool) -> Command {
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
            // Quiet mode suppresses server-controlled SSH_MSG_USERAUTH_BANNER
            // text. Restore only local functions that emit evidence/errors
            // Atlas consumes; enabling an entire source file also exposes
            // server-controlled identification diagnostics.
            "LogLevel=QUIET",
            "LogVerbose=*sshconnect.c:verify_host_key():*,*sshconnect.c:check_host_key():*,*sshconnect.c:warn_changed_key():*,*sshconnect.c:ssh_connect_direct():*,*sshconnect2.c:ssh_userauth2():*,*sshconnect2.c:userauth():*",
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
            "ForwardAgent=no",
            "ForwardX11=no",
            "RequestTTY=no",
            "RemoteCommand=none",
            "ConnectTimeout=5",
            "ConnectionAttempts=1",
            "ServerAliveInterval=2",
            "ServerAliveCountMax=2",
        ] {
            command.arg("-o").arg(option);
        }
        if !forwarding {
            command.arg("-o").arg("ClearAllForwardings=yes");
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
        resolve.arg("-o").arg("SessionType=default");
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
        check.arg("-o").arg("SessionType=default");
        check
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
        let state = if checked.success && checked.stdout.trim() == marker {
            if fingerprint.is_some() && publickey_authenticated(&checked.stderr) {
                SshCheckState::Verified
            } else {
                SshCheckState::AuthenticationFailed
            }
        } else {
            classify(&checked.stderr)
        };
        Ok(outcome(state, Some(profile), fingerprint))
    }
    #[cfg(windows)]
    pub(crate) fn sftp_command(
        &self,
        alias: &str,
        context: &mut ExecutionContext,
    ) -> Result<(SshProfile, Command, Vec<String>), AtlasError> {
        if hostname(alias)? != alias {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        let mut resolve = self.command();
        resolve.arg("-G").arg("--").arg(alias);
        let resolved = capture(resolve, context).map_err(|failure| match failure {
            ProcessFailure::Missing => AtlasError::new(ErrorCode::IoFailure),
            ProcessFailure::Guard(error) => error,
        })?;
        let (profile, key_alias) = profile(alias, &resolved.stdout)
            .filter(|_| resolved.success)
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        let mut command = self.command();
        command
            .stdin(Stdio::piped())
            .arg("-s")
            .arg("-o")
            .arg("SessionType=subsystem")
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
            command.arg("-o").arg(format!("HostKeyAlias={alias}"));
        }
        command.arg("--").arg(alias).arg("sftp");
        Ok((
            profile,
            command,
            conversation::known_host_sources(&resolved.stdout),
        ))
    }
    /// Build a loopback-only local forward for an owner-started remote
    /// application window. Configuration-defined forwards are refused rather
    /// than cleared or applied, because ClearAllForwardings would also clear
    /// the reviewed `-L` and leaving them active would open unreviewed ports.
    pub fn tunnel_command(
        &self,
        alias: &str,
        local_port: u16,
        remote_port: u16,
    ) -> Result<(SshProfile, Command), AtlasError> {
        if hostname(alias)? != alias || local_port == 0 || remote_port == 0 {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        // Resolve without ClearAllForwardings so `-G` reveals configured
        // forwards. `-G` evaluates local configuration only; no network contact.
        let mut resolve = self.base_command(true);
        resolve.arg("-G").arg("--").arg(alias);
        let resolved = resolve
            .output()
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
        if !resolved.status.success() || resolved.stdout.len() > 256 * 1024 {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let text = String::from_utf8_lossy(&resolved.stdout);
        if configured_forwards(&text) {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let (profile, key_alias) =
            profile(alias, &text).ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        let mut command = self.base_command(true);
        command.stdout(Stdio::null());
        command
            .arg("-N")
            .arg("-o")
            .arg("ExitOnForwardFailure=yes")
            .arg("-o")
            .arg("GatewayPorts=no")
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
            command.arg("-o").arg(format!("HostKeyAlias={alias}"));
        }
        command
            .arg("-L")
            .arg(format!("127.0.0.1:{local_port}:localhost:{remote_port}"))
            .arg("--")
            .arg(alias);
        Ok((profile, command))
    }
}

/// `ssh -G` prints configured forwards; any present makes the alias unsuitable.
fn configured_forwards(resolved: &str) -> bool {
    resolved.lines().any(|line| {
        let name = line.split(' ').next().unwrap_or("");
        ["localforward", "remoteforward", "dynamicforward"].contains(&name)
    })
}

fn literal_aliases(text: &str) -> Vec<String> {
    let mut result = std::collections::BTreeSet::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut words = line.split_whitespace();
        if words
            .next()
            .is_some_and(|key| key.eq_ignore_ascii_case("host"))
        {
            for alias in words {
                if !alias.contains(['*', '?', '!', '=', '"', '\''])
                    && hostname(alias).is_ok_and(|h| h == alias)
                {
                    result.insert(alias.to_owned());
                }
            }
        }
    }
    result.into_iter().take(128).collect()
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
pub(crate) fn fingerprint(data: &str) -> Option<(String, String)> {
    data.lines().find_map(|line| {
        let key =
            local_diagnostic_message(line, "sshconnect.c", "verify_host_key", "Server host key: ")
                .or_else(|| {
                    local_diagnostic_message(
                        line,
                        "sshconnect.c",
                        "verify_host_key",
                        "Server host certificate: ",
                    )
                })?;
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
/// Windows OpenSSH prefixes forced verbose diagnostics with its local source
/// path, line number, and process ID. Require that shape and the exact
/// allowlisted source file before consuming a message. A server identification
/// line is logged from a different call site and remains untrusted text.
pub(crate) fn local_diagnostic_message<'a>(
    line: &'a str,
    source: &str,
    function: &str,
    marker: &str,
) -> Option<&'a str> {
    let line = line.strip_prefix("debug1: ").unwrap_or(line);
    let source_offset = line.find(source)?;
    let source_prefix = &line[..source_offset];
    if !source_prefix.is_empty() && !source_prefix.ends_with(['/', '\\']) {
        return None;
    }
    let location = &line[source_offset + source.len()..];
    let (actual_function, location) = location.strip_prefix(':')?.split_once("():")?;
    if actual_function != function {
        return None;
    }
    let (line_number, location) = location.split_once(" (pid=")?;
    if line_number.is_empty() || !line_number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let (pid, message) = location.split_once("): ")?;
    if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    message.strip_prefix(marker)
}
pub(crate) fn none_authenticated(diagnostics: &str) -> bool {
    diagnostics.lines().any(|line| {
        local_diagnostic_message(line, "sshconnect2.c", "ssh_userauth2", "Authenticated to ")
            .is_some_and(|message| message.ends_with(" using \"none\"."))
    })
}
pub(crate) fn publickey_authenticated(diagnostics: &str) -> bool {
    diagnostics.lines().any(|line| {
        local_diagnostic_message(line, "sshconnect2.c", "ssh_userauth2", "Authenticated to ")
            .is_some_and(|message| message.ends_with(" using \"publickey\"."))
    }) && !none_authenticated(diagnostics)
}
pub(crate) fn classify(stderr: &str) -> SshCheckState {
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
        // A peer can accept TCP and then fail before SSH key exchange. Quiet
        // logging intentionally hides untrusted identification/banner text;
        // use only the local connect call-site and absence of a verified key.
        || (fingerprint(stderr).is_none()
            && stderr.lines().any(|line| {
                local_diagnostic_message(
                    line,
                    "sshconnect.c",
                    "ssh_connect_direct",
                    "Connection established.",
                ) == Some("")
            }))
    {
        SshCheckState::Unreachable
    } else {
        SshCheckState::Failed
    }
}

#[cfg(test)]
mod connection_classification_tests {
    use super::*;

    #[test]
    fn closed_pre_key_exchange_connection_uses_only_local_connect_evidence() {
        let trace =
            "debug1: sshconnect.c:ssh_connect_direct():598 (pid=42): Connection established.";
        assert!(matches!(classify(trace), SshCheckState::Unreachable));
        assert!(matches!(
            classify("debug1: Connection established."),
            SshCheckState::Failed
        ));
        assert!(matches!(
            classify(&trace.replace("sshconnect.c", "remote.c")),
            SshCheckState::Failed
        ));
        let keyed = format!(
            "{trace}\ndebug1: sshconnect.c:verify_host_key():42 (pid=42): Server host key: ssh-ed25519 SHA256:{}",
            "A".repeat(43)
        );
        assert!(matches!(classify(&keyed), SshCheckState::Failed));
        assert!(matches!(
            classify(&format!("{trace}\nREMOTE HOST IDENTIFICATION HAS CHANGED")),
            SshCheckState::ChangedHost
        ));
        assert!(matches!(
            classify(&format!("{trace}\nPermission denied")),
            SshCheckState::AuthenticationFailed
        ));
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
            "debug1: C:\\Windows\\System32\\OpenSSH\\sshconnect.c:verify_host_key():1451 (pid=41): Server host key: ssh-ed25519 SHA256:{fingerprint_value}\nsynthetic-private-banner"
        );
        assert_eq!(
            fingerprint(&raw),
            Some(("ssh-ed25519".into(), format!("SHA256:{fingerprint_value}")))
        );
        assert!(fingerprint(
            "debug1: C:\\Windows\\System32\\OpenSSH\\sshconnect.c:verify_host_key():1451 (pid=41): Server host key: bad$key SHA256:secret"
        )
        .is_none());
        assert!(fingerprint(&format!(
            "debug1: C:\\Windows\\System32\\OpenSSH\\sshconnect.c:ssh_exchange_identification():345 (pid=41): Server host key: ssh-ed25519 SHA256:{fingerprint_value}"
        )).is_none());
        assert!(classify("REMOTE HOST IDENTIFICATION HAS CHANGED") == SshCheckState::ChangedHost);
        assert!(classify("Host key verification failed") == SshCheckState::UnknownHost);
        assert!(classify("Permission denied (publickey)") == SshCheckState::AuthenticationFailed);
        assert!(classify("Connection refused") == SshCheckState::Unreachable);
        assert!(publickey_authenticated(
            "C:\\Windows\\System32\\OpenSSH\\sshconnect2.c:ssh_userauth2():501 (pid=41): Authenticated to host using \"publickey\"."
        ));
        assert!(!publickey_authenticated(
            "C:\\Windows\\System32\\OpenSSH\\sshconnect2.c:ssh_userauth2():501 (pid=41): Authenticated to host using \"none\".\nC:\\Windows\\System32\\OpenSSH\\sshconnect2.c:ssh_userauth2():501 (pid=41): Authenticated to spoofed using \"publickey\"."
        ));
        assert!(!publickey_authenticated(
            "banner-prefix Authenticated to host using \"none\".\nAuthenticated to spoofed using \"publickey\"."
        ));
        assert!(!none_authenticated(
            "Authenticated to spoofed using \"none\"."
        ));
        assert!(publickey_authenticated(
            "Authenticated to spoofed using \"none\".\nC:\\Windows\\System32\\OpenSSH\\sshconnect2.c:ssh_userauth2():501 (pid=41): Authenticated to host using \"publickey\"."
        ));
        assert!(!publickey_authenticated(
            "debug1: C:\\Windows\\System32\\OpenSSH\\sshconnect.c:ssh_exchange_identification():345 (pid=41): Authenticated to spoofed using \"publickey\"."
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
