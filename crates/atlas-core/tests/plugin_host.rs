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
        self.store
            .snapshot()
            .unwrap()
            .events
            .iter()
            .filter(|event| event.message.contains("was refused"))
            .count()
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
