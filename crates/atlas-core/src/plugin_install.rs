//! Installing and verifying plugins (S12/F22/T045).
//!
//! A plugin comes from a folder, a zip or `.tar.gz` archive, or an HTTPS
//! address the owner chose. Everything is read into memory within fixed
//! limits, every path is checked, the manifest is validated, and only then
//! are the files written to a staging folder inside the plugins directory
//! and published with one rename. Nothing a plugin ships is run, and a
//! refusal leaves nothing behind.
//!
//! Each installed plugin records a SHA-256 hash of its files. The hash is
//! shown to the owner and checked again at every start-up; a plugin whose
//! files changed is refused until it is removed and installed again.
//!
//! An archive may come with a minisign signature (`<archive>.minisig`). It
//! is checked only when the owner supplies the author's public key: then a
//! missing or wrong signature refuses the install. No key is trusted by
//! default (owner decision D058).
use crate::plugin::{MANIFEST_FILE, PluginManifest, validate_manifest};
use crate::{AtlasError, CONTRACT_VERSION, ErrorCode};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use ts_rs::{Config, TS};

/// Largest archive or download accepted.
pub const MAX_ARCHIVE_BYTES: u64 = 5 * 1024 * 1024;
/// Largest total size of a plugin's files once unpacked.
pub const MAX_UNPACKED_BYTES: u64 = 20 * 1024 * 1024;
/// Largest single file in a plugin.
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// Most files in a plugin, folders not counted.
pub const MAX_PLUGIN_FILES: usize = 256;
/// Deepest folder nesting inside a plugin.
pub const MAX_PATH_DEPTH: usize = 8;
/// Longest path inside a plugin, in characters.
pub const MAX_PATH_CHARS: usize = 200;
/// A signature file is small; anything larger is not one.
pub const MAX_SIGNATURE_BYTES: u64 = 4096;
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
pub const SIGNATURE_SUFFIX: &str = ".minisig";
/// Adds one PEM certificate as the only trusted root for downloads. For the
/// native test suite's local HTTPS server only.
pub const TEST_CA_ENV: &str = "ATLAS_PLUGIN_TEST_CA";
/// Prefixes of InfraWerf's own working folders inside the plugins directory.
pub const STAGING_PREFIX: &str = ".staging-";
pub const REMOVING_PREFIX: &str = ".removing-";

/// Where a plugin is installed from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstallSource {
    /// A folder on this PC holding `infrawerf-plugin.json`.
    Folder { path: String },
    /// A `.zip` or `.tar.gz` file on this PC.
    Archive { path: String },
    /// An `https://` address of a `.zip` or `.tar.gz` file.
    Address { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InstallRequest {
    pub source: InstallSource,
    /// The SHA-256 the owner expects: of the archive for an archive or an
    /// address, of the plugin's files for a folder. 64 hexadecimal digits.
    #[serde(default)]
    #[ts(optional)]
    pub expected_sha256: Option<String>,
    /// The author's minisign public key. When given, the archive must carry
    /// a valid signature by that key.
    #[serde(default)]
    #[ts(optional)]
    pub public_key: Option<String>,
}

/// Why an install was refused. Stable codes, shown with a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum InstallProblemCode {
    /// The source could not be read, or the address could not be fetched.
    Unreadable,
    /// The address is not a plain `https://` address.
    NotHttps,
    /// Neither a zip nor a gzip-compressed tar archive.
    UnsupportedArchive,
    /// A path that leaves the plugin's folder, is absolute, or is not a
    /// plain relative name.
    PathTraversal,
    /// A link, device or other entry that is not a plain file or folder.
    UnsafeEntry,
    /// Too large, too many files or too deep.
    TooLarge,
    /// No `infrawerf-plugin.json`, or more than one candidate folder.
    NoManifest,
    /// The manifest is not valid.
    InvalidManifest,
    /// The SHA-256 is not the one the owner expected.
    HashMismatch,
    /// A public key was given but there is no signature.
    SignatureMissing,
    /// The signature is not valid for this archive and key.
    SignatureInvalid,
    /// The public key is not a minisign public key.
    InvalidPublicKey,
    /// A signature check was asked for a folder.
    SignatureNotPossible,
    /// A plugin with this ID, or a folder with its name, already exists.
    AlreadyInstalled,
    /// An update whose manifest has another plugin's ID.
    DifferentPlugin,
    /// The update is an older version and the owner did not confirm it.
    Downgrade,
    /// The installed plugin changed between checking and applying.
    ChangedSincePreview,
    /// The update was checked too long ago, or was already applied.
    PreviewExpired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InstallProblem {
    pub code: InstallProblemCode,
    pub message: String,
}

impl InstallProblem {
    pub fn new(code: InstallProblemCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// The result of an install, for the owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InstallReport {
    pub contract_version: u32,
    /// Set when the plugin was installed (it is disabled until reviewed), or
    /// with `already_installed`, the plugin that is already there.
    pub plugin_id: Option<String>,
    pub plugin_name: Option<String>,
    pub content_sha256: Option<String>,
    pub problem: Option<InstallProblem>,
}

impl InstallReport {
    pub fn refused(problem: InstallProblem) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            plugin_id: None,
            plugin_name: None,
            content_sha256: None,
            problem: Some(problem),
        }
    }
}

/// What InfraWerf found when it last checked an installed plugin's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Integrity {
    /// The files match the hash recorded at install.
    Verified,
    /// The files differ from the hash recorded at install.
    Changed,
    /// Not checked since it was installed in this run.
    NotChecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Folder,
    Archive,
    Address,
}

/// How a plugin was installed, as shown on its page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InstallRecordView {
    pub source_kind: SourceKind,
    /// The folder, file or address it came from.
    pub source: String,
    /// SHA-256 of its files, checked at every start-up.
    pub content_sha256: String,
    /// SHA-256 of the archive or download, if it came from one.
    pub archive_sha256: Option<String>,
    /// The minisign key ID whose signature was verified, if any.
    pub signer_key_id: Option<String>,
    pub installed_at: String,
    pub integrity: Integrity,
    pub checked_at: Option<String>,
}

/// A plugin read and checked, ready to publish.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub manifest: PluginManifest,
    pub files: Vec<(String, Vec<u8>)>,
    pub content_sha256: String,
    pub archive_sha256: Option<String>,
    pub signer_key_id: Option<String>,
    pub source_kind: SourceKind,
    pub source: String,
}

fn problem(code: InstallProblemCode, message: impl Into<String>) -> InstallProblem {
    InstallProblem::new(code, message)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The hash of a plugin's files: each file's path, size and SHA-256 in path
/// order. Folders are not part of it, so an empty folder changes nothing.
pub fn content_hash(files: &[(String, Vec<u8>)]) -> String {
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (path, bytes) in sorted {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(bytes.len().to_string().as_bytes());
        hasher.update([0]);
        hasher.update(sha256_hex(bytes).as_bytes());
        hasher.update(b"\n");
    }
    hex(&hasher.finalize())
}

const RESERVED_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A path inside a plugin, as `a/b/c`: relative, made of plain names that
/// are safe on Windows, never `.` or `..`. Either separator is accepted in
/// the input; the result always uses `/`.
pub fn clean_path(raw: &str) -> Result<Vec<String>, InstallProblem> {
    let refuse = |why: &str| {
        problem(
            InstallProblemCode::PathTraversal,
            format!(
                "The archive holds \"{}\": {why}",
                raw.chars().take(120).collect::<String>()
            ),
        )
    };
    if raw.is_empty() || raw.starts_with('/') || raw.starts_with('\\') {
        return Err(refuse("a path must be relative to the plugin's folder."));
    }
    let trimmed = raw.trim_end_matches(['/', '\\']);
    let parts: Vec<&str> = trimmed.split(['/', '\\']).collect();
    let mut clean = Vec::new();
    for part in parts {
        if part == "." || part == ".." {
            return Err(refuse("a path may not climb out of the plugin's folder."));
        }
        let lower = part.to_ascii_lowercase();
        let stem = lower.split('.').next().unwrap_or_default();
        if part.is_empty()
            || part.ends_with('.')
            || part.ends_with(' ')
            || RESERVED_NAMES.contains(&stem)
            || part
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c))
        {
            return Err(refuse(
                "a name may not be empty, a drive, a device name, or hold < > : \" | ? * or control characters.",
            ));
        }
        clean.push(part.to_owned());
    }
    if clean.len() > MAX_PATH_DEPTH || trimmed.chars().count() > MAX_PATH_CHARS {
        return Err(problem(
            InstallProblemCode::TooLarge,
            format!(
                "A path is at most {MAX_PATH_CHARS} characters and {MAX_PATH_DEPTH} folders deep."
            ),
        ));
    }
    Ok(clean)
}

/// Collects files within the limits, refusing duplicates (ignoring case,
/// as Windows does).
struct Collector {
    files: BTreeMap<String, (Vec<String>, Vec<u8>)>,
    total: u64,
}

impl Collector {
    fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            total: 0,
        }
    }

    fn size_ok(&self, size: u64) -> Result<(), InstallProblem> {
        if size > MAX_FILE_BYTES || self.total + size > MAX_UNPACKED_BYTES {
            return Err(problem(
                InstallProblemCode::TooLarge,
                format!(
                    "A plugin's files are at most {} MiB in all and {} MiB each.",
                    MAX_UNPACKED_BYTES / 1024 / 1024,
                    MAX_FILE_BYTES / 1024 / 1024
                ),
            ));
        }
        Ok(())
    }

    /// Reads one file, never more than the limits allow, whatever its
    /// header claims.
    fn add(
        &mut self,
        path: Vec<String>,
        declared: u64,
        reader: &mut dyn Read,
    ) -> Result<(), InstallProblem> {
        if self.files.len() >= MAX_PLUGIN_FILES {
            return Err(problem(
                InstallProblemCode::TooLarge,
                format!("A plugin holds at most {MAX_PLUGIN_FILES} files."),
            ));
        }
        self.size_ok(declared)?;
        let room = (MAX_FILE_BYTES).min(MAX_UNPACKED_BYTES - self.total);
        let mut bytes = Vec::new();
        reader.take(room + 1).read_to_end(&mut bytes).map_err(|_| {
            problem(
                InstallProblemCode::Unreadable,
                "A file in the plugin could not be read.",
            )
        })?;
        self.size_ok(bytes.len() as u64)?;
        self.total += bytes.len() as u64;
        let key = path.join("/").to_lowercase();
        if self.files.insert(key, (path, bytes)).is_some() {
            return Err(problem(
                InstallProblemCode::PathTraversal,
                "The archive holds two files with the same name (ignoring upper and lower case).",
            ));
        }
        Ok(())
    }

    /// The plugin's files, relative to the folder holding the manifest. A
    /// manifest at the top wins; otherwise exactly one top-level folder may
    /// hold it, as archives made from a folder do.
    fn finish(self) -> Result<Vec<(String, Vec<u8>)>, InstallProblem> {
        let at_top = self
            .files
            .values()
            .any(|(path, _)| path.len() == 1 && path[0] == MANIFEST_FILE);
        let prefix: Option<String> = if at_top {
            None
        } else {
            let mut tops: Vec<&String> = self.files.values().map(|(path, _)| &path[0]).collect();
            tops.dedup();
            let first = tops.first().copied().cloned();
            let single = tops.iter().all(|top| Some(*top) == first.as_ref());
            match first {
                Some(top)
                    if single
                        && self
                            .files
                            .values()
                            .any(|(path, _)| path.len() == 2 && path[1] == MANIFEST_FILE) =>
                {
                    Some(top)
                }
                _ => {
                    return Err(problem(
                        InstallProblemCode::NoManifest,
                        format!(
                            "No {MANIFEST_FILE} at the top of the plugin, or in one single folder at the top."
                        ),
                    ));
                }
            }
        };
        Ok(self
            .files
            .into_values()
            .map(|(path, bytes)| {
                let skip = usize::from(prefix.is_some());
                (path[skip..].join("/"), bytes)
            })
            .collect())
    }
}

fn unpack_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, InstallProblem> {
    let unreadable = || {
        problem(
            InstallProblemCode::UnsupportedArchive,
            "The zip file could not be read.",
        )
    };
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| unreadable())?;
    if archive.len() > MAX_PLUGIN_FILES * 2 {
        return Err(problem(
            InstallProblemCode::TooLarge,
            format!("A plugin holds at most {MAX_PLUGIN_FILES} files."),
        ));
    }
    let mut collector = Collector::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| unreadable())?;
        let name = entry.name().map_err(|_| unreadable())?.into_owned();
        let path = clean_path(&name)?;
        if entry.is_symlink() {
            return Err(problem(
                InstallProblemCode::UnsafeEntry,
                format!("\"{name}\" is a link. A plugin holds plain files and folders only."),
            ));
        }
        if entry.is_dir() {
            continue;
        }
        if !entry.is_file() {
            return Err(problem(
                InstallProblemCode::UnsafeEntry,
                format!("\"{name}\" is not a plain file."),
            ));
        }
        let declared = entry.size();
        collector.add(path, declared, &mut entry)?;
    }
    collector.finish()
}

fn unpack_tar_gz(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, InstallProblem> {
    let unreadable = || {
        problem(
            InstallProblemCode::UnsupportedArchive,
            "The .tar.gz file could not be read.",
        )
    };
    // The decompressed stream is bounded too, so a small archive cannot
    // expand without limit while headers are read.
    let limit = MAX_UNPACKED_BYTES + (MAX_PLUGIN_FILES as u64 + 64) * 4096;
    let decoder = flate2::read::GzDecoder::new(bytes).take(limit);
    let mut archive = tar::Archive::new(decoder);
    let mut collector = Collector::new();
    let mut entries = 0usize;
    for entry in archive.entries().map_err(|_| unreadable())? {
        let mut entry = entry.map_err(|_| unreadable())?;
        entries += 1;
        if entries > MAX_PLUGIN_FILES * 2 {
            return Err(problem(
                InstallProblemCode::TooLarge,
                format!("A plugin holds at most {MAX_PLUGIN_FILES} files."),
            ));
        }
        let name = String::from_utf8(entry.path_bytes().into_owned()).map_err(|_| {
            problem(
                InstallProblemCode::PathTraversal,
                "A path in the archive is not text.",
            )
        })?;
        let kind = entry.header().entry_type();
        let path = clean_path(&name)?;
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            return Err(problem(
                InstallProblemCode::UnsafeEntry,
                format!(
                    "\"{name}\" is a link, device or other special entry. A plugin holds plain files and folders only."
                ),
            ));
        }
        let declared = entry.size();
        collector.add(path, declared, &mut entry)?;
    }
    collector.finish()
}

/// Reads an archive by its content, not its name.
pub fn unpack(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, InstallProblem> {
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err(problem(
            InstallProblemCode::TooLarge,
            format!(
                "An archive is at most {} MiB.",
                MAX_ARCHIVE_BYTES / 1024 / 1024
            ),
        ));
    }
    match bytes {
        [b'P', b'K', 3, 4, ..] | [b'P', b'K', 5, 6, ..] => unpack_zip(bytes),
        [0x1f, 0x8b, ..] => unpack_tar_gz(bytes),
        _ => Err(problem(
            InstallProblemCode::UnsupportedArchive,
            "This is not a .zip or .tar.gz file.",
        )),
    }
}

/// Reads a folder's files with the same limits as an archive. Links and
/// other special entries are refused rather than followed.
pub fn read_folder(root: &Path) -> Result<Vec<(String, Vec<u8>)>, InstallProblem> {
    let mut collector = Collector::new();
    let mut pending: Vec<(PathBuf, Vec<String>)> = vec![(root.to_path_buf(), Vec::new())];
    let unreadable = |what: &Path| {
        problem(
            InstallProblemCode::Unreadable,
            format!("{} could not be read.", what.display()),
        )
    };
    let root_meta = std::fs::symlink_metadata(root).map_err(|_| unreadable(root))?;
    if !root_meta.is_dir() {
        return Err(unreadable(root));
    }
    let mut folders = 0usize;
    while let Some((folder, prefix)) = pending.pop() {
        folders += 1;
        if folders > MAX_PLUGIN_FILES {
            return Err(problem(
                InstallProblemCode::TooLarge,
                "The folder holds too many folders.",
            ));
        }
        let mut entries: Vec<_> = std::fs::read_dir(&folder)
            .map_err(|_| unreadable(&folder))?
            .filter_map(Result::ok)
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let mut path = prefix.clone();
            path.push(name.clone());
            let joined = path.join("/");
            let clean = clean_path(&joined)?;
            let meta =
                std::fs::symlink_metadata(entry.path()).map_err(|_| unreadable(&entry.path()))?;
            if is_link(&meta) {
                return Err(problem(
                    InstallProblemCode::UnsafeEntry,
                    format!("\"{joined}\" is a link. A plugin holds plain files and folders only."),
                ));
            }
            if meta.is_dir() {
                pending.push((entry.path(), path));
            } else if meta.is_file() {
                let mut file =
                    std::fs::File::open(entry.path()).map_err(|_| unreadable(&entry.path()))?;
                collector.add(clean, meta.len(), &mut file)?;
            } else {
                return Err(problem(
                    InstallProblemCode::UnsafeEntry,
                    format!("\"{joined}\" is not a plain file."),
                ));
            }
        }
    }
    collector.finish()
}

#[cfg(windows)]
fn is_link(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    // Symbolic links, junctions and any other reparse point.
    meta.file_type().is_symlink() || meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_link(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

/// The hash of an installed plugin's folder, for the start-up check.
pub fn folder_hash(folder: &Path) -> Option<String> {
    read_folder_exact(folder)
        .ok()
        .map(|files| content_hash(&files))
}

/// Reads an installed folder as it is, without moving the manifest.
fn read_folder_exact(root: &Path) -> Result<Vec<(String, Vec<u8>)>, InstallProblem> {
    // An installed plugin always has its manifest at the top, so the
    // normal reader returns its files unchanged.
    read_folder(root)
}

/// A minisign public key: the key itself and its ID as minisign prints it.
pub fn public_key(text: &str) -> Result<(minisign_verify::PublicKey, String), InstallProblem> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("untrusted comment:"))
        .unwrap_or_default();
    let invalid = || {
        problem(
            InstallProblemCode::InvalidPublicKey,
            "That is not a minisign public key. It is one line that starts with RW.",
        )
    };
    let key = minisign_verify::PublicKey::from_base64(line).map_err(|_| invalid())?;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(line)
        .map_err(|_| invalid())?;
    let id: Vec<u8> = raw
        .get(2..10)
        .ok_or_else(invalid)?
        .iter()
        .rev()
        .copied()
        .collect();
    Ok((key, hex(&id).to_uppercase()))
}

/// Checks an archive's signature with the owner's key. Returns the key ID.
pub fn verify_signature(
    archive: &[u8],
    signature: Option<&[u8]>,
    key: &str,
) -> Result<String, InstallProblem> {
    let (key, id) = public_key(key)?;
    let signature = signature.ok_or_else(|| {
        problem(
            InstallProblemCode::SignatureMissing,
            format!("You gave a public key, but there is no {SIGNATURE_SUFFIX} signature next to the archive."),
        )
    })?;
    let invalid = || {
        problem(
            InstallProblemCode::SignatureInvalid,
            "The signature does not match this archive and public key. The archive may have been changed, or it was signed with another key.",
        )
    };
    let text = std::str::from_utf8(signature).map_err(|_| invalid())?;
    let decoded = minisign_verify::Signature::decode(text).map_err(|_| invalid())?;
    key.verify(archive, &decoded, false)
        .map_err(|_| invalid())?;
    Ok(id)
}

fn expected(value: Option<&str>) -> Result<Option<String>, InstallProblem> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) if v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Ok(Some(v.to_ascii_lowercase()))
        }
        Some(_) => Err(problem(
            InstallProblemCode::HashMismatch,
            "An expected SHA-256 is 64 hexadecimal digits.",
        )),
    }
}

fn check_hash(actual: &str, wanted: &Option<String>, what: &str) -> Result<(), InstallProblem> {
    match wanted {
        Some(wanted) if wanted != actual => Err(problem(
            InstallProblemCode::HashMismatch,
            format!("The {what} has SHA-256 {actual}, not the {wanted} you expected."),
        )),
        _ => Ok(()),
    }
}

/// An address InfraWerf may fetch: `https://`, a host, no user name or
/// password, at most 2000 characters.
pub fn https_address(url: &str) -> Result<(), InstallProblem> {
    let rest = url.strip_prefix("https://").unwrap_or_default();
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if url.len() > 2000
        || host.is_empty()
        || host.contains('@')
        || url.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(problem(
            InstallProblemCode::NotHttps,
            "Use a plain https:// address, with no user name or password.",
        ));
    }
    Ok(())
}

fn agent() -> Result<ureq::Agent, InstallProblem> {
    use ureq::tls::{Certificate, RootCerts, TlsConfig};
    let mut tls = TlsConfig::builder();
    if let Some(path) = std::env::var_os(TEST_CA_ENV) {
        let pem = std::fs::read(path).map_err(|_| {
            problem(
                InstallProblemCode::Unreadable,
                "The test certificate could not be read.",
            )
        })?;
        let certificates: Vec<Certificate<'static>> = ureq::tls::parse_pem(&pem)
            .filter_map(Result::ok)
            .filter_map(|item| match item {
                ureq::tls::PemItem::Certificate(certificate) => Some(certificate),
                _ => None,
            })
            .collect();
        tls = tls.root_certs(RootCerts::new_with_certs(&certificates));
    }
    let config = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(5)
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        .user_agent("InfraWerf plugin installer")
        .tls_config(tls.build())
        .build();
    Ok(ureq::Agent::new_with_config(config))
}

/// Fetches one address, at most `limit` bytes. `Ok(None)` when the server
/// says there is nothing there (404 or 410).
fn fetch(agent: &ureq::Agent, url: &str, limit: u64) -> Result<Option<Vec<u8>>, InstallProblem> {
    let failed = |why: String| {
        problem(
            InstallProblemCode::Unreadable,
            format!("{url} could not be fetched: {why}"),
        )
    };
    let response = match agent.get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404 | 410)) => return Ok(None),
        Err(ureq::Error::StatusCode(code)) => {
            return Err(failed(format!("the server answered {code}.")));
        }
        Err(error) => return Err(failed(short(&error.to_string()))),
    };
    let mut body = response.into_body();
    match body.with_config().limit(limit).read_to_vec() {
        Ok(bytes) => Ok(Some(bytes)),
        Err(ureq::Error::BodyExceedsLimit(_)) => Err(problem(
            InstallProblemCode::TooLarge,
            format!(
                "An archive is at most {} MiB.",
                MAX_ARCHIVE_BYTES / 1024 / 1024
            ),
        )),
        Err(error) => Err(failed(short(&error.to_string()))),
    }
}

fn short(text: &str) -> String {
    text.chars().take(200).collect()
}

fn read_limited(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, InstallProblem> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(problem(
                InstallProblemCode::Unreadable,
                format!("{} could not be read.", path.display()),
            ));
        }
    };
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).map_err(|_| {
        problem(
            InstallProblemCode::Unreadable,
            format!("{} could not be read.", path.display()),
        )
    })?;
    if bytes.len() as u64 > limit {
        return Err(problem(
            InstallProblemCode::TooLarge,
            format!(
                "An archive is at most {} MiB.",
                MAX_ARCHIVE_BYTES / 1024 / 1024
            ),
        ));
    }
    Ok(Some(bytes))
}

/// Reads and checks a plugin from its source. Nothing is written.
pub fn prepare(request: &InstallRequest) -> Result<Prepared, InstallProblem> {
    let wanted = expected(request.expected_sha256.as_deref())?;
    let key = request
        .public_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty());
    let (files, archive_sha256, signer_key_id, source_kind, source) = match &request.source {
        InstallSource::Folder { path } => {
            if key.is_some() {
                return Err(problem(
                    InstallProblemCode::SignatureNotPossible,
                    "A signature belongs to an archive. Install the signed archive instead of a folder.",
                ));
            }
            let files = read_folder(Path::new(path))?;
            check_hash(&content_hash(&files), &wanted, "plugin's files")?;
            (files, None, None, SourceKind::Folder, path.clone())
        }
        InstallSource::Archive { path } => {
            let bytes = read_limited(Path::new(path), MAX_ARCHIVE_BYTES)?.ok_or_else(|| {
                problem(
                    InstallProblemCode::Unreadable,
                    format!("{path} does not exist."),
                )
            })?;
            let archive_hash = sha256_hex(&bytes);
            check_hash(&archive_hash, &wanted, "archive")?;
            let signer = match key {
                Some(key) => {
                    let signature = read_limited(
                        Path::new(&format!("{path}{SIGNATURE_SUFFIX}")),
                        MAX_SIGNATURE_BYTES,
                    )?;
                    Some(verify_signature(&bytes, signature.as_deref(), key)?)
                }
                None => None,
            };
            (
                unpack(&bytes)?,
                Some(archive_hash),
                signer,
                SourceKind::Archive,
                path.clone(),
            )
        }
        InstallSource::Address { url } => {
            https_address(url)?;
            if let Some(key) = key {
                public_key(key)?;
            }
            let agent = agent()?;
            let bytes = fetch(&agent, url, MAX_ARCHIVE_BYTES)?.ok_or_else(|| {
                problem(
                    InstallProblemCode::Unreadable,
                    format!("{url} could not be fetched: the server has nothing there."),
                )
            })?;
            let archive_hash = sha256_hex(&bytes);
            check_hash(&archive_hash, &wanted, "download")?;
            let signer = match key {
                Some(key) => {
                    let signature = fetch(
                        &agent,
                        &format!("{url}{SIGNATURE_SUFFIX}"),
                        MAX_SIGNATURE_BYTES,
                    )?;
                    Some(verify_signature(&bytes, signature.as_deref(), key)?)
                }
                None => None,
            };
            (
                unpack(&bytes)?,
                Some(archive_hash),
                signer,
                SourceKind::Address,
                url.clone(),
            )
        }
    };
    let manifest_bytes = files
        .iter()
        .find(|(path, _)| path == MANIFEST_FILE)
        .map(|(_, bytes)| bytes.clone())
        .ok_or_else(|| {
            problem(
                InstallProblemCode::NoManifest,
                format!("No {MANIFEST_FILE} was found."),
            )
        })?;
    let manifest = validate_manifest(&manifest_bytes).map_err(|issues| {
        problem(
            InstallProblemCode::InvalidManifest,
            issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })?;
    Ok(Prepared {
        manifest,
        content_sha256: content_hash(&files),
        files,
        archive_sha256,
        signer_key_id,
        source_kind,
        source,
    })
}

/// Writes a prepared plugin to a staging folder in `directory` and moves it
/// into place as `<directory>/<plugin id>` with one rename. On any failure
/// the staging folder is removed and nothing is installed.
pub fn publish(directory: &Path, prepared: &Prepared) -> Result<PathBuf, AtlasError> {
    let io = |_| AtlasError::new(ErrorCode::IoFailure);
    std::fs::create_dir_all(directory).map_err(io)?;
    let target = directory.join(&prepared.manifest.id);
    if target.exists() {
        return Err(AtlasError::new(ErrorCode::Conflict));
    }
    let staging = directory.join(format!("{STAGING_PREFIX}{}", uuid::Uuid::new_v4().simple()));
    let written = (|| -> Result<(), AtlasError> {
        std::fs::create_dir(&staging).map_err(io)?;
        for (path, bytes) in &prepared.files {
            let file = staging.join(path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            std::fs::write(&file, bytes).map_err(io)?;
        }
        // What was written is what was checked.
        if folder_hash(&staging).as_deref() != Some(prepared.content_sha256.as_str()) {
            return Err(AtlasError::new(ErrorCode::IoFailure));
        }
        std::fs::rename(&staging, &target).map_err(io)
    })();
    if written.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    written.map(|()| target)
}

/// Removes InfraWerf's own leftover working folders: an install or a
/// removal that was interrupted.
pub fn clean_working_folders(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(STAGING_PREFIX) || name.starts_with(REMOVING_PREFIX) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        InstallSource::decl(config),
        InstallRequest::decl(config),
        InstallProblemCode::decl(config),
        InstallProblem::decl(config),
        InstallReport::decl(config),
        Integrity::decl(config),
        SourceKind::decl(config),
        InstallRecordView::decl(config),
    ]
    .into_iter()
    .map(|d| format!("export {d}\n"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_the_plugin_and_are_safe_on_windows() {
        for good in ["a", "a/b.json", "dir\\file.txt", "folder/", "x/y/z.md"] {
            assert!(clean_path(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "\\x",
            "../x",
            "a/../../x",
            "a/./b",
            "C:/x",
            "c:x",
            "a//b",
            "con",
            "NUL.txt",
            "a/com1.json",
            "trailing.",
            "trailing ",
            "a|b",
            "a?b",
            "a*b",
            "a\u{1}b",
            "a:stream",
        ] {
            assert!(clean_path(bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            clean_path(&"a/".repeat(9)).unwrap_err().code,
            InstallProblemCode::TooLarge
        );
    }

    #[test]
    fn the_content_hash_ignores_order_and_sees_every_byte_and_name() {
        let a = vec![
            ("x".to_owned(), b"1".to_vec()),
            ("y".to_owned(), b"2".to_vec()),
        ];
        let b = vec![
            ("y".to_owned(), b"2".to_vec()),
            ("x".to_owned(), b"1".to_vec()),
        ];
        assert_eq!(content_hash(&a), content_hash(&b));
        let c = vec![
            ("x".to_owned(), b"1".to_vec()),
            ("y".to_owned(), b"3".to_vec()),
        ];
        let d = vec![
            ("x".to_owned(), b"1".to_vec()),
            ("z".to_owned(), b"2".to_vec()),
        ];
        assert_ne!(content_hash(&a), content_hash(&c));
        assert_ne!(content_hash(&a), content_hash(&d));
    }

    #[test]
    fn only_plain_https_addresses_are_fetched() {
        assert!(https_address("https://example.org/p.zip").is_ok());
        for bad in [
            "http://example.org/p.zip",
            "file:///C:/p.zip",
            "https://",
            "https://user:pw@example.org/p.zip",
            "https://example.org/a b.zip",
            "ftp://example.org/p.zip",
        ] {
            assert_eq!(
                https_address(bad).unwrap_err().code,
                InstallProblemCode::NotHttps,
                "{bad}"
            );
        }
    }
}

// ---------------------------------------------------------------- updates
//
// An update is a plugin with the same ID as one already there, read and
// checked exactly like an install (F22/T046). Before it applies, the owner
// sees what changes. It keeps working without a new review only when it
// asks for nothing new: no added capability, and no new or changed command,
// application or access. Anything else switches the plugin off until the
// owner reviews and enables it again, and no permission is ever added.

/// How one contribution differs between two versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContributionChangeKind {
    Added,
    Removed,
    /// Its command is new or different: the owner must read it again.
    CommandChanged,
    /// Its application's port or path is new or different.
    AppChanged,
    /// It uses different capabilities, or sits at another point.
    AccessChanged,
    /// Only its title, order or presentation changed.
    Presentation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContributionChange {
    pub id: String,
    pub title: String,
    pub change: ContributionChangeKind,
    /// The new command as the owner reads it, when it is new or changed.
    pub command: Option<String>,
}

/// What an update changes, shown before it applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateDiff {
    pub from_version: String,
    pub to_version: String,
    /// The new version is older than the installed one.
    pub downgrade: bool,
    pub added_capabilities: Vec<crate::extensions::Capability>,
    pub removed_capabilities: Vec<crate::extensions::Capability>,
    pub contributions: Vec<ContributionChange>,
    /// The update asks for something new, so the plugin is switched off
    /// until the owner reviews and enables it again.
    pub needs_review: bool,
    /// Key ID verified at the previous install or update, if any.
    pub previous_signer: Option<String>,
    /// Key ID verified for this update, if a public key was given.
    pub signer: Option<String>,
    pub content_sha256: String,
    pub archive_sha256: Option<String>,
}

/// The answer to "check this update": what it would change, or why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdatePreview {
    pub contract_version: u32,
    /// Pass this to apply exactly what was checked and shown.
    pub token: Option<String>,
    pub plugin_id: String,
    pub plugin_name: String,
    pub diff: Option<UpdateDiff>,
    pub problem: Option<InstallProblem>,
}

impl UpdatePreview {
    pub fn refused(plugin_id: &str, plugin_name: &str, problem: InstallProblem) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            token: None,
            plugin_id: plugin_id.to_owned(),
            plugin_name: plugin_name.to_owned(),
            diff: None,
            problem: Some(problem),
        }
    }
}

/// Semantic Versioning 2.0.0 precedence. Build metadata is ignored.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    fn split(v: &str) -> (Vec<u64>, Option<Vec<String>>) {
        let v = v.split('+').next().unwrap_or_default();
        let (core, pre) = match v.split_once('-') {
            Some((core, pre)) => (core, Some(pre.split('.').map(str::to_owned).collect())),
            None => (v, None),
        };
        (
            core.split('.').map(|n| n.parse().unwrap_or(0)).collect(),
            pre,
        )
    }
    let (ca, pa) = split(a);
    let (cb, pb) = split(b);
    match ca.cmp(&cb) {
        Ordering::Equal => {}
        other => return other,
    }
    match (pa, pb) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(pa), Some(pb)) => {
            for (x, y) in pa.iter().zip(pb.iter()) {
                let order = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
            pa.len().cmp(&pb.len())
        }
    }
}

/// Compares the installed manifest with an update's.
pub fn diff(old: &PluginManifest, new: &PluginManifest) -> UpdateDiff {
    use std::collections::BTreeSet;
    let before: BTreeSet<_> = old.capabilities.iter().copied().collect();
    let after: BTreeSet<_> = new.capabilities.iter().copied().collect();
    let mut contributions = Vec::new();
    for entry in &new.contributions {
        let previous = old.contributions.iter().find(|c| c.id == entry.id);
        let same_access = |p: &crate::plugin::ManifestContribution| {
            let a: BTreeSet<_> = p.capabilities.iter().collect();
            let b: BTreeSet<_> = entry.capabilities.iter().collect();
            p.point == entry.point && a == b
        };
        let change = match previous {
            None => Some(ContributionChangeKind::Added),
            Some(p) if !same_access(p) => Some(ContributionChangeKind::AccessChanged),
            Some(p) if p.command != entry.command => Some(ContributionChangeKind::CommandChanged),
            Some(p) if p.app != entry.app => Some(ContributionChangeKind::AppChanged),
            Some(p) if p != entry => Some(ContributionChangeKind::Presentation),
            Some(_) => None,
        };
        if let Some(change) = change {
            let shows_command = matches!(
                change,
                ContributionChangeKind::Added
                    | ContributionChangeKind::CommandChanged
                    | ContributionChangeKind::AccessChanged
            );
            contributions.push(ContributionChange {
                id: entry.id.clone(),
                title: entry.title.clone(),
                change,
                command: entry
                    .command
                    .as_ref()
                    .filter(|_| shows_command)
                    .map(ToString::to_string),
            });
        }
    }
    for entry in &old.contributions {
        if !new.contributions.iter().any(|c| c.id == entry.id) {
            contributions.push(ContributionChange {
                id: entry.id.clone(),
                title: entry.title.clone(),
                change: ContributionChangeKind::Removed,
                command: None,
            });
        }
    }
    let added_capabilities: Vec<_> = after.difference(&before).copied().collect();
    // Any new contribution, and any changed command, application or access,
    // is something the owner has not reviewed, even with the same
    // capabilities.
    let unseen = contributions.iter().any(|change| {
        !matches!(
            change.change,
            ContributionChangeKind::Removed | ContributionChangeKind::Presentation
        )
    });
    UpdateDiff {
        from_version: old.version.clone(),
        to_version: new.version.clone(),
        downgrade: compare_versions(&new.version, &old.version) == std::cmp::Ordering::Less,
        needs_review: !added_capabilities.is_empty() || unseen,
        added_capabilities,
        removed_capabilities: before.difference(&after).copied().collect(),
        contributions,
        previous_signer: None,
        signer: None,
        content_sha256: String::new(),
        archive_sha256: None,
    }
}

/// Replaces an installed plugin folder with a prepared update. The old
/// folder is kept, renamed aside, until the new one is in place; if moving
/// the new one in fails, the old one is put back. Returns the parked old
/// folder, for the caller to delete once its records are saved, or to put
/// back with `restore` if they cannot be.
pub fn replace(directory: &Path, folder: &str, prepared: &Prepared) -> Result<PathBuf, AtlasError> {
    let io = |_| AtlasError::new(ErrorCode::IoFailure);
    let target = directory.join(folder);
    let staging = directory.join(format!("{STAGING_PREFIX}{}", uuid::Uuid::new_v4().simple()));
    let parked = directory.join(format!(
        "{REMOVING_PREFIX}{}",
        uuid::Uuid::new_v4().simple()
    ));
    let written = (|| -> Result<(), AtlasError> {
        std::fs::create_dir(&staging).map_err(io)?;
        for (path, bytes) in &prepared.files {
            let file = staging.join(path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            std::fs::write(&file, bytes).map_err(io)?;
        }
        if folder_hash(&staging).as_deref() != Some(prepared.content_sha256.as_str()) {
            return Err(AtlasError::new(ErrorCode::IoFailure));
        }
        std::fs::rename(&target, &parked).map_err(io)?;
        if let Err(error) = std::fs::rename(&staging, &target) {
            let _ = std::fs::rename(&parked, &target);
            return Err(io(error));
        }
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    written.map(|()| parked)
}

/// Puts the previous version back after a failed update.
pub fn restore(directory: &Path, folder: &str, parked: &Path) {
    let target = directory.join(folder);
    let aside = directory.join(format!("{STAGING_PREFIX}{}", uuid::Uuid::new_v4().simple()));
    if std::fs::rename(&target, &aside).is_ok() && std::fs::rename(parked, &target).is_ok() {
        let _ = std::fs::remove_dir_all(&aside);
    }
}

/// The manifest's own hash, as the owner's review records it.
pub fn manifest_hash(prepared: &Prepared) -> String {
    prepared
        .files
        .iter()
        .find(|(path, _)| path == MANIFEST_FILE)
        .map(|(_, bytes)| sha256_hex(bytes))
        .unwrap_or_default()
}

pub fn update_contracts(config: &Config) -> String {
    [
        ContributionChangeKind::decl(config),
        ContributionChange::decl(config),
        UpdateDiff::decl(config),
        UpdatePreview::decl(config),
    ]
    .into_iter()
    .map(|d| format!("export {d}\n"))
    .collect()
}
