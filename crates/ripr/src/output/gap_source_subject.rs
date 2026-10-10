//! Source-state subject stamp for editor-consumed gap artifacts (#4544).
//!
//! A gap decision ledger or actionable-gaps report names files and lines in
//! the workspace it was computed from. Nothing about a line number says which
//! revision it belongs to, so after a branch switch, edit, or commit an editor
//! consumer could place branch A's gap at `anchor.file:anchor.line` on branch
//! B's file and present it as current.
//!
//! The stamp is taken where the analysis runs, not where a derived report is
//! written: `ripr check --format json` and `--format repo-exposure-json` add a
//! `source_subject` with the content digest of every file their gap-bearing
//! output names, read in the analysis run. The gap ledger writer and the xtask
//! actionable-gaps writer only copy digests from that input stamp
//! ([`derive_source_subject`]); they never hash the workspace, so a report
//! written after a checkout or an edit cannot vouch for records computed
//! before it. The LSP validator in `lsp::gap_artifacts` recomputes the
//! digests from the current workspace and is the only consumer authority.

mod shared;

pub(crate) use shared::{
    GapSourceSubject, GapSourceSubjectFile, SOURCE_SUBJECT_DIGEST_ALGORITHM,
    actionable_packet_named_paths,
};

use crate::analysis::committed_source::frozen::{self, FrozenSourceAuthority, fs as frozen_fs};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;

/// Bind even empty and lexically accepted projections to the actual context.
pub(crate) fn frozen_stamp_context(
    root: &Path,
) -> Result<Option<Arc<FrozenSourceAuthority>>, String> {
    let Some(authority) = frozen::current() else {
        return Ok(None);
    };
    require_frozen_stamp_context(root, &authority)?;
    Ok(Some(authority))
}

pub(crate) fn require_frozen_stamp_context(
    root: &Path,
    authority: &Arc<FrozenSourceAuthority>,
) -> Result<(), String> {
    authority
        .ensure_clean()
        .map_err(|error| error.to_string())?;
    if !frozen::current().is_some_and(|current| Arc::ptr_eq(&current, authority)) {
        return Err(authority
            .refuse_external_effect("source subject context changed during projection")
            .to_string());
    }
    let canonical = frozen_fs::canonicalize(root).map_err(|error| error.to_string())?;
    authority
        .ensure_clean()
        .map_err(|error| error.to_string())?;
    // The frozen canonicalizer joins the empty relative root, which can
    // retain a trailing separator. Compare native path components while
    // keeping the real canonicalization, context, and subroot checks.
    if canonical != authority.logical_root() {
        return Err(authority
            .refuse_external_effect("source subject requires its exact logical root")
            .to_string());
    }
    Ok(())
}

pub(crate) fn subject_relative_path(root: &Path, raw: &str) -> Option<String> {
    if frozen::current().is_none() {
        return shared::subject_relative_path_with(root, raw, &mut |path| {
            std::fs::canonicalize(path)
        });
    }
    let authority = frozen_stamp_context(root).ok()??;
    let result =
        shared::subject_relative_path_with(root, raw, &mut |path| frozen_fs::canonicalize(path));
    require_frozen_stamp_context(root, &authority).ok()?;
    result
}

pub(crate) fn actionable_packet_subject_paths(root: &Path, packet: &Value) -> BTreeSet<String> {
    if frozen::current().is_none() {
        return shared::actionable_packet_subject_paths(root, packet);
    }
    let Ok(Some(authority)) = frozen_stamp_context(root) else {
        return BTreeSet::new();
    };
    let paths = shared::actionable_packet_subject_paths_with(root, packet, &mut |path| {
        frozen_fs::canonicalize(path)
    });
    if require_frozen_stamp_context(root, &authority).is_err() {
        return BTreeSet::new();
    }
    paths
}

pub(crate) fn derive_source_subject(
    input_stamp: Option<&Value>,
    input_root: &Path,
    output_root: &Path,
    required: &BTreeSet<String>,
) -> Result<GapSourceSubject, &'static str> {
    if frozen::current().is_none() {
        return shared::derive_source_subject(input_stamp, input_root, output_root, required);
    }
    let authority = frozen::current().ok_or("input_source_subject_frozen")?;
    require_frozen_stamp_context(input_root, &authority)
        .map_err(|error| frozen_projection_refusal(&authority, error))?;
    require_frozen_stamp_context(output_root, &authority)
        .map_err(|error| frozen_projection_refusal(&authority, error))?;
    let subject = shared::derive_source_subject_with(
        input_stamp,
        input_root,
        output_root,
        required,
        &mut |path| frozen_fs::canonicalize(path),
    );
    require_frozen_stamp_context(input_root, &authority)
        .map_err(|error| frozen_projection_refusal(&authority, error))?;
    require_frozen_stamp_context(output_root, &authority)
        .map_err(|error| frozen_projection_refusal(&authority, error))?;
    subject
}

fn frozen_projection_refusal(
    authority: &Arc<FrozenSourceAuthority>,
    error: String,
) -> &'static str {
    authority.refuse_external_effect(&error);
    "input_source_subject_frozen"
}

/// The result of comparing a stamp with the current workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceSubjectCheck {
    /// Every stamped file still has its stamped content.
    Current,
    /// A stamped file changed, was deleted, or appeared since the analysis
    /// read it. Carries the repo-relative path of the first differing file.
    Stale(String),
    /// The artifact cannot be matched to the current workspace: the stamp is
    /// missing, malformed, uses an unknown digest, omits a file the records
    /// name, or a stamped file cannot be read.
    Unverifiable(&'static str),
}

/// `sha256:<hex>` of one workspace file, `Ok(None)` when it does not exist.
pub(crate) fn source_file_digest(root: &Path, relative: &str) -> Result<Option<String>, String> {
    if let Some(authority) = frozen_stamp_context(root)? {
        // Retain no source bytes, but authenticate the complete admitted blob.
        let read = frozen_fs::read_prefix(root.join(relative), 0);
        // A known physical deletion is sticky NotFound, not named-tree absence.
        require_frozen_stamp_context(root, &authority)?;
        return match read {
            Ok(_) => {
                let file = authority
                    .inventory()
                    .files()
                    .find(|(path, _)| *path == Path::new(relative))
                    .map(|(_, file)| file)
                    .ok_or_else(|| {
                        authority
                            .refuse_external_effect("source subject read lacks inventory identity")
                            .to_string()
                    })?;
                use std::fmt::Write as _;
                let mut digest = String::from("sha256:");
                for byte in &file.sha256 {
                    write!(&mut digest, "{byte:02x}").map_err(|error| error.to_string())?;
                }
                require_frozen_stamp_context(root, &authority)?;
                Ok(Some(digest))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!(
                "read source subject file {relative} failed: {error}"
            )),
        };
    }
    match std::fs::read(root.join(relative)) {
        Ok(bytes) => Ok(Some(format!("sha256:{:x}", Sha256::digest(bytes)))),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("read source subject file {relative} failed: {err}")),
    }
}

/// Files a gap decision ledger record makes claims about: the anchor the
/// diagnostic is placed on and the repair route's target and related test.
pub(crate) fn gap_record_subject_paths(root: &Path, record: &Value) -> BTreeSet<String> {
    [
        &["anchor", "file"][..],
        &["repair_route", "target_file"][..],
        &["repair_route", "related_test"][..],
    ]
    .iter()
    .filter_map(|path| {
        path.iter()
            .try_fold(record, |value, key| value.get(*key))
            .and_then(Value::as_str)
    })
    .filter_map(|raw| subject_relative_path(root, raw))
    .collect()
}

/// Every workspace file an analysis output value names under a path-bearing
/// key (`file`, `path`, `source_file`, `target_file`, `target_test`,
/// `related_test`, `test`, `related_test_or_observer`), at any depth. Used by
/// the repo-exposure producer so its stamp covers whatever a derived ledger
/// record or actionable-gaps packet later names from the same evidence.
pub(crate) fn named_files_in_value(root: &Path, value: &Value, files: &mut BTreeSet<String>) {
    const PATH_KEYS: [&str; 8] = [
        "file",
        "path",
        "source_file",
        "target_file",
        "target_test",
        "related_test",
        "test",
        "related_test_or_observer",
    ];
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if PATH_KEYS.contains(&key.as_str()) {
                    collect_path_strings(root, child, files);
                }
                named_files_in_value(root, child, files);
            }
        }
        Value::Array(values) => {
            for child in values {
                named_files_in_value(root, child, files);
            }
        }
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn collect_path_strings(root: &Path, value: &Value, files: &mut BTreeSet<String>) {
    match value {
        Value::String(raw) => {
            if let Some(path) = subject_relative_path(root, raw) {
                files.insert(path);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_path_strings(root, child, files);
            }
        }
        Value::Object(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Stamp the given repo-relative files with their current content digests.
/// Only analysis producers call this, in the run that read the files.
pub(crate) fn stamp_source_subject(
    root: &Path,
    paths: &BTreeSet<String>,
) -> Result<GapSourceSubject, String> {
    let authority = frozen_stamp_context(root)?;
    let files = paths
        .iter()
        .map(|path| {
            Ok(GapSourceSubjectFile {
                path: path.clone(),
                digest: source_file_digest(root, path)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if let Some(authority) = authority {
        require_frozen_stamp_context(root, &authority)?;
    }
    Ok(GapSourceSubject {
        digest_algorithm: SOURCE_SUBJECT_DIGEST_ALGORITHM.to_string(),
        files,
    })
}

/// Append a top-level `"source_subject"` member to a rendered JSON object
/// document, keeping the document's own formatting. Returns the document
/// unchanged when there is nothing to stamp or a file cannot be read, so a
/// derived report falls back to `unverifiable_subject` instead of carrying a
/// partial stamp.
pub(crate) fn append_source_subject_member(
    rendered: String,
    root: &Path,
    paths: &BTreeSet<String>,
) -> String {
    if let Some(authority) = frozen::current() {
        return match append_source_subject_member_checked(rendered.clone(), root, paths) {
            Ok(stamped) => stamped,
            Err(error) => {
                authority.refuse_external_effect(&error);
                rendered
            }
        };
    }
    if paths.is_empty() {
        return rendered;
    }
    let Ok(stamp) = stamp_source_subject(root, paths) else {
        return rendered;
    };
    let Ok(stamp) = serde_json::to_string(&stamp) else {
        return rendered;
    };
    let trimmed = rendered.trim_end();
    let Some(body) = trimmed.strip_suffix('}') else {
        return rendered;
    };
    let body = body.trim_end();
    let separator = if body.ends_with('{') { "" } else { "," };
    let newline = if rendered.ends_with('\n') { "\n" } else { "" };
    format!("{body}{separator}\n  \"source_subject\": {stamp}\n}}{newline}")
}

/// Fallible stamping for active frozen producers. Ordinary output keeps the
/// existing formatting and fallback behavior.
pub(crate) fn append_source_subject_member_checked(
    rendered: String,
    root: &Path,
    paths: &BTreeSet<String>,
) -> Result<String, String> {
    let Some(authority) = frozen_stamp_context(root)? else {
        return Ok(append_source_subject_member(rendered, root, paths));
    };
    let value: Value = serde_json::from_str(&rendered)
        .map_err(|error| format!("parse source subject JSON failed: {error}"))?;
    if !value.is_object() {
        return Err("source subject JSON must be an object".to_string());
    }
    require_frozen_stamp_context(root, &authority)?;
    if paths.is_empty() {
        return Ok(rendered);
    }
    let stamp = stamp_source_subject(root, paths)?;
    let stamp = serde_json::to_string(&stamp)
        .map_err(|error| format!("serialize source subject failed: {error}"))?;
    let body = rendered
        .trim_end()
        .strip_suffix('}')
        .ok_or("source subject JSON lacks object terminator")?
        .trim_end();
    let separator = if body.ends_with('{') { "" } else { "," };
    let newline = if rendered.ends_with('\n') { "\n" } else { "" };
    let stamped = format!("{body}{separator}\n  \"source_subject\": {stamp}\n}}{newline}");
    require_frozen_stamp_context(root, &authority)?;
    Ok(stamped)
}

/// Compare an artifact's `source_subject` with the current workspace.
/// `required` is every file the artifact's records name; each must be
/// stamped, so a stamp cannot vouch for a subset of the claim.
pub(crate) fn check_source_subject(
    root: &Path,
    stamp: Option<&Value>,
    required: &BTreeSet<String>,
) -> SourceSubjectCheck {
    let Some(stamp) = stamp else {
        return SourceSubjectCheck::Unverifiable("source_subject_missing");
    };
    let Ok(subject) = serde_json::from_value::<GapSourceSubject>(stamp.clone()) else {
        return SourceSubjectCheck::Unverifiable("source_subject_malformed");
    };
    if subject.digest_algorithm != SOURCE_SUBJECT_DIGEST_ALGORITHM {
        return SourceSubjectCheck::Unverifiable("source_subject_unsupported_digest");
    }
    let mut stamped = BTreeSet::new();
    for file in &subject.files {
        if subject_relative_path(root, &file.path).as_deref() != Some(file.path.as_str()) {
            return SourceSubjectCheck::Unverifiable("source_subject_malformed");
        }
        stamped.insert(file.path.as_str());
    }
    if required.iter().any(|path| !stamped.contains(path.as_str())) {
        return SourceSubjectCheck::Unverifiable("source_subject_incomplete");
    }
    for file in &subject.files {
        match source_file_digest(root, &file.path) {
            Ok(current) if current == file.digest => {}
            Ok(_) => return SourceSubjectCheck::Stale(file.path.clone()),
            Err(_) => return SourceSubjectCheck::Unverifiable("source_subject_unreadable"),
        }
    }
    SourceSubjectCheck::Current
}

#[cfg(test)]
pub(crate) fn with_source_subject_for_test(root: &Path, mut artifact: Value) -> Value {
    let mut paths = BTreeSet::new();
    for key in ["records", "gap_records"] {
        if let Some(records) = artifact.get(key).and_then(Value::as_array) {
            for record in records {
                paths.extend(gap_record_subject_paths(root, record));
            }
        }
    }
    if let Some(packets) = artifact.get("packets").and_then(Value::as_array) {
        for packet in packets {
            paths.extend(actionable_packet_subject_paths(root, packet));
        }
    }
    if let (Ok(stamp), Some(object)) = (
        stamp_source_subject(root, &paths)
            .and_then(|stamp| serde_json::to_value(stamp).map_err(|err| err.to_string())),
        artifact.as_object_mut(),
    ) {
        object.insert("source_subject".to_string(), stamp);
    }
    artifact
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> Result<std::path::PathBuf, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system time before epoch: {err}"))?;
        let root = std::env::temp_dir().join(format!(
            "ripr-gap-source-subject-{label}-{}-{}",
            std::process::id(),
            now.as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("mkdir: {err}"))?;
        Ok(root)
    }

    fn stamp_value(root: &Path, paths: &BTreeSet<String>) -> Result<Value, String> {
        serde_json::to_value(stamp_source_subject(root, paths)?).map_err(|err| err.to_string())
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    struct NamedStampFixture {
        root: std::path::PathBuf,
        physical: std::path::PathBuf,
        authority: Arc<FrozenSourceAuthority>,
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    impl NamedStampFixture {
        fn new(label: &str) -> Result<Self, String> {
            let root = temp_root(label)?;
            std::fs::create_dir_all(root.join("tests")).map_err(|error| error.to_string())?;
            std::fs::create_dir_all(root.join("folder.rs")).map_err(|error| error.to_string())?;
            for (path, bytes) in [
                ("src/lib.rs", "pub fn head() -> bool { true }\n"),
                ("tests/check.rs", "fn head_test() {}\n"),
                ("folder.rs/inside.rs", "named directory child\n"),
            ] {
                std::fs::write(root.join(path), bytes).map_err(|error| error.to_string())?;
            }
            for args in [
                &["-c", "init.templateDir=", "init", "-q"][..],
                &["config", "user.email", "stamp@example.invalid"][..],
                &["config", "user.name", "stamp fixture"][..],
                &["add", "."][..],
                &["commit", "-qm", "named stamp head"][..],
            ] {
                crate::testing::fixture_git::fixture_git_ok(&root, args)?;
            }
            let prepared =
                crate::analysis::git_candidate_execution::prepare_named_tree(&root, "HEAD", None)
                    .map_err(|error| error.to_string())?;
            let physical = prepared.physical_root().to_path_buf();
            let authority = prepared
                .frozen_source_authority(&root)
                .map_err(|error| error.to_string())?;
            Ok(Self {
                root,
                physical,
                authority,
            })
        }
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    impl Drop for NamedStampFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(all(
        target_os = "linux",
        feature = "lang-rust",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    mod snapshot_link_fixture {
        use std::fs::{File, Metadata, OpenOptions};
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        use std::path::{Path, PathBuf};

        #[cfg(target_arch = "x86_64")]
        const DIRECTORY: i32 = 0x10000;
        #[cfg(target_arch = "aarch64")]
        const DIRECTORY: i32 = 0x4000;
        #[cfg(target_arch = "x86_64")]
        const NOFOLLOW: i32 = 0x20000;
        #[cfg(target_arch = "aarch64")]
        const NOFOLLOW: i32 = 0x8000;
        const NONBLOCK: i32 = 0x800;

        #[derive(Clone, Copy, PartialEq, Eq)]
        struct DirectoryIdentity {
            dev: u64,
            ino: u64,
            uid: u32,
            gid: u32,
            mode: u32,
            nlink: u64,
        }

        impl DirectoryIdentity {
            fn capture(metadata: &Metadata) -> Result<Self, String> {
                if !metadata.is_dir() {
                    return Err("fixture parent is not a directory".into());
                }
                Ok(Self {
                    dev: metadata.dev(),
                    ino: metadata.ino(),
                    uid: metadata.uid(),
                    gid: metadata.gid(),
                    mode: metadata.mode(),
                    nlink: metadata.nlink(),
                })
            }
        }

        struct Directory {
            file: File,
            identity: DirectoryIdentity,
        }

        impl Directory {
            fn open(path: &Path) -> Result<Self, String> {
                let file = OpenOptions::new()
                    .read(true)
                    .custom_flags(DIRECTORY | NOFOLLOW | NONBLOCK)
                    .open(path)
                    .map_err(|error| error.to_string())?;
                let identity = DirectoryIdentity::capture(
                    &file.metadata().map_err(|error| error.to_string())?,
                )?;
                let directory = Self { file, identity };
                directory.check(path)?;
                Ok(directory)
            }

            fn child(&self, leaf: &str) -> PathBuf {
                Path::new("/proc/self/fd")
                    .join(self.file.as_raw_fd().to_string())
                    .join(leaf)
            }

            fn check(&self, path: &Path) -> Result<(), String> {
                for metadata in [self.file.metadata(), std::fs::symlink_metadata(path)] {
                    let metadata = metadata.map_err(|error| error.to_string())?;
                    if DirectoryIdentity::capture(&metadata)? != self.identity {
                        return Err("fixture parent identity changed".into());
                    }
                }
                Ok(())
            }
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        struct LinkIdentity {
            dev: u64,
            ino: u64,
            uid: u32,
            gid: u32,
            mode: u32,
            nlink: u64,
            len: u64,
            mtime: i64,
            mtime_nsec: i64,
            ctime: i64,
            ctime_nsec: i64,
        }

        impl LinkIdentity {
            fn capture(metadata: &Metadata, target_len: usize, uid: u32) -> Result<Self, String> {
                if !metadata.file_type().is_symlink()
                    || metadata.nlink() != 1
                    || metadata.uid() != uid
                    || metadata.len() != target_len as u64
                {
                    return Err("fixture link type, owner, count or length changed".into());
                }
                Ok(Self {
                    dev: metadata.dev(),
                    ino: metadata.ino(),
                    uid: metadata.uid(),
                    gid: metadata.gid(),
                    mode: metadata.mode(),
                    nlink: metadata.nlink(),
                    len: metadata.len(),
                    mtime: metadata.mtime(),
                    mtime_nsec: metadata.mtime_nsec(),
                    ctime: metadata.ctime(),
                    ctime_nsec: metadata.ctime_nsec(),
                })
            }
        }

        // This owns one link created by a cooperating test. It grants no
        // production cleanup authority or concurrent-writer atomicity.
        pub(super) struct OwnedLink {
            physical: PathBuf,
            target: PathBuf,
            root: Directory,
            source: Directory,
            created: Option<LinkIdentity>,
            attempted: bool,
        }

        impl OwnedLink {
            pub(super) fn pin(physical: &Path, target: &Path) -> Result<Self, String> {
                let target_len = target.as_os_str().as_bytes().len();
                if target_len == 0 || target_len > 4096 {
                    return Err("fixture link target exceeds its admitted length".into());
                }
                let root = Directory::open(physical)?;
                let source = Directory::open(&root.child("src"))?;
                Ok(Self {
                    physical: physical.to_path_buf(),
                    target: target.to_path_buf(),
                    root,
                    source,
                    created: None,
                    attempted: false,
                })
            }

            fn parents(&self) -> Result<(), String> {
                self.root.check(&self.physical)?;
                self.source.check(&self.root.child("src"))
            }

            pub(super) fn path(&self) -> PathBuf {
                self.source.child("lib.rs")
            }

            fn observe(&self) -> Result<LinkIdentity, String> {
                self.parents()?;
                let path = self.path();
                let before = LinkIdentity::capture(
                    &std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?,
                    self.target.as_os_str().as_bytes().len(),
                    self.source.identity.uid,
                )?;
                if std::fs::read_link(&path).map_err(|error| error.to_string())? != self.target {
                    return Err("fixture link target changed".into());
                }
                let after = LinkIdentity::capture(
                    &std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?,
                    self.target.as_os_str().as_bytes().len(),
                    self.source.identity.uid,
                )?;
                self.parents()?;
                if after != before {
                    return Err("fixture link changed during observation".into());
                }
                Ok(after)
            }

            pub(super) fn record_created(&mut self) -> Result<(), String> {
                if self.created.is_some() || self.attempted {
                    return Err("fixture link capture was already claimed".into());
                }
                self.created = Some(self.observe()?);
                Ok(())
            }

            pub(super) fn remove_once(&mut self) -> Result<(), String> {
                if self.attempted {
                    return Err("fixture link teardown was already claimed".into());
                }
                self.attempted = true;
                let created = self.created.ok_or("fixture link was not recorded")?;
                if self.observe()? != created {
                    return Err("fixture link identity changed before teardown".into());
                }
                self.parents()?;
                let path = self.path();
                std::fs::remove_file(&path).map_err(|error| error.to_string())?;
                self.parents()?;
                match std::fs::symlink_metadata(path) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error.to_string()),
                    Ok(_) => Err("fixture link remains after exact unlink".into()),
                }
            }
        }
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    fn stamp_error<T>(result: Result<T, String>, family: &str) -> Result<String, String> {
        match result {
            Err(error) if error.contains(family) => Ok(error),
            Err(error) => Err(format!("wrong stamp refusal for {family}: {error}")),
            Ok(_) => Err(format!("stamp admitted expected refusal {family}")),
        }
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_stamp_accepts_exact_root_with_canonical_trailing_separator() -> Result<(), String> {
        let fixture = NamedStampFixture::new("canonical-root-separator")?;
        frozen::with_context(Some(fixture.authority.clone()), || {
            let canonical =
                frozen_fs::canonicalize(&fixture.root).map_err(|error| error.to_string())?;
            assert_eq!(canonical, fixture.authority.logical_root());
            assert_ne!(
                canonical.as_os_str(),
                fixture.authority.logical_root().as_os_str(),
                "actual frozen canonical root must reach the trailing-separator discriminator",
            );
            for root in [
                fixture.root.clone(),
                fixture.root.join(""),
                fixture.root.join("."),
            ] {
                let bound = frozen_stamp_context(&root)?.ok_or("missing active stamp authority")?;
                assert!(Arc::ptr_eq(&bound, &fixture.authority));
                let empty = stamp_source_subject(&root, &BTreeSet::new())?;
                assert!(empty.files.is_empty());
                let digest = source_file_digest(&root, "src/lib.rs")?
                    .ok_or("named source digest is absent")?;
                assert!(digest.starts_with("sha256:"));
            }
            fixture
                .authority
                .ensure_clean()
                .map_err(|error| error.to_string())
        })
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_stamp_preserves_named_head_after_dirty_staged_and_deleted_live_files()
    -> Result<(), String> {
        let fixture = NamedStampFixture::new("named-head")?;
        let paths = BTreeSet::from([
            "src/lib.rs".to_string(),
            "tests/check.rs".to_string(),
            "folder.rs/inside.rs".to_string(),
        ]);
        let ordinary =
            append_source_subject_member("{\"findings\": []}\n".into(), &fixture.root, &paths);
        std::fs::write(fixture.root.join("src/lib.rs"), "dirty head replacement\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(fixture.root.join("tests/check.rs"), "staged replacement\n")
            .map_err(|error| error.to_string())?;
        crate::testing::fixture_git::fixture_git_ok(&fixture.root, &["add", "tests/check.rs"])?;
        std::fs::remove_file(fixture.root.join("folder.rs/inside.rs"))
            .map_err(|error| error.to_string())?;
        frozen::with_context(Some(fixture.authority.clone()), || {
            let frozen = append_source_subject_member_checked(
                "{\"findings\": []}\n".into(),
                &fixture.root.join("."),
                &paths,
            )?;
            assert_eq!(frozen, ordinary);
            assert_eq!(source_file_digest(&fixture.root, "absent.rs")?, None);
            fixture
                .authority
                .ensure_clean()
                .map_err(|error| error.to_string())?;
            let named = fixture
                .root
                .join("src/lib.rs")
                .to_string_lossy()
                .into_owned();
            assert_eq!(
                subject_relative_path(&fixture.root, &named),
                Some("src/lib.rs".into())
            );
            let packet =
                json!({"source_file": named, "related_test_or_observer": "tests/check.rs::head"});
            assert_eq!(
                actionable_packet_subject_paths(&fixture.root, &packet),
                BTreeSet::from(["src/lib.rs".to_string(), "tests/check.rs".to_string()]),
            );
            let value: Value = serde_json::from_str(&frozen).map_err(|error| error.to_string())?;
            let copied = derive_source_subject(
                value.get("source_subject"),
                &fixture.root,
                &fixture.root,
                &paths,
            )
            .map_err(str::to_string)?;
            assert_eq!(
                serde_json::to_value(copied).map_err(|error| error.to_string())?,
                value["source_subject"]
            );
            fixture
                .authority
                .ensure_clean()
                .map_err(|error| error.to_string())
        })
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_stamp_distinguishes_named_absence_from_deleted_or_changed_snapshot()
    -> Result<(), String> {
        for (label, mutation) in [("deleted-snapshot", 0), ("changed-snapshot", 1)] {
            let fixture = NamedStampFixture::new(label)?;
            frozen::with_context(Some(fixture.authority.clone()), || {
                assert_eq!(source_file_digest(&fixture.root, "never.rs")?, None);
                fixture
                    .authority
                    .ensure_clean()
                    .map_err(|error| error.to_string())?;
                if mutation == 0 {
                    std::fs::remove_file(fixture.physical.join("src/lib.rs"))
                        .map_err(|error| error.to_string())?;
                } else {
                    let mut bytes = std::fs::read(fixture.physical.join("src/lib.rs"))
                        .map_err(|error| error.to_string())?;
                    bytes[0] = b'X';
                    std::fs::write(fixture.physical.join("src/lib.rs"), bytes)
                        .map_err(|error| error.to_string())?;
                }
                let failure = stamp_error(
                    source_file_digest(&fixture.root, "src/lib.rs"),
                    "src/lib.rs",
                )?;
                let retained = fixture
                    .authority
                    .ensure_clean()
                    .map_err(|error| error.to_string());
                assert_eq!(stamp_error(retained, "src/lib.rs")?, failure);
                stamp_error(
                    stamp_source_subject(&fixture.root, &BTreeSet::new()),
                    "src/lib.rs",
                )?;
                Ok::<_, String>(())
            })?;
        }
        let recovery = NamedStampFixture::new("snapshot-recovery")?;
        frozen::with_context(Some(recovery.authority.clone()), || {
            let digest = source_file_digest(&recovery.root, "src/lib.rs")?
                .ok_or("fresh named source lacks digest")?;
            assert!(digest.starts_with("sha256:"));
            recovery
                .authority
                .ensure_clean()
                .map_err(|error| error.to_string())
        })
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_stamp_refuses_link_and_directory_reads_without_successful_fallback()
    -> Result<(), String> {
        let link = NamedStampFixture::new("snapshot-link")?;
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        let mut owned_link =
            snapshot_link_fixture::OwnedLink::pin(&link.physical, &link.root.join("src/lib.rs"))?;
        std::fs::remove_file(link.physical.join("src/lib.rs"))
            .map_err(|error| error.to_string())?;
        std::os::unix::fs::symlink(
            link.root.join("src/lib.rs"),
            link.physical.join("src/lib.rs"),
        )
        .map_err(|error| error.to_string())?;
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        owned_link.record_created()?;
        let refusal = frozen::with_context(Some(link.authority.clone()), || {
            stamp_error(source_file_digest(&link.root, "src/lib.rs"), "symlink")
        });
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        let refusal = match (refusal, owned_link.remove_once()) {
            (Ok(refusal), Ok(())) => Ok(refusal),
            (Err(primary), Ok(())) => Err(primary),
            (Ok(_), Err(teardown)) => Err(teardown),
            (Err(primary), Err(teardown)) => {
                Err(format!("{primary}; owned fixture teardown: {teardown}"))
            }
        };
        let refusal = refusal?;
        let sticky = link
            .authority
            .ensure_clean()
            .map_err(|error| error.to_string());
        assert_eq!(stamp_error(sticky, "symlink")?, refusal);
        let directory = NamedStampFixture::new("snapshot-directory")?;
        frozen::with_context(Some(directory.authority.clone()), || {
            let paths = BTreeSet::from(["folder.rs".to_string()]);
            stamp_error(
                append_source_subject_member_checked("{}".into(), &directory.root, &paths),
                "folder.rs",
            )?;
            // The underlying directory error is nonsticky; the actual fallible
            // return, rather than only the authority latch, must carry refusal.
            directory
                .authority
                .ensure_clean()
                .map_err(|error| error.to_string())?;
            Ok::<_, String>(())
        })
    }

    #[cfg(all(
        target_os = "linux",
        feature = "lang-rust",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn frozen_stamp_link_teardown_preserves_an_unknown_replacement() -> Result<(), String> {
        let fixture = NamedStampFixture::new("snapshot-link-replacement")?;
        let mut owned = snapshot_link_fixture::OwnedLink::pin(
            &fixture.physical,
            &fixture.root.join("src/lib.rs"),
        )?;
        let path = owned.path();
        std::fs::remove_file(&path).map_err(|error| error.to_string())?;
        std::os::unix::fs::symlink(fixture.root.join("src/lib.rs"), &path)
            .map_err(|error| error.to_string())?;
        owned.record_created()?;
        // The test deliberately replaces its recorded link. An unconditional
        // cleanup would delete this different object and fail the control.
        std::fs::remove_file(&path).map_err(|error| error.to_string())?;
        std::fs::write(&path, b"unowned replacement").map_err(|error| error.to_string())?;
        let error = owned
            .remove_once()
            .err()
            .ok_or("unknown replacement was removed")?;
        assert!(error.contains("fixture link type"));
        assert_eq!(
            std::fs::read(&path).map_err(|error| error.to_string())?,
            b"unowned replacement",
        );
        assert!(
            std::fs::symlink_metadata(&path)
                .map_err(|error| error.to_string())?
                .file_type()
                .is_file()
        );
        assert!(owned.remove_once().is_err(), "refused teardown was retried");
        Ok(())
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_stamp_propagates_actual_unreadable_snapshot_failure() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;
        let fixture = NamedStampFixture::new("snapshot-unreadable")?;
        let physical = fixture.physical.join("src/lib.rs");
        std::fs::set_permissions(&physical, std::fs::Permissions::from_mode(0o0))
            .map_err(|error| error.to_string())?;
        // The actual OS must establish this premise; an elevated process does
        // not supply an unreadable-file counterexample.
        let denied = std::fs::File::open(&physical)
            .err()
            .ok_or("host did not establish the unreadable snapshot premise")?;
        assert_eq!(denied.kind(), ErrorKind::PermissionDenied);
        let refused = frozen::with_context(Some(fixture.authority.clone()), || {
            let failure = stamp_error(
                source_file_digest(&fixture.root, "src/lib.rs"),
                "src/lib.rs",
            )?;
            let retained = stamp_error(
                fixture
                    .authority
                    .ensure_clean()
                    .map_err(|error| error.to_string()),
                "src/lib.rs",
            )?;
            assert_eq!(failure, retained);
            Ok::<_, String>(())
        });
        std::fs::set_permissions(&physical, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        refused
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_stamp_binds_root_before_empty_or_lexical_path_projection() -> Result<(), String> {
        for (label, subroot) in [("wrong-root", false), ("subroot", true)] {
            let fixture = NamedStampFixture::new(label)?;
            let wrong = if subroot {
                fixture.root.join("src")
            } else {
                fixture.root.join("../foreign")
            };
            frozen::with_context(Some(fixture.authority.clone()), || {
                assert_eq!(subject_relative_path(&wrong, "src/lib.rs"), None);
                stamp_error(stamp_source_subject(&wrong, &BTreeSet::new()), "frozen")?;
                stamp_error(
                    fixture
                        .authority
                        .ensure_clean()
                        .map_err(|error| error.to_string()),
                    "frozen",
                )?;
                Ok::<_, String>(())
            })?;
        }
        let outside = NamedStampFixture::new("outside-before-io")?;
        let foreign = outside.root.with_extension("outside.rs");
        std::fs::create_dir(&foreign).map_err(|error| error.to_string())?;
        let name = foreign
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("foreign fixture directory lacks UTF-8 identity")?;
        let relative = format!("../{name}");
        // The ordinary reader reaches the actual directory and reports its
        // read failure. The frozen path gate refuses before that I/O boundary.
        let ordinary = stamp_error(source_file_digest(&outside.root, &relative), &relative)?;
        let refused = frozen::with_context(Some(outside.authority.clone()), || {
            let failure = stamp_error(
                source_file_digest(&outside.root, &relative),
                "outside repository",
            )?;
            assert_ne!(failure, ordinary);
            stamp_error(
                outside
                    .authority
                    .ensure_clean()
                    .map_err(|error| error.to_string()),
                "outside repository",
            )?;
            Ok::<_, String>(())
        });
        std::fs::remove_dir(&foreign).map_err(|error| error.to_string())?;
        refused
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_stamp_propagates_parse_nonobject_and_keeps_ordinary_fallback() -> Result<(), String> {
        let fixture = NamedStampFixture::new("checked-append")?;
        for rendered in ["not-json", "[]"] {
            assert_eq!(
                append_source_subject_member(rendered.into(), &fixture.root, &BTreeSet::new()),
                rendered,
            );
        }
        frozen::with_context(Some(fixture.authority.clone()), || {
            stamp_error(
                append_source_subject_member_checked(
                    "not-json".into(),
                    &fixture.root,
                    &BTreeSet::new(),
                ),
                "parse source subject JSON",
            )?;
            stamp_error(
                append_source_subject_member_checked("[]".into(), &fixture.root, &BTreeSet::new()),
                "must be an object",
            )?;
            // Nonsticky parser errors still refuse through the checked API.
            fixture
                .authority
                .ensure_clean()
                .map_err(|error| error.to_string())?;
            assert_eq!(
                append_source_subject_member_checked(
                    "{}\n".into(),
                    &fixture.root,
                    &BTreeSet::new()
                )?,
                "{}\n",
            );
            let fallback =
                append_source_subject_member("[]".into(), &fixture.root, &BTreeSet::new());
            assert_eq!(fallback, "[]");
            stamp_error(
                fixture
                    .authority
                    .ensure_clean()
                    .map_err(|error| error.to_string()),
                "must be an object",
            )?;
            Ok::<_, String>(())
        })
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shared_subject_resolver_routes_outside_canonicalization_through_the_supplied_boundary()
    -> Result<(), String> {
        let root = Path::new("/repo");
        let mut calls = Vec::new();
        let path = shared::subject_relative_path_with(root, "/outside.rs", &mut |path| {
            calls.push(path.to_path_buf());
            if path == root {
                Ok(root.to_path_buf())
            } else {
                Err(std::io::Error::new(
                    ErrorKind::PermissionDenied,
                    "resolver refuses outside",
                ))
            }
        });
        assert_eq!(path, None);
        assert_eq!(
            calls,
            vec![root.to_path_buf(), std::path::PathBuf::from("/outside.rs")]
        );
        let mut calls = Vec::new();
        let packet = json!({"source_file": "/outside.rs"});
        let paths = shared::actionable_packet_subject_paths_with(root, &packet, &mut |path| {
            calls.push(path.to_path_buf());
            if path == root {
                Ok(root.to_path_buf())
            } else {
                Err(std::io::Error::new(
                    ErrorKind::PermissionDenied,
                    "resolver refuses outside",
                ))
            }
        });
        assert!(paths.is_empty());
        assert_eq!(
            calls,
            vec![root.to_path_buf(), std::path::PathBuf::from("/outside.rs")]
        );
        Ok(())
    }

    #[test]
    fn subject_relative_path_normalizes_selectors_and_rejects_escape() {
        let root = Path::new("/repo");
        assert_eq!(
            subject_relative_path(root, "tests/pricing.rs::discount_threshold"),
            Some("tests/pricing.rs".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "./src\\lib.rs"),
            Some("src/lib.rs".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "/repo/src/lib.rs"),
            Some("src/lib.rs".to_string())
        );
        assert_eq!(subject_relative_path(root, "/elsewhere/src/lib.rs"), None);
        assert_eq!(subject_relative_path(root, "../src/lib.rs"), None);
        assert_eq!(subject_relative_path(root, "  "), None);
        assert_eq!(subject_relative_path(root, ""), None);
        assert_eq!(subject_relative_path(root, "not a path"), None);
    }

    #[test]
    fn subject_relative_path_preserves_whitespace_identity() {
        let root = Path::new("/repo");
        assert_eq!(
            subject_relative_path(root, " leading.py"),
            Some(" leading.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "leading.py"),
            Some("leading.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "leading.py "),
            Some("leading.py ".to_string())
        );
        assert_eq!(
            subject_relative_path(root, " spaced/discount.py"),
            Some(" spaced/discount.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "foo bar.py"),
            Some("foo bar.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, " quoted\t.py"),
            Some(" quoted\t.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, " leading.py::discount_threshold"),
            Some(" leading.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "./ spaced\\file.rs"),
            Some(" spaced/file.rs".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "/repo/ leading.py"),
            Some(" leading.py".to_string())
        );
        assert_ne!(
            subject_relative_path(root, " leading.py"),
            subject_relative_path(root, "leading.py")
        );
    }

    #[test]
    fn subject_relative_path_rejects_prefix_and_root_escape() {
        let root = Path::new("/repo");
        assert_eq!(subject_relative_path(root, "/outside.py"), None);
        assert_eq!(subject_relative_path(root, "../outside.py"), None);
        // `check-local-context` forbids contiguous drive-letter path literals.
        let drive_relative = format!("{}:outside.py", 'C');
        let drive_absolute = format!("{}:/outside.py", 'C');
        if cfg!(windows) {
            assert_eq!(
                subject_relative_path(root, &drive_relative),
                None,
                "drive-relative identity must not join outside the workspace"
            );
            assert_eq!(subject_relative_path(root, &drive_absolute), None);
            assert_eq!(
                check_source_subject(
                    root,
                    Some(&json!({
                        "digest_algorithm": "sha256",
                        "files": [{"path": drive_relative, "digest": null}]
                    })),
                    &BTreeSet::new()
                ),
                SourceSubjectCheck::Unverifiable("source_subject_malformed"),
                "currentness must fail closed before reading a prefixed stamp"
            );
        } else {
            assert_eq!(
                subject_relative_path(root, &drive_relative).as_deref(),
                Some(drive_relative.as_str()),
                "a drive-letter lookalike is an ordinary Unix filename"
            );
            assert_eq!(
                subject_relative_path(root, &drive_absolute).as_deref(),
                Some(drive_absolute.as_str()),
                "a drive-absolute lookalike is a Unix directory named like a drive"
            );
        }
    }

    #[test]
    fn source_subject_stays_current_until_a_stamped_file_changes() -> Result<(), String> {
        let root = temp_root("edit")?;
        std::fs::write(root.join("src/lib.rs"), "fn a() {}\n").map_err(|err| err.to_string())?;
        let paths = BTreeSet::from(["src/lib.rs".to_string()]);
        let stamp = stamp_value(&root, &paths)?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Current
        );
        std::fs::write(root.join("src/lib.rs"), "fn b() {}\n").map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale("src/lib.rs".to_string())
        );
        std::fs::remove_file(root.join("src/lib.rs")).map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale("src/lib.rs".to_string())
        );
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn source_subject_keeps_whitespace_paths_and_stales_only_the_changed_identity()
    -> Result<(), String> {
        let root = temp_root("whitespace-identity")?;
        let spaced = " leading.py";
        let plain = "leading.py";
        let nested = " spaced/discount.py";
        std::fs::create_dir_all(root.join(" spaced")).map_err(|err| err.to_string())?;
        std::fs::write(root.join(spaced), "def spaced():\n    return 1\n")
            .map_err(|err| err.to_string())?;
        std::fs::write(root.join(plain), "def plain():\n    return 2\n")
            .map_err(|err| err.to_string())?;
        std::fs::write(root.join(nested), "def nested():\n    return 3\n")
            .map_err(|err| err.to_string())?;

        let paths = BTreeSet::from([spaced.to_string(), plain.to_string(), nested.to_string()]);
        let spaced_only = BTreeSet::from([spaced.to_string()]);
        let stamp = stamp_value(&root, &paths)?;
        let spaced_stamp = stamp_value(&root, &spaced_only)?;
        let stamped_paths: Vec<&str> = stamp["files"]
            .as_array()
            .ok_or("stamp files missing")?
            .iter()
            .filter_map(|file| file["path"].as_str())
            .collect();
        assert_eq!(
            stamped_paths,
            vec![spaced, nested, plain],
            "BTreeSet order is lexical; identities must stay exact"
        );
        assert_ne!(stamp["files"][0]["digest"], stamp["files"][2]["digest"]);
        assert_eq!(
            stamp["files"][0]["digest"].as_str(),
            source_file_digest(&root, spaced)?.as_deref()
        );
        assert_eq!(
            stamp["files"][2]["digest"].as_str(),
            source_file_digest(&root, plain)?.as_deref()
        );
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Current
        );

        std::fs::write(root.join(plain), "def plain():\n    return 9\n")
            .map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&spaced_stamp), &spaced_only),
            SourceSubjectCheck::Current,
            "editing the namesake must not stale the whitespace-bearing subject"
        );
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale(plain.to_string())
        );

        std::fs::write(root.join(plain), "def plain():\n    return 2\n")
            .map_err(|err| err.to_string())?;
        std::fs::write(root.join(spaced), "def spaced():\n    return 8\n")
            .map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale(spaced.to_string())
        );

        let named = json!({
            "file": spaced,
            "related_test": format!("{plain}::discount_threshold"),
            "nested": {"path": nested}
        });
        let mut collected = BTreeSet::new();
        named_files_in_value(&root, &named, &mut collected);
        assert_eq!(collected, paths);

        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn source_subject_absent_file_stamp_goes_stale_when_the_file_appears() -> Result<(), String> {
        let root = temp_root("appear")?;
        let paths = BTreeSet::from(["src/new.rs".to_string()]);
        let stamp = stamp_value(&root, &paths)?;
        assert_eq!(stamp["files"][0]["digest"], Value::Null);
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Current
        );
        std::fs::write(root.join("src/new.rs"), "fn c() {}\n").map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale("src/new.rs".to_string())
        );
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn source_subject_missing_malformed_or_incomplete_is_unverifiable() -> Result<(), String> {
        let root = temp_root("unverifiable")?;
        std::fs::write(root.join("src/lib.rs"), "fn a() {}\n").map_err(|err| err.to_string())?;
        let paths = BTreeSet::from(["src/lib.rs".to_string()]);
        assert_eq!(
            check_source_subject(&root, None, &paths),
            SourceSubjectCheck::Unverifiable("source_subject_missing")
        );
        assert_eq!(
            check_source_subject(&root, Some(&json!({"files": "x"})), &paths),
            SourceSubjectCheck::Unverifiable("source_subject_malformed")
        );
        assert_eq!(
            check_source_subject(
                &root,
                Some(&json!({"digest_algorithm": "md5", "files": []})),
                &paths
            ),
            SourceSubjectCheck::Unverifiable("source_subject_unsupported_digest")
        );
        assert_eq!(
            check_source_subject(
                &root,
                Some(&json!({"digest_algorithm": "sha256", "files": []})),
                &paths
            ),
            SourceSubjectCheck::Unverifiable("source_subject_incomplete")
        );
        assert_eq!(
            check_source_subject(
                &root,
                Some(&json!({
                    "digest_algorithm": "sha256",
                    "files": [{"path": "../outside.rs", "digest": null}]
                })),
                &BTreeSet::new()
            ),
            SourceSubjectCheck::Unverifiable("source_subject_malformed")
        );
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn subject_paths_name_the_anchor_route_and_packet_files() {
        let root = Path::new("/repo");
        let record = json!({
            "anchor": {"file": "src/pricing.rs", "line": 42},
            "repair_route": {
                "target_file": "tests/pricing.rs",
                "related_test": "tests/pricing.rs::discount_threshold"
            }
        });
        assert_eq!(
            gap_record_subject_paths(root, &record),
            BTreeSet::from(["src/pricing.rs".to_string(), "tests/pricing.rs".to_string()])
        );
        let packet = json!({
            "source_file": "src/pricing.rs",
            "primary_anchor": {"file": "src/lib.rs", "line": 3}
        });
        assert_eq!(
            actionable_packet_subject_paths(root, &packet),
            BTreeSet::from(["src/lib.rs".to_string(), "src/pricing.rs".to_string()])
        );
    }

    #[test]
    fn packet_subject_paths_cover_every_related_test_or_observer_shape() {
        let root = Path::new("/repo");
        for (shape, observer) in [
            ("string", json!("tests/observer.rs::observes")),
            (
                "object",
                json!({"file": "tests/observer.rs", "test": "observes"}),
            ),
            (
                "array",
                json!([
                    "not a path",
                    {"related_test": "tests/observer.rs::observes"},
                    ["tests/nested.rs::deeper"]
                ]),
            ),
        ] {
            let packet = json!({
                "source_file": "src/pricing.rs",
                "target_test": "tests/pricing.rs::discount_threshold",
                "target_file": "tests/pricing_extra.rs",
                "related_test_or_observer": observer
            });
            let paths = actionable_packet_subject_paths(root, &packet);
            assert!(paths.contains("tests/observer.rs"), "{shape}: {paths:?}");
            assert!(paths.contains("src/pricing.rs"), "{shape}: {paths:?}");
            assert!(paths.contains("tests/pricing.rs"), "{shape}: {paths:?}");
            assert!(
                paths.contains("tests/pricing_extra.rs"),
                "{shape}: {paths:?}"
            );
            assert_eq!(
                shape == "array",
                paths.contains("tests/nested.rs"),
                "{shape}"
            );
        }
    }

    #[test]
    fn absolute_paths_resolve_against_a_relative_root() -> Result<(), String> {
        // `cargo test` runs in the crate directory, which holds `src/lib.rs`.
        let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
        let absolute = cwd.join("src/lib.rs").display().to_string();
        assert_eq!(
            subject_relative_path(Path::new("."), &absolute).as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            subject_relative_path(Path::new("src/.."), &absolute).as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            subject_relative_path(Path::new("src"), &absolute).as_deref(),
            Some("lib.rs")
        );
        Ok(())
    }

    #[test]
    fn derive_copies_digests_and_never_hashes() -> Result<(), String> {
        let input = json!({
            "digest_algorithm": "sha256",
            "files": [
                {"path": "src/lib.rs", "digest": "sha256:aa"},
                {"path": "tests/it.rs", "digest": null},
                {"path": "src/unused.rs", "digest": "sha256:bb"}
            ]
        });
        let required = BTreeSet::from(["src/lib.rs".to_string(), "tests/it.rs".to_string()]);
        // `/nonexistent` cannot be read, so a copied digest cannot come from hashing.
        let root = Path::new("/nonexistent-ripr-root");
        let derived = derive_source_subject(Some(&input), root, root, &required)?;
        assert_eq!(
            serde_json::to_value(&derived).map_err(|err| err.to_string())?,
            json!({
                "digest_algorithm": "sha256",
                "files": [
                    {"path": "src/lib.rs", "digest": "sha256:aa"},
                    {"path": "tests/it.rs", "digest": null}
                ]
            })
        );
        // An input produced under a parent root is rebased onto the output root.
        let rebased = derive_source_subject(
            Some(&json!({
                "digest_algorithm": "sha256",
                "files": [{"path": "crate/src/lib.rs", "digest": "sha256:cc"}]
            })),
            Path::new("/nonexistent-ripr-root"),
            Path::new("/nonexistent-ripr-root/crate"),
            &BTreeSet::from(["src/lib.rs".to_string()]),
        )?;
        assert_eq!(rebased.files[0].digest.as_deref(), Some("sha256:cc"));

        for (stamp, reason) in [
            (None, "input_source_subject_missing"),
            (Some(json!({"files": 1})), "input_source_subject_malformed"),
            (
                Some(json!({"digest_algorithm": "md5", "files": []})),
                "input_source_subject_unsupported_digest",
            ),
            (
                Some(json!({"digest_algorithm": "sha256", "files": [
                    {"path": "src/lib.rs", "digest": "sha256:aa"}
                ]})),
                "input_source_subject_incomplete",
            ),
        ] {
            assert_eq!(
                derive_source_subject(stamp.as_ref(), root, root, &required),
                Err(reason)
            );
        }
        assert_eq!(
            derive_source_subject(None, root, root, &BTreeSet::new()).map(|s| s.files.len()),
            Ok(0)
        );

        let whitespace = json!({
            "digest_algorithm": "sha256",
            "files": [{"path": " leading.py", "digest": "sha256:aa"}]
        });
        let required_whitespace = BTreeSet::from([" leading.py".to_string()]);
        let derived_whitespace =
            derive_source_subject(Some(&whitespace), root, root, &required_whitespace)?;
        assert_eq!(derived_whitespace.files[0].path, " leading.py");
        assert_eq!(
            derived_whitespace.files[0].digest.as_deref(),
            Some("sha256:aa")
        );
        Ok(())
    }

    #[test]
    fn append_source_subject_member_stamps_named_files_or_leaves_the_document() -> Result<(), String>
    {
        let root = temp_root("append")?;
        std::fs::write(root.join("src/lib.rs"), "abc\n").map_err(|err| err.to_string())?;
        let paths = BTreeSet::from(["src/lib.rs".to_string()]);
        let rendered = "{\n  \"schema_version\": \"0.1\"\n}\n".to_string();
        let stamped = append_source_subject_member(rendered.clone(), &root, &paths);
        let value: Value = serde_json::from_str(&stamped).map_err(|err| err.to_string())?;
        assert_eq!(value["schema_version"], json!("0.1"));
        assert_eq!(value["source_subject"], stamp_value(&root, &paths)?);
        assert!(stamped.ends_with("}\n"));
        assert_eq!(
            append_source_subject_member(rendered.clone(), &root, &BTreeSet::new()),
            rendered
        );
        assert_eq!(
            append_source_subject_member("[]".to_string(), &root, &paths),
            "[]"
        );

        let mut named = BTreeSet::new();
        named_files_in_value(
            &root,
            &json!({
                "file": "src/lib.rs",
                "evidence": [{"related_test": "tests/it.rs::case", "name": "src/ignored.rs"}],
                "related_tests": [{"test": "not a path"}]
            }),
            &mut named,
        );
        assert_eq!(
            named,
            BTreeSet::from(["src/lib.rs".to_string(), "tests/it.rs".to_string()])
        );
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }
}
