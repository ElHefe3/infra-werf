//! Managed channels pin the authenticated peer before remote execution and
//! wait for genuine public-key authentication before releasing an input gate.
use super::*;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Write};
use tempfile::TempDir;
use zeroize::Zeroizing;

#[derive(Clone)]
pub(crate) struct HostKey {
    algorithm: String,
    public: String,
}
pub(crate) fn known_host_sources(resolved: &str) -> Vec<String> {
    resolved
        .lines()
        .filter_map(|line| {
            line.strip_prefix("userknownhostsfile ")
                .or_else(|| line.strip_prefix("globalknownhostsfile "))
        })
        .map(normalize)
        .collect()
}
fn normalize(value: &str) -> String {
    let value = value.replace('\\', "/");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}
fn public_key(line: &str, expected: &str) -> Option<HostKey> {
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() < 3
        || fields[0].starts_with('@')
        || ![
            "ssh-ed25519",
            "ssh-rsa",
            "ecdsa-sha2-nistp256",
            "ecdsa-sha2-nistp384",
            "ecdsa-sha2-nistp521",
        ]
        .contains(&fields[1])
        || fields[2].len() > 16384
    {
        return None;
    }
    let raw = STANDARD.decode(fields[2]).ok()?;
    (format!("SHA256:{}", STANDARD_NO_PAD.encode(Sha256::digest(raw))) == expected).then(|| {
        HostKey {
            algorithm: fields[1].into(),
            public: fields[2].into(),
        }
    })
}
/// Read only the public trust-file line already used by strict OpenSSH. A
/// diagnostic path must match a configured public trust source, so remote
/// stderr cannot redirect this reader to an identity/private file.
pub(crate) fn authenticated_host_key(
    sources: &[String],
    diagnostics: &str,
    expected: &str,
) -> Option<HostKey> {
    use std::io::{BufRead, BufReader};
    diagnostics
        .lines()
        .filter_map(|line| {
            super::local_diagnostic_message(line, "sshconnect.c", "check_host_key", "Found key in ")
        })
        .find_map(|reference| {
            let (path, line) = reference.rsplit_once(':')?;
            let index: usize = line.parse().ok()?;
            if path.len() > 4096 || index == 0 || index > 100000 {
                return None;
            }
            let normalized = normalize(path);
            let allowed = sources.iter().any(|source| {
                source.match_indices(&normalized).any(|(start, _)| {
                    let end = start + normalized.len();
                    (start == 0 || source[..start].ends_with([' ', '\t', '\"', '\'']))
                        && (end == source.len()
                            || source[end..].starts_with([' ', '\t', '\"', '\'']))
                })
            });
            if !allowed {
                return None;
            }
            let file = File::open(path).ok()?;
            let line = BufReader::new(file.take(1024 * 1024))
                .lines()
                .nth(index - 1)?
                .ok()?;
            public_key(&line, expected)
        })
}

fn fail(code: ErrorCode) -> AtlasError {
    AtlasError::new(code)
}

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
pub(crate) struct Peer {
    options: SshOptions,
    profile: SshProfile,
    fingerprint: String,
    pin: TempDir,
    algorithm: String,
}
pub(crate) struct Reply {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub prompted: bool,
}
impl SshOptions {
    pub(crate) fn pin_peer(
        &self,
        expected: &SshProfile,
        fingerprint: &str,
        key: &HostKey,
        context: &mut ExecutionContext,
    ) -> Result<Peer, AtlasError> {
        let mut resolve = self.command();
        resolve.args(["-G", "--", &expected.alias]);
        let resolved = capture(resolve, context).map_err(process_error)?;
        let current = profile(&expected.alias, &resolved.stdout)
            .filter(|_| resolved.success)
            .ok_or_else(|| fail(ErrorCode::Denied))?
            .0;
        if current != *expected {
            return Err(fail(ErrorCode::HostTrustFailure));
        }
        let key = public_key(
            &format!("atlas-session-pin {} {}", key.algorithm, key.public),
            fingerprint,
        )
        .ok_or_else(|| fail(ErrorCode::HostTrustFailure))?;
        let pin = tempfile::tempdir().map_err(|_| fail(ErrorCode::IoFailure))?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(pin.path().join("known_hosts"))
            .map_err(|_| fail(ErrorCode::IoFailure))?;
        writeln!(file, "atlas-session-pin {} {}", key.algorithm, key.public)
            .map_err(|_| fail(ErrorCode::IoFailure))?;
        file.flush().map_err(|_| fail(ErrorCode::IoFailure))?;
        let algorithm = if key.algorithm == "ssh-rsa" {
            "rsa-sha2-512,rsa-sha2-256".into()
        } else {
            key.algorithm
        };
        Ok(Peer {
            options: self.clone(),
            profile: current,
            fingerprint: fingerprint.into(),
            pin,
            algorithm,
        })
    }
}
fn process_error(value: ProcessFailure) -> AtlasError {
    match value {
        ProcessFailure::Missing => fail(ErrorCode::IoFailure),
        ProcessFailure::Guard(error) => error,
    }
}
impl Peer {
    fn pinned_options(&self, command: &mut Command) {
        command
            .arg("-o")
            .arg("SessionType=default")
            .arg("-o")
            .arg("KnownHostsCommand=none")
            .arg("-o")
            .arg("ProxyCommand=none")
            .arg("-o")
            .arg("ProxyJump=none")
            .arg("-o")
            .arg(format!("Hostname={}", self.profile.hostname))
            .arg("-o")
            .arg("HostKeyAlias=atlas-session-pin")
            .arg("-o")
            .arg(format!("HostKeyAlgorithms={}", self.algorithm))
            .arg("-o")
            .arg(format!(
                "UserKnownHostsFile=\"{}\"",
                self.pin
                    .path()
                    .join("known_hosts")
                    .to_string_lossy()
                    .replace('\\', "/")
            ))
            .arg("-o")
            .arg("GlobalKnownHostsFile=none")
            .arg("-l")
            .arg(&self.profile.user)
            .arg("-p")
            .arg(self.profile.port.to_string());
    }
    pub(crate) fn terminal_command(&self, log_path: &Path) -> Command {
        let mut command = self.options.command();
        command
            .env("TERM", "xterm-256color")
            .arg("-E")
            .arg(log_path)
            .arg("-tt");
        self.pinned_options(&mut command);
        command.arg("--").arg(&self.profile.alias);
        command
    }
    pub(crate) fn run(
        &self,
        args: &[&str],
        password: Option<(&str, &Zeroizing<String>)>,
        context: &mut ExecutionContext,
        valid: &dyn Fn() -> Result<(), AtlasError>,
    ) -> Result<Reply, AtlasError> {
        valid()?;
        context.checkpoint()?;
        let gate = format!("ATLAS_GATE_{}", OperationId::generate().as_str());
        let remote_command = args
            .iter()
            .map(|value| quote(value))
            .collect::<Vec<_>>()
            .join(" ");
        // With a password prompt, merge only the remote command's stderr into
        // its stdout. OpenSSH's local authentication evidence stays on the
        // separate local stderr pipe.
        let remote_command = if password.is_some() {
            format!("{remote_command} 2>&1")
        } else {
            remote_command
        };
        let script = format!(
            "IFS= read -r atlas_gate && [ \"$atlas_gate\" = {} ] || exit 126; exec {remote_command}",
            quote(&gate),
        );
        let mut command = self.options.command();
        command.stdin(Stdio::piped());
        self.pinned_options(&mut command);
        command
            .arg("--")
            .arg(&self.profile.alias)
            .args(["/bin/sh", "-c", &quote(&script)]);
        conversation(command, &self.fingerprint, &gate, password, context, valid)
    }
}
fn spawn_reader<R: Read + Send + 'static>(
    mut pipe: R,
    send: mpsc::SyncSender<Chunk>,
    stderr: bool,
) -> Result<std::thread::JoinHandle<()>, AtlasError> {
    std::thread::Builder::new()
        .name("atlas-managed-output".into())
        .spawn(move || {
            let mut bytes = [0u8; 4096];
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
        .map_err(|_| fail(ErrorCode::IoFailure))
}
fn conversation(
    mut command: Command,
    fingerprint_expected: &str,
    gate: &str,
    password: Option<(&str, &Zeroizing<String>)>,
    context: &mut ExecutionContext,
    valid: &dyn Fn() -> Result<(), AtlasError>,
) -> Result<Reply, AtlasError> {
    let mut child = RunningChild(command.spawn().map_err(|_| fail(ErrorCode::IoFailure))?);
    let mut input = child.0.stdin.take();
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| fail(ErrorCode::IoFailure))?;
    let stderr = child
        .0
        .stderr
        .take()
        .ok_or_else(|| fail(ErrorCode::IoFailure))?;
    let (send, receive) = mpsc::sync_channel(4);
    let out_reader = spawn_reader(stdout, send.clone(), false)?;
    let err_reader = match spawn_reader(stderr, send, true) {
        Ok(reader) => reader,
        Err(error) => {
            drop(receive);
            drop(child);
            let _ = out_reader.join();
            return Err(error);
        }
    };
    let result = (|| {
        let mut out = Zeroizing::new(Vec::new());
        // LogLevel=QUIET suppresses server-controlled auth banners; the
        // configured LogVerbose patterns selectively restore OpenSSH's local
        // host-key and auth method diagnostics on this pipe.
        let mut authentication_trace = Zeroizing::new(Vec::new());
        let mut finished = 0;
        let mut admitted = false;
        let mut prompted = false;
        loop {
            context.checkpoint()?;
            valid()?;
            match receive.recv_timeout(Duration::from_millis(10)) {
                Ok(Chunk::Data(stderr, bytes)) => {
                    context.account_output(bytes.len())?;
                    if stderr {
                        if !admitted {
                            authentication_trace.extend_from_slice(&bytes);
                        }
                    } else {
                        out.extend_from_slice(&bytes);
                    }
                }
                Ok(Chunk::Done) => finished += 1,
                Ok(Chunk::Failed) => return Err(fail(ErrorCode::IoFailure)),
                Err(mpsc::RecvTimeoutError::Disconnected) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let trace = String::from_utf8_lossy(&authentication_trace);
            if !admitted
                && fingerprint(&trace).is_some_and(|(_, actual)| actual != fingerprint_expected)
            {
                return Err(fail(ErrorCode::HostTrustFailure));
            }
            if !admitted && none_authenticated(&trace) {
                return Err(fail(ErrorCode::Unauthenticated));
            }
            if !admitted && publickey_authenticated(&trace) {
                if fingerprint(&trace).is_none_or(|(_, value)| value != fingerprint_expected) {
                    return Err(fail(ErrorCode::HostTrustFailure));
                }
                context.checkpoint()?;
                valid()?;
                let pipe = input.as_mut().ok_or_else(|| fail(ErrorCode::IoFailure))?;
                writeln!(pipe, "{gate}").map_err(|_| fail(ErrorCode::IoFailure))?;
                admitted = true;
                if password.is_none() {
                    input.take();
                }
            }
            if admitted
                && !prompted
                && let Some((prompt, secret)) = password
                && out
                    .windows(prompt.len())
                    .any(|bytes| bytes == prompt.as_bytes())
            {
                context.checkpoint()?;
                valid()?;
                let mut pipe = input.take().ok_or_else(|| fail(ErrorCode::IoFailure))?;
                pipe.write_all(secret.as_bytes())
                    .and_then(|_| pipe.write_all(b"\n"))
                    .map_err(|_| fail(ErrorCode::IoFailure))?;
                prompted = true;
            }
            if finished == 2
                && let Some(status) = child.0.try_wait().map_err(|_| fail(ErrorCode::IoFailure))?
            {
                if !admitted {
                    return Err(fail(
                        if matches!(
                            classify(&trace),
                            SshCheckState::UnknownHost | SshCheckState::ChangedHost
                        ) {
                            ErrorCode::HostTrustFailure
                        } else if trace.contains("using \"none\"")
                            || classify(&trace) == SshCheckState::AuthenticationFailed
                        {
                            ErrorCode::Unauthenticated
                        } else {
                            ErrorCode::IoFailure
                        },
                    ));
                }
                context.checkpoint()?;
                valid()?;
                return Ok(Reply {
                    success: status.success(),
                    exit_code: status.code(),
                    stdout: password.map_or_else(
                        || String::from_utf8_lossy(&out).into_owned(),
                        |(prompt, _)| String::from_utf8_lossy(&out).replace(prompt, ""),
                    ),
                    prompted,
                });
            }
        }
    })();
    drop(input);
    drop(receive);
    drop(child);
    let _ = out_reader.join();
    let _ = err_reader.join();
    result
}
