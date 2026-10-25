use atlas_core::*;
use chrono::{TimeZone, Utc};
use serde_json::json;

#[test]
fn safe_input_and_id_contracts_reject_wrong_types_and_unknown_fields() {
    for input in [
        json!({"name":"A","host":"a.home","private_key":"no"}),
        json!([]),
    ] {
        assert!(serde_json::from_value::<AddServer>(input).is_err());
    }
    for input in [
        json!({"x":true}),
        json!({"x":null}),
        json!({"name":null}),
        json!({"report":{}}),
    ] {
        assert!(serde_json::from_value::<UpdateServer>(input).is_err());
    }
    assert!(UpdateServer::default().validate().is_err());
    let update: UpdateServer = serde_json::from_value(json!({"x":123.5})).unwrap();
    update.validate().unwrap();
    for id in ["", " leading", "line\nbreak"] {
        assert!(ServerId::parse(id).is_err());
    }
    let id = ServerId::generate();
    assert_eq!(
        serde_json::from_value::<ServerId>(json!(id.as_str())).unwrap(),
        id
    );
}

#[test]
fn coordinates_hostnames_and_observations_keep_reference_boundaries() {
    for x in [f64::NAN, f64::INFINITY, 10_000.1] {
        assert!(coordinate(x).is_err());
    }
    for host in [
        "-oProxyCommand=bad",
        "ssh://foo",
        "user@foo",
        "a..b",
        "x y",
        "",
    ] {
        assert!(hostname(host).is_err());
    }
    for host in [
        "alpha.home",
        "192.0.2.5",
        "::1",
        "fe80::1%eth0",
        "host.home.",
    ] {
        assert!(hostname(host).is_ok());
    }
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    assert!(observation_time("2026-10-25T13:00:00Z", now).is_err());
    assert!(observation_time("2026-10-25T12:00:00", now).is_err());
    assert_eq!(
        Freshness::for_saved("2026-10-25T11:45:00Z", now).unwrap(),
        Freshness::Saved
    );
    assert_eq!(
        Freshness::for_saved("2026-10-25T11:44:59Z", now).unwrap(),
        Freshness::Stale
    );
}

#[test]
fn errors_are_distinct_and_do_not_include_underlying_secret_output() {
    let codes = [
        ErrorCode::InvalidInput,
        ErrorCode::Unauthenticated,
        ErrorCode::Denied,
        ErrorCode::HostTrustFailure,
        ErrorCode::ExpiredSession,
        ErrorCode::IoFailure,
    ];
    let encoded: std::collections::HashSet<_> = codes
        .into_iter()
        .map(|code| serde_json::to_string(&AtlasError::new(code)).unwrap())
        .collect();
    assert_eq!(encoded.len(), 6);
    assert!(
        encoded
            .iter()
            .all(|v| !v.contains("password") && !v.contains("private_key"))
    );
}

#[test]
fn protected_results_cannot_be_baseline_inventory_and_absent_metrics_stay_unknown() {
    let protected = json!({"session_id":"s1","operation_id":"op1","expires_at":"2026-10-25T12:00:00Z","text":"protected output"});
    assert!(serde_json::from_value::<ProtectedText>(protected.clone()).is_ok());
    assert!(serde_json::from_value::<BaselineInventory>(protected).is_err());
    let metrics = serde_json::to_value(LiveMetrics {
        cpu_percent: None,
        memory_used_kib: None,
        disk_used_bytes: None,
    })
    .unwrap();
    assert!(metrics["cpu_percent"].is_null());
    let reachable = HostReachability {
        state: Reachability::Reachable,
        ..Default::default()
    };
    assert_eq!(reachable.state, Reachability::Reachable);
    assert_eq!(
        serde_json::to_value(Health::Unknown).unwrap(),
        json!("unknown")
    );
}

#[test]
fn progress_is_bounded_and_does_not_invent_an_unknown_total() {
    let mut p = OperationProgress {
        operation_id: OperationId::parse("o1").unwrap(),
        session_id: SessionId::parse("s1").unwrap(),
        phase: OperationPhase::Running,
        bytes_completed: 4,
        bytes_total: None,
    };
    p.validate().unwrap();
    assert!(serde_json::to_value(&p).unwrap()["bytes_total"].is_null());
    p.bytes_total = Some(3);
    assert!(p.validate().is_err());
    p.bytes_total = None;
    p.bytes_completed = MAX_SAFE_INTEGER + 1;
    assert!(p.validate().is_err());
}
