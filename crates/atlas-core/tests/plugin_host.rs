//! Plugin grants and the enforcement point (T041). Every capability is
//! attempted in every state; only what the owner granted is admitted.
use atlas_core::extensions::Capability;
use atlas_core::plugin_host::*;
use atlas_core::*;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const PLUGIN: &str = "org.example.everything";

const ALL: [Capability; 12] = [
    Capability::ReadSavedState,
    Capability::EditMap,
    Capability::NetworkProbe,
    Capability::SshSession,
    Capability::RunApprovedCommand,
    Capability::ReadFiles,
    Capability::WriteFiles,
    Capability::TransferFiles,
    Capability::InteractiveTerminal,
    Capability::Elevate,
    Capability::ControlWorkloads,
    Capability::LoopbackTunnel,
];
const READ: [Capability; 5] = [
    Capability::ReadSavedState,
    Capability::NetworkProbe,
    Capability::SshSession,
    Capability::RunApprovedCommand,
    Capability::ReadFiles,
];
const CHANGE: [Capability; 5] = [
    Capability::EditMap,
    Capability::WriteFiles,
    Capability::TransferFiles,
    Capability::ControlWorkloads,
    Capability::LoopbackTunnel,
];

/// A plugin that requests everything a plugin may request.
fn everything(version: &str) -> String {
    serde_json::json!({
        "id": PLUGIN, "name": "Everything", "version": version, "api_version": 1,
        "capabilities": ["read_saved_state", "edit_map", "network_probe", "ssh_session", "run_approved_command", "read_files", "write_files", "transfer_files", "elevate", "control_workloads", "loopback_tunnel"],
        "contributions": [
            {"id": "tab", "point": "workspace_tab", "title": "Everything", "capabilities": ["read_saved_state", "edit_map", "network_probe", "ssh_session", "run_approved_command", "read_files", "write_files", "transfer_files", "elevate", "control_workloads", "loopback_tunnel"]}
        ]
    })
    .to_string()
}

fn install(plugins: &Path, folder: &str, manifest: &str) {
    std::fs::create_dir_all(plugins.join(folder)).unwrap();
    std::fs::write(plugins.join(folder).join("infrawerf-plugin.json"), manifest).unwrap();
}

struct Fixture {
    _directory: tempfile::TempDir,
    database: PathBuf,
    plugins: PathBuf,
    store: Store,
    alpha: ServerId,
    beta: ServerId,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("atlas.sqlite3");
    let plugins = directory.path().join("plugins");
    let mut store = Store::create(&database).unwrap();
    let mut add = |name: &str, host: &str| {
        store
            .add_server(AddServer {
                name: name.into(),
                host: host.into(),
            })
            .unwrap()
    };
    let alpha = add("Alpha", "alpha.invalid");
    let beta = add("Beta", "beta.invalid");
    install(&plugins, "everything", &everything("1.0.0"));
    Fixture {
        _directory: directory,
        database,
        plugins,
        store,
        alpha,
        beta,
    }
}

impl Fixture {
    fn ask(&mut self, server: &ServerId, capability: Capability) -> PluginDecision {
        self.store
            .plugin_authorize(&self.plugins, PLUGIN, server, capability)
            .unwrap()
    }
    fn refusal(&mut self, server: &ServerId, capability: Capability) -> Refusal {
        let decision = self.ask(server, capability);
        assert!(
            !decision.allowed && !decision.needs_confirmation,
            "{capability:?}"
        );
        decision.refusal.unwrap()
    }
    fn view(&self) -> PluginView {
        self.store
            .plugins_state(&self.plugins)
            .unwrap()
            .plugins
            .into_iter()
            .find(|plugin| plugin.id == PLUGIN)
            .unwrap()
    }
    fn refusals_logged(&self) -> usize {
        self.log()
            .iter()
            .filter(|entry| entry.outcome == ActivityOutcome::Refused)
            .count()
    }
    fn log(&self) -> Vec<PluginActivity> {
        self.store.plugin_activity(500).unwrap().entries
    }
    fn grantable() -> Vec<Capability> {
        READ.iter().chain(CHANGE.iter()).copied().collect()
    }
}

#[test]
fn a_found_plugin_has_no_trust_and_no_grants_and_every_call_is_refused_and_logged() {
    let mut f = fixture();
    let view = f.view();
    assert_eq!(view.status, PluginStatus::Disabled);
    assert_eq!(view.trust, None);
    assert!(view.grants.is_empty());
    assert_eq!(view.capabilities.len(), 11);
    assert!(
        view.capabilities
            .iter()
            .all(|c| !c.summary.is_empty() && c.used_by == ["Everything"])
    );
    let alpha = f.alpha.clone();
    for capability in ALL {
        assert_eq!(
            f.refusal(&alpha, capability),
            Refusal::PluginDisabled,
            "{capability:?}"
        );
    }
    assert_eq!(f.refusals_logged(), ALL.len());
    // Granting does not enable: the plugin still has no trust level.
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &Fixture::grantable())
        .unwrap();
    for capability in ALL {
        assert_eq!(
            f.refusal(&alpha, capability),
            Refusal::PluginDisabled,
            "{capability:?}"
        );
    }
    assert_eq!(f.view().trust, None);
}

#[test]
fn only_granted_capabilities_are_admitted_and_only_as_far_as_the_trust_level_goes() {
    let mut f = fixture();
    let (alpha, beta) = (f.alpha.clone(), f.beta.clone());
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::ReadOnly)
        .unwrap();
    assert_eq!(f.view().status, PluginStatus::Enabled);
    assert_eq!(f.view().trust, Some(TrustLevel::ReadOnly));

    // Enabled with nothing granted: every capability is still refused.
    for capability in ALL {
        let expected = match capability {
            Capability::InteractiveTerminal => Refusal::NotRequested,
            Capability::Elevate => Refusal::NoElevationException,
            _ => Refusal::NotGranted,
        };
        assert_eq!(f.refusal(&alpha, capability), expected, "{capability:?}");
    }

    // Grant two capabilities on Alpha only.
    let some = [Capability::ReadSavedState, Capability::WriteFiles];
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &some)
        .unwrap();
    assert_eq!(
        f.view().grants,
        [ServerGrant {
            server_id: alpha.clone(),
            capabilities: some.to_vec()
        }]
    );
    assert!(f.ask(&alpha, Capability::ReadSavedState).allowed);
    // Read-only trust refuses a granted change.
    assert_eq!(
        f.refusal(&alpha, Capability::WriteFiles),
        Refusal::TrustTooLow
    );
    for capability in ALL {
        if !some.contains(&capability) {
            assert!(!f.ask(&alpha, capability).allowed, "{capability:?}");
        }
        // Nothing was granted on Beta.
        assert!(!f.ask(&beta, capability).allowed, "{capability:?} on Beta");
    }

    // Grant everything grantable on Alpha, then walk the three trust levels.
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &Fixture::grantable())
        .unwrap();
    for (trust, change_allowed, confirm) in [
        (TrustLevel::ReadOnly, false, false),
        (TrustLevel::ConfirmChanges, true, true),
        (TrustLevel::TrustedAutomation, true, false),
    ] {
        f.store.plugin_enable(&f.plugins, PLUGIN, trust).unwrap();
        for capability in READ {
            let decision = f.ask(&alpha, capability);
            assert!(
                decision.allowed && !decision.needs_confirmation,
                "{trust:?} {capability:?}"
            );
            assert_eq!(decision.refusal, None);
        }
        for capability in CHANGE {
            let decision = f.ask(&alpha, capability);
            assert_eq!(decision.allowed, change_allowed, "{trust:?} {capability:?}");
            assert_eq!(
                decision.needs_confirmation, confirm,
                "{trust:?} {capability:?}"
            );
        }
        // No trust level reaches sudo or the terminal.
        assert_eq!(
            f.refusal(&alpha, Capability::Elevate),
            Refusal::NoElevationException
        );
        assert_eq!(
            f.refusal(&alpha, Capability::InteractiveTerminal),
            Refusal::NotRequested
        );
        for capability in ALL {
            assert!(
                !f.ask(&beta, capability).allowed,
                "{trust:?} {capability:?} on Beta"
            );
        }
    }
}

#[test]
fn grants_are_bounded_by_the_manifest_and_sudo_cannot_be_granted() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    let code = |result: Result<(), AtlasError>| result.unwrap_err().code;
    // Sudo, a capability the manifest never asked for, and a repeated one.
    for bad in [
        vec![Capability::Elevate],
        vec![Capability::InteractiveTerminal],
        vec![Capability::ReadFiles, Capability::ReadFiles],
    ] {
        assert_eq!(
            code(f.store.plugin_set_grant(&f.plugins, PLUGIN, &alpha, &bad)),
            ErrorCode::InvalidInput,
            "{bad:?}"
        );
    }
    let stranger = ServerId::generate();
    assert_eq!(
        code(
            f.store
                .plugin_set_grant(&f.plugins, PLUGIN, &stranger, &[Capability::ReadFiles])
        ),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(
            f.store
                .plugin_set_grant(&f.plugins, "org.example.absent", &alpha, &[])
        ),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(
            f.store
                .plugin_enable(&f.plugins, "org.example.absent", TrustLevel::ReadOnly)
        ),
        ErrorCode::NotFound
    );
    assert!(f.view().grants.is_empty());
    // Unknown plugin and unknown server are refusals too, and are logged.
    let before = f.refusals_logged();
    let decision = f
        .store
        .plugin_authorize(
            &f.plugins,
            "org.example.absent",
            &alpha,
            Capability::ReadFiles,
        )
        .unwrap();
    assert_eq!(decision.refusal, Some(Refusal::UnknownPlugin));
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::TrustedAutomation)
        .unwrap();
    assert_eq!(
        f.refusal(&stranger, Capability::ReadFiles),
        Refusal::UnknownServer
    );
    assert_eq!(f.refusals_logged(), before + 2);
}

#[test]
fn withdrawing_disabling_and_removing_a_server_each_stop_access() {
    let mut f = fixture();
    let (alpha, beta) = (f.alpha.clone(), f.beta.clone());
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::TrustedAutomation)
        .unwrap();
    for server in [&alpha, &beta] {
        f.store
            .plugin_set_grant(&f.plugins, PLUGIN, server, &Fixture::grantable())
            .unwrap();
    }
    assert!(f.ask(&alpha, Capability::WriteFiles).allowed);
    assert!(f.ask(&beta, Capability::WriteFiles).allowed);

    // Withdraw one capability, then everything, on Alpha. Beta is untouched.
    let fewer: Vec<Capability> = Fixture::grantable()
        .into_iter()
        .filter(|c| *c != Capability::WriteFiles)
        .collect();
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &fewer)
        .unwrap();
    assert_eq!(
        f.refusal(&alpha, Capability::WriteFiles),
        Refusal::NotGranted
    );
    assert!(f.ask(&alpha, Capability::ReadFiles).allowed);
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &[])
        .unwrap();
    for capability in ALL {
        assert!(!f.ask(&alpha, capability).allowed, "{capability:?}");
    }
    assert!(f.ask(&beta, Capability::WriteFiles).allowed);

    // Disabling stops everything at once and forgets the trust level.
    f.store.plugin_disable(&f.plugins, PLUGIN).unwrap();
    assert_eq!(
        f.refusal(&beta, Capability::ReadSavedState),
        Refusal::PluginDisabled
    );
    let view = f.view();
    assert_eq!((view.status, view.trust), (PluginStatus::Disabled, None));
    assert_eq!(
        f.store.plugin_disable(&f.plugins, PLUGIN).unwrap_err().code,
        ErrorCode::NotFound
    );

    // Removing a server removes its grants; a new server starts with none.
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::TrustedAutomation)
        .unwrap();
    assert!(f.ask(&beta, Capability::ReadSavedState).allowed);
    f.store.remove_server(&beta).unwrap();
    assert_eq!(
        f.refusal(&beta, Capability::ReadSavedState),
        Refusal::UnknownServer
    );
    assert!(f.view().grants.is_empty());
}

#[test]
fn a_manifest_changed_after_review_is_treated_as_disabled_until_reviewed_again() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::TrustedAutomation)
        .unwrap();
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &Fixture::grantable())
        .unwrap();
    assert!(f.ask(&alpha, Capability::ReadFiles).allowed);

    install(&f.plugins, "everything", &everything("1.0.1"));
    let view = f.view();
    assert_eq!(
        (view.status, view.trust),
        (PluginStatus::ChangedSinceReview, None)
    );
    for capability in ALL {
        assert_eq!(
            f.refusal(&alpha, capability),
            Refusal::ChangedSinceReview,
            "{capability:?}"
        );
    }
    // Enabling again is the new review.
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::ReadOnly)
        .unwrap();
    assert!(f.ask(&alpha, Capability::ReadFiles).allowed);
    assert_eq!(
        f.refusal(&alpha, Capability::WriteFiles),
        Refusal::TrustTooLow
    );

    // A removed plugin is refused as unknown.
    std::fs::remove_dir_all(f.plugins.join("everything")).unwrap();
    let decision = f.ask(&alpha, Capability::ReadFiles);
    assert_eq!(decision.refusal, Some(Refusal::UnknownPlugin));
}

#[test]
fn discovery_reads_manifests_only_and_reports_unusable_folders() {
    let f = fixture();
    install(&f.plugins, "broken", "{ not json");
    install(&f.plugins, "copy", &everything("2.0.0"));
    std::fs::create_dir_all(f.plugins.join("empty")).unwrap();
    std::fs::write(f.plugins.join("loose-file.json"), "{}").unwrap();
    let state = f.store.plugins_state(&f.plugins).unwrap();
    assert_eq!(state.plugins.len(), 1);
    assert_eq!(
        state.plugins[0].folder,
        "copy".to_owned().min("everything".to_owned())
    );
    let invalid: Vec<(&str, &str)> = state
        .invalid
        .iter()
        .map(|entry| (entry.folder.as_str(), entry.problems[0].as_str()))
        .collect();
    assert_eq!(invalid.len(), 3);
    assert_eq!(invalid[0].0, "broken");
    assert!(invalid[0].1.starts_with("malformed_json"));
    assert_eq!(invalid[1].0, "empty");
    assert!(invalid[1].1.contains("missing or unreadable"));
    assert_eq!(invalid[2].0, "everything");
    assert!(
        invalid[2]
            .1
            .contains("already used by the plugin in \"copy\"")
    );
    // A missing plugins directory is simply no plugins.
    let none = f.store.plugins_state(&f.plugins.join("absent")).unwrap();
    assert!(none.plugins.is_empty() && none.invalid.is_empty());
}

#[test]
fn grants_survive_a_restart_and_the_file_keeps_its_schema_version() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::ConfirmChanges)
        .unwrap();
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &[Capability::WriteFiles])
        .unwrap();
    let Fixture {
        _directory,
        database,
        plugins,
        store,
        ..
    } = f;
    drop(store);
    // The previous release opens files at this version; adding the plugin
    // tables must not change it.
    let raw = Connection::open(&database).unwrap();
    let version: i32 = raw
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 3);
    drop(raw);
    let mut store = Store::open(&database).unwrap();
    let view = store.plugins_state(&plugins).unwrap().plugins.remove(0);
    assert_eq!(view.trust, Some(TrustLevel::ConfirmChanges));
    assert_eq!(view.grants[0].capabilities, [Capability::WriteFiles]);
    let decision = store
        .plugin_authorize(&plugins, PLUGIN, &alpha, Capability::WriteFiles)
        .unwrap();
    assert!(decision.allowed && decision.needs_confirmation);

    // A file from before this version gains the tables when opened.
    let older = database.with_file_name("older.sqlite3");
    drop(Store::create(&older).unwrap());
    let raw = Connection::open(&older).unwrap();
    raw.execute_batch("DROP TABLE plugin_state; DROP TABLE plugin_grant;")
        .unwrap();
    drop(raw);
    let store = Store::open(&older).unwrap();
    assert!(
        store.plugins_state(&plugins).unwrap().plugins[0]
            .grants
            .is_empty()
    );
}

#[test]
fn every_action_is_logged_with_plugin_server_action_and_outcome_and_nothing_else() {
    let mut f = fixture();
    let (alpha, beta) = (f.alpha.clone(), f.beta.clone());
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::ConfirmChanges)
        .unwrap();
    f.store
        .plugin_set_grant(
            &f.plugins,
            PLUGIN,
            &alpha,
            &[Capability::ReadFiles, Capability::WriteFiles],
        )
        .unwrap();
    assert!(f.ask(&alpha, Capability::ReadFiles).allowed);
    assert!(f.ask(&alpha, Capability::WriteFiles).needs_confirmation);
    assert_eq!(f.refusal(&beta, Capability::ReadFiles), Refusal::NotGranted);
    // An unknown caller's ID is never written down.
    f.store
        .plugin_authorize(
            &f.plugins,
            "password=hunter2 -----BEGIN PRIVATE KEY-----",
            &alpha,
            Capability::ReadFiles,
        )
        .unwrap();
    f.store.plugin_disable(&f.plugins, PLUGIN).unwrap();

    let log = f.log();
    let lines: Vec<(&str, Option<&ServerId>, &str, ActivityOutcome, &str)> = log
        .iter()
        .rev()
        .map(|e| {
            (
                e.plugin_name.as_str(),
                e.server_id.as_ref(),
                e.action.as_str(),
                e.outcome,
                e.detail.as_str(),
            )
        })
        .collect();
    use ActivityOutcome::*;
    assert_eq!(
        lines,
        [
            ("Everything", None, "enable", Done, "confirm_changes"),
            (
                "Everything",
                Some(&alpha),
                "set_permissions",
                Done,
                "read_files, write_files"
            ),
            ("Everything", Some(&alpha), "read_files", Allowed, ""),
            (
                "Everything",
                Some(&alpha),
                "write_files",
                NeedsConfirmation,
                ""
            ),
            (
                "Everything",
                Some(&beta),
                "read_files",
                Refused,
                "not_granted"
            ),
            (
                "Unknown plugin",
                Some(&alpha),
                "read_files",
                Refused,
                "unknown_plugin"
            ),
            ("Everything", None, "disable", Done, ""),
        ]
    );
    assert!(
        log.iter()
            .all(|e| e.plugin_id == PLUGIN || e.plugin_id.is_empty())
    );
    // Newest first, with increasing IDs and a timestamp on each.
    assert!(log.windows(2).all(|pair| pair[0].id > pair[1].id));
    assert!(log.iter().all(|e| e.at.contains('T')));
    // Nothing a caller supplied as free text reaches the stored log.
    let stored = serde_json::to_string(&log).unwrap().to_lowercase();
    for secret in ["hunter2", "password", "private key", "begin"] {
        assert!(!stored.contains(secret), "{secret}");
    }
    // The general activity history is not used for plugin calls any more.
    assert!(
        f.store
            .snapshot()
            .unwrap()
            .events
            .iter()
            .all(|event| !event.message.contains("Everything"))
    );
}

#[test]
fn the_log_stays_within_the_history_retention_bound() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    f.store.set_history_limit(200).unwrap();
    for _ in 0..260 {
        f.ask(&alpha, Capability::ReadFiles);
    }
    let log = f.store.plugin_activity(500).unwrap();
    assert_eq!(log.kept, 200);
    assert_eq!(log.entries.len(), 200);
    // The newest are kept; a page is capped.
    assert_eq!(log.entries[0].id, 260);
    assert_eq!(f.store.plugin_activity(10).unwrap().entries.len(), 10);
    f.store.set_history_limit(1000).unwrap();
    assert_eq!(
        f.store.plugin_activity(u32::MAX).unwrap().entries.len(),
        200
    );
}

#[test]
fn the_kill_switch_refuses_everything_at_once_and_restores_exactly_what_was_set() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::TrustedAutomation)
        .unwrap();
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &Fixture::grantable())
        .unwrap();
    assert!(f.ask(&alpha, Capability::WriteFiles).allowed);
    let before = f.view();

    f.store.plugins_set_all_off(true).unwrap();
    assert!(f.store.plugins_state(&f.plugins).unwrap().all_off);
    for capability in ALL {
        assert_eq!(
            f.refusal(&alpha, capability),
            Refusal::AllPluginsOff,
            "{capability:?}"
        );
    }
    // Repeating it is not a second event.
    f.store.plugins_set_all_off(true).unwrap();
    let switches = |f: &Fixture| {
        f.log()
            .iter()
            .filter(|e| e.action.starts_with("all_plugins_"))
            .map(|e| e.action.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(switches(&f), ["all_plugins_off"]);
    // It survives a restart of the store.
    let Fixture {
        _directory,
        database,
        plugins,
        store,
        ..
    } = f;
    drop(store);
    let mut store = Store::open(&database).unwrap();
    assert!(store.plugins_state(&plugins).unwrap().all_off);
    let decision = store
        .plugin_authorize(&plugins, PLUGIN, &alpha, Capability::ReadFiles)
        .unwrap();
    assert_eq!(decision.refusal, Some(Refusal::AllPluginsOff));

    // Turning plugins back on restores the owner's settings unchanged.
    store.plugins_set_all_off(false).unwrap();
    let state = store.plugins_state(&plugins).unwrap();
    assert!(!state.all_off);
    assert_eq!(state.plugins[0], before);
    assert!(
        store
            .plugin_authorize(&plugins, PLUGIN, &alpha, Capability::WriteFiles)
            .unwrap()
            .allowed
    );
}

#[test]
fn trusted_automation_can_be_revoked_without_disabling_and_takes_effect_on_the_next_call() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    f.store
        .plugin_enable(&f.plugins, PLUGIN, TrustLevel::TrustedAutomation)
        .unwrap();
    f.store
        .plugin_set_grant(&f.plugins, PLUGIN, &alpha, &Fixture::grantable())
        .unwrap();
    let decision = f.ask(&alpha, Capability::ControlWorkloads);
    assert!(decision.allowed && !decision.needs_confirmation);

    f.store.plugin_revoke_trusted(&f.plugins, PLUGIN).unwrap();
    let view = f.view();
    assert_eq!(
        (view.status, view.trust),
        (PluginStatus::Enabled, Some(TrustLevel::ConfirmChanges))
    );
    for capability in CHANGE {
        let decision = f.ask(&alpha, capability);
        assert!(
            decision.allowed && decision.needs_confirmation,
            "{capability:?}"
        );
    }
    assert!(!f.ask(&alpha, Capability::ReadFiles).needs_confirmation);
    assert_eq!(
        f.refusal(&alpha, Capability::Elevate),
        Refusal::NoElevationException
    );
    // There is nothing to revoke twice, or on a plugin that never had it.
    assert_eq!(
        f.store
            .plugin_revoke_trusted(&f.plugins, PLUGIN)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(
        f.store
            .plugin_revoke_trusted(&f.plugins, "org.example.absent")
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert!(
        f.log()
            .iter()
            .any(|e| e.action == "revoke_trusted_automation" && e.detail == "confirm_changes")
    );
}

const PACK: &str = "org.example.host-basics";

fn pack_fixture() -> Fixture {
    let f = fixture();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/host-basics/infrawerf-plugin.json");
    install(
        &f.plugins,
        "host-basics",
        &std::fs::read_to_string(example).unwrap(),
    );
    f
}

fn run_log(f: &Fixture) -> Vec<(String, ActivityOutcome, String)> {
    let mut entries = f.store.plugin_activity(500).unwrap().entries;
    entries.reverse();
    entries
        .into_iter()
        .filter(|e| e.plugin_id == PACK || e.plugin_id.is_empty())
        .map(|e| (e.action, e.outcome, e.detail))
        .collect()
}

#[test]
fn a_pack_command_is_listed_and_admitted_only_where_the_owner_allowed_it() {
    let mut f = pack_fixture();
    let both = [Capability::SshSession, Capability::RunApprovedCommand];
    let listed = |f: &Fixture, server: &ServerId| -> Vec<String> {
        f.store
            .pack_commands(&f.plugins, server)
            .unwrap()
            .into_iter()
            .map(|run| run.contribution.id)
            .collect()
    };
    // The page shows the exact command before anything is enabled.
    let state = f.store.plugins_state(&f.plugins).unwrap();
    let view = state.plugins.iter().find(|p| p.id == PACK).unwrap();
    let shown: Vec<String> = view
        .contributions
        .iter()
        .map(|c| c.command.as_ref().unwrap().to_string())
        .collect();
    assert!(shown.contains(&"systemctl is-active {unit}".to_owned()));
    // Found but not enabled: nothing is listed and a run is refused.
    assert!(listed(&f, &f.alpha).is_empty());
    let alpha = f.alpha.clone();
    let beta = f.beta.clone();
    assert_eq!(
        f.store
            .plugin_begin_run(&f.plugins, PACK, &alpha, "kernel")
            .unwrap()
            .unwrap_err(),
        Refusal::PluginDisabled
    );
    // Enabled read-only with one of the two permissions: still nothing.
    f.store
        .plugin_enable(&f.plugins, PACK, TrustLevel::ReadOnly)
        .unwrap();
    f.store
        .plugin_set_grant(&f.plugins, PACK, &alpha, &[Capability::SshSession])
        .unwrap();
    assert!(listed(&f, &alpha).is_empty());
    assert_eq!(
        f.store
            .plugin_begin_run(&f.plugins, PACK, &alpha, "kernel")
            .unwrap()
            .unwrap_err(),
        Refusal::NotGranted
    );
    // Both permissions on one server: every command there, none elsewhere.
    f.store
        .plugin_set_grant(&f.plugins, PACK, &alpha, &both)
        .unwrap();
    assert_eq!(
        listed(&f, &alpha),
        ["disk-usage", "kernel", "system-state", "service-active"]
    );
    assert!(listed(&f, &beta).is_empty());
    let run = f
        .store
        .plugin_begin_run(&f.plugins, PACK, &alpha, "kernel")
        .unwrap()
        .unwrap();
    assert_eq!(run.command.argv, ["uname", "-r"]);
    assert_eq!(
        f.store
            .plugin_begin_run(&f.plugins, PACK, &beta, "kernel")
            .unwrap()
            .unwrap_err(),
        Refusal::NotGranted
    );
    // Unknown contributions and unknown plugins run nothing.
    assert!(
        f.store
            .plugin_begin_run(&f.plugins, PACK, &alpha, "absent")
            .is_err()
    );
    assert_eq!(
        f.store
            .plugin_begin_run(&f.plugins, "org.example.absent", &alpha, "kernel")
            .unwrap()
            .unwrap_err(),
        Refusal::UnknownPlugin
    );
    // A run's end is logged with the contribution only.
    f.store
        .plugin_finish_run(&run, &alpha, ActivityOutcome::Done)
        .unwrap();
    f.store
        .plugin_finish_run(&run, &alpha, ActivityOutcome::Cancelled)
        .unwrap();
    let log = run_log(&f);
    let tail: Vec<_> = log[log.len() - 2..].to_vec();
    assert_eq!(
        tail,
        [
            ("run_command".into(), ActivityOutcome::Done, "kernel".into()),
            (
                "run_command".into(),
                ActivityOutcome::Cancelled,
                "kernel".into()
            ),
        ]
    );
    // The admitted run logged one allowed entry per capability it uses.
    assert!(log.contains(&(
        "run_approved_command".into(),
        ActivityOutcome::Allowed,
        String::new()
    )));
    // The kill switch, a withdrawn permission and a changed manifest each
    // empty the list and refuse the next run.
    f.store.plugins_set_all_off(true).unwrap();
    assert!(listed(&f, &alpha).is_empty());
    assert_eq!(
        f.store
            .plugin_begin_run(&f.plugins, PACK, &alpha, "kernel")
            .unwrap()
            .unwrap_err(),
        Refusal::AllPluginsOff
    );
    f.store.plugins_set_all_off(false).unwrap();
    assert_eq!(listed(&f, &alpha).len(), 4);
    f.store
        .plugin_set_grant(&f.plugins, PACK, &alpha, &[])
        .unwrap();
    assert!(listed(&f, &alpha).is_empty());
    f.store
        .plugin_set_grant(&f.plugins, PACK, &alpha, &both)
        .unwrap();
    // The owner reviewed `uname -r`. A pack that now says something else
    // is switched off until it is reviewed again.
    let path = f.plugins.join("host-basics/infrawerf-plugin.json");
    let changed = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\"-r\"", "\"-a\"");
    std::fs::write(&path, changed).unwrap();
    assert!(listed(&f, &alpha).is_empty());
    assert_eq!(
        f.store
            .plugin_begin_run(&f.plugins, PACK, &alpha, "kernel")
            .unwrap()
            .unwrap_err(),
        Refusal::ChangedSinceReview
    );
}

const MAP_PACK: &str = "org.example.homelab-map";

fn map_fixture() -> Fixture {
    let f = fixture();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/homelab-map/infrawerf-plugin.json");
    install(
        &f.plugins,
        "homelab-map",
        &std::fs::read_to_string(example).unwrap(),
    );
    f
}

#[test]
fn pack_badges_apps_and_relationship_kinds_appear_only_where_the_owner_allowed_them() {
    let mut f = map_fixture();
    let (alpha, beta) = (f.alpha.clone(), f.beta.clone());
    let badges = |f: &Fixture| -> Vec<(ServerId, String)> {
        f.store
            .pack_badges(&f.plugins)
            .unwrap()
            .into_iter()
            .map(|b| (b.server_id, b.observation))
            .collect()
    };
    let kinds = |f: &Fixture| f.store.relationship_kinds(&f.plugins).unwrap();
    let apps = |f: &Fixture, server: &ServerId| -> Vec<(u16, String)> {
        f.store
            .pack_apps(&f.plugins, server)
            .unwrap()
            .into_iter()
            .map(|a| (a.port, a.path))
            .collect()
    };
    // Found, not enabled: nothing anywhere, and an open is refused.
    assert!(badges(&f).is_empty() && kinds(&f).is_empty() && apps(&f, &alpha).is_empty());
    assert_eq!(
        f.store
            .plugin_begin_app(&f.plugins, MAP_PACK, &alpha, "status-page")
            .unwrap()
            .unwrap_err(),
        Refusal::PluginDisabled
    );
    // Read-only, everything allowed on alpha: badge and kind, but no app,
    // because opening a tunnel is a change.
    f.store
        .plugin_enable(&f.plugins, MAP_PACK, TrustLevel::ReadOnly)
        .unwrap();
    let all = [
        Capability::ReadSavedState,
        Capability::SshSession,
        Capability::RunApprovedCommand,
        Capability::LoopbackTunnel,
    ];
    f.store
        .plugin_set_grant(&f.plugins, MAP_PACK, &alpha, &all)
        .unwrap();
    assert_eq!(badges(&f), [(alpha.clone(), "system-state".to_owned())]);
    assert_eq!(kinds(&f).len(), 1);
    assert_eq!(kinds(&f)[0].server_ids, [alpha.clone()]);
    assert!(apps(&f, &alpha).is_empty());
    assert_eq!(
        f.store
            .plugin_begin_app(&f.plugins, MAP_PACK, &alpha, "status-page")
            .unwrap()
            .unwrap_err(),
        Refusal::TrustTooLow
    );
    // A badge needs the command it shows to be allowed too.
    f.store
        .plugin_set_grant(&f.plugins, MAP_PACK, &beta, &[Capability::ReadSavedState])
        .unwrap();
    assert_eq!(badges(&f).len(), 1);
    // The kind can be chosen only between two servers it is allowed on.
    let choice = || AddRelationship {
        source: f.alpha.clone(),
        target: f.beta.clone(),
        label: "nightly".into(),
        kind: Some(RelationshipKindChoice {
            plugin_id: MAP_PACK.into(),
            contribution_id: "backs-up-to".into(),
        }),
    };
    let first = choice();
    assert_eq!(kinds(&f)[0].server_ids, [alpha.clone(), beta.clone()]);
    f.store
        .add_relationship_with_kinds(&f.plugins, first)
        .unwrap();
    let edge = f.store.snapshot().unwrap().edges.pop().unwrap();
    assert_eq!(
        edge.kind,
        Some(RelationshipKindRef {
            plugin_id: MAP_PACK.into(),
            kind_id: "backs-up-to".into(),
            title: "Backs up to".into()
        })
    );
    // A kind cannot be sent through the plain path, nor made up.
    let mut plain = choice();
    plain.label = "other".into();
    assert_eq!(
        f.store.add_relationship(plain).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    let mut made_up = choice();
    made_up.label = "made up".into();
    made_up.kind.as_mut().unwrap().contribution_id = "state-badge".into();
    assert_eq!(
        f.store
            .add_relationship_with_kinds(&f.plugins, made_up)
            .unwrap_err()
            .code,
        ErrorCode::Denied
    );
    // Confirm each change: the app is listed with the reviewed port and
    // path, and the press of Open is the confirmation.
    f.store
        .plugin_enable(&f.plugins, MAP_PACK, TrustLevel::ConfirmChanges)
        .unwrap();
    assert_eq!(apps(&f, &alpha), [(8099, "/status".to_owned())]);
    assert!(apps(&f, &beta).is_empty());
    let app = f
        .store
        .plugin_begin_app(&f.plugins, MAP_PACK, &alpha, "status-page")
        .unwrap()
        .unwrap();
    f.store
        .plugin_finish_app(&app, &alpha, ActivityOutcome::Done)
        .unwrap();
    let log = f.store.plugin_activity(500).unwrap().entries;
    assert_eq!(log[0].action, ACTION_OPEN_APP);
    assert_eq!(log[0].detail, "status-page");
    // Disabling removes everything, but the documented relationship keeps
    // its kind's title.
    f.store.plugin_disable(&f.plugins, MAP_PACK).unwrap();
    assert!(badges(&f).is_empty() && kinds(&f).is_empty() && apps(&f, &alpha).is_empty());
    let edge = f.store.snapshot().unwrap().edges.pop().unwrap();
    assert_eq!(edge.kind.unwrap().title, "Backs up to");
    // Removing the relationship removes its kind.
    f.store.remove_relationship(&edge.id).unwrap();
    assert!(f.store.snapshot().unwrap().edges.is_empty());
    let database = f.database.clone();
    drop(f.store);
    let left: i64 = Connection::open(database)
        .unwrap()
        .query_row("SELECT count(*) FROM edge_kind", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn a_plugin_view_is_offered_only_where_the_owner_allowed_it() {
    let mut f = fixture();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/host-dashboard/infrawerf-plugin.json");
    install(
        &f.plugins,
        "host-dashboard",
        &std::fs::read_to_string(example).unwrap(),
    );
    let plugin = "org.example.host-dashboard";
    let (alpha, beta) = (f.alpha.clone(), f.beta.clone());
    let views = |f: &Fixture, server: &ServerId| -> Vec<String> {
        f.store
            .plugin_views(&f.plugins, server)
            .unwrap()
            .into_iter()
            .map(|v| format!("{}:{}", v.tab.contribution_id, v.entry))
            .collect()
    };
    assert!(views(&f, &alpha).is_empty());
    f.store
        .plugin_enable(&f.plugins, plugin, TrustLevel::ReadOnly)
        .unwrap();
    assert!(views(&f, &alpha).is_empty());
    f.store
        .plugin_set_grant(&f.plugins, plugin, &alpha, &[Capability::ReadSavedState])
        .unwrap();
    assert_eq!(views(&f, &alpha), ["dashboard:view/index.html"]);
    assert!(views(&f, &beta).is_empty());
    let source = f.store.plugin_views(&f.plugins, &alpha).unwrap().remove(0);
    assert_eq!(source.folder, f.plugins.join("host-dashboard"));
    // Turned off, disabled or withdrawn: gone.
    f.store.plugins_set_all_off(true).unwrap();
    assert!(views(&f, &alpha).is_empty());
    f.store.plugins_set_all_off(false).unwrap();
    assert_eq!(views(&f, &alpha).len(), 1);
    f.store
        .plugin_set_grant(&f.plugins, plugin, &alpha, &[])
        .unwrap();
    assert!(views(&f, &alpha).is_empty());
    // What a view does is logged with only a fixed word and a host.
    f.store
        .plugin_view_event(
            &f.plugins,
            plugin,
            &alpha,
            ACTION_VIEW_BLOCKED,
            ActivityOutcome::Refused,
            "content_policy: connect-src https://example.com
",
        )
        .unwrap();
    let entry = f.store.plugin_activity(5).unwrap().entries.remove(0);
    assert_eq!(entry.action, ACTION_VIEW_BLOCKED);
    assert_eq!(
        entry.detail,
        "content_policy: connect-src https://example.com"
    );
}
