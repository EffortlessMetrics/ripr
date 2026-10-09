//! Existing retained-fixture path, byte-count and digest checks.

use std::fs;
use std::path::{Component, Path};

use serde_json::Value;
use sha2::{Digest, Sha256};

#[cfg(test)]
mod tests;

pub(super) fn local_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

pub(super) fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

pub(super) fn verify_file(root: &Path, entry: &Value) -> Result<(), String> {
    let path = entry["path"]
        .as_str()
        .filter(|path| local_path(path))
        .ok_or_else(|| "upstream evidence needs a contained relative file path".to_string())?;
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("resolve evidence root {}: {error}", root.display()))?;
    let candidate = fs::canonicalize(root.join(path))
        .map_err(|error| format!("resolve evidence file {path}: {error}"))?;
    if !candidate.starts_with(&canonical_root) {
        return Err(format!(
            "upstream evidence path escapes fixture root: {path}"
        ));
    }
    let bytes = fs::read(&candidate).map_err(|error| format!("{path}: {error}"))?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if entry["sha256"].as_str() != Some(actual.as_str()) {
        return Err(format!("upstream evidence digest mismatch: {path}"));
    }
    if entry["bytes"].as_u64() != Some(bytes.len() as u64) {
        return Err(format!("upstream evidence size mismatch: {path}"));
    }
    Ok(())
}
