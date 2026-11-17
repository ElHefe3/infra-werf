//! Installing, verifying and removing plugins (T045). Every bad archive is
//! refused with its own code and leaves nothing in the plugins folder.
use atlas_core::extensions::Capability;
use atlas_core::plugin_host::*;
use atlas_core::plugin_install::*;
use atlas_core::*;
use std::path::PathBuf;

const PACK: &str = "org.example.host-basics";
const MANIFEST: &str = "infrawerf-plugin.json";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin-archives")
}
fn archive(name: &str) -> InstallRequest {
    InstallRequest {
        source: InstallSource::Archive {
            path: fixtures().join(name).to_string_lossy().into_owned(),
        },
        expected_sha256: None,
        public_key: None,
    }
}
fn key(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).unwrap()
}

struct Fixture {
    _directory: tempfile::TempDir,
    plugins: PathBuf,
    store: Store,
    server: ServerId,
}
fn fixture() -> Fixture {
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
    Fixture {
        _directory: directory,
        plugins,
        store,
        server,
    }
}
impl Fixture {
    fn install(&mut self, request: &InstallRequest) -> InstallReport {
        match prepare(request) {
            Ok(prepared) => self.store.plugin_publish(&self.plugins, &prepared).unwrap(),
            Err(problem) => InstallReport::refused(problem),
        }
    }
    fn view(&self) -> Option<PluginView> {
        self.store
            .plugins_state(&self.plugins)
            .unwrap()
            .plugins
            .into_iter()
            .find(|p| p.id == PACK)
    }
    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.plugins)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

#[test]
fn each_bad_archive_is_refused_with_its_own_code_and_leaves_nothing_behind() {
    let mut checked = 0;
    for entry in std::fs::read_dir(fixtures()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix("bad-") else {
            continue;
        };
        if name.ends_with(".minisig") {
            continue;
        }
        let code = rest.split(['-', '.']).next().unwrap().to_owned();
        let mut f = fixture();
        let mut request = archive(&name);
        if code == "signature_invalid" {
            request.public_key = Some(key("author.pub"));
        }
        let report = f.install(&request);
        let problem = report.problem.unwrap_or_else(|| panic!("{name} installed"));
        assert_eq!(
            serde_json::to_value(problem.code).unwrap(),
            serde_json::Value::String(code.clone()),
            "{name}: {}",
            problem.message
        );
        assert!(!problem.message.is_empty());
        assert!(f.entries().is_empty(), "{name} left {:?}", f.entries());
        assert!(f.view().is_none());
        checked += 1;
    }
    assert_eq!(checked, 16);
}

#[test]
fn a_plugin_installs_from_a_folder_and_from_each_archive_kind_and_is_disabled() {
    let source = tempfile::tempdir().unwrap();
    let folder = source.path().join("my-pack");
    std::fs::create_dir_all(folder.join("docs")).unwrap();
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/host-basics/infrawerf-plugin.json"),
        folder.join(MANIFEST),
    )
    .unwrap();
    std::fs::write(folder.join("docs/notes.md"), "notes").unwrap();
    let from_folder = InstallRequest {
        source: InstallSource::Folder {
            path: folder.to_string_lossy().into_owned(),
        },
        expected_sha256: None,
        public_key: None,
    };
    for (request, kind) in [
        (from_folder, SourceKind::Folder),
        (archive("good.zip"), SourceKind::Archive),
        (archive("good-folder.tar.gz"), SourceKind::Archive),
    ] {
        let mut f = fixture();
        let report = f.install(&request);
        assert_eq!(report.problem, None);
        assert_eq!(report.plugin_id.as_deref(), Some(PACK));
        assert_eq!(f.entries(), [PACK]);
        let view = f.view().unwrap();
        // Installing allows nothing.
        assert_eq!(view.status, PluginStatus::Disabled);
        assert!(view.grants.is_empty());
        let install = view.install.unwrap();
        assert_eq!(install.source_kind, kind);
        assert_eq!(install.integrity, Integrity::Verified);
        assert_eq!(Some(install.content_sha256.clone()), report.content_sha256);
        assert_eq!(
            folder_hash(&f.plugins.join(PACK)),
            Some(install.content_sha256)
        );
        // An archive made from a folder is unpacked without its top folder.
        assert!(f.plugins.join(PACK).join(MANIFEST).exists());
        // The same plugin cannot be installed twice.
        let again = f.install(&request).problem.unwrap();
        assert_eq!(again.code, InstallProblemCode::AlreadyInstalled);
        assert_eq!(f.entries(), [PACK]);
        let log = f.store.plugin_activity(50).unwrap().entries;
        assert_eq!(log[0].action, ACTION_INSTALL);
        assert_eq!(log[0].plugin_id, PACK);
    }
}

#[test]
fn hashes_and_signatures_are_checked_when_the_owner_asks() {
    let good = std::fs::read(fixtures().join("good.zip")).unwrap();
    let hash = sha256_hex(&good);
    let mut f = fixture();
    let mut request = archive("good.zip");
    request.expected_sha256 = Some("0".repeat(64));
    assert_eq!(
        f.install(&request).problem.unwrap().code,
        InstallProblemCode::HashMismatch
    );
    request.expected_sha256 = Some("not a hash".into());
    assert_eq!(
        f.install(&request).problem.unwrap().code,
        InstallProblemCode::HashMismatch
    );
    // The wrong key, a malformed key and a key with no signature file are
    // each refused.
    request.expected_sha256 = Some(hash.to_uppercase());
    request.public_key = Some(key("other.pub"));
    assert_eq!(
        f.install(&request).problem.unwrap().code,
        InstallProblemCode::SignatureInvalid
    );
    request.public_key = Some("RWnot-a-key".into());
    assert_eq!(
        f.install(&request).problem.unwrap().code,
        InstallProblemCode::InvalidPublicKey
    );
    let mut unsigned = archive("bad-unsupported_archive.zip");
    unsigned.public_key = Some(key("author.pub"));
    assert_eq!(
        f.install(&unsigned).problem.unwrap().code,
        InstallProblemCode::SignatureMissing
    );
    let folder = InstallRequest {
        source: InstallSource::Folder {
            path: fixtures().to_string_lossy().into_owned(),
        },
        expected_sha256: None,
        public_key: Some(key("author.pub")),
    };
    assert_eq!(
        f.install(&folder).problem.unwrap().code,
        InstallProblemCode::SignatureNotPossible
    );
    assert!(f.entries().is_empty());
    // The right hash and key: installed, with the signer recorded.
    request.public_key = Some(key("author.pub"));
    let report = f.install(&request);
    assert_eq!(report.problem, None);
    let install = f.view().unwrap().install.unwrap();
    assert_eq!(install.archive_sha256.as_deref(), Some(hash.as_str()));
    assert_eq!(install.signer_key_id.as_deref(), Some("BAEDC8198EBE6C24"));
    assert!(key("author.pub").contains("BAEDC8198EBE6C24"));
    // The tar.gz signature verifies too.
    let mut f = fixture();
    let mut tar = archive("good-folder.tar.gz");
    tar.public_key = Some(key("author.pub"));
    assert_eq!(f.install(&tar).problem, None);
}

#[test]
fn changed_files_are_found_at_start_up_and_refuse_everything() {
    let mut f = fixture();
    f.install(&archive("good.zip"));
    let server = f.server.clone();
    f.store
        .plugin_enable(&f.plugins, PACK, TrustLevel::ReadOnly)
        .unwrap();
    let both = [Capability::SshSession, Capability::RunApprovedCommand];
    f.store
        .plugin_set_grant(&f.plugins, PACK, &server, &both)
        .unwrap();
    assert!(f.store.plugins_verify(&f.plugins).unwrap().is_empty());
    assert_eq!(f.store.pack_commands(&f.plugins, &server).unwrap().len(), 4);
    // A file other than the manifest changes: the manifest check cannot see
    // it, the start-up check does.
    std::fs::write(f.plugins.join(PACK).join("README.md"), "changed").unwrap();
    assert_eq!(f.view().unwrap().status, PluginStatus::Enabled);
    assert_eq!(f.store.plugins_verify(&f.plugins).unwrap(), [PACK]);
    let view = f.view().unwrap();
    assert_eq!(view.status, PluginStatus::FilesChanged);
    assert_eq!(view.trust, None);
    assert_eq!(view.install.unwrap().integrity, Integrity::Changed);
    assert!(
        f.store
            .pack_commands(&f.plugins, &server)
            .unwrap()
            .is_empty()
    );
    let decision = f
        .store
        .plugin_authorize(&f.plugins, PACK, &server, Capability::SshSession)
        .unwrap();
    assert_eq!(decision.refusal, Some(Refusal::FilesChanged));
    // An extra file is a change too.
    std::fs::write(
        f.plugins.join(PACK).join("README.md"),
        "# Host basics\n\nA test copy of the example pack.\n",
    )
    .unwrap();
    std::fs::write(f.plugins.join(PACK).join("extra.txt"), "x").unwrap();
    assert_eq!(f.store.plugins_verify(&f.plugins).unwrap(), [PACK]);
    // Putting things back is seen at the next check.
    std::fs::remove_file(f.plugins.join(PACK).join("extra.txt")).unwrap();
    assert!(f.store.plugins_verify(&f.plugins).unwrap().is_empty());
    assert_eq!(f.view().unwrap().status, PluginStatus::Enabled);
    // Interrupted work is cleared at start-up and never read as a plugin.
    for leftover in [".staging-1", ".removing-2"] {
        std::fs::create_dir(f.plugins.join(leftover)).unwrap();
        std::fs::copy(
            f.plugins.join(PACK).join(MANIFEST),
            f.plugins.join(leftover).join(MANIFEST),
        )
        .unwrap();
    }
    assert_eq!(f.store.plugins_state(&f.plugins).unwrap().plugins.len(), 1);
    f.store.plugins_verify(&f.plugins).unwrap();
    assert_eq!(f.entries(), [PACK]);
}

#[test]
fn removal_clears_files_trust_permissions_and_the_record_but_keeps_history() {
    let mut f = fixture();
    f.install(&archive("good.zip"));
    let server = f.server.clone();
    f.store
        .plugin_enable(&f.plugins, PACK, TrustLevel::ReadOnly)
        .unwrap();
    f.store
        .plugin_set_grant(&f.plugins, PACK, &server, &[Capability::SshSession])
        .unwrap();
    f.store.plugin_remove(&f.plugins, PACK).unwrap();
    assert!(f.entries().is_empty());
    assert!(f.view().is_none());
    // Installing it again starts from nothing.
    f.install(&archive("good.zip"));
    let view = f.view().unwrap();
    assert_eq!(view.status, PluginStatus::Disabled);
    assert!(view.grants.is_empty());
    let log = f.store.plugin_activity(50).unwrap().entries;
    let actions: Vec<&str> = log.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(
        actions[..5],
        [
            ACTION_INSTALL,
            ACTION_REMOVE,
            ACTION_SET_PERMISSIONS,
            ACTION_ENABLE,
            ACTION_INSTALL
        ]
    );
    // A folder placed by hand can be removed too; anything that is not a
    // plain folder in the plugins directory is refused.
    std::fs::create_dir(f.plugins.join("by-hand")).unwrap();
    f.store.plugin_remove(&f.plugins, "by-hand").unwrap();
    for bad in [
        "",
        "..",
        "../plugins",
        ".staging-x",
        "a/b",
        "C:\\x",
        "missing",
    ] {
        assert!(f.store.plugin_remove(&f.plugins, bad).is_err(), "{bad}");
    }
    assert_eq!(f.entries(), [PACK]);
}

#[test]
fn the_install_page_documents_every_code_and_limit() {
    let page = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/plugin-api/install.md"),
    )
    .unwrap();
    let codes = [
        InstallProblemCode::Unreadable,
        InstallProblemCode::NotHttps,
        InstallProblemCode::UnsupportedArchive,
        InstallProblemCode::PathTraversal,
        InstallProblemCode::UnsafeEntry,
        InstallProblemCode::TooLarge,
        InstallProblemCode::NoManifest,
        InstallProblemCode::InvalidManifest,
        InstallProblemCode::HashMismatch,
        InstallProblemCode::SignatureMissing,
        InstallProblemCode::SignatureInvalid,
        InstallProblemCode::InvalidPublicKey,
        InstallProblemCode::SignatureNotPossible,
        InstallProblemCode::AlreadyInstalled,
    ];
    for code in codes {
        let name = serde_json::to_value(code).unwrap();
        assert!(page.contains(&format!("`{}`", name.as_str().unwrap())), "{name}");
    }
    for limit in [
        format!("{} MiB", MAX_ARCHIVE_BYTES / 1024 / 1024),
        format!("{} MiB in all", MAX_UNPACKED_BYTES / 1024 / 1024),
        format!("{} MiB each", MAX_FILE_BYTES / 1024 / 1024),
        format!("{MAX_PLUGIN_FILES} files"),
        format!("{MAX_PATH_CHARS} characters"),
        format!("{MAX_PATH_DEPTH} folders deep"),
    ] {
        assert!(page.contains(&limit), "{limit}");
    }
}
