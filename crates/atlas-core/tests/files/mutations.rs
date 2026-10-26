use super::*;
fn entry_id(page: &FilePage, label: &str) -> String {
    page.entries
        .iter()
        .find(|entry| entry.resource.label == label)
        .unwrap_or_else(|| panic!("missing fixture entry {label}"))
        .resource
        .id
        .clone()
}
fn confirmed(
    service: &FileSessions,
    target: &ServerTarget,
    id: &SessionId,
    change: FileChange,
) -> FilePage {
    let category = match &change {
        FileChange::Mkdir { .. } => "mkdir",
        FileChange::Rename { .. } => "rename",
        FileChange::Move { .. } => "move",
        FileChange::Delete { .. } => "delete",
    };
    let preview = service
        .preview_change(target, id, change)
        .unwrap_or_else(|failure| panic!("fixture {category} preview failed: {:?}", failure.code));
    let receipt = service
        .commit_change(target, id, &preview.operation_id)
        .unwrap();
    assert_eq!(
        code(service.commit_change(target, id, &preview.operation_id)),
        ErrorCode::NotFound
    );
    page(service.read(target, id, list(&receipt.refresh.id)).unwrap())
}
pub(super) fn journey(
    service: &FileSessions,
    target: &ServerTarget,
    id: &SessionId,
    current: &FilePage,
    fixture: &Path,
) -> OperationId {
    let home_id = current
        .breadcrumbs
        .iter()
        .find(|entry| entry.label == "fixture")
        .unwrap()
        .id
        .clone();
    let home = page(service.read(target, id, list(&home_id)).unwrap());
    let mut work = page(
        service
            .read(target, id, list(&entry_id(&home, "write-area")))
            .unwrap(),
    );
    let actual = fixture.join("filesystem/home/fixture/write-area");
    let preview = service
        .preview_change(
            target,
            id,
            FileChange::Mkdir {
                parent_id: work.directory.id.clone(),
                name: "cancelled 日本語".into(),
            },
        )
        .unwrap();
    assert!(!actual.join("cancelled 日本語").exists());
    service
        .discard_change(target, id, &preview.operation_id)
        .unwrap();
    assert_eq!(
        code(service.commit_change(target, id, &preview.operation_id)),
        ErrorCode::NotFound
    );
    assert!(!actual.join("cancelled 日本語").exists());
    assert_eq!(
        code(service.preview_change(
            target,
            id,
            FileChange::Mkdir {
                parent_id: work.directory.id.clone(),
                name: "../escape".into()
            }
        )),
        ErrorCode::InvalidInput
    );
    assert_eq!(
        code(service.preview_change(
            target,
            id,
            FileChange::Delete {
                resource_id: work.directory.id.clone()
            }
        )),
        ErrorCode::Denied
    );
    assert_eq!(
        code(service.preview_change(
            target,
            id,
            FileChange::Rename {
                resource_id: entry_id(&work, "source file.txt"),
                name: "existing.txt".into()
            }
        )),
        ErrorCode::Conflict
    );
    assert!(
        work.entries
            .iter()
            .any(|entry| entry.resource.label == "odd-\\\\xff-source")
    );
    work = confirmed(
        service,
        target,
        id,
        FileChange::Mkdir {
            parent_id: work.directory.id.clone(),
            name: "created 日本語".into(),
        },
    );
    assert!(actual.join("created 日本語").is_dir());
    let original = std::fs::read(actual.join("source file.txt")).unwrap();
    work = confirmed(
        service,
        target,
        id,
        FileChange::Rename {
            resource_id: entry_id(&work, "source file.txt"),
            name: "renamed 日本語.txt".into(),
        },
    );
    assert_eq!(
        std::fs::read(actual.join("renamed 日本語.txt")).unwrap(),
        original
    );
    let pin = match service
        .read(
            target,
            id,
            FileAction::PinSource {
                resource_id: entry_id(&work, "renamed 日本語.txt"),
            },
        )
        .unwrap()
    {
        FileResult::Selection {
            resource: Some(value),
        } => value,
        _ => panic!("pin expected"),
    };
    let destination = page(
        service
            .read(target, id, list(&entry_id(&work, "target")))
            .unwrap(),
    );
    let moved = confirmed(
        service,
        target,
        id,
        FileChange::Move {
            resource_id: pin.id,
            parent_id: destination.directory.id,
        },
    );
    assert!(!actual.join("renamed 日本語.txt").exists());
    assert_eq!(
        std::fs::read(actual.join("target/renamed 日本語.txt")).unwrap(),
        original
    );
    let root_id = moved
        .breadcrumbs
        .iter()
        .find(|entry| entry.label == "write-area")
        .unwrap()
        .id
        .clone();
    work = page(service.read(target, id, list(&root_id)).unwrap());
    let pin = match service
        .read(
            target,
            id,
            FileAction::PinSource {
                resource_id: entry_id(&work, "odd-\\xff-source"),
            },
        )
        .unwrap()
    {
        FileResult::Selection {
            resource: Some(value),
        } => value,
        _ => panic!("pin expected"),
    };
    let destination = page(
        service
            .read(target, id, list(&entry_id(&work, "target")))
            .unwrap(),
    );
    let moved = confirmed(
        service,
        target,
        id,
        FileChange::Move {
            resource_id: pin.id,
            parent_id: destination.directory.id,
        },
    );
    let deleted = confirmed(
        service,
        target,
        id,
        FileChange::Delete {
            resource_id: entry_id(&moved, "odd-\\xff-source"),
        },
    );
    assert!(
        !deleted
            .entries
            .iter()
            .any(|entry| entry.resource.label == "odd-\\xff-source")
    );
    let root_id = deleted
        .breadcrumbs
        .iter()
        .find(|entry| entry.label == "write-area")
        .unwrap()
        .id
        .clone();
    work = page(service.read(target, id, list(&root_id)).unwrap());
    work = confirmed(
        service,
        target,
        id,
        FileChange::Delete {
            resource_id: entry_id(&work, "folder-link"),
        },
    );
    assert_eq!(
        std::fs::read_to_string(actual.join("target/keep.txt")).unwrap(),
        "symlink target survives"
    );
    let preview = service
        .preview_change(
            target,
            id,
            FileChange::Delete {
                resource_id: entry_id(&work, "nonempty"),
            },
        )
        .unwrap();
    assert_eq!(
        code(service.commit_change(target, id, &preview.operation_id)),
        ErrorCode::RemoteRejected
    );
    assert_eq!(
        std::fs::read_to_string(actual.join("nonempty/keep.txt")).unwrap(),
        "never recurse"
    );
    work = page(service.read(target, id, list(&work.directory.id)).unwrap());
    let preview = service
        .preview_change(
            target,
            id,
            FileChange::Rename {
                resource_id: entry_id(&work, "existing.txt"),
                name: "stale.txt".into(),
            },
        )
        .unwrap();
    std::fs::write(actual.join("existing.txt"), "concurrent edit changes size").unwrap();
    assert_eq!(
        code(service.commit_change(target, id, &preview.operation_id)),
        ErrorCode::Conflict
    );
    assert!(!actual.join("stale.txt").exists());
    let preview = service
        .preview_change(
            target,
            id,
            FileChange::Mkdir {
                parent_id: work.directory.id.clone(),
                name: "concurrent.txt".into(),
            },
        )
        .unwrap();
    std::fs::write(actual.join("concurrent.txt"), "keep collision").unwrap();
    assert_eq!(
        code(service.commit_change(target, id, &preview.operation_id)),
        ErrorCode::Conflict
    );
    assert_eq!(
        std::fs::read_to_string(actual.join("concurrent.txt")).unwrap(),
        "keep collision"
    );
    let preview = service
        .preview_change(
            target,
            id,
            FileChange::Rename {
                resource_id: entry_id(&work, "existing.txt"),
                name: "race-target.txt".into(),
            },
        )
        .unwrap();
    assert_eq!(
        code(service.commit_change(target, id, &preview.operation_id)),
        ErrorCode::RemoteRejected
    );
    assert!(actual.join("existing.txt").exists());
    assert_eq!(
        std::fs::read_to_string(actual.join("race-target.txt")).unwrap(),
        "concurrent target preserved"
    );
    work = page(service.read(target, id, list(&work.directory.id)).unwrap());
    let preview = service
        .preview_change(
            target,
            id,
            FileChange::Mkdir {
                parent_id: entry_id(&work, "denied"),
                name: "no-permission".into(),
            },
        )
        .unwrap();
    assert_eq!(
        code(service.commit_change(target, id, &preview.operation_id)),
        ErrorCode::Denied
    );
    assert!(!actual.join("denied/no-permission").exists());
    work = page(service.read(target, id, list(&work.directory.id)).unwrap());
    work = confirmed(
        service,
        target,
        id,
        FileChange::Delete {
            resource_id: entry_id(&work, "created 日本語"),
        },
    );
    assert!(!actual.join("created 日本語").exists());
    service
        .preview_change(
            target,
            id,
            FileChange::Mkdir {
                parent_id: work.directory.id,
                name: "locked-never-created".into(),
            },
        )
        .unwrap()
        .operation_id
}
pub(super) fn unknown_outcome(
    options: SshOptions,
    target: &ServerTarget,
    leases: &Arc<AtomicUsize>,
    fixture: &Path,
) {
    let broker = OperationBroker::new(Duration::from_secs(1), 65536).unwrap();
    let actor = broker.issue_backend_authority().unwrap();
    let service = FileSessions::new(broker.clone(), actor, options, SessionPolicy::default());
    let session = service
        .open(
            target,
            &service.prepare(target).unwrap().operation_id,
            Lease::new(leases),
        )
        .unwrap();
    let home = page(
        service
            .read(target, &session.session_id, list(&session.home.id))
            .unwrap(),
    );
    let work = page(
        service
            .read(
                target,
                &session.session_id,
                list(&entry_id(&home, "write-area")),
            )
            .unwrap(),
    );
    let preview = service
        .preview_change(
            target,
            &session.session_id,
            FileChange::Mkdir {
                parent_id: work.directory.id,
                name: "apply-then-stall".into(),
            },
        )
        .unwrap();
    assert_eq!(
        code(service.commit_change(target, &session.session_id, &preview.operation_id)),
        ErrorCode::OutcomeUnknown
    );
    assert!(
        fixture
            .join("filesystem/home/fixture/write-area/apply-then-stall")
            .is_dir()
    );
    assert_eq!(
        code(service.commit_change(target, &session.session_id, &preview.operation_id)),
        ErrorCode::ExpiredSession
    );
    wait_for(|| leases.load(Ordering::SeqCst) == 0 && broker.active_workers() == 0);
}
