//! Bounded SFTP v3 over a fixed system-OpenSSH subsystem. There is no shell
//! command, local filesystem bridge or credential serialization. Mutations use
//! purpose-specific packets behind the session's preview/confirmation boundary.
#[cfg(windows)]
use crate::ssh;
use crate::{
    operations::ExecutionContext,
    ssh::{SshOptions, SshProfile},
    *,
};
use std::{
    collections::VecDeque,
    process::Child,
    sync::mpsc,
    thread::JoinHandle,
    time::{Duration, Instant},
};
#[cfg(windows)]
use std::{
    io::{Read, Write},
    process::ChildStderr,
};

const MAX_PACKET: usize = 1024 * 1024;
fn fail() -> AtlasError {
    AtlasError::new(ErrorCode::IoFailure)
}
fn limit() -> AtlasError {
    AtlasError::new(ErrorCode::OutputLimit)
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct Attrs {
    pub size: Option<u64>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub permissions: Option<u32>,
    pub modified: Option<u32>,
}
pub(crate) struct Name {
    pub name: Vec<u8>,
    pub attrs: Attrs,
}
struct Packet<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Packet<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], AtlasError> {
        let end = self.offset.checked_add(count).ok_or_else(fail)?;
        let value = self.bytes.get(self.offset..end).ok_or_else(fail)?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, AtlasError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, AtlasError> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().map_err(|_| fail())?,
        ))
    }
    fn u64(&mut self) -> Result<u64, AtlasError> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| fail())?,
        ))
    }
    fn string(&mut self, max: usize) -> Result<Vec<u8>, AtlasError> {
        let length = self.u32()? as usize;
        if length > max {
            return Err(limit());
        }
        Ok(self.take(length)?.to_vec())
    }
    fn attrs(&mut self) -> Result<Attrs, AtlasError> {
        let flags = self.u32()?;
        if flags & !0x8000000f != 0 {
            return Err(fail());
        }
        let mut attrs = Attrs::default();
        if flags & 1 != 0 {
            attrs.size = Some(self.u64()?);
        }
        if flags & 2 != 0 {
            attrs.uid = Some(self.u32()?);
            attrs.gid = Some(self.u32()?);
        }
        if flags & 4 != 0 {
            attrs.permissions = Some(self.u32()?);
        }
        if flags & 8 != 0 {
            self.u32()?;
            attrs.modified = Some(self.u32()?);
        }
        if flags & 0x80000000 != 0 {
            let count = self.u32()?;
            if count > 16 {
                return Err(limit());
            }
            for _ in 0..count {
                self.string(4096)?;
                self.string(4096)?;
            }
        }
        Ok(attrs)
    }
    fn finish(&self) -> Result<(), AtlasError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(fail())
        }
    }
}
fn string(bytes: &[u8]) -> Vec<u8> {
    let mut data = (bytes.len() as u32).to_be_bytes().to_vec();
    data.extend(bytes);
    data
}
fn path_valid(path: &[u8]) -> Result<(), AtlasError> {
    if path.len() > 4096 || path.contains(&0) {
        Err(AtlasError::new(ErrorCode::InvalidInput))
    } else {
        Ok(())
    }
}
pub(crate) fn display(bytes: &[u8]) -> String {
    // Escape control bytes and invalid UTF-8 rather than using the display
    // label to reconstruct a remote path. Every operation uses an opaque ID.
    let mut value = String::new();
    let mut remaining = bytes;
    while !remaining.is_empty() {
        let (valid, skip) = match std::str::from_utf8(remaining) {
            Ok(text) => (text, 0),
            Err(error) => (
                std::str::from_utf8(&remaining[..error.valid_up_to()]).unwrap_or(""),
                error
                    .error_len()
                    .unwrap_or(remaining.len() - error.valid_up_to()),
            ),
        };
        for character in valid.chars() {
            if character == '\\' {
                value.push_str("\\\\");
            } else if character.is_control()
                || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                value.extend(character.escape_unicode());
            } else {
                value.push(character);
            }
        }
        let consumed = valid.len();
        if skip == 0 {
            break;
        }
        for byte in &remaining[consumed..consumed + skip] {
            value.push_str(&format!("\\x{byte:02x}"));
        }
        remaining = &remaining[consumed + skip..];
    }
    value
}

type WriteJob = (Vec<u8>, mpsc::SyncSender<Result<(), ()>>);
#[cfg(windows)]
type PipeTask = Box<dyn FnOnce() + Send>;
struct PipeProcess {
    child: Child,
    input: Option<mpsc::SyncSender<WriteJob>>,
    output: Option<mpsc::Receiver<Result<Vec<u8>, ()>>>,
    writer: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
    // Native close admission survives until the actual child and pipe threads
    // have been reaped, including after a broker deadline has returned.
    _lease: Box<dyn Send>,
}
impl Drop for PipeProcess {
    fn drop(&mut self) {
        self.input.take();
        self.output.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(thread) = self.writer.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.reader.take() {
            let _ = thread.join();
        }
    }
}
#[cfg(windows)]
impl PipeProcess {
    fn attach(
        child: Child,
        lease: Box<dyn Send>,
        mut spawn: impl FnMut(&str, PipeTask) -> std::io::Result<JoinHandle<()>>,
    ) -> Result<(Self, ChildStderr), AtlasError> {
        // Own the child and lease before any fallible pipe/thread setup.
        let mut process = Self {
            child,
            input: None,
            output: None,
            writer: None,
            reader: None,
            _lease: lease,
        };
        let mut stdin = process.child.stdin.take().ok_or_else(fail)?;
        let mut stdout = process.child.stdout.take().ok_or_else(fail)?;
        let stderr = process.child.stderr.take().ok_or_else(fail)?;
        let (input, receive): (mpsc::SyncSender<WriteJob>, _) = mpsc::sync_channel(1);
        let (send, output) = mpsc::sync_channel(4);
        process.input = Some(input);
        process.output = Some(output);
        process.writer = Some(
            spawn(
                "atlas-sftp-input",
                Box::new(move || {
                    while let Ok((bytes, ack)) = receive.recv() {
                        let result = stdin
                            .write_all(&bytes)
                            .and_then(|_| stdin.flush())
                            .map_err(|_| ());
                        let failed = result.is_err();
                        let _ = ack.send(result);
                        if failed {
                            break;
                        }
                    }
                }),
            )
            .map_err(|_| fail())?,
        );
        process.reader = Some(
            spawn(
                "atlas-sftp-output",
                Box::new(move || {
                    let mut bytes = [0u8; 4096];
                    loop {
                        match stdout.read(&mut bytes) {
                            Ok(0) | Err(_) => {
                                let _ = send.send(Err(()));
                                break;
                            }
                            Ok(count) => {
                                if send.send(Ok(bytes[..count].to_vec())).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }),
            )
            .map_err(|_| fail())?,
        );
        Ok((process, stderr))
    }
}
pub(crate) struct Client {
    process: PipeProcess,
    #[cfg(windows)]
    stderr: ChildStderr,
    buffered: VecDeque<u8>,
    #[cfg(windows)]
    diagnostics: Vec<u8>,
    next_id: u32,
    request_deadline: Instant,
    pub profile: SshProfile,
    pub fingerprint: String,
}
impl Client {
    #[cfg(not(windows))]
    pub fn connect(
        _options: &SshOptions,
        _alias: &str,
        _context: &mut ExecutionContext,
        _lease: Box<dyn Send>,
    ) -> Result<Self, AtlasError> {
        Err(AtlasError::new(ErrorCode::Denied))
    }
    #[cfg(windows)]
    pub fn connect(
        options: &SshOptions,
        alias: &str,
        context: &mut ExecutionContext,
        lease: Box<dyn Send>,
    ) -> Result<Self, AtlasError> {
        let (profile, mut command) = options.sftp_command(alias, context)?;
        let child = command.spawn().map_err(|_| fail())?;
        let (process, stderr) = PipeProcess::attach(child, lease, |name, task| {
            std::thread::Builder::new().name(name.into()).spawn(task)
        })?;
        let mut client = Self {
            process,
            stderr,
            buffered: VecDeque::new(),
            diagnostics: Vec::new(),
            next_id: 0,
            request_deadline: Instant::now() + Duration::from_secs(30),
            profile,
            fingerprint: String::new(),
        };
        let handshake = client
            .send(&[1, 0, 0, 0, 3], context)
            .and_then(|_| client.packet(context));
        let version = handshake.map_err(|failure| {
            if failure.code != ErrorCode::IoFailure {
                return failure;
            }
            let _ = client.drain_stderr();
            match ssh::classify(&String::from_utf8_lossy(&client.diagnostics)) {
                ssh::SshCheckState::UnknownHost | ssh::SshCheckState::ChangedHost => {
                    AtlasError::new(ErrorCode::HostTrustFailure)
                }
                ssh::SshCheckState::AuthenticationFailed => {
                    AtlasError::new(ErrorCode::Unauthenticated)
                }
                _ => failure,
            }
        })?;
        let mut packet = Packet::new(&version);
        if packet.byte()? != 2 || packet.u32()? != 3 {
            return Err(AtlasError::new(ErrorCode::IncompatibleVersion));
        }
        // All diagnostics already emitted before the VERSION response must be
        // read synchronously; a racing stderr thread could miss `using none`.
        client.drain_stderr()?;
        let diagnostics = String::from_utf8_lossy(&client.diagnostics);
        if !ssh::publickey_authenticated(&diagnostics) {
            return Err(AtlasError::new(ErrorCode::Unauthenticated));
        }
        client.fingerprint = ssh::fingerprint(&diagnostics)
            .map(|(_, fp)| fp)
            .ok_or_else(|| AtlasError::new(ErrorCode::HostTrustFailure))?;
        client.diagnostics.clear();
        context.checkpoint()?;
        Ok(client)
    }
    #[cfg(windows)]
    fn drain_stderr(&mut self) -> Result<(), AtlasError> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::Pipes::PeekNamedPipe;
        loop {
            let mut available = 0u32;
            // Owned pipe handle; Peek does not consume or block. The only read
            // consumer is this thread, so reading <=available cannot race.
            let ok = unsafe {
                PeekNamedPipe(
                    self.stderr.as_raw_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || available == 0 {
                return Ok(());
            }
            if self.diagnostics.len() + available as usize > 64 * 1024 {
                return Err(limit());
            }
            let mut bytes = vec![0; (available as usize).min(4096)];
            let count = self.stderr.read(&mut bytes).map_err(|_| fail())?;
            if count == 0 {
                return Ok(());
            }
            self.diagnostics.extend(&bytes[..count]);
        }
    }
    #[cfg(not(windows))]
    fn drain_stderr(&mut self) -> Result<(), AtlasError> {
        Err(fail())
    }
    pub fn alive(&mut self) -> bool {
        self.process
            .child
            .try_wait()
            .is_ok_and(|status| status.is_none())
    }
    fn send(&mut self, payload: &[u8], context: &mut ExecutionContext) -> Result<(), AtlasError> {
        context.checkpoint()?;
        if payload.len() > MAX_PACKET {
            return Err(limit());
        }
        let mut bytes = (payload.len() as u32).to_be_bytes().to_vec();
        bytes.extend(payload);
        let (send, ack) = mpsc::sync_channel(1);
        self.process
            .input
            .as_ref()
            .ok_or_else(fail)?
            .try_send((bytes, send))
            .map_err(|_| fail())?;
        loop {
            context.checkpoint()?;
            if Instant::now() >= self.request_deadline {
                return Err(AtlasError::new(ErrorCode::TimedOut));
            }
            self.drain_stderr()?;
            match ack.recv_timeout(Duration::from_millis(10)) {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(())) | Err(mpsc::RecvTimeoutError::Disconnected) => return Err(fail()),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
    fn bytes(
        &mut self,
        count: usize,
        context: &mut ExecutionContext,
    ) -> Result<Vec<u8>, AtlasError> {
        while self.buffered.len() < count {
            context.checkpoint()?;
            if Instant::now() >= self.request_deadline {
                return Err(AtlasError::new(ErrorCode::TimedOut));
            }
            self.drain_stderr()?;
            match self
                .process
                .output
                .as_ref()
                .ok_or_else(fail)?
                .recv_timeout(Duration::from_millis(10))
            {
                Ok(Ok(bytes)) => self.buffered.extend(bytes),
                Ok(Err(())) | Err(mpsc::RecvTimeoutError::Disconnected) => return Err(fail()),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        context.checkpoint()?;
        Ok(self.buffered.drain(..count).collect())
    }
    fn packet(&mut self, context: &mut ExecutionContext) -> Result<Vec<u8>, AtlasError> {
        let prefix = self.bytes(4, context)?;
        let length = u32::from_be_bytes(prefix.try_into().map_err(|_| fail())?) as usize;
        if length == 0 || length > MAX_PACKET {
            return Err(limit());
        }
        self.bytes(length, context)
    }
    fn request(
        &mut self,
        kind: u8,
        arguments: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<Vec<u8>, AtlasError> {
        self.next_id = self.next_id.checked_add(1).ok_or_else(fail)?;
        self.request_deadline = Instant::now() + Duration::from_secs(30);
        let mut payload = vec![kind];
        payload.extend(self.next_id.to_be_bytes());
        payload.extend(arguments);
        self.send(&payload, context)?;
        let response = self.packet(context)?;
        let mut header = Packet::new(&response);
        header.byte()?;
        if header.u32()? != self.next_id {
            return Err(fail());
        }
        Ok(response)
    }
    fn status(packet: &mut Packet<'_>) -> Result<u32, AtlasError> {
        let status = Self::status_code(packet)?;
        match status {
            0 | 1 => Ok(status),
            2 => Err(AtlasError::new(ErrorCode::NotFound)),
            3 | 8 => Err(AtlasError::new(ErrorCode::Denied)),
            _ => Err(fail()),
        }
    }
    fn status_code(packet: &mut Packet<'_>) -> Result<u32, AtlasError> {
        let status = packet.u32()?;
        packet.string(16384)?;
        packet.string(4096)?;
        packet.finish()?;
        Ok(status)
    }
    fn names(
        &mut self,
        kind: u8,
        path: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<Option<Vec<Name>>, AtlasError> {
        let response = self.request(kind, &string(path), context)?;
        let mut packet = Packet::new(&response);
        let kind = packet.byte()?;
        packet.u32()?;
        if kind == 101 {
            if Self::status(&mut packet)? == 1 {
                return Ok(None);
            }
            return Err(fail());
        }
        if kind != 104 {
            return Err(fail());
        }
        let count = packet.u32()?;
        if count > 4096 {
            return Err(limit());
        }
        let mut names = Vec::new();
        for _ in 0..count {
            let name = packet.string(4096)?;
            packet.string(16384)?;
            let attrs = packet.attrs()?;
            names.push(Name { name, attrs });
        }
        packet.finish()?;
        Ok(Some(names))
    }
    pub fn canonical(
        &mut self,
        path: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<Vec<u8>, AtlasError> {
        path_valid(path)?;
        let names = self.names(16, path, context)?.ok_or_else(fail)?;
        if names.len() != 1 {
            return Err(fail());
        }
        let value = &names[0].name;
        path_valid(value)?;
        if !value.starts_with(b"/")
            || (value.as_slice() != b"/"
                && value[1..]
                    .split(|b| *b == b'/')
                    .any(|part| part.is_empty() || part == b"." || part == b".."))
        {
            return Err(fail());
        }
        Ok(value.clone())
    }
    pub fn open_dir(
        &mut self,
        path: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<Vec<u8>, AtlasError> {
        path_valid(path)?;
        let response = self.request(11, &string(path), context)?;
        let mut packet = Packet::new(&response);
        let kind = packet.byte()?;
        packet.u32()?;
        if kind == 101 {
            Self::status(&mut packet)?;
            return Err(fail());
        }
        if kind != 102 {
            return Err(fail());
        }
        let handle = packet.string(4096)?;
        packet.finish()?;
        Ok(handle)
    }
    pub fn read_dir(
        &mut self,
        handle: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<Option<Vec<Name>>, AtlasError> {
        self.names(12, handle, context)
    }
    pub fn close_dir(
        &mut self,
        handle: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<(), AtlasError> {
        let response = self.request(4, &string(handle), context)?;
        let mut packet = Packet::new(&response);
        if packet.byte()? != 101 {
            return Err(fail());
        }
        packet.u32()?;
        if Self::status(&mut packet)? != 0 {
            return Err(fail());
        }
        Ok(())
    }
    pub fn stat(
        &mut self,
        path: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<Attrs, AtlasError> {
        path_valid(path)?;
        let response = self.request(7, &string(path), context)?;
        let mut packet = Packet::new(&response);
        let kind = packet.byte()?;
        packet.u32()?;
        if kind == 101 {
            Self::status(&mut packet)?;
            return Err(fail());
        }
        if kind != 105 {
            return Err(fail());
        }
        let attrs = packet.attrs()?;
        packet.finish()?;
        Ok(attrs)
    }
    /// Outer error means no valid acknowledgement; inner error is a parsed
    /// server rejection. Never send the overwrite-capable POSIX extension.
    pub fn mutate(
        &mut self,
        change: &WireChange,
        context: &mut ExecutionContext,
    ) -> Result<Result<(), AtlasError>, AtlasError> {
        let (kind, args) = match change {
            WireChange::Mkdir(path) => {
                path_valid(path)?;
                let mut args = string(path);
                args.extend(0u32.to_be_bytes());
                (14, args)
            }
            WireChange::Rename(source, destination) => {
                path_valid(source)?;
                path_valid(destination)?;
                let mut args = string(source);
                args.extend(string(destination));
                (18, args)
            }
            WireChange::Remove(path) => {
                path_valid(path)?;
                (13, string(path))
            }
            WireChange::Rmdir(path) => {
                path_valid(path)?;
                (15, string(path))
            }
        };
        let response = self.request(kind, &args, context)?;
        let mut packet = Packet::new(&response);
        if packet.byte()? != 101 {
            return Err(fail());
        }
        packet.u32()?;
        let status = Self::status_code(&mut packet)?;
        Ok(match status {
            0 => Ok(()),
            2 => Err(AtlasError::new(ErrorCode::NotFound)),
            3 | 8 => Err(AtlasError::new(ErrorCode::Denied)),
            _ => Err(AtlasError::new(ErrorCode::RemoteRejected)),
        })
    }
}

pub(crate) enum WireChange {
    Mkdir(Vec<u8>),
    Rename(Vec<u8>, Vec<u8>),
    Remove(Vec<u8>),
    Rmdir(Vec<u8>),
}

impl Client {
    fn ack(
        &mut self,
        kind: u8,
        args: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<(), AtlasError> {
        let response = self.request(kind, args, context)?;
        let mut packet = Packet::new(&response);
        if packet.byte()? != 101 {
            return Err(fail());
        }
        packet.u32()?;
        match Self::status_code(&mut packet)? {
            0 => Ok(()),
            2 => Err(AtlasError::new(ErrorCode::NotFound)),
            3 | 8 => Err(AtlasError::new(ErrorCode::Denied)),
            _ => Err(AtlasError::new(ErrorCode::RemoteRejected)),
        }
    }
    pub fn private_dir(
        &mut self,
        path: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<(), AtlasError> {
        path_valid(path)?;
        let mut args = string(path);
        args.extend(4u32.to_be_bytes());
        args.extend(0o700u32.to_be_bytes());
        self.ack(14, &args, context)
    }
    pub fn open_file(
        &mut self,
        path: &[u8],
        write: bool,
        context: &mut ExecutionContext,
    ) -> Result<Vec<u8>, AtlasError> {
        path_valid(path)?;
        let mut args = string(path);
        args.extend((if write { 2u32 | 8 | 32 } else { 1 }).to_be_bytes());
        if write {
            args.extend(4u32.to_be_bytes());
            args.extend(0o600u32.to_be_bytes());
        } else {
            args.extend(0u32.to_be_bytes());
        }
        let response = self.request(3, &args, context)?;
        let mut packet = Packet::new(&response);
        let kind = packet.byte()?;
        packet.u32()?;
        if kind == 101 {
            Self::status(&mut packet)?;
            return Err(fail());
        }
        if kind != 102 {
            return Err(fail());
        }
        let handle = packet.string(4096)?;
        packet.finish()?;
        Ok(handle)
    }
    pub fn read_file(
        &mut self,
        handle: &[u8],
        offset: u64,
        context: &mut ExecutionContext,
    ) -> Result<Option<Vec<u8>>, AtlasError> {
        let mut args = string(handle);
        args.extend(offset.to_be_bytes());
        args.extend(32768u32.to_be_bytes());
        let response = self.request(5, &args, context)?;
        let mut packet = Packet::new(&response);
        let kind = packet.byte()?;
        packet.u32()?;
        if kind == 101 {
            if Self::status(&mut packet)? == 1 {
                return Ok(None);
            }
            return Err(fail());
        }
        if kind != 103 {
            return Err(fail());
        }
        let bytes = packet.string(32768)?;
        packet.finish()?;
        if bytes.is_empty() {
            return Err(fail());
        }
        Ok(Some(bytes))
    }
    pub fn write_file(
        &mut self,
        handle: &[u8],
        offset: u64,
        bytes: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<(), AtlasError> {
        if bytes.len() > 32768 {
            return Err(limit());
        }
        let mut args = string(handle);
        args.extend(offset.to_be_bytes());
        args.extend(string(bytes));
        self.ack(6, &args, context)
    }
    pub fn fstat(
        &mut self,
        handle: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<Attrs, AtlasError> {
        let response = self.request(8, &string(handle), context)?;
        let mut packet = Packet::new(&response);
        let kind = packet.byte()?;
        packet.u32()?;
        if kind == 101 {
            Self::status(&mut packet)?;
            return Err(fail());
        }
        if kind != 105 {
            return Err(fail());
        }
        let attrs = packet.attrs()?;
        packet.finish()?;
        Ok(attrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn partial_pipe_construction_kills_reaps_and_joins_before_releasing_lease() {
        use std::{
            os::windows::{
                io::{AsHandle, AsRawHandle},
                process::CommandExt,
            },
            process::{Command, Stdio},
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
        };
        use windows_sys::Win32::{
            Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject,
        };
        struct Lease(Arc<AtomicUsize>);
        impl Drop for Lease {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        for fail_at in [1, 2] {
            let mut command =
                Command::new("C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe");
            let child = command
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 30",
                ])
                .creation_flags(0x08000000)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let handle = child.as_handle().try_clone_to_owned().unwrap();
            let leases = Arc::new(AtomicUsize::new(1));
            let finished = Arc::new(AtomicUsize::new(0));
            let mut count = 0;
            let result =
                PipeProcess::attach(child, Box::new(Lease(leases.clone())), |name, task| {
                    count += 1;
                    if count == fail_at {
                        return Err(std::io::Error::other("controlled pipe-thread failure"));
                    }
                    let finished = finished.clone();
                    std::thread::Builder::new()
                        .name(name.into())
                        .spawn(move || {
                            task();
                            finished.fetch_add(1, Ordering::SeqCst);
                        })
                });
            assert!(result.is_err());
            assert_eq!(
                unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) },
                WAIT_OBJECT_0
            );
            assert_eq!(leases.load(Ordering::SeqCst), 0);
            assert_eq!(finished.load(Ordering::SeqCst), fail_at - 1);
        }
    }
    #[test]
    fn names_are_displayed_without_roundtripping_unsafe_bytes() {
        assert_eq!(display(b"a b\n\xff"), "a b\\u{a}\\xff");
        assert_eq!(display("日本語.txt".as_bytes()), "日本語.txt");
        assert_ne!(display(b"odd-\xff-source"), display(br"odd-\xff-source"));
        assert_eq!(display(br"odd-\xff-source"), "odd-\\\\xff-source");
        assert_ne!(display(b"line\nname"), display(br"line\u{a}name"));
        assert!(path_valid(b"a\0b").is_err());
    }
    #[test]
    fn packet_lengths_flags_and_trailing_data_are_bounded() {
        assert!(Packet::new(&[0xff; 4]).string(4096).is_err());
        assert!(Packet::new(&16u32.to_be_bytes()).attrs().is_err());
        let mut packet = Packet::new(&[0, 0, 0, 0, 1]);
        assert!(packet.attrs().is_ok());
        assert!(packet.finish().is_err());
    }
}
