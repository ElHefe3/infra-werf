#![cfg(windows)]
//! Explicit real-system OpenSSH fixture proof: mise run test:files.
use atlas_core::{files::*, operations::*, ssh::SshOptions, *};
#[path = "files/mutations.rs"]
mod mutation_proof;
#[path = "files/transfers.rs"]
mod transfer_proof;
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(mut input) = self.0.stdin.take() {
            let _ = input.write_all(b"\n");
        }
        let _ = self.0.wait();
    }
}
struct Lease(Arc<AtomicUsize>);
impl Lease {
    fn new(count: &Arc<AtomicUsize>) -> Box<Self> {
        count.fetch_add(1, Ordering::SeqCst);
        Box::new(Self(count.clone()))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "fixture cleanup deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn code<T>(result: Result<T, AtlasError>) -> ErrorCode {
    match result {
        Ok(_) => panic!("expected denial"),
        Err(error) => error.code,
    }
}
fn page(result: FileResult) -> FilePage {
    match result {
        FileResult::Directory { page } => page,
        _ => panic!("directory expected"),
    }
}
fn list(id: &str) -> FileAction {
    FileAction::List {
        resource_id: id.into(),
        cursor: None,
    }
}

#[test]
#[ignore = "requires pinned loopback SSH fixture; run mise run test:files"]
fn actual_sftp_lifetime_handles_revocation_and_cleanup() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let directory = tempfile::tempdir_in(root.join(".tools")).unwrap();
    let mut command = Command::new(root.join(".tools/ssh-fixture-venv/Scripts/python.exe"));
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
    let mut fixture = Fixture(
        command
            .arg(root.join("scripts/ssh_fixture.py"))
            .arg(directory.path().join("fixture"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(fixture.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let info: serde_json::Value = serde_json::from_str(&line).unwrap();
    let config = std::path::PathBuf::from(info["config"].as_str().unwrap());
    let options = SshOptions::configured(
        std::path::PathBuf::from("C:/Windows/System32/OpenSSH/ssh.exe"),
        config,
    );
    let target = ServerTarget::new(ServerId::parse("fixture").unwrap(), "atlas-trusted").unwrap();
    let broker = OperationBroker::default();
    let actor = broker.issue_backend_authority().unwrap();
    let service = FileSessions::new(
        broker.clone(),
        actor.clone(),
        options.clone(),
        SessionPolicy::default(),
    );
    let leases = Arc::new(AtomicUsize::new(0));
    let open = || {
        service
            .open(
                &target,
                &service.prepare(&target).unwrap().operation_id,
                Lease::new(&leases),
            )
            .unwrap()
    };
    let session = open();
    assert_eq!(leases.load(Ordering::SeqCst), 1);
    let home = page(
        service
            .read(&target, &session.session_id, list(&session.home.id))
            .unwrap(),
    );
    let denied = home
        .entries
        .iter()
        .find(|entry| entry.resource.label == "denied")
        .unwrap();
    assert_eq!(
        code(service.read(&target, &session.session_id, list(&denied.resource.id))),
        ErrorCode::Denied
    );
    let hello = home
        .entries
        .iter()
        .find(|entry| entry.resource.label == "hello.txt")
        .unwrap();
    assert!(matches!(
        service
            .read(
                &target,
                &session.session_id,
                FileAction::Stat {
                    resource_id: hello.resource.id.clone()
                }
            )
            .unwrap(),
        FileResult::Metadata { .. }
    ));
    let large = home
        .entries
        .iter()
        .find(|entry| entry.resource.label == "large")
        .unwrap();
    let first = page(
        service
            .read(&target, &session.session_id, list(&large.resource.id))
            .unwrap(),
    );
    let cursor = first.next_cursor.unwrap();
    let next = || FileAction::List {
        resource_id: first.directory.id.clone(),
        cursor: Some(cursor.clone()),
    };
    let second = page(service.read(&target, &session.session_id, next()).unwrap());
    assert!(!second.entries.is_empty());
    assert_eq!(
        code(service.read(&target, &session.session_id, next())),
        ErrorCode::Denied
    );
    assert_eq!(
        code(service.read(
            &target,
            &session.session_id,
            FileAction::Stat {
                resource_id: first.entries[0].resource.id.clone()
            }
        )),
        ErrorCode::Denied
    );
    let stale_mutation = mutation_proof::journey(
        &service,
        &target,
        &session.session_id,
        &second,
        &directory.path().join("fixture"),
    );
    service.close(&session.session_id).unwrap();
    assert_eq!(
        code(service.commit_change(&target, &session.session_id, &stale_mutation)),
        ErrorCode::ExpiredSession
    );
    assert_eq!(
        service.take_closed(),
        vec![Some(session.session_id.clone())]
    );
    assert_eq!(
        code(service.read(&target, &session.session_id, list(&second.directory.id))),
        ErrorCode::ExpiredSession
    );
    wait_for(|| leases.load(Ordering::SeqCst) == 0);
    let session = open();
    broker.revoke(&actor).unwrap();
    assert_eq!(service.reap(), vec![session.session_id]);
    wait_for(|| leases.load(Ordering::SeqCst) == 0);

    for (idle, absolute) in [
        (Duration::from_millis(80), Duration::from_secs(10)),
        (Duration::from_secs(10), Duration::from_millis(80)),
    ] {
        let broker = OperationBroker::default();
        let actor = broker.issue_backend_authority().unwrap();
        let service = FileSessions::new(
            broker,
            actor,
            options.clone(),
            SessionPolicy::shortened(idle, absolute).unwrap(),
        );
        let session = service
            .open(
                &target,
                &service.prepare(&target).unwrap().operation_id,
                Lease::new(&leases),
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(120));
        assert_eq!(service.reap(), vec![session.session_id.clone()]);
        assert_eq!(
            code(service.read(&target, &session.session_id, list(&session.home.id))),
            ErrorCode::ExpiredSession
        );
        wait_for(|| leases.load(Ordering::SeqCst) == 0);
    }
    let broker = OperationBroker::new(Duration::from_secs(1), 65536).unwrap();
    let actor = broker.issue_backend_authority().unwrap();
    let slow = ServerTarget::new(ServerId::parse("slow").unwrap(), "atlas-slow").unwrap();
    let service = FileSessions::new(
        broker.clone(),
        actor,
        options.clone(),
        SessionPolicy::default(),
    );
    assert_eq!(
        code(service.open(
            &slow,
            &service.prepare(&slow).unwrap().operation_id,
            Lease::new(&leases)
        )),
        ErrorCode::TimedOut
    );
    wait_for(|| leases.load(Ordering::SeqCst) == 0 && broker.active_workers() == 0);
    mutation_proof::unknown_outcome(options, &target, &leases, &directory.path().join("fixture"));
    transfer_proof::journey(&root, &directory.path().join("fixture"), &target, &leases);
    println!(
        "PASS: confirmed mutations, raw names/symlink identity, cancellation, collision/stale target/denial/non-recursion and unknown write outcome without retry"
    );
    println!(
        "PASS: actual SFTP paging/opaque handles, lock/revoke, independent idle/absolute expiry, bounded timeout and actual child/lease cleanup"
    );
}
