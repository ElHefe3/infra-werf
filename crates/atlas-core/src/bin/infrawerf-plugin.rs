//! Author tool for plugin manifests (S12/F19/T039).
//!
//!   infrawerf-plugin check <manifest or plugin folder> [--json]
//!   infrawerf-plugin schema [--write | --check]
//!
//! `check` only reads the manifest. It runs nothing the plugin ships.
use atlas_core::plugin::{
    MANIFEST_FILE, MAX_MANIFEST_BYTES, PLUGIN_API_VERSION, manifest_schema, validate_manifest,
};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "Usage:\n  infrawerf-plugin check <manifest or plugin folder> [--json]\n  infrawerf-plugin schema [--write | --check]";

fn schema_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plugin-api/manifest.schema.json")
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    let file = if path.is_dir() {
        path.join(MANIFEST_FILE)
    } else {
        path.to_path_buf()
    };
    let unreadable = |error: std::io::Error| format!("Could not read {}: {error}", file.display());
    // Read one byte past the limit so an oversized file is reported as such
    // without loading it.
    let mut bytes = Vec::new();
    std::fs::File::open(&file)
        .map_err(unreadable)?
        .take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    Ok(bytes)
}

fn check(path: &Path, json: bool) -> ExitCode {
    let bytes = match read(path) {
        Ok(bytes) => bytes,
        Err(message) => {
            // Not a verdict on the manifest: there was nothing to check.
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match validate_manifest(&bytes) {
        Ok(manifest) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "valid": true, "api_version": manifest.api_version, "id": manifest.id, "version": manifest.version, "issues": [] })
                );
            } else {
                let count = |n: usize, one: &str, many: &str| {
                    format!("{n} {}", if n == 1 { one } else { many })
                };
                println!(
                    "{} {} is valid for plugin API v{} ({}, {} requested).",
                    manifest.id,
                    manifest.version,
                    manifest.api_version,
                    count(
                        manifest.contributions.len(),
                        "contribution",
                        "contributions"
                    ),
                    count(manifest.capabilities.len(), "capability", "capabilities")
                );
            }
            ExitCode::SUCCESS
        }
        Err(issues) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "valid": false, "issues": issues })
                );
            } else {
                eprintln!(
                    "Not a valid manifest for plugin API v{PLUGIN_API_VERSION}: {} problem{}.",
                    issues.len(),
                    if issues.len() == 1 { "" } else { "s" }
                );
                for issue in &issues {
                    eprintln!("  {issue}");
                }
            }
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    match args.first().map(String::as_str) {
        Some("check") => match args.iter().skip(1).find(|arg| !arg.starts_with("--")) {
            Some(path) => check(Path::new(path), flag("--json")),
            None => {
                eprintln!("{USAGE}");
                ExitCode::from(2)
            }
        },
        Some("schema") => {
            let generated = manifest_schema();
            let destination = schema_path();
            if flag("--write") {
                if let Err(error) = std::fs::write(&destination, &generated) {
                    eprintln!("Could not write {}: {error}", destination.display());
                    return ExitCode::from(1);
                }
                println!("Generated {}", destination.display());
            } else if flag("--check") {
                let current = std::fs::read_to_string(&destination).unwrap_or_default();
                if current.replace("\r\n", "\n") != generated {
                    eprintln!(
                        "The published plugin manifest schema is stale; run mise run exec -- cargo run -p atlas-core --bin infrawerf-plugin -- schema --write"
                    );
                    return ExitCode::from(1);
                }
                println!("Published plugin manifest schema matches the Rust definitions");
            } else {
                print!("{generated}");
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
