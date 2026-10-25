use atlas_core::{operations::*, *};
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

fn fixture(
    duration: Duration,
    output: usize,
) -> (OperationBroker, Authority, ServerTarget, SessionId) {
    let broker = OperationBroker::new(duration, output).unwrap();
    let authority = broker.issue_backend_authority().unwrap();
    let target = ServerTarget::new(ServerId::parse("alpha").unwrap(), "alpha.invalid").unwrap();
    let scope = SessionScope::new(vec!["/home/atlas".into()], vec!["unit-a".into()]).unwrap();
    let session = broker
        .register_authenticated_session(&authority, target.clone(), scope, Duration::from_secs(60))
        .unwrap();
    (broker, authority, target, session)
}
fn request(
    target: &ServerTarget,
    session: &SessionId,
    inspection: Inspection,
) -> InspectionRequest {
    InspectionRequest {
        server_id: target.server_id().clone(),
        session_id: session.clone(),
        inspection,
    }
}
fn code<T>(result: Result<T, AtlasError>) -> ErrorCode {
    match result {
        Err(error) => error.code,
        Ok(_) => panic!("expected denial"),
    }
}
fn workers_finished(broker: &OperationBroker) {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while broker.active_workers() != 0 && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(broker.active_workers(), 0);
}

#[test]
fn authority_session_host_and_revocation_are_bound_before_execution() {
    let (broker, authority, target, session) = fixture(Duration::from_secs(1), 128);
    let input = || request(&target, &session, Inspection::SystemSummary {});
    assert_eq!(
        code(broker.prepare(None, &target, input())),
        ErrorCode::Unauthenticated
    );
    let other = broker.issue_backend_authority().unwrap();
    assert_eq!(
        code(broker.prepare(Some(&other), &target, input())),
        ErrorCode::Denied
    );
    let foreign = OperationBroker::default()
        .issue_backend_authority()
        .unwrap();
    assert_eq!(
        code(broker.prepare(Some(&foreign), &target, input())),
        ErrorCode::Denied
    );
    let moved = ServerTarget::new(target.server_id().clone(), "different.invalid").unwrap();
    assert_eq!(
        code(broker.prepare(Some(&authority), &moved, input())),
        ErrorCode::Denied
    );
    let wrong_server = ServerTarget::new(ServerId::parse("beta").unwrap(), target.host()).unwrap();
    assert_eq!(
        code(broker.prepare(Some(&authority), &wrong_server, input())),
        ErrorCode::Denied
    );
    let missing = SessionId::parse("not-a-session").unwrap();
    assert_eq!(
        code(broker.prepare(
            Some(&authority),
            &target,
            request(&target, &missing, Inspection::SystemSummary {})
        )),
        ErrorCode::ExpiredSession
    );
    let ticket = broker.prepare(Some(&authority), &target, input()).unwrap();
    let wrong = broker.execute(Some(&authority), &ticket.operation_id, &moved, |_, _| {
        panic!("must not execute")
    });
    assert_eq!(code(wrong), ErrorCode::Denied);
    assert_eq!(
        code(broker.cancel(Some(&other), &ticket.operation_id)),
        ErrorCode::Denied
    );
    broker.revoke(&authority).unwrap();
    assert_eq!(
        code(broker.execute(
            Some(&authority),
            &ticket.operation_id,
            &target,
            |_, _| panic!("must not execute")
        )),
        ErrorCode::Denied
    );
}

#[test]
fn unknown_operations_and_extra_shell_credential_fields_fail_closed() {
    for inspection in [
        json!({"kind":"shell","command":"id"}),
        json!({"kind":"rename","from":"/a","to":"/b"}),
        json!({"kind":"sudo","password":"synthetic-secret"}),
        json!({"kind":"system_summary","command":"id"}),
    ] {
        let raw = json!({"server_id":"alpha","session_id":"session", "inspection":inspection});
        assert!(serde_json::from_value::<InspectionRequest>(raw).is_err());
    }
    assert!(serde_json::from_value::<InspectionRequest>(json!({"server_id":"alpha","session_id":"session","inspection":{"kind":"system_summary"},"authority":"claimed"})).is_err());
    for host in [
        "-oProxyCommand=bad",
        "alpha; echo bad",
        "user@alpha",
        "alpha\ncommand",
    ] {
        assert!(ServerTarget::new(ServerId::parse("a").unwrap(), host).is_err());
    }
    let raw_secret =
        br#"{"server_id":"alpha","session_id":"session","inspection":{"kind":"synthetic-secret"}}"#;
    let failure = InspectionRequest::decode(raw_secret).err().unwrap();
    assert_eq!(
        failure.message,
        AtlasError::new(ErrorCode::InvalidInput).message
    );
    assert_eq!(
        code(InspectionRequest::decode(&vec![b'x'; 32 * 1024 + 1])),
        ErrorCode::InvalidInput
    );
}

#[test]
fn paths_and_workloads_are_scoped_without_constructing_shell_commands() {
    let (broker, authority, target, session) = fixture(Duration::from_secs(1), 128);
    for path in [
        "relative",
        "/home/atlas/../other",
        "/home/atlas/./file",
        "/home//atlas/file",
        "/home/atlas/file\nnext",
        "/home/atlas/file\0",
        "/home/atlas\\file",
    ] {
        assert_eq!(
            code(broker.prepare(
                Some(&authority),
                &target,
                request(
                    &target,
                    &session,
                    Inspection::FileMetadata { path: path.into() }
                )
            )),
            ErrorCode::InvalidInput
        );
    }
    for path in ["/etc/passwd", "/home/atlas-other/file"] {
        assert_eq!(
            code(broker.prepare(
                Some(&authority),
                &target,
                request(
                    &target,
                    &session,
                    Inspection::FileMetadata { path: path.into() }
                )
            )),
            ErrorCode::Denied
        );
    }
    let literal = "/home/atlas/$(echo literal); 'name'";
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(
                &target,
                &session,
                Inspection::FileMetadata {
                    path: literal.into(),
                },
            ),
        )
        .unwrap();
    broker
        .execute(
            Some(&authority),
            &ticket.operation_id,
            &target,
            move |plan, context| {
                let InspectionPlan::FileMetadata(path) = plan else {
                    panic!("wrong plan")
                };
                assert_eq!(path.path(), literal);
                assert!(path.check_resolved("/home/atlas/file").is_ok());
                assert_eq!(code(path.check_resolved("/etc/shadow")), ErrorCode::Denied);
                assert_eq!(
                    code(path.check_resolved("/home/atlas-other/file")),
                    ErrorCode::Denied
                );
                context.write("metadata")
            },
        )
        .unwrap();
    for workload in ["unit-b", "unit-a; reboot", "different-session-unit"] {
        assert_eq!(
            code(broker.prepare(
                Some(&authority),
                &target,
                request(
                    &target,
                    &session,
                    Inspection::ServiceStatus {
                        workload_id: workload.into()
                    }
                )
            )),
            ErrorCode::Denied
        );
    }
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(
                &target,
                &session,
                Inspection::ServiceStatus {
                    workload_id: "unit-a".into(),
                },
            ),
        )
        .unwrap();
    broker
        .execute(
            Some(&authority),
            &ticket.operation_id,
            &target,
            |plan, context| {
                let InspectionPlan::ServiceStatus { workload_id } = plan else {
                    panic!("wrong plan")
                };
                assert_eq!(workload_id, "unit-a");
                context.write("saved nowhere")
            },
        )
        .unwrap();
}

#[test]
fn tickets_execute_once_and_cancelled_requests_cannot_restart() {
    let (broker, authority, target, session) = fixture(Duration::from_secs(1), 128);
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let response = broker
        .execute(
            Some(&authority),
            &ticket.operation_id,
            &target,
            move |_, context| {
                calls.fetch_add(1, Ordering::SeqCst);
                context.write("protected-fixture-text")
            },
        )
        .unwrap();
    assert_eq!(response.text, "protected-fixture-text");
    assert_eq!(response.session_id, session);
    assert_eq!(response.operation_id, ticket.operation_id);
    assert!(
        broker
            .execute(
                Some(&authority),
                &ticket.operation_id,
                &target,
                |_, _| panic!("no replay")
            )
            .is_err()
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let cancelled = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    broker
        .cancel(Some(&authority), &cancelled.operation_id)
        .unwrap();
    assert_eq!(
        code(broker.execute(
            Some(&authority),
            &cancelled.operation_id,
            &target,
            |_, _| panic!("no cancelled execution")
        )),
        ErrorCode::Cancelled
    );
}

#[test]
fn output_limits_are_sticky_and_raw_executor_failures_are_redacted() {
    let (broker, authority, target, session) = fixture(Duration::from_secs(1), 8);
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    let result = broker.execute(
        Some(&authority),
        &ticket.operation_id,
        &target,
        |_, context| {
            context.write("secret")?;
            let _ = context.write("too long");
            Ok(()) // Ignoring a sink error must not turn it into partial success.
        },
    );
    assert_eq!(code(result), ErrorCode::OutputLimit);
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    let failure = broker
        .execute(Some(&authority), &ticket.operation_id, &target, |_, _| {
            Err(AtlasError {
                code: ErrorCode::IoFailure,
                message: "synthetic-secret-raw-output".into(),
            })
        })
        .err()
        .unwrap();
    assert_eq!(
        failure.message,
        AtlasError::new(ErrorCode::IoFailure).message
    );
    let audit = broker.audit().unwrap();
    assert_eq!(audit.len(), 2);
    assert_eq!(audit[0].failure, Some(ErrorCode::OutputLimit));
    assert_eq!(audit[1].failure, Some(ErrorCode::IoFailure));
    // Audit has no field capable of retaining executor payloads or arguments.
    assert!(
        audit
            .iter()
            .all(|entry| entry.server_id == *target.server_id())
    );
}

#[test]
fn timeout_returns_without_waiting_for_an_uncooperative_read_only_executor() {
    let (broker, authority, target, session) = fixture(Duration::from_millis(40), 128);
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    let (started, worker_started) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let other_broker = broker.clone();
    let other_authority = authority.clone();
    let other_target = target.clone();
    let execute = std::thread::spawn(move || {
        other_broker.execute(
            Some(&other_authority),
            &ticket.operation_id,
            &other_target,
            move |_, context| {
                started.send(()).unwrap();
                let _ = released.recv_timeout(Duration::from_secs(1));
                context.write("late secret must not escape")
            },
        )
    });
    worker_started.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(code(execute.join().unwrap()), ErrorCode::TimedOut);
    assert_eq!(broker.active_workers(), 1); // Still counted until actually gone.
    release.send(()).unwrap();
    let limit = std::time::Instant::now() + Duration::from_secs(1);
    while broker.active_workers() != 0 && std::time::Instant::now() < limit {
        std::thread::yield_now();
    }
    assert_eq!(broker.active_workers(), 0);
}

#[test]
fn mid_execution_revocation_discards_results_and_expired_sessions_deny_access() {
    let (broker, authority, target, session) = fixture(Duration::from_secs(1), 128);
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    let (started, ready) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let other = broker.clone();
    let caller = authority.clone();
    let destination = target.clone();
    let execute = std::thread::spawn(move || {
        other.execute(
            Some(&caller),
            &ticket.operation_id,
            &destination,
            move |_, context| {
                context.write("private-before-revocation")?;
                started.send(()).unwrap();
                let _ = released.recv_timeout(Duration::from_secs(1));
                Ok(())
            },
        )
    });
    ready.recv_timeout(Duration::from_secs(1)).unwrap();
    broker.revoke(&authority).unwrap();
    release.send(()).unwrap();
    assert_eq!(code(execute.join().unwrap()), ErrorCode::Denied);
    let active = broker.issue_backend_authority().unwrap();
    let expires = broker
        .register_authenticated_session(
            &active,
            target.clone(),
            SessionScope::new(vec![], vec![]).unwrap(),
            Duration::from_millis(20),
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(
        code(broker.prepare(
            Some(&active),
            &target,
            request(&target, &expires, Inspection::SystemSummary {})
        )),
        ErrorCode::ExpiredSession
    );
}

#[test]
fn concurrent_replay_and_running_cancellation_do_not_execute_twice() {
    let (broker, authority, target, session) = fixture(Duration::from_secs(2), 128);
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    let (started, ready) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let other = broker.clone();
    let caller = authority.clone();
    let destination = target.clone();
    let id = ticket.operation_id.clone();
    let execute = std::thread::spawn(move || {
        other.execute(Some(&caller), &id, &destination, move |_, context| {
            started.send(()).unwrap();
            let _ = released.recv_timeout(Duration::from_secs(1));
            context.write("late result")
        })
    });
    ready.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(
        code(broker.execute(
            Some(&authority),
            &ticket.operation_id,
            &target,
            |_, _| panic!("no duplicate worker")
        )),
        ErrorCode::Conflict
    );
    broker
        .cancel(Some(&authority), &ticket.operation_id)
        .unwrap();
    release.send(()).unwrap();
    assert_eq!(code(execute.join().unwrap()), ErrorCode::Cancelled);
    assert!(
        broker
            .execute(
                Some(&authority),
                &ticket.operation_id,
                &target,
                |_, _| panic!("no retry of cancelled ticket")
            )
            .is_err()
    );
}

#[test]
fn concurrent_worker_budget_stays_bounded_and_recovers_after_completion() {
    let (broker, authority, target, session) = fixture(Duration::from_secs(5), 128);
    let barrier = Arc::new(std::sync::Barrier::new(5));
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut executions = Vec::new();
    for _ in 0..4 {
        let ticket = broker
            .prepare(
                Some(&authority),
                &target,
                request(&target, &session, Inspection::SystemSummary {}),
            )
            .unwrap();
        let other = broker.clone();
        let caller = authority.clone();
        let destination = target.clone();
        let ready = barrier.clone();
        let released = release.clone();
        executions.push(std::thread::spawn(move || {
            other.execute(
                Some(&caller),
                &ticket.operation_id,
                &destination,
                move |_, context| {
                    ready.wait();
                    while !released.load(Ordering::Acquire) {
                        context.checkpoint()?;
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    context.write("ok")
                },
            )
        }));
    }
    barrier.wait();
    assert_eq!(broker.active_workers(), 4);
    let ticket = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    assert_eq!(
        code(broker.execute(
            Some(&authority),
            &ticket.operation_id,
            &target,
            |_, _| panic!("worker budget exceeded")
        )),
        ErrorCode::Conflict
    );
    release.store(true, Ordering::Release);
    for execution in executions {
        assert_eq!(execution.join().unwrap().unwrap().text, "ok");
    }
    workers_finished(&broker);
    assert_eq!(
        broker
            .execute(
                Some(&authority),
                &ticket.operation_id,
                &target,
                |_, context| context.write("recovered")
            )
            .unwrap()
            .text,
        "recovered"
    );
}

#[test]
fn connection_bootstrap_never_grants_or_substitutes_for_an_ssh_session() {
    let (broker, authority, target, session) = fixture(Duration::from_secs(1), 128);
    assert_eq!(
        code(broker.prepare_connection_check(None, &target)),
        ErrorCode::Unauthenticated
    );
    let ticket = broker
        .prepare_connection_check(Some(&authority), &target)
        .unwrap();
    assert_eq!(
        code(broker.execute(
            Some(&authority),
            &ticket.operation_id,
            &target,
            |_, _| panic!("bootstrap is not protected access")
        )),
        ErrorCode::Denied
    );
    let text = broker
        .execute_connection_check(
            Some(&authority),
            &ticket.operation_id,
            &target,
            |plan, context| {
                assert!(matches!(plan, InspectionPlan::ConnectionCheck));
                context.write("connection evidence only")
            },
        )
        .unwrap();
    assert_eq!(text, "connection evidence only");
    let fake_session = SessionId::parse(ticket.operation_id.as_str()).unwrap();
    assert_eq!(
        code(broker.prepare(
            Some(&authority),
            &target,
            request(&target, &fake_session, Inspection::SystemSummary {})
        )),
        ErrorCode::ExpiredSession
    );
    let protected = broker
        .prepare(
            Some(&authority),
            &target,
            request(&target, &session, Inspection::SystemSummary {}),
        )
        .unwrap();
    assert_eq!(
        code(broker.execute_connection_check(
            Some(&authority),
            &protected.operation_id,
            &target,
            |_, _| panic!("wrong ticket kind")
        )),
        ErrorCode::Denied
    );
}

#[test]
fn captured_subprocess_bytes_use_the_same_sticky_output_budget() {
    let (broker, authority, target, _) = fixture(Duration::from_secs(1), 8);
    let ticket = broker
        .prepare_connection_check(Some(&authority), &target)
        .unwrap();
    let result = broker.execute_connection_check(
        Some(&authority),
        &ticket.operation_id,
        &target,
        |_, context| {
            context.account_output(5)?;
            let _ = context.write("safe");
            Ok(())
        },
    );
    assert_eq!(code(result), ErrorCode::OutputLimit);
}
