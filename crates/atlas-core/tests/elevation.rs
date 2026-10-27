#![cfg(windows)]
use atlas_core::ssh::{SshCheckState, SshVerification};
use atlas_core::{files::*, operations::*, *};
#[path = "support/linux.rs"]
mod fixture;
use std::{
    io::Write,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
struct Lease(Arc<AtomicUsize>);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
fn lease(count: &Arc<AtomicUsize>) -> Box<dyn Send> {
    count.fetch_add(1, Ordering::SeqCst);
    Box::new(Lease(count.clone()))
}
fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < end, "fixture cleanup deadline");
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn code<T>(result: Result<T, AtlasError>) -> ErrorCode {
    match result {
        Ok(_) => panic!("expected denied action"),
        Err(error) => error.code,
    }
}
fn open(service: &FileSessions, target: &ServerTarget, leases: &Arc<AtomicUsize>) -> FileSession {
    service
        .open(
            target,
            &service.prepare(target).unwrap().operation_id,
            lease(leases),
            None,
        )
        .unwrap()
}

#[test]
#[ignore = "requires disposable local Linux SSH/sudo image; run mise run test:elevation"]
fn actual_fresh_sudo_and_protected_channel_lifetime() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let linux = fixture::Linux::start(&root);
    let broker = OperationBroker::default();
    let authority = broker.issue_backend_authority().unwrap();
    let service = FileSessions::new(
        broker.clone(),
        authority.clone(),
        linux.options(),
        SessionPolicy::default(),
    );
    let target = ServerTarget::new(ServerId::parse("fixture").unwrap(), "atlas-linux").unwrap();
    let leases = Arc::new(AtomicUsize::new(0));

    // The isolated SSH server sends a banner containing fake OpenSSH host-key
    // and public-key-success diagnostics. Its third account succeeds through
    // SSH's "none" method; neither verification nor the sudo channel may treat
    // those server bytes as local authentication evidence.
    let none_target =
        ServerTarget::new(ServerId::parse("fixture-none").unwrap(), "atlas-linux-none").unwrap();
    let none_session = open(&service, &none_target, &leases);
    assert!(
        linux
            .exec(&["passwd", "-d", "fixture-none"], None)
            .status
            .success()
    );
    assert!(
        linux
            .exec(
                &["rm", "--", "/home/fixture-none/.ssh/authorized_keys"],
                None
            )
            .status
            .success()
    );
    let check = broker
        .prepare_connection_check(Some(&authority), &none_target)
        .unwrap();
    let none_options = linux.options();
    let checked = broker
        .execute_connection_check(
            Some(&authority),
            &check.operation_id,
            &none_target,
            move |_, context| {
                let response = none_options.verify("atlas-linux-none", context)?;
                context.write(&serde_json::to_string(&response).unwrap())
            },
        )
        .unwrap();
    let checked: SshVerification = serde_json::from_str(&checked).unwrap();
    assert_eq!(
        serde_json::to_string(&checked.state).unwrap(),
        serde_json::to_string(&SshCheckState::AuthenticationFailed).unwrap(),
        "the none-auth banner fixture must not verify a connection"
    );
    let preview = service
        .preview_elevation(&none_target, &none_session.session_id)
        .unwrap();
    assert_eq!(
        code(service.confirm_elevation(
            &none_target,
            &none_session.session_id,
            &preview.operation_id,
            linux.password()
        )),
        ErrorCode::Unauthenticated
    );
    assert!(
        !linux
            .exec(&["test", "-e", "/tmp/atlas-none-sudo-password"], None)
            .status
            .success(),
        "server banner spoof caused a sudo password to be sent before public-key authentication"
    );
    service.close(&none_session.session_id).unwrap();

    let session = open(&service, &target, &leases);
    // Minimal utilities and podman-docker compatibility both occur on owner
    // hosts. Reproduce them only inside this disposable fixture container.
    let minimal_tools = r#"
for utility in hostname getconf; do
  printf '#!/bin/sh\nexit 127\n' > /usr/local/bin/$utility
  chmod 755 /usr/local/bin/$utility
done
cp /usr/local/bin/docker /tmp/atlas-docker-before-compat
cat > /usr/local/bin/docker <<'ATLAS_COMPAT'
#!/bin/sh
if [ "$1" = --version ]; then printf 'podman version 99.0.0\n'; exit 0; fi
exit 125
ATLAS_COMPAT
chmod 755 /usr/local/bin/docker
"#;
    assert!(
        linux
            .exec(&["sh", "-c", minimal_tools], None)
            .status
            .success()
    );
    let minimal_inventory = service
        .collect_inventory(&target, &session.session_id)
        .unwrap()
        .inventory
        .expect("procfs fallback collection must be complete");
    assert!(!minimal_inventory.hostname.is_empty());
    assert!(minimal_inventory.cpus > 0);
    assert!(
        minimal_inventory
            .containers
            .iter()
            .any(|row| row.engine == "podman")
    );
    assert!(
        minimal_inventory
            .containers
            .iter()
            .all(|row| row.engine != "docker")
    );
    assert!(
        linux
            .exec(
                &[
                    "mv",
                    "--",
                    "/tmp/atlas-docker-before-compat",
                    "/usr/local/bin/docker"
                ],
                None
            )
            .status
            .success()
    );
    assert!(
        linux
            .exec(
                &[
                    "rm",
                    "--",
                    "/usr/local/bin/hostname",
                    "/usr/local/bin/getconf"
                ],
                None
            )
            .status
            .success()
    );
    let trust = std::fs::read(linux.info["known_hosts"].as_str().unwrap()).unwrap();
    let original_config = std::fs::read(linux.info["config"].as_str().unwrap()).unwrap();
    assert_eq!(
        session.fingerprint,
        linux.info["fingerprint"].as_str().unwrap()
    );
    // Establish an intentionally global timestamp in this container only.
    assert!(
        linux
            .exec(
                &[
                    "runuser",
                    "-u",
                    "fixture",
                    "--",
                    "sudo",
                    "-S",
                    "-p",
                    "[FIXTURE]",
                    "-v"
                ],
                Some(format!("{}\n", linux.password()).as_bytes())
            )
            .status
            .success()
    );
    let preview = service
        .preview_elevation(&target, &session.session_id)
        .unwrap();
    assert!(preview.action.contains("without changing"));
    assert_eq!(preview.server, "atlas-linux");
    assert_eq!(
        code(service.confirm_elevation(
            &target,
            &session.session_id,
            &preview.operation_id,
            "wrong fixture password".to_owned()
        )),
        ErrorCode::Unauthenticated
    );
    assert!(
        linux
            .exec(
                &["runuser", "-u", "fixture", "--", "sudo", "-n", "-N", "-v"],
                None
            )
            .status
            .success(),
        "existing global timestamp was invalidated"
    );
    let preview = service
        .preview_elevation(&target, &session.session_id)
        .unwrap();
    assert!(
        service
            .confirm_elevation(
                &target,
                &session.session_id,
                &preview.operation_id,
                linux.password()
            )
            .unwrap()
            .verified
    );
    assert_eq!(
        code(service.confirm_elevation(
            &target,
            &session.session_id,
            &preview.operation_id,
            linux.password()
        )),
        ErrorCode::Denied
    );
    let preview = service
        .preview_elevation(&target, &session.session_id)
        .unwrap();
    assert_eq!(
        code(service.confirm_elevation(
            &target,
            &session.session_id,
            &preview.operation_id,
            "invalid\ninput".to_owned()
        )),
        ErrorCode::InvalidInput
    );
    service
        .cancel_elevation(&target, &session.session_id, &preview.operation_id)
        .unwrap();
    assert_eq!(
        code(service.confirm_elevation(
            &target,
            &session.session_id,
            &preview.operation_id,
            linux.password()
        )),
        ErrorCode::Denied
    );

    // Protected installation streams the source as the SSH user into a
    // private stage, then performs one exact root-owned placement after a
    // fresh password review. The destination must remain untouched on wrong
    // credentials, cancellation, and collisions.
    let local = tempfile::tempdir().unwrap();
    let payload_path = local.path().join("protected payload.bin");
    let payload = b"fixture-only protected configuration\n";
    std::fs::File::create(&payload_path)
        .unwrap()
        .write_all(payload)
        .unwrap();
    let uid: u32 = String::from_utf8(linux.exec(&["id", "-u", "fixture"], None).stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let gid: u32 = String::from_utf8(linux.exec(&["id", "-g", "fixture"], None).stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let suffix = SessionId::generate().as_str().replace('-', "");
    let destination = format!("/root/atlas-protected-{suffix}");
    let request = ProtectedPlacementRequest {
        destination: destination.clone(),
        owner: uid,
        group: gid,
        mode: 0o640,
    };
    let mut bidi_path = request.clone();
    bidi_path.destination = format!("/root/atlas-{}\u{202e}txt", suffix);
    assert_eq!(
        code(service.preview_protected_placement(&target, &session.session_id, bidi_path)),
        ErrorCode::InvalidInput
    );
    let stage = service
        .stage_protected_upload(
            &target,
            &session.session_id,
            &session.home.id,
            LocalTransfer::upload(&payload_path).unwrap(),
        )
        .unwrap();
    assert!(!stage.cleanup_needed);
    assert_eq!(stage.sha256.as_deref().unwrap().len(), 64);
    let stage_directory = Path::new(&stage.staging_location)
        .parent()
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(
        String::from_utf8(
            linux
                .exec(&["stat", "-c", "%a:%u", stage_directory], None)
                .stdout
        )
        .unwrap()
        .trim(),
        format!("700:{uid}")
    );
    assert_eq!(
        String::from_utf8(
            linux
                .exec(
                    &["stat", "-c", "%a:%u", stage.staging_location.as_str()],
                    None
                )
                .stdout
        )
        .unwrap()
        .trim(),
        format!("600:{uid}")
    );
    let review = service
        .preview_protected_placement(&target, &session.session_id, request.clone())
        .unwrap();
    assert!(review.action.contains(&destination));
    assert_eq!(
        code(service.confirm_protected_placement(
            &target,
            &session.session_id,
            &review.operation_id,
            "wrong placement password".to_owned()
        )),
        ErrorCode::Unauthenticated
    );
    assert!(
        !linux
            .exec(&["test", "-e", &destination], None)
            .status
            .success()
    );
    assert!(
        linux
            .exec(&["test", "-f", &stage.staging_location], None)
            .status
            .success()
    );

    let cancelled = service
        .preview_protected_placement(&target, &session.session_id, request.clone())
        .unwrap();
    service
        .cancel_elevation(&target, &session.session_id, &cancelled.operation_id)
        .unwrap();
    assert_eq!(
        code(service.confirm_protected_placement(
            &target,
            &session.session_id,
            &cancelled.operation_id,
            linux.password()
        )),
        ErrorCode::Denied
    );
    assert!(
        !linux
            .exec(&["test", "-e", &destination], None)
            .status
            .success()
    );

    let review = service
        .preview_protected_placement(&target, &session.session_id, request.clone())
        .unwrap();
    let receipt = service
        .confirm_protected_placement(
            &target,
            &session.session_id,
            &review.operation_id,
            linux.password(),
        )
        .unwrap();
    assert_eq!(receipt.destination, destination);
    assert_eq!(receipt.owner, uid);
    assert_eq!(receipt.group, gid);
    assert_eq!(receipt.mode, "0640");
    assert_eq!(receipt.sha256, stage.sha256.unwrap());
    assert!(!receipt.cleanup_needed);
    assert_eq!(
        linux.exec(&["cat", "--", &destination], None).stdout,
        payload
    );
    assert_eq!(
        String::from_utf8(
            linux
                .exec(&["stat", "-c", "%u:%g:%a", &destination], None)
                .stdout
        )
        .unwrap()
        .trim(),
        format!("{uid}:{gid}:640")
    );
    assert!(
        !linux
            .exec(&["test", "-e", &stage.staging_location], None)
            .status
            .success()
    );

    let collision = format!("{destination}-collision");
    assert!(
        linux
            .exec(
                &["sh", "-c", "printf sentinel > \"$1\"", "sh", &collision],
                None
            )
            .status
            .success()
    );
    let second_stage = service
        .stage_protected_upload(
            &target,
            &session.session_id,
            &session.home.id,
            LocalTransfer::upload(&payload_path).unwrap(),
        )
        .unwrap();
    let collision_request = ProtectedPlacementRequest {
        destination: collision.clone(),
        ..request
    };
    let review = service
        .preview_protected_placement(&target, &session.session_id, collision_request)
        .unwrap();
    assert_eq!(
        code(service.confirm_protected_placement(
            &target,
            &session.session_id,
            &review.operation_id,
            linux.password()
        )),
        ErrorCode::Conflict
    );
    assert_eq!(
        linux.exec(&["cat", "--", &collision], None).stdout,
        b"sentinel"
    );
    assert!(
        linux
            .exec(&["test", "-f", &second_stage.staging_location], None)
            .status
            .success()
    );
    service
        .cancel_protected_placement(&target, &session.session_id, &session.home.id)
        .unwrap();
    assert_eq!(
        code(service.cancel_protected_placement(&target, &session.session_id, &session.home.id,)),
        ErrorCode::NotFound
    );
    service
        .read(
            &target,
            &session.session_id,
            FileAction::List {
                resource_id: session.home.id.clone(),
                cursor: None,
            },
        )
        .unwrap();
    assert!(
        !linux
            .exec(&["test", "-e", &second_stage.staging_location], None)
            .status
            .success()
    );
    assert!(
        linux
            .exec(&["rm", "--", &destination, &collision], None)
            .status
            .success()
    );

    let acknowledgement_session = open(&service, &target, &leases);
    let acknowledgement_stage = service
        .stage_protected_upload(
            &target,
            &acknowledgement_session.session_id,
            &acknowledgement_session.home.id,
            LocalTransfer::upload(&payload_path).unwrap(),
        )
        .unwrap();
    // Model the UI lock landing after staging completed but before the
    // frontend consumed the returned cleanup handle.
    service.close(&acknowledgement_session.session_id).unwrap();
    service
        .cancel_protected_placement(
            &target,
            &acknowledgement_session.session_id,
            &acknowledgement_session.home.id,
        )
        .unwrap();
    assert!(
        !linux
            .exec(
                &["test", "-e", &acknowledgement_stage.staging_location],
                None
            )
            .status
            .success()
    );

    let idle_service = FileSessions::new(
        broker.clone(),
        authority.clone(),
        linux.options(),
        SessionPolicy::shortened(Duration::from_secs(1), Duration::from_secs(10)).unwrap(),
    );
    let idle_session = open(&idle_service, &target, &leases);
    let idle_stage = idle_service
        .stage_protected_upload(
            &target,
            &idle_session.session_id,
            &idle_session.home.id,
            LocalTransfer::upload(&payload_path).unwrap(),
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(1200));
    assert!(idle_service.reap().contains(&idle_session.session_id));
    assert!(
        !linux
            .exec(&["test", "-e", &idle_stage.staging_location], None)
            .status
            .success(),
        "idle expiry did not retry and remove its retained private stage"
    );

    let absolute_service = FileSessions::new(
        broker.clone(),
        authority.clone(),
        linux.options(),
        SessionPolicy::shortened(Duration::from_secs(1), Duration::from_secs(3)).unwrap(),
    );
    let absolute_session = open(&absolute_service, &target, &leases);
    let absolute_stage = absolute_service
        .stage_protected_upload(
            &target,
            &absolute_session.session_id,
            &absolute_session.home.id,
            LocalTransfer::upload(&payload_path).unwrap(),
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(3200));
    assert!(
        absolute_service
            .reap()
            .contains(&absolute_session.session_id)
    );
    assert_eq!(
        code(absolute_service.cancel_protected_placement(
            &target,
            &absolute_session.session_id,
            &absolute_session.home.id,
        )),
        ErrorCode::ExpiredSession
    );
    assert_eq!(
        String::from_utf8(
            linux
                .exec(
                    &[
                        "stat",
                        "-c",
                        "%a:%u",
                        Path::new(&absolute_stage.staging_location)
                            .parent()
                            .unwrap()
                            .to_str()
                            .unwrap(),
                    ],
                    None,
                )
                .stdout
        )
        .unwrap()
        .trim(),
        format!("700:{uid}")
    );
    assert_eq!(
        String::from_utf8(
            linux
                .exec(
                    &[
                        "stat",
                        "-c",
                        "%a:%u",
                        absolute_stage.staging_location.as_str(),
                    ],
                    None,
                )
                .stdout
        )
        .unwrap()
        .trim(),
        format!("600:{uid}")
    );
    let absolute_stage_directory = Path::new(&absolute_stage.staging_location)
        .parent()
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        linux
            .exec(
                &["rm", "--", absolute_stage.staging_location.as_str()],
                None
            )
            .status
            .success()
    );
    assert!(
        linux
            .exec(&["rmdir", "--", absolute_stage_directory], None)
            .status
            .success()
    );

    let staging_session = open(&service, &target, &leases);
    let large_path = local.path().join("cancel during protected staging.bin");
    std::fs::File::create(&large_path)
        .unwrap()
        .set_len(32 * 1024 * 1024)
        .unwrap();
    let worker = service.clone();
    let worker_target = target.clone();
    let worker_session = staging_session.session_id.clone();
    let worker_resource = staging_session.home.id.clone();
    let transfer = std::thread::spawn(move || {
        worker.stage_protected_upload(
            &worker_target,
            &worker_session,
            &worker_resource,
            LocalTransfer::upload(&large_path).unwrap(),
        )
    });
    wait(|| {
        !linux
            .exec(
                &[
                    "find",
                    "/home/fixture",
                    "-maxdepth",
                    "1",
                    "-type",
                    "d",
                    "-name",
                    ".atlas-protected-*",
                    "-print",
                    "-quit",
                ],
                None,
            )
            .stdout
            .is_empty()
    });
    service.release_view(&staging_session.session_id).unwrap();
    assert_eq!(code(transfer.join().unwrap()), ErrorCode::Cancelled);
    wait(|| {
        linux
            .exec(
                &[
                    "find",
                    "/home/fixture",
                    "-maxdepth",
                    "1",
                    "-type",
                    "d",
                    "-name",
                    ".atlas-protected-*",
                    "-print",
                    "-quit",
                ],
                None,
            )
            .stdout
            .is_empty()
    });
    wait(|| leases.load(Ordering::SeqCst) == 1);

    let different =
        ServerTarget::new(ServerId::parse("different").unwrap(), "atlas-linux").unwrap();
    assert_eq!(
        code(service.preview_elevation(&different, &session.session_id)),
        ErrorCode::Denied
    );
    let nopass =
        ServerTarget::new(ServerId::parse("nopass").unwrap(), "atlas-linux-nopass").unwrap();
    let nopass_session = open(&service, &nopass, &leases);
    let preview = service
        .preview_elevation(&nopass, &nopass_session.session_id)
        .unwrap();
    assert_eq!(
        code(service.confirm_elevation(
            &nopass,
            &nopass_session.session_id,
            &preview.operation_id,
            linux.password()
        )),
        ErrorCode::Denied
    );
    service.close(&nopass_session.session_id).unwrap();
    // A real authenticated command is delayed before reading its input gate.
    assert!(
        linux
            .exec(
                &[
                    "sh",
                    "-c",
                    "cat > /home/fixture/.ssh/rc; chown fixture:fixture /home/fixture/.ssh/rc"
                ],
                Some(b"touch /home/fixture/rc-entered\nsleep 60\n")
            )
            .status
            .success()
    );
    let preview = service
        .preview_elevation(&target, &session.session_id)
        .unwrap();
    let worker = service.clone();
    let worker_target = target.clone();
    let worker_session = session.session_id.clone();
    let operation = preview.operation_id.clone();
    let password = linux.password();
    let thread = std::thread::spawn(move || {
        worker.confirm_elevation(&worker_target, &worker_session, &operation, password)
    });
    wait(|| {
        linux
            .exec(&["test", "-f", "/home/fixture/rc-entered"], None)
            .status
            .success()
    });
    service
        .cancel_elevation(&target, &session.session_id, &preview.operation_id)
        .unwrap();
    assert_eq!(code(thread.join().unwrap()), ErrorCode::Cancelled);
    wait(|| broker.active_workers() == 0);
    assert_eq!(leases.load(Ordering::SeqCst), 1);
    assert!(
        linux
            .exec(
                &[
                    "rm",
                    "--",
                    "/home/fixture/.ssh/rc",
                    "/home/fixture/rc-entered"
                ],
                None
            )
            .status
            .success()
    );
    // A current protected session cannot be rebound via edited SSH config.
    let preview = service
        .preview_elevation(&target, &session.session_id)
        .unwrap();
    std::fs::write(
        linux.info["config"].as_str().unwrap(),
        String::from_utf8(original_config.clone())
            .unwrap()
            .replace("Hostname 127.0.0.1", "Hostname 127.0.0.2"),
    )
    .unwrap();
    assert_eq!(
        code(service.confirm_elevation(
            &target,
            &session.session_id,
            &preview.operation_id,
            linux.password()
        )),
        ErrorCode::HostTrustFailure
    );
    wait(|| leases.load(Ordering::SeqCst) == 0);
    std::fs::write(linux.info["config"].as_str().unwrap(), &original_config).unwrap();
    // Absolute expiry revokes an active delayed operation and its SSH child.
    let expiring = FileSessions::new(
        broker.clone(),
        authority,
        linux.options(),
        SessionPolicy::shortened(Duration::from_secs(5), Duration::from_secs(5)).unwrap(),
    );
    let short = open(&expiring, &target, &leases);
    assert!(
        linux
            .exec(
                &[
                    "sh",
                    "-c",
                    "cat > /home/fixture/.ssh/rc; chown fixture:fixture /home/fixture/.ssh/rc"
                ],
                Some(b"sleep 60\n")
            )
            .status
            .success()
    );
    let preview = expiring
        .preview_elevation(&target, &short.session_id)
        .unwrap();
    let failure = code(expiring.confirm_elevation(
        &target,
        &short.session_id,
        &preview.operation_id,
        linux.password(),
    ));
    assert!(matches!(
        failure,
        ErrorCode::ExpiredSession | ErrorCode::TimedOut
    ));
    wait(|| leases.load(Ordering::SeqCst) == 0 && broker.active_workers() == 0);
    assert!(
        linux
            .exec(&["rm", "--", "/home/fixture/.ssh/rc"], None)
            .status
            .success()
    );
    let session = open(&service, &target, &leases);
    let preview = service
        .preview_elevation(&target, &session.session_id)
        .unwrap();
    // Real host-key rotation leaves the old authenticated SFTP channel alive,
    // but new managed command channels must stop before sudo/password input.
    assert!(linux.exec(&["sh","-c","rm -f /etc/ssh/ssh_host_ed25519_key /etc/ssh/ssh_host_ed25519_key.pub; ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key; kill -HUP 1"],None).status.success());
    assert_eq!(
        code(service.confirm_elevation(
            &target,
            &session.session_id,
            &preview.operation_id,
            linux.password()
        )),
        ErrorCode::HostTrustFailure
    );
    wait(|| leases.load(Ordering::SeqCst) == 0);
    assert_eq!(
        std::fs::read(linux.info["known_hosts"].as_str().unwrap()).unwrap(),
        trust
    );
    assert_eq!(
        std::fs::read(linux.info["config"].as_str().unwrap()).unwrap(),
        original_config
    );
    let audit = serde_json::to_string(
        &broker
            .audit()
            .unwrap()
            .iter()
            .map(|entry| (&entry.operation_id, &entry.server_id, entry.failure))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(!audit.contains(&linux.password()));
    assert!(!audit.contains("ATLAS_SUDO"));
    println!(
        "PASS: banner spoof and SSH none-auth rejected without sudo password delivery; fresh sudo wrong-password/success/replay; protected staging 0700/0600, bidi-path rejection, lock-after-stage acknowledgement cleanup, idle-expiry cleanup, absolute-expiry session disposal with private stage permissions, duplicate cleanup preserves active session, in-flight release cancellation/partial-stage cleanup, review cancel/wrong-password, exact content/owner/group/mode and cleanup, collision no-overwrite; host/config/key rotation/expiry and child/lease cleanup; redacted audit"
    );
}
