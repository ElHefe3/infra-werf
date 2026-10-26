//! A bounded Windows ConPTY host for an explicitly authenticated OpenSSH shell.
//! Input and output stay transient and are never written to application storage.
use crate::{AtlasError, ErrorCode};
use std::sync::Arc;

#[derive(Debug)]
pub enum Event {
    Data(Vec<u8>),
    Ended,
}

#[cfg(windows)]
mod windows {
    use super::*;
    use crate::{operations::ExecutionContext, ssh::conversation::Peer};
    use std::{
        ffi::{OsStr, OsString},
        fs::File,
        io::{Read, Write},
        mem::{size_of, zeroed},
        os::windows::{
            ffi::OsStrExt,
            io::{FromRawHandle, RawHandle},
        },
        process::Command,
        ptr::{null, null_mut},
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, SyncSender, TrySendError},
        },
        thread::{self, JoinHandle},
    };
    use tempfile::TempDir;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            Console::{COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole},
            Pipes::CreatePipe,
            Threading::{
                CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
                EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
                STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
            },
        },
    };

    const MAX_INPUT: usize = 16 * 1024;
    const INPUT_QUEUE: usize = 32;
    const OUTPUT_QUEUE: usize = 4;
    const PIPE_CHUNK: usize = 4096;

    fn fail(code: ErrorCode) -> AtlasError {
        AtlasError::new(code)
    }

    struct Handle(HANDLE);
    unsafe impl Send for Handle {}
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    struct AttributeList(Vec<usize>);
    impl AttributeList {
        fn new(console: HPCON) -> Result<Self, AtlasError> {
            let mut required = 0usize;
            unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut required) };
            if required == 0 || required > 64 * 1024 {
                return Err(fail(ErrorCode::IoFailure));
            }
            let mut storage = vec![0usize; required.div_ceil(size_of::<usize>())];
            let list = storage.as_mut_ptr().cast();
            if unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut required) } == 0 {
                return Err(fail(ErrorCode::IoFailure));
            }
            let updated = unsafe {
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                    // This attribute takes the HPCON handle value itself, not
                    // a pointer to a local copy of the handle.
                    console as *const std::ffi::c_void,
                    size_of::<HPCON>(),
                    null_mut(),
                    null(),
                )
            };
            if updated == 0 {
                unsafe { DeleteProcThreadAttributeList(list) };
                return Err(fail(ErrorCode::IoFailure));
            }
            Ok(Self(storage))
        }
        fn as_ptr(&mut self) -> *mut std::ffi::c_void {
            self.0.as_mut_ptr().cast()
        }
    }
    impl Drop for AttributeList {
        fn drop(&mut self) {
            unsafe { DeleteProcThreadAttributeList(self.0.as_mut_ptr().cast()) };
        }
    }

    struct PseudoConsole(HPCON);
    unsafe impl Send for PseudoConsole {}
    impl Drop for PseudoConsole {
        fn drop(&mut self) {
            if self.0 != 0 {
                unsafe { ClosePseudoConsole(self.0) };
            }
        }
    }

    fn quote_argument(value: &OsStr) -> Vec<u16> {
        let source = value.encode_wide().collect::<Vec<_>>();
        let needs_quotes = source.is_empty()
            || source
                .iter()
                .any(|unit| *unit == 0x20 || *unit == 0x09 || *unit == b'"' as u16);
        if !needs_quotes {
            return source;
        }
        let mut output = vec![b'"' as u16];
        let mut slashes = 0usize;
        for unit in source {
            if unit == b'\\' as u16 {
                slashes += 1;
            } else if unit == b'"' as u16 {
                output.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2 + 1));
                output.push(unit);
                slashes = 0;
            } else {
                output.extend(std::iter::repeat_n(b'\\' as u16, slashes));
                output.push(unit);
                slashes = 0;
            }
        }
        output.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
        output.push(b'"' as u16);
        output
    }

    fn command_line(command: &Command) -> Result<(Vec<u16>, Vec<u16>), AtlasError> {
        let program = command.get_program();
        let mut line = quote_argument(program);
        for argument in command.get_args() {
            line.push(b' ' as u16);
            line.extend(quote_argument(argument));
        }
        line.push(0);
        let mut application = program.encode_wide().collect::<Vec<_>>();
        if application.is_empty() || application.contains(&0) {
            return Err(fail(ErrorCode::InvalidInput));
        }
        application.push(0);
        Ok((application, line))
    }

    fn environment_block(command: &Command) -> Result<Vec<u16>, AtlasError> {
        let mut variables = std::env::vars_os().collect::<Vec<(OsString, OsString)>>();
        for (key, value) in command.get_envs() {
            let position = variables.iter().position(|(existing, _)| {
                existing
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&key.to_string_lossy())
            });
            match (position, value) {
                (Some(position), Some(value)) => variables[position].1 = value.to_os_string(),
                (Some(position), None) => {
                    variables.remove(position);
                }
                (None, Some(value)) => variables.push((key.to_os_string(), value.to_os_string())),
                (None, None) => {}
            }
        }
        variables.sort_by_cached_key(|(key, _)| key.to_string_lossy().to_lowercase());
        let mut block = Vec::new();
        for (key, value) in variables {
            let name = key.encode_wide().collect::<Vec<_>>();
            let value = value.encode_wide().collect::<Vec<_>>();
            if name.is_empty() || name.contains(&0) || value.contains(&0) {
                return Err(fail(ErrorCode::InvalidInput));
            }
            block.extend(name);
            block.push(b'=' as u16);
            block.extend(value);
            block.push(0);
        }
        block.push(0);
        if block.len() == 1 {
            block.push(0);
        }
        Ok(block)
    }

    struct Child {
        console: PseudoConsole,
        process: Handle,
        input: File,
        output: File,
    }
    impl Child {
        fn spawn(command: &Command, cols: u16, rows: u16) -> Result<Self, AtlasError> {
            if !(2..=500).contains(&cols) || !(2..=300).contains(&rows) {
                return Err(fail(ErrorCode::InvalidInput));
            }
            let (mut in_read, mut in_write, mut out_read, mut out_write): (
                HANDLE,
                HANDLE,
                HANDLE,
                HANDLE,
            ) = (null_mut(), null_mut(), null_mut(), null_mut());
            if unsafe { CreatePipe(&mut in_read, &mut in_write, null(), 0) } == 0
                || unsafe { CreatePipe(&mut out_read, &mut out_write, null(), 0) } == 0
            {
                for handle in [in_read, in_write, out_read, out_write] {
                    if !handle.is_null() {
                        unsafe { CloseHandle(handle) };
                    }
                }
                return Err(fail(ErrorCode::IoFailure));
            }
            let in_read = Handle(in_read);
            let in_write = Handle(in_write);
            let out_read = Handle(out_read);
            let out_write = Handle(out_write);
            let mut hpc = 0 as HPCON;
            let created = unsafe {
                CreatePseudoConsole(
                    COORD {
                        X: cols as i16,
                        Y: rows as i16,
                    },
                    in_read.0,
                    out_write.0,
                    0,
                    &mut hpc,
                )
            };
            if created < 0 {
                return Err(fail(ErrorCode::IoFailure));
            }
            let console = PseudoConsole(hpc);
            let mut attributes = AttributeList::new(console.0)?;
            let (application, mut line) = command_line(command)?;
            let mut environment = environment_block(command)?;
            let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
            startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            startup.StartupInfo.dwFlags |= STARTF_USESTDHANDLES;
            startup.lpAttributeList = attributes.as_ptr();
            let mut information: PROCESS_INFORMATION = unsafe { zeroed() };
            let started = unsafe {
                CreateProcessW(
                    application.as_ptr(),
                    line.as_mut_ptr(),
                    null(),
                    null(),
                    0,
                    EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                    environment.as_mut_ptr().cast(),
                    null(),
                    (&startup as *const STARTUPINFOEXW).cast(),
                    &mut information,
                )
            };
            if started == 0 {
                return Err(fail(ErrorCode::IoFailure));
            }
            drop(attributes);
            drop(in_read);
            drop(out_write);
            drop(Handle(information.hThread));
            let input = unsafe { File::from_raw_handle(in_write.0 as RawHandle) };
            std::mem::forget(in_write);
            let output = unsafe { File::from_raw_handle(out_read.0 as RawHandle) };
            std::mem::forget(out_read);
            Ok(Self {
                console,
                process: Handle(information.hProcess),
                input,
                output,
            })
        }
    }

    pub struct Process {
        console: Mutex<Option<PseudoConsole>>,
        process: Handle,
        input: Mutex<Option<SyncSender<Vec<u8>>>>,
        closing: Arc<AtomicBool>,
        ended: Arc<AtomicBool>,
        writer: Option<JoinHandle<()>>,
        reader: Option<JoinHandle<()>>,
        relay: Option<JoinHandle<()>>,
        _log_directory: TempDir,
        _peer: Peer,
    }
    unsafe impl Send for Process {}

    impl Process {
        #[allow(clippy::too_many_arguments)]
        pub fn start(
            peer: Peer,
            cols: u16,
            rows: u16,
            context: &mut ExecutionContext,
            valid: &dyn Fn() -> Result<(), AtlasError>,
            on_event: Arc<dyn Fn(Event) + Send + Sync>,
        ) -> Result<Self, AtlasError> {
            context.checkpoint()?;
            valid()?;
            let log_directory = TempDir::new().map_err(|_| fail(ErrorCode::IoFailure))?;
            let log_path = log_directory.path().join("openssh.log");
            let command = peer.terminal_command(&log_path);
            let Child {
                console,
                process,
                input,
                output,
            } = Child::spawn(&command, cols, rows)?;
            let closing = Arc::new(AtomicBool::new(false));
            let ended = Arc::new(AtomicBool::new(false));
            let mut result = Self {
                console: Mutex::new(Some(console)),
                process,
                input: Mutex::new(None),
                closing: closing.clone(),
                ended: ended.clone(),
                writer: None,
                reader: None,
                relay: None,
                _log_directory: log_directory,
                _peer: peer,
            };
            let (input_send, input_receive) = mpsc::sync_channel::<Vec<u8>>(INPUT_QUEUE);
            *result
                .input
                .lock()
                .map_err(|_| fail(ErrorCode::IoFailure))? = Some(input_send);
            result.writer = Some(
                thread::Builder::new()
                    .name("atlas-terminal-input".into())
                    .spawn(move || {
                        let mut input = input;
                        while let Ok(bytes) = input_receive.recv() {
                            if input.write_all(&bytes).and_then(|_| input.flush()).is_err() {
                                break;
                            }
                        }
                    })
                    .map_err(|_| fail(ErrorCode::IoFailure))?,
            );
            let (send, receive) = mpsc::sync_channel::<Option<Vec<u8>>>(OUTPUT_QUEUE);
            let relay_closing = closing.clone();
            let relay_ended = ended.clone();
            result.relay = Some(
                thread::Builder::new()
                    .name("atlas-terminal-events".into())
                    .spawn(move || {
                        while let Ok(message) = receive.recv() {
                            match message {
                                Some(bytes) if !relay_closing.load(Ordering::Acquire) => {
                                    on_event(Event::Data(bytes))
                                }
                                Some(_) => {}
                                None => {
                                    relay_ended.store(true, Ordering::Release);
                                    if !relay_closing.load(Ordering::Acquire) {
                                        on_event(Event::Ended);
                                    }
                                }
                            }
                        }
                    })
                    .map_err(|_| fail(ErrorCode::IoFailure))?,
            );
            let reader_ended = ended.clone();
            result.reader = Some(
                thread::Builder::new()
                    .name("atlas-terminal-output".into())
                    .spawn(move || {
                        let mut output = output;
                        let mut buffer = [0u8; PIPE_CHUNK];
                        loop {
                            match output.read(&mut buffer) {
                                Ok(0) | Err(_) => break,
                                Ok(count) if send.send(Some(buffer[..count].to_vec())).is_err() => {
                                    break;
                                }
                                Ok(_) => {}
                            }
                        }
                        reader_ended.store(true, Ordering::Release);
                        let _ = send.send(None);
                    })
                    .map_err(|_| fail(ErrorCode::IoFailure))?,
            );

            context.checkpoint()?;
            valid()?;
            if result.is_closed() {
                return Err(fail(ErrorCode::IoFailure));
            }
            Ok(result)
        }

        pub fn write(&self, bytes: &[u8]) -> Result<(), AtlasError> {
            if self.is_closed() {
                return Err(fail(ErrorCode::ExpiredSession));
            }
            if bytes.len() > MAX_INPUT {
                return Err(fail(ErrorCode::OutputLimit));
            }
            let sender = self
                .input
                .lock()
                .map_err(|_| fail(ErrorCode::IoFailure))?
                .as_ref()
                .cloned()
                .ok_or_else(|| fail(ErrorCode::ExpiredSession))?;
            sender
                .try_send(bytes.to_vec())
                .map_err(|failure| match failure {
                    TrySendError::Full(_) => fail(ErrorCode::Conflict),
                    TrySendError::Disconnected(_) => fail(ErrorCode::ExpiredSession),
                })
        }

        pub fn resize(&self, cols: u16, rows: u16) -> Result<(), AtlasError> {
            if !(2..=500).contains(&cols) || !(2..=300).contains(&rows) {
                return Err(fail(ErrorCode::InvalidInput));
            }
            if self.is_closed() {
                return Err(fail(ErrorCode::ExpiredSession));
            }
            let console = self
                .console
                .lock()
                .map_err(|_| fail(ErrorCode::IoFailure))?;
            let Some(console) = console.as_ref() else {
                return Err(fail(ErrorCode::ExpiredSession));
            };
            if unsafe {
                ResizePseudoConsole(
                    console.0,
                    COORD {
                        X: cols as i16,
                        Y: rows as i16,
                    },
                )
            } < 0
            {
                return Err(fail(ErrorCode::IoFailure));
            }
            Ok(())
        }

        pub fn is_closed(&self) -> bool {
            self.closing.load(Ordering::Acquire) || self.ended.load(Ordering::Acquire)
        }

        pub fn close(&mut self) {
            if self.closing.swap(true, Ordering::AcqRel) {
                return;
            }
            if let Ok(input) = self.input.get_mut() {
                input.take();
            }
            if let Ok(console) = self.console.get_mut() {
                if let Some(console) = console.take() {
                    drop(console);
                }
            }
            match unsafe { WaitForSingleObject(self.process.0, 5000) } {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => {
                    unsafe { TerminateProcess(self.process.0, 1) };
                    unsafe { WaitForSingleObject(self.process.0, 5000) };
                }
                _ => {}
            }
            for handle in [self.writer.take(), self.reader.take(), self.relay.take()]
                .into_iter()
                .flatten()
            {
                let _ = handle.join();
            }
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            self.close();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::quote_argument;
        use std::ffi::OsStr;

        fn quoted(value: &str) -> String {
            String::from_utf16(&quote_argument(OsStr::new(value))).unwrap()
        }

        #[test]
        fn windows_command_line_quotes_spaces_quotes_and_trailing_slashes() {
            assert_eq!(quoted(""), "\"\"");
            assert_eq!(quoted("ssh.exe"), "ssh.exe");
            assert_eq!(
                quoted("C:\\Program Files\\OpenSSH\\ssh.exe"),
                "\"C:\\Program Files\\OpenSSH\\ssh.exe\""
            );
            assert_eq!(quoted("a\"b"), "\"a\\\"b\"");
            assert_eq!(quoted("C:\\Program Files\\"), "\"C:\\Program Files\\\\\"");
        }
    }
}

#[cfg(windows)]
pub struct Process(windows::Process);
#[cfg(not(windows))]
pub struct Process;

impl Process {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start(
        peer: crate::ssh::conversation::Peer,
        cols: u16,
        rows: u16,
        context: &mut crate::operations::ExecutionContext,
        valid: &dyn Fn() -> Result<(), AtlasError>,
        on_event: Arc<dyn Fn(Event) + Send + Sync>,
    ) -> Result<Self, AtlasError> {
        #[cfg(windows)]
        {
            windows::Process::start(peer, cols, rows, context, valid, on_event).map(Self)
        }
        #[cfg(not(windows))]
        {
            let _ = (peer, cols, rows, context, valid, on_event);
            Err(AtlasError::new(ErrorCode::Denied))
        }
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), AtlasError> {
        #[cfg(windows)]
        {
            self.0.write(bytes)
        }
        #[cfg(not(windows))]
        {
            let _ = bytes;
            Err(AtlasError::new(ErrorCode::Denied))
        }
    }
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), AtlasError> {
        #[cfg(windows)]
        {
            self.0.resize(cols, rows)
        }
        #[cfg(not(windows))]
        {
            let _ = (cols, rows);
            Err(AtlasError::new(ErrorCode::Denied))
        }
    }
    pub fn is_closed(&self) -> bool {
        #[cfg(windows)]
        {
            self.0.is_closed()
        }
        #[cfg(not(windows))]
        {
            true
        }
    }
    pub fn close(&mut self) {
        #[cfg(windows)]
        self.0.close();
    }
}
