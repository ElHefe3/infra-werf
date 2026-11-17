//! Plugin updates (T046): each case has the plugin at two versions. An
//! update that asks for nothing new keeps the owner's review; anything new
//! switches the plugin off until it is reviewed, and nothing is granted.
use atlas_core::extensions::Capability;
use atlas_core::plugin_host::*;
use atlas_core::plugin_install::*;
use atlas_core::*;
use std::path::PathBuf;

const PACK: &str = "org.example.host-basics";
const BOTH: [Capability; 2] = [Capability::SshSession, Capability::RunApprovedCommand];

fn version(case: &str, which: &str) -> InstallRequest {
    InstallRequest {
        source: InstallSource::Folder {
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/plugin-updates")
                .join(case)
                .join(which)
                .to_string_lossy()
                .into_owned(),
        },
        expected_sha256: None,
        public_key: None,
    }
}

struct Fixture {
    _directory: tempfile::TempDir,
    plugins: PathBuf,
    store: Store,
    server: ServerId,
}

/// Version 1 of a case, installed, enabled read-only and allowed
/// everything it asks for on one server.
fn installed(case: &str) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let plugins = directory.path().join("plugins");
    std::fs::create_dir(&plugins).unwrap();
    let mut store = Store::create(&directory.path().join("atlas.sqlite3")).unwrap();
    let server = store
        .add_server(AddServer {
            name: "Alpha".into(),
            host: "alpha.invalid".into(),
        })
        .unwrap();
    let prepared = prepare(&version(case, "v1")).unwrap();
    assert_eq!(
        store.plugin_publish(&plugins, &prepared).unwrap().problem,
        None
    );
    store
        .plugin_enable(&plugins, PACK, TrustLevel::ReadOnly)
        .unwrap();
    let all: Vec<Capability> = prepared.manifest.capabilities.clone();
    store
        .plugin_set_grant(&plugins, PACK, &server, &all)
        .unwrap();
    Fixture {
        _directory: directory,
        plugins,
        store,
        server,
    }
}

impl Fixture {
    fn view(&self) -> PluginView {
        self.store
            .plugins_state(&self.plugins)
            .unwrap()
            .plugins
            .into_iter()
            .find(|p| p.id == PACK)
            .unwrap()
    }
    fn hash(&self) -> String {
        folder_hash(&self.plugins.join(PACK)).unwrap()
    }
    /// Checks then applies, as the interface does.
    fn update(&mut self, case: &str, downgrade: bool) -> Result<UpdateDiff, InstallProblem> {
        let prepared = prepare(&version(case, "v2"))?;
        let (previous, shown) = self
            .store
            .plugin_update_diff(&self.plugins, PACK, &prepared)
            .unwrap()?;
        let applied = self
            .store
            .plugin_apply_update(&self.plugins, PACK, &prepared, &previous, downgrade)
            .unwrap()?;
        assert_eq!(applied, shown, "what applies is what was shown");
        Ok(applied)
    }
    fn allowed(&mut self, capability: Capability) -> bool {
        let server = self.server.clone();
        self.store
            .plugin_authorize(&self.plugins, PACK, &server, capability)
            .unwrap()
            .allowed
    }
    /// The newest update entry in the log.
    fn last_log(&self) -> PluginActivity {
        self.store
            .plugin_activity(50)
            .unwrap()
            .entries
            .into_iter()
            .find(|e| e.action == ACTION_UPDATE)
            .unwrap()
    }
}

#[test]
fn an_update_asking_for_nothing_new_applies_keeps_the_review_and_is_logged() {
    for (case, removed) in [("same", vec![]), ("fewer", vec![Capability::NetworkProbe])] {
        let mut f = installed(case);
        if case == "fewer" {
            assert!(f.allowed(Capability::NetworkProbe));
        }
        let diff = f.update(case, false).unwrap();
        assert!(!diff.needs_review, "{case}: {diff:?}");
        assert!(diff.added_capabilities.is_empty());
        assert_eq!(diff.removed_capabilities, removed);
        let view = f.view();
        assert_eq!(view.status, PluginStatus::Enabled, "{case}");
        assert_eq!(view.trust, Some(TrustLevel::ReadOnly));
        assert!(BOTH.iter().all(|c| f.allowed(*c)), "{case}");
        // A capability the update no longer asks for is gone, permission
        // and all.
        if case == "fewer" {
            assert!(!f.allowed(Capability::NetworkProbe));
            assert_eq!(view.grants[0].capabilities, BOTH);
            assert!(
                diff.contributions
                    .iter()
                    .any(|c| c.id == "ping" && c.change == ContributionChangeKind::Removed)
            );
        } else {
            assert_eq!(
                diff.contributions
                    .iter()
                    .map(|c| (c.id.as_str(), c.change))
                    .collect::<Vec<_>>(),
                [("kernel", ContributionChangeKind::Presentation)]
            );
        }
        assert_eq!(view.version, diff.to_version);
        let install = view.install.unwrap();
        assert_eq!(install.integrity, Integrity::Verified);
        assert_eq!(install.content_sha256, f.hash());
        let log = f.last_log();
        assert_eq!(
            (log.action.as_str(), log.outcome),
            (ACTION_UPDATE, ActivityOutcome::Done)
        );
        assert_eq!(
            log.detail,
            format!("{} to {}", diff.from_version, diff.to_version)
        );
    }
}

#[test]
fn an_update_asking_for_anything_new_stays_off_until_reviewed_and_grants_nothing() {
    for (case, change) in [
        ("added-capability", ContributionChangeKind::Added),
        ("changed-command", ContributionChangeKind::CommandChanged),
        ("new-contribution", ContributionChangeKind::Added),
    ] {
        let mut f = installed(case);
        let diff = f.update(case, false).unwrap();
        assert!(diff.needs_review, "{case}");
        assert!(
            diff.contributions.iter().any(|c| c.change == change),
            "{case}: {diff:?}"
        );
        if case == "added-capability" {
            assert_eq!(diff.added_capabilities, [Capability::LoopbackTunnel]);
        }
        if case == "changed-command" {
            let changed = &diff.contributions[0];
            assert_eq!(changed.command.as_deref(), Some("uname -a"));
        }
        // Switched off: nothing is allowed, not even what was allowed before.
        let view = f.view();
        assert_eq!(view.status, PluginStatus::ChangedSinceReview, "{case}");
        assert!(BOTH.iter().all(|c| !f.allowed(*c)), "{case}");
        assert!(
            f.store
                .pack_commands(&f.plugins, &f.server.clone())
                .unwrap()
                .is_empty()
        );
        let log = f.last_log();
        assert!(
            log.detail.ends_with(", needs your review"),
            "{}",
            log.detail
        );
        // Reviewing and enabling again restores what was allowed before,
        // and nothing more: the new capability was never granted.
        f.store
            .plugin_enable(&f.plugins, PACK, TrustLevel::ConfirmChanges)
            .unwrap();
        assert!(BOTH.iter().all(|c| f.allowed(*c)), "{case}");
        assert_eq!(f.view().grants[0].capabilities, BOTH);
        if case == "added-capability" {
            assert!(!f.allowed(Capability::LoopbackTunnel));
            let server = f.server.clone();
            assert_eq!(
                f.store
                    .plugin_authorize(&f.plugins, PACK, &server, Capability::LoopbackTunnel)
                    .unwrap()
                    .refusal,
                Some(Refusal::NotGranted)
            );
        }
    }
}

#[test]
fn a_failed_update_leaves_the_previous_version_working() {
    let mut f = installed("same");
    let before = f.hash();
    let still_working = |f: &mut Fixture| {
        assert_eq!(f.hash(), before);
        assert_eq!(f.view().status, PluginStatus::Enabled);
        assert_eq!(f.view().version, "1.0.0");
        assert!(BOTH.iter().all(|c| f.allowed(*c)));
        let entries: Vec<String> = std::fs::read_dir(&f.plugins)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, [PACK]);
    };
    // Refused while checking: an invalid manifest, another plugin's ID.
    assert_eq!(
        f.update("invalid", false).unwrap_err().code,
        InstallProblemCode::InvalidManifest
    );
    still_working(&mut f);
    assert_eq!(
        f.update("different-plugin", false).unwrap_err().code,
        InstallProblemCode::DifferentPlugin
    );
    still_working(&mut f);
    // A downgrade needs the owner's confirmation; with it, it applies.
    let mut g = installed("downgrade");
    assert_eq!(
        g.update("downgrade", false).unwrap_err().code,
        InstallProblemCode::Downgrade
    );
    assert_eq!(g.view().version, "1.0.0");
    let diff = g.update("downgrade", true).unwrap();
    assert!(diff.downgrade);
    assert_eq!(g.last_log().detail, "1.0.0 to 0.9.0, downgrade");
    // The installed plugin changed between checking and applying.
    let prepared = prepare(&version("same", "v2")).unwrap();
    let (previous, _) = f
        .store
        .plugin_update_diff(&f.plugins, PACK, &prepared)
        .unwrap()
        .unwrap();
    assert_eq!(
        f.store
            .plugin_apply_update(&f.plugins, PACK, &prepared, &"0".repeat(64), false)
            .unwrap()
            .unwrap_err()
            .code,
        InstallProblemCode::ChangedSincePreview
    );
    still_working(&mut f);
    // The files are swapped back if the records cannot be saved: replace
    // and restore leave the previous version exactly as it was.
    let parked = replace(&f.plugins, PACK, &prepared).unwrap();
    assert_ne!(f.hash(), before);
    restore(&f.plugins, PACK, &parked);
    still_working(&mut f);
    // And the update still applies afterwards.
    f.store
        .plugin_apply_update(&f.plugins, PACK, &prepared, &previous, false)
        .unwrap()
        .unwrap();
    assert_eq!(f.view().version, "1.0.1");
}

#[test]
fn an_update_shows_the_previous_and_new_signer_and_resets_a_changed_plugin() {
    let mut f = installed("same");
    // A plugin whose files changed is not carried over by an update: it
    // must be reviewed again.
    std::fs::write(f.plugins.join(PACK).join("README.md"), "tampered").unwrap();
    f.store.plugins_verify(&f.plugins).unwrap();
    assert_eq!(f.view().status, PluginStatus::FilesChanged);
    let diff = f.update("same", false).unwrap();
    assert!(!diff.needs_review);
    let view = f.view();
    assert_eq!(view.status, PluginStatus::Disabled);
    assert_eq!(view.install.unwrap().integrity, Integrity::Verified);
    // The signer of the previous install is shown next to the update's.
    let archives = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin-archives");
    let directory = tempfile::tempdir().unwrap();
    let plugins = directory.path().join("plugins");
    let mut store = Store::create(&directory.path().join("atlas.sqlite3")).unwrap();
    let signed = prepare(&InstallRequest {
        source: InstallSource::Archive {
            path: archives.join("good.zip").to_string_lossy().into_owned(),
        },
        expected_sha256: None,
        public_key: Some(std::fs::read_to_string(archives.join("author.pub")).unwrap()),
    })
    .unwrap();
    store.plugin_publish(&plugins, &signed).unwrap();
    let unsigned = prepare(&version("same", "v2")).unwrap();
    let (_, diff) = store
        .plugin_update_diff(&plugins, PACK, &unsigned)
        .unwrap()
        .unwrap();
    assert_eq!(diff.previous_signer.as_deref(), Some("BAEDC8198EBE6C24"));
    assert_eq!(diff.signer, None);
}

#[test]
fn versions_are_ordered_as_semantic_versioning_says() {
    use std::cmp::Ordering::*;
    for (a, b, order) in [
        ("1.0.0", "1.0.1", Less),
        ("1.10.0", "1.9.9", Greater),
        ("2.0.0-beta.1", "2.0.0", Less),
        ("2.0.0-beta.2", "2.0.0-beta.10", Less),
        ("2.0.0-alpha", "2.0.0-beta", Less),
        ("2.0.0-1", "2.0.0-alpha", Less),
        ("1.0.0+build.5", "1.0.0", Equal),
    ] {
        assert_eq!(compare_versions(a, b), order, "{a} {b}");
    }
}
