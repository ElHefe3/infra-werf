//! Real, disposable local Linux fixture; credentials never appear in Debug/logs.
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Output, Stdio},
};
pub struct Linux {
    child: Child,
    pub info: serde_json::Value,
    _directory: tempfile::TempDir,
}
impl Linux {
    pub fn start(root: &Path) -> Self {
        let directory = tempfile::tempdir_in(root.join(".tools")).unwrap();
        let mut command = Command::new(root.join(".tools/ssh-fixture-venv/Scripts/python.exe"));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .arg(root.join("scripts/linux_fixture.py"))
            .arg(directory.path().join("fixture"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert!(!line.is_empty(), "Linux fixture unavailable");
        let info = serde_json::from_str(&line).unwrap();
        Self {
            child,
            info,
            _directory: directory,
        }
    }
    pub fn options(&self) -> atlas_core::ssh::SshOptions {
        atlas_core::ssh::SshOptions::configured(
            "C:/Windows/System32/OpenSSH/ssh.exe".into(),
            self.info["config"].as_str().unwrap().into(),
        )
    }
    pub fn exec(&self, args: &[&str], input: Option<&[u8]>) -> Output {
        let mut command = Command::new("podman");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .args([
                "--connection",
                "podman-machine-default",
                "exec",
                "-i",
                self.info["container"].as_str().unwrap(),
            ])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        if let Some(input) = input {
            child.stdin.take().unwrap().write_all(input).unwrap();
        } else {
            child.stdin.take();
        }
        child.wait_with_output().unwrap()
    }
    pub fn password(&self) -> String {
        self.info["password"].as_str().unwrap().into()
    }
}
impl Drop for Linux {
    fn drop(&mut self) {
        if let Some(mut input) = self.child.stdin.take() {
            let _ = input.write_all(b"\n");
        }
        let _ = self.child.wait();
    }
}
