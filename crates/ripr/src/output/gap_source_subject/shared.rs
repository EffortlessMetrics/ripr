//! The part of the gap `source_subject` contract that both the ripr crate and
//! the unpublished `xtask` actionable-gaps writer need (#4544).
//!
//! `xtask` compiles this exact file through a `#[path]` module declaration,
//! so it must stay self-contained: no `crate::` paths, only `serde`,
//! `serde_json`, and `std`. Keeping one definition here means the xtask
//! writer and the LSP validator cannot disagree about which files a packet
//! names or how a path is spelled in a stamp.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

/// The only digest algorithm a `source_subject` stamp may declare.
pub(crate) const SOURCE_SUBJECT_DIGEST_ALGORITHM: &str = "sha256";

/// Artifact-level source-state identity: one entry per workspace file the
/// artifact names, sorted by path.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct GapSourceSubject {
    pub(crate) digest_algorithm: String,
    pub(crate) files: Vec<GapSourceSubjectFile>,
}

/// One stamped workspace file. `digest` is `sha256:<hex>` of the file bytes
/// the analysis run saw, or `null` when the named file was absent.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct GapSourceSubjectFile {
    pub(crate) path: String,
    pub(crate) digest: Option<String>,
}

/// `root` as an absolute, lexically normalized path. A relative root is
/// resolved against the process working directory, which is how the CLI
/// resolves `--root .`.
pub(crate) fn absolute_root(root: &Path) -> PathBuf {
    let joined = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(root))
            .unwrap_or_else(|_| root.to_path_buf())
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Normalize a path an artifact names to the repo-relative spelling the stamp
/// uses, or `None` when it does not name a workspace file:
///
/// - a `path::test_name` selector keeps its file part;
/// - the file part must look like a file (its last segment has an extension),
///   so a bare test or observer name is not mistaken for a path;
/// - an absolute path must lie inside `root` (compared against the absolute
///   and the canonical root, so `--root .` and symlinked roots both work);
/// - a relative path is root-relative, except that ripr's own display form
///   `<relative root>/<file>` is recognized when `root` is itself relative;
/// - traversal (`..`) is rejected.
pub(crate) fn subject_relative_path(root: &Path, raw: &str) -> Option<String> {
    let file = raw.split_once("::").map_or(raw, |(file, _)| file).trim();
    if file.is_empty() || file.chars().any(char::is_whitespace) {
        return None;
    }
    let normalized = file.replace('\\', "/");
    let path = Path::new(&normalized);
    let relative = if path.is_absolute() {
        strip_absolute_root(root, path)?
    } else {
        strip_relative_root_prefix(root, path)
    };
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if !parts.last()?.contains('.') {
        return None;
    }
    Some(parts.join("/"))
}

fn strip_absolute_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let absolute = absolute_root(root);
    if let Ok(relative) = path.strip_prefix(&absolute) {
        return Some(relative.to_path_buf());
    }
    let canonical_root = std::fs::canonicalize(&absolute).ok()?;
    if let Ok(relative) = path.strip_prefix(&canonical_root) {
        return Some(relative.to_path_buf());
    }
    let canonical_path = std::fs::canonicalize(path).ok()?;
    canonical_path
        .strip_prefix(&canonical_root)
        .ok()
        .map(Path::to_path_buf)
}

fn strip_relative_root_prefix(root: &Path, path: &Path) -> PathBuf {
    if root.is_absolute() {
        return path.to_path_buf();
    }
    let root_parts = root
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect::<Vec<_>>();
    if root_parts.is_empty() {
        return path.to_path_buf();
    }
    let path_parts = path
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect::<Vec<_>>();
    if path_parts.len() > root_parts.len() && path_parts.starts_with(&root_parts) {
        return path_parts[root_parts.len()..].iter().collect();
    }
    path.to_path_buf()
}

/// Every path-bearing string an actionable-gaps packet names, in every shape
/// the LSP accepts: `source_file`, `target_test`, `target_file`,
/// `primary_anchor.file`, and `related_test_or_observer` as a string
/// (`tests/x.rs::name`), an object (`file`, `path`, `related_test`, `test`,
/// `target_file`), or an array of either. This is the single definition of
/// "files a packet names" for the LSP path validator, the stamp check, and
/// the xtask stamp writer.
pub(crate) fn actionable_packet_named_paths(packet: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    for key in ["source_file", "target_test", "target_file"] {
        push_string(packet.get(key), &mut paths);
    }
    push_string(
        packet
            .get("primary_anchor")
            .and_then(|anchor| anchor.get("file")),
        &mut paths,
    );
    if let Some(observer) = packet.get("related_test_or_observer") {
        push_observer_paths(observer, &mut paths);
    }
    paths
}

fn push_observer_paths(value: &Value, paths: &mut Vec<String>) {
    match value {
        Value::String(_) => push_string(Some(value), paths),
        Value::Object(object) => {
            for key in ["file", "path", "related_test", "test", "target_file"] {
                push_string(object.get(key), paths);
            }
        }
        Value::Array(values) => {
            for value in values {
                push_observer_paths(value, paths);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn push_string(value: Option<&Value>, paths: &mut Vec<String>) {
    if let Some(text) = value.and_then(Value::as_str)
        && !text.trim().is_empty()
    {
        paths.push(text.to_string());
    }
}

/// Repo-relative files an actionable-gaps packet makes claims about.
pub(crate) fn actionable_packet_subject_paths(root: &Path, packet: &Value) -> BTreeSet<String> {
    actionable_packet_named_paths(packet)
        .iter()
        .filter_map(|raw| subject_relative_path(root, raw))
        .collect()
}

/// Derive a derived artifact's stamp from the analysis output it was built
/// from. Every `required` file (spelled relative to `output_root`) must be in
/// the input's stamp (spelled relative to `input_root`); its digest is copied.
/// Nothing is hashed here: a derived artifact may only vouch for the bytes
/// the analysis saw. The error is the reason the stamp is unavailable.
pub(crate) fn derive_source_subject(
    input_stamp: Option<&Value>,
    input_root: &Path,
    output_root: &Path,
    required: &BTreeSet<String>,
) -> Result<GapSourceSubject, &'static str> {
    if required.is_empty() {
        return Ok(GapSourceSubject {
            digest_algorithm: SOURCE_SUBJECT_DIGEST_ALGORITHM.to_string(),
            files: Vec::new(),
        });
    }
    let stamp = input_stamp.ok_or("input_source_subject_missing")?;
    let Ok(subject) = serde_json::from_value::<GapSourceSubject>(stamp.clone()) else {
        return Err("input_source_subject_malformed");
    };
    if subject.digest_algorithm != SOURCE_SUBJECT_DIGEST_ALGORITHM {
        return Err("input_source_subject_unsupported_digest");
    }
    let input_absolute = absolute_root(input_root);
    let mut digests = BTreeMap::new();
    for file in subject.files {
        if subject_relative_path(input_root, &file.path).as_deref() != Some(file.path.as_str()) {
            return Err("input_source_subject_malformed");
        }
        let absolute = input_absolute.join(&file.path);
        if let Some(path) = subject_relative_path(output_root, &absolute.to_string_lossy()) {
            digests.insert(path, file.digest);
        }
    }
    let files = required
        .iter()
        .map(|path| {
            digests
                .get(path)
                .map(|digest| GapSourceSubjectFile {
                    path: path.clone(),
                    digest: digest.clone(),
                })
                .ok_or("input_source_subject_incomplete")
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(GapSourceSubject {
        digest_algorithm: SOURCE_SUBJECT_DIGEST_ALGORITHM.to_string(),
        files,
    })
}
