use super::*;
use sha2::{Digest, Sha256};
fn terminal(row: &TransferView) -> bool {
    matches!(
        row.state,
        TransferState::Complete
            | TransferState::Cancelled
            | TransferState::Failed
            | TransferState::OutcomeUnknown
    )
}
fn wait_job(service: &FileSessions, id: &OperationId) -> TransferView {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let row = service
            .transfer_views()
            .unwrap()
            .into_iter()
            .find(|row| row.id == *id)
            .unwrap();
        if terminal(&row) {
            return row;
        }
        assert!(Instant::now() < deadline, "transfer deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn complete(row: &TransferView) {
    assert!(
        row.state == TransferState::Complete,
        "job failure {:?}",
        row.failure
    );
    assert!(row.verified);
    assert!(!row.cleanup_needed);
}
fn open(
    service: &FileSessions,
    target: &ServerTarget,
    leases: &Arc<AtomicUsize>,
) -> (FileSession, FilePage) {
    let session = service
        .open(
            target,
            &service.prepare(target).unwrap().operation_id,
            Lease::new(leases),
            None,
        )
        .unwrap();
    let home = page(
        service
            .read(target, &session.session_id, list(&session.home.id))
            .unwrap(),
    );
    let folder = home
        .entries
        .iter()
        .find(|entry| entry.resource.label == "write-area")
        .unwrap();
    let view = page(
        service
            .read(target, &session.session_id, list(&folder.resource.id))
            .unwrap(),
    );
    (session, view)
}
pub fn journey(root: &Path, directory: &Path, target: &ServerTarget, leases: &Arc<AtomicUsize>) {
    let options = SshOptions::configured(
        "C:/Windows/System32/OpenSSH/ssh.exe".into(),
        directory.join("config"),
    );
    let broker = OperationBroker::default();
    let actor = broker.issue_backend_authority().unwrap();
    let service = FileSessions::new(
        broker.clone(),
        actor,
        options.clone(),
        SessionPolicy::default(),
    );
    let local = tempfile::tempdir_in(root.join(".tools")).unwrap();
    let remote = directory.join("filesystem/home/fixture/write-area");
    let mode = directory.join("transfer-mode");
    let (released, _) = open(&service, target, leases);
    service.release_view(&released.session_id).unwrap();
    wait_for(|| leases.load(Ordering::SeqCst) == 0);
    let (session, mut view) = open(&service, target, leases);
    for (label, size) in [
        ("empty.bin", 0),
        ("small spaced 日本語.bin", 713),
        ("large.bin", 8 * 1024 * 1024),
    ] {
        let source = local.path().join(label);
        let mut file = std::fs::File::create(&source).unwrap();
        let block: Vec<u8> = (0..32768).map(|n| (n % 251) as u8).collect();
        let mut remaining = size;
        while remaining > 0 {
            let count = remaining.min(block.len());
            file.write_all(&block[..count]).unwrap();
            remaining -= count;
        }
        drop(file);
        let expected = format!("{:x}", Sha256::digest(std::fs::read(&source).unwrap()));
        let upload = service
            .enqueue_transfer(
                target,
                &session.session_id,
                &view.directory.id,
                LocalTransfer::upload(&source).unwrap(),
            )
            .unwrap();
        let done = wait_job(&service, &upload.id);
        complete(&done);
        assert_eq!(done.bytes, size.to_string());
        assert_eq!(done.checksum.as_deref(), Some(expected.as_str()));
        assert_eq!(
            std::fs::read(remote.join(label)).unwrap(),
            std::fs::read(&source).unwrap()
        );
        view = page(
            service
                .read(target, &session.session_id, list(&view.directory.id))
                .unwrap(),
        );
        let entry = view
            .entries
            .iter()
            .find(|entry| entry.resource.label == label)
            .unwrap();
        let destination = local.path().join(format!("download-{label}"));
        let download = service
            .enqueue_transfer(
                target,
                &session.session_id,
                &entry.resource.id,
                LocalTransfer::download(&destination).unwrap(),
            )
            .unwrap();
        let done = wait_job(&service, &download.id);
        complete(&done);
        assert_eq!(done.checksum.as_deref(), Some(expected.as_str()));
        assert_eq!(
            std::fs::read(destination).unwrap(),
            std::fs::read(source).unwrap()
        );
    }
    // Actual queue, cooperative cancellation and private-stage cleanup.
    std::fs::write(&mode, "slow").unwrap();
    let source = local.path().join("cancel.bin");
    std::fs::write(&source, vec![42; 1024 * 1024]).unwrap();
    let active = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&source).unwrap(),
        )
        .unwrap();
    let queued = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&source).unwrap(),
        )
        .unwrap();
    wait_for(|| {
        service
            .transfer_views()
            .unwrap()
            .iter()
            .any(|row| row.id == active.id && row.bytes != "0")
    });
    service.cancel_transfer(&active.id).unwrap();
    service.cancel_transfer(&queued.id).unwrap();
    assert!(wait_job(&service, &active.id).state == TransferState::Cancelled);
    assert!(wait_job(&service, &queued.id).state == TransferState::Cancelled);
    assert!(!remote.join("cancel.bin").exists());
    assert!(!std::fs::read_dir(&remote).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".atlas-upload-")
    }));
    // Cancel while the final remote absence preflight is held: never publish.
    std::fs::write(&mode, "hold-final-stat").unwrap();
    let final_source = local.path().join("cancel-final.bin");
    std::fs::write(&final_source, b"cancel before publication").unwrap();
    let final_job = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&final_source).unwrap(),
        )
        .unwrap();
    wait_for(|| directory.join("hold-stat-ready").exists());
    service.cancel_transfer(&final_job.id).unwrap();
    std::fs::write(directory.join("release-stat"), b"release").unwrap();
    assert!(wait_job(&service, &final_job.id).state == TransferState::Cancelled);
    assert!(!remote.join("cancel-final.bin").exists());
    std::fs::write(&mode, "slow").unwrap();
    // Leaving Files removes every browsing capability, retaining only approved jobs.
    let hidden_source = local.path().join("background.bin");
    std::fs::write(&hidden_source, vec![91; 512 * 1024]).unwrap();
    let background = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&hidden_source).unwrap(),
        )
        .unwrap();
    service.release_view(&session.session_id).unwrap();
    assert_eq!(
        code(service.read(target, &session.session_id, list(&view.directory.id))),
        ErrorCode::ExpiredSession
    );
    assert!(wait_job(&service, &background.id).state == TransferState::Complete);
    wait_for(|| leases.load(Ordering::SeqCst) == 0 && broker.active_workers() == 0);
    assert!(
        service
            .transfer_views()
            .unwrap()
            .iter()
            .all(|row| row.remote.is_none() && row.checksum.is_none())
    );
    std::fs::write(&mode, "").unwrap();
    let (session, mut view) = open(&service, target, leases);
    // Lost cleanup I/O invalidates the channel even when user cancellation wins.
    std::fs::write(&mode, "slow").unwrap();
    let cleanup_source = local.path().join("cleanup.bin");
    std::fs::write(&cleanup_source, vec![83; 1024 * 1024]).unwrap();
    let cleanup = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&cleanup_source).unwrap(),
        )
        .unwrap();
    wait_for(|| {
        service
            .transfer_views()
            .unwrap()
            .iter()
            .any(|row| row.id == cleanup.id && row.bytes != "0")
    });
    std::fs::write(&mode, "disconnect-cleanup").unwrap();
    service.cancel_transfer(&cleanup.id).unwrap();
    let stopped = wait_job(&service, &cleanup.id);
    assert!(stopped.state == TransferState::Cancelled);
    assert!(stopped.cleanup_needed);
    wait_for(|| leases.load(Ordering::SeqCst) == 0);
    assert_eq!(
        code(service.read(target, &session.session_id, list(&view.directory.id))),
        ErrorCode::ExpiredSession
    );
    std::fs::write(&mode, "").unwrap();
    let fresh = open(&service, target, leases);
    let session = fresh.0;
    view = fresh.1;
    // Same-name collisions preserve content; local no-clobber is enforced again at publication.
    let collision = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&local.path().join("large.bin")).unwrap(),
        )
        .unwrap();
    assert!(wait_job(&service, &collision.id).failure == Some(ErrorCode::Conflict));
    let source = view
        .entries
        .iter()
        .find(|entry| entry.resource.label == "large.bin")
        .unwrap();
    let destination = local.path().join("raced.bin");
    let chosen = LocalTransfer::download(&destination).unwrap();
    std::fs::write(&destination, b"preserve concurrent file").unwrap();
    let race = service
        .enqueue_transfer(target, &session.session_id, &source.resource.id, chosen)
        .unwrap();
    assert!(wait_job(&service, &race.id).failure == Some(ErrorCode::Conflict));
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"preserve concurrent file"
    );
    // Byte corruption is rejected before the uploaded file becomes visible.
    let corrupt = local.path().join("corrupt.bin");
    std::fs::write(&corrupt, b"checksum fixture").unwrap();
    std::fs::write(&mode, "corrupt-read").unwrap();
    let job = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&corrupt).unwrap(),
        )
        .unwrap();
    assert!(wait_job(&service, &job.id).failure == Some(ErrorCode::Conflict));
    assert!(!remote.join("corrupt.bin").exists());
    // Interrupted upload retains only a private partial; explicit fresh retry succeeds.
    std::fs::write(&mode, "disconnect-write").unwrap();
    let retry_source = local.path().join("retry.bin");
    std::fs::write(&retry_source, b"retry content").unwrap();
    let job = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&retry_source).unwrap(),
        )
        .unwrap();
    let failed = wait_job(&service, &job.id);
    assert!(failed.state == TransferState::Failed);
    assert!(failed.cleanup_needed);
    assert!(!remote.join("retry.bin").exists());
    wait_for(|| leases.load(Ordering::SeqCst) == 0);
    std::fs::write(&mode, "").unwrap();
    let fresh = open(&service, target, leases);
    let session = fresh.0;
    view = fresh.1;
    let job = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&retry_source).unwrap(),
        )
        .unwrap();
    complete(&wait_job(&service, &job.id));
    // Lost publication acknowledgement is uncertain and never automatically replayed.
    std::fs::write(&mode, "publish-lost-ack").unwrap();
    let lost = local.path().join("lost.bin");
    std::fs::write(&lost, b"published once").unwrap();
    let job = service
        .enqueue_transfer(
            target,
            &session.session_id,
            &view.directory.id,
            LocalTransfer::upload(&lost).unwrap(),
        )
        .unwrap();
    assert!(wait_job(&service, &job.id).state == TransferState::OutcomeUnknown);
    assert_eq!(
        std::fs::read(remote.join("lost.bin")).unwrap(),
        b"published once"
    );
    wait_for(|| leases.load(Ordering::SeqCst) == 0);
    std::fs::write(&mode, "").unwrap();
    // Absolute expiry stops a live download, publishes nothing and removes its local temp.
    let actor = broker.issue_backend_authority().unwrap();
    let expiring = FileSessions::new(
        broker.clone(),
        actor,
        options,
        SessionPolicy::shortened(Duration::from_secs(2), Duration::from_secs(2)).unwrap(),
    );
    let (session, view) = open(&expiring, target, leases);
    let source = view
        .entries
        .iter()
        .find(|entry| entry.resource.label == "large.bin")
        .unwrap();
    std::fs::write(&mode, "slow").unwrap();
    let destination = local.path().join("expired.bin");
    let job = expiring
        .enqueue_transfer(
            target,
            &session.session_id,
            &source.resource.id,
            LocalTransfer::download(&destination).unwrap(),
        )
        .unwrap();
    assert!(wait_job(&expiring, &job.id).state == TransferState::Failed);
    assert!(!destination.exists());
    wait_for(|| leases.load(Ordering::SeqCst) == 0 && broker.active_workers() == 0);
    std::fs::write(&mode, "").unwrap();
    assert!(!std::fs::read_dir(local.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".atlas-download-")
    }));
    let stats: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("stats.json")).unwrap()).unwrap();
    assert!(stats["trusted"]["max_chunk"].as_u64().unwrap() <= 32768);
    println!(
        "PASS: actual 0/713/8MiB SHA256 round trips, bounded32KiB streaming, queue/cancel, hidden-view jobs, collisions, corruption, interruption/explicit retry, lost ACK, expiry and temp cleanup"
    );
}
