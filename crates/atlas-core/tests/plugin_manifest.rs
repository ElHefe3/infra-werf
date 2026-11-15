//! Plugin manifest fixtures (T039): valid manifests load, and each invalid
//! fixture is refused with the stable code its file is named after.
use atlas_core::extensions::{ExtensionInfo, ExtensionPoint, builtin};
use atlas_core::plugin::*;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn fixtures(kind: &str) -> Vec<(String, Vec<u8>)> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plugin-manifests")
        .join(kind);
    let mut found: Vec<(String, Vec<u8>)> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_stem().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&path).unwrap(),
            )
        })
        .collect();
    found.sort();
    found
}

fn codes(bytes: &[u8]) -> Vec<String> {
    validate_manifest(bytes)
        .unwrap_err()
        .iter()
        .map(|issue| issue.code.code())
        .collect()
}

#[test]
fn valid_manifests_load_and_register_like_built_in_contributions() {
    let valid = fixtures("valid");
    assert_eq!(valid.len(), 2);
    for (name, bytes) in valid {
        let manifest =
            validate_manifest(&bytes).unwrap_or_else(|issues| panic!("{name}: {issues:?}"));
        assert_eq!(manifest.api_version, PLUGIN_API_VERSION);
        // The same registry that holds built-in contributions accepts them.
        let mut registry = builtin();
        registry
            .declare(ExtensionInfo {
                id: manifest.id.clone(),
                title: manifest.name.clone(),
                enabled: true,
                optional: true,
            })
            .unwrap();
        for contribution in manifest.registry_contributions() {
            assert!(contribution.id.starts_with(&format!("{}.", manifest.id)));
            assert_eq!(contribution.owner, manifest.id);
            registry.register(contribution).unwrap();
        }
        // A manifest round-trips unchanged.
        let again = serde_json::to_vec(&manifest).unwrap();
        assert_eq!(validate_manifest(&again).unwrap(), manifest);
    }
    let full = validate_manifest(&fixtures("valid")[0].1).unwrap();
    let entries = full.registry_contributions();
    assert_eq!(entries[0].id, "org.example.backup-overview.backups-tab");
    assert_eq!(entries[0].order, 200);
    assert_eq!(entries[1].order, DEFAULT_PLUGIN_ORDER);
    assert_eq!(entries[1].point, ExtensionPoint::ServerBadge);
}

#[test]
fn each_invalid_fixture_is_refused_with_exactly_its_own_code() {
    let mut covered = BTreeSet::new();
    for (name, bytes) in fixtures("invalid") {
        let issues = validate_manifest(&bytes).unwrap_err();
        assert_eq!(codes(&bytes), [name.clone()], "{name}: {issues:?}");
        assert!(!issues[0].message.is_empty());
        covered.insert(name);
    }
    // Two rules need generated input rather than a fixture file.
    let large = vec![b' '; MAX_MANIFEST_BYTES + 1];
    assert_eq!(codes(&large), ["too_large"]);
    covered.insert("too_large".into());
    let mut many: serde_json::Value = serde_json::from_slice(&fixtures("valid")[1].1).unwrap();
    many["contributions"] = (0..=MAX_PLUGIN_CONTRIBUTIONS)
        .map(|n| serde_json::json!({ "id": format!("item-{n}"), "point": "menu_item", "title": "Item", "capabilities": ["read_saved_state"] }))
        .collect();
    assert_eq!(
        codes(&serde_json::to_vec(&many).unwrap()),
        ["too_many_contributions"]
    );
    covered.insert("too_many_contributions".into());
    // Every rule has a proof, and every code is distinct.
    let all: BTreeSet<String> = ManifestRule::ALL.iter().map(|rule| rule.code()).collect();
    assert_eq!(all.len(), ManifestRule::ALL.len());
    assert_eq!(covered, all);
}

#[test]
fn issues_name_the_field_and_all_problems_are_reported_together() {
    let manifest = br#"{"id":"org.example.demo","name":"Demo","version":"one","api_version":1,
      "capabilities":["read_saved_state"],
      "contributions":[{"id":"a","point":"menu_item","title":" A","capabilities":["read_saved_state","edit_map"]}]}"#;
    let issues = validate_manifest(manifest).unwrap_err();
    let found: Vec<(String, &str)> = issues
        .iter()
        .map(|issue| (issue.code.code(), issue.field.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            ("invalid_version".to_string(), "version"),
            ("invalid_title".to_string(), "contributions[0].title"),
            (
                "capability_not_requested".to_string(),
                "contributions[0].capabilities[1]"
            ),
            (
                "capability_not_permitted".to_string(),
                "contributions[0].capabilities[1]"
            ),
        ]
    );
    // Structural problems are reported with nested paths too.
    let nested = br#"{"id":"org.example.demo","name":"Demo","version":"1.0.0","api_version":1,"capabilities":[4],
      "contributions":[{"id":"a","point":"menu_item","title":"A","capabilities":["read_saved_state"],"script":"rm -rf /"}]}"#;
    let found: Vec<(String, String)> = validate_manifest(nested)
        .unwrap_err()
        .into_iter()
        .map(|issue| (issue.code.code(), issue.field))
        .collect();
    assert_eq!(
        found,
        [
            ("wrong_type".to_string(), "capabilities[0]".to_string()),
            (
                "unknown_field".to_string(),
                "contributions[0].script".to_string()
            ),
        ]
    );
    assert_eq!(codes(b"[]"), ["wrong_type"]);
    // An optional field may be null; a required one may not.
    let optional = String::from_utf8(fixtures("valid")[1].1.clone()).unwrap();
    validate_manifest(optional.replacen('{', "{\"author\": null,", 1).as_bytes()).unwrap();
    assert_eq!(
        codes(
            optional
                .replace("\"name\": \"Note\"", "\"name\": null")
                .as_bytes()
        ),
        ["wrong_type"]
    );
    assert_eq!(codes(b"\xff\xfe"), ["malformed_json"]);
}

#[test]
fn a_newer_api_version_is_refused_with_a_clear_message() {
    let text = String::from_utf8(fixtures("valid")[1].1.clone())
        .unwrap()
        .replace(
            "\"api_version\": 1",
            &format!("\"api_version\": {}", PLUGIN_API_VERSION + 1),
        );
    let issues = validate_manifest(text.as_bytes()).unwrap_err();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, ManifestRule::NewerApiVersion);
    assert_eq!(issues[0].field, "api_version");
    assert_eq!(
        issues[0].message,
        format!(
            "This plugin needs plugin API v{}. This InfraWerf supports up to v{}. Update InfraWerf to use it.",
            PLUGIN_API_VERSION + 1,
            PLUGIN_API_VERSION
        )
    );
    for bad in ["0", "-1", "1.5", "4294967296"] {
        let text = text.replace(
            &format!("\"api_version\": {}", PLUGIN_API_VERSION + 1),
            &format!("\"api_version\": {bad}"),
        );
        assert_eq!(codes(text.as_bytes()), ["invalid_api_version"], "{bad}");
    }
}

#[test]
fn versions_and_identifiers_follow_their_rules() {
    for good in [
        "0.0.0",
        "1.2.3",
        "10.20.30",
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-0.3.7",
        "1.0.0+20130313144700",
        "1.0.0-beta+exp.sha.5114f85",
    ] {
        assert!(semantic_version(good), "{good}");
    }
    for bad in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "01.2.3",
        "1.02.3",
        "v1.2.3",
        "1.2.3-",
        "1.2.3-01",
        "1.2.3+",
        "1.2.3-a..b",
        "1.2.x",
    ] {
        assert!(!semantic_version(bad), "{bad}");
    }
    let with_id = |id: &str| {
        String::from_utf8(fixtures("valid")[1].1.clone())
            .unwrap()
            .replace("org.example.note", id)
    };
    for good in ["org.example.note", "io.github.someone.my-plugin2"] {
        validate_manifest(with_id(good).as_bytes()).unwrap();
    }
    for bad in [
        "note",
        "example.note",
        "org.example.Note",
        "org.example.2note",
        "org.example.note-",
        "org..note",
        "org.example.note!",
    ] {
        assert_eq!(codes(with_id(bad).as_bytes()), ["invalid_id"], "{bad}");
    }
    assert_eq!(
        codes(with_id("infrawerf.example.note").as_bytes()),
        ["reserved_id"]
    );
    assert_eq!(
        codes(with_id(&format!("org.example.{}", "n".repeat(60))).as_bytes()),
        ["invalid_id"]
    );
}

#[test]
fn the_published_schema_is_generated_from_the_validated_types() {
    let generated = manifest_schema();
    let published = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/plugin-api/manifest.schema.json"),
    )
    .unwrap();
    assert_eq!(published.replace("\r\n", "\n"), generated);

    // The validator's field tables match the schema generated from the types.
    let schema: serde_json::Value = serde_json::from_str(&generated).unwrap();
    let fields = |object: &serde_json::Value| -> BTreeSet<(String, bool)> {
        let required: Vec<&str> = object["required"]
            .as_array()
            .map(|list| list.iter().filter_map(|name| name.as_str()).collect())
            .unwrap_or_default();
        object["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(|name| (name.clone(), required.contains(&name.as_str())))
            .collect()
    };
    let declared = declared_fields().map(|list| {
        list.into_iter()
            .map(|(name, required)| (name.to_string(), required))
            .collect::<BTreeSet<_>>()
    });
    assert_eq!(fields(&schema), declared[0]);
    assert_eq!(
        fields(&schema["$defs"]["ManifestContribution"]),
        declared[1]
    );
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["$defs"]["ManifestContribution"]["additionalProperties"],
        false
    );
}
