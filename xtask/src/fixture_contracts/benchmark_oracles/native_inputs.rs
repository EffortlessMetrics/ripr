//! Retained logical workspace inputs, never a live historical filesystem walk.

use std::collections::BTreeMap;

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Input {
    kind: String,
    bytes: u64,
    sha256: String,
}

fn identity(value: &Value, kind: &str) -> Result<Input, String> {
    let digest = text(value, "sha256")?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("native input digest must contain 64 hexadecimal characters".to_string());
    }
    Ok(Input {
        kind: kind.to_string(),
        bytes: value["bytes"]
            .as_u64()
            .ok_or("native input needs a byte count")?,
        sha256: digest.to_string(),
    })
}

fn entries(value: &Value, kind_field: &str) -> Result<BTreeMap<String, Input>, String> {
    let rows = value
        .as_array()
        .filter(|rows| !rows.is_empty())
        .ok_or("native input inventory must be a nonempty array")?;
    let mut result = BTreeMap::new();
    for row in rows {
        let path = text(row, "path")?;
        let kind = text(row, kind_field)?;
        if !local_path(path) || !matches!(kind, "file" | "symlink") {
            return Err(
                "native input needs a contained logical path and file/symlink kind".to_string(),
            );
        }
        if result
            .insert(path.to_string(), identity(row, kind)?)
            .is_some()
        {
            return Err(format!("native input repeats logical path {path}"));
        }
    }
    Ok(result)
}

pub(super) fn validate(
    root: &Path,
    key: &Value,
    pairing: &Value,
    production: &str,
    test: &str,
    before: &Value,
    after: &Value,
) -> Result<(), String> {
    let before = retained_json(root, before)?;
    let after = retained_json(root, after)?;
    if before != after {
        return Err("native full-workspace input fence changed".to_string());
    }
    let actual = entries(&before, "kind")?;
    let production_path = text(key, "production_source_path")?;
    let test_path = text(key, "test_source_path")?;
    if production_path == test_path || production_path == "Cargo.lock" || test_path == "Cargo.lock"
    {
        return Err("native source, test and lock paths must be distinct".to_string());
    }
    let substitutions = [
        (
            production_path,
            identity(&key["sources"][production], "file")?,
        ),
        (test_path, identity(&key["tests"][test], "file")?),
        ("Cargo.lock", identity(&pairing["lock"], "file")?),
    ];
    for (path, expected) in &substitutions {
        if actual.get(*path) != Some(expected) {
            return Err(format!(
                "native input has wrong source/test/lock identity at {path}"
            ));
        }
    }
    selected_files(&actual, key)?;
    if let Some(workspace) = pairing.get("original_workspace") {
        let parent = retained_json(root, &workspace["inventories"]["parent"])?;
        let mut expected = entries(&parent, "type")?;
        for path in [production_path, test_path] {
            if !expected.contains_key(path) {
                return Err(format!(
                    "original parent inventory is missing substituted path {path}"
                ));
            }
        }
        for (path, identity) in substitutions {
            let _ = expected.insert(path.to_string(), identity);
        }
        if actual != expected {
            return Err(
                "native inputs differ from the complete parent/lock/source/test footprint"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn selected_files(actual: &BTreeMap<String, Input>, subject: &Value) -> Result<(), String> {
    for field in ["package_manifest_path", "library_source_path"] {
        let path = text(subject, field)?;
        if actual.get(path).is_none_or(|entry| entry.kind != "file") {
            return Err(format!("native input is missing selected {field}: {path}"));
        }
    }
    Ok(())
}

pub(super) fn validate_selected_subject(
    root: &Path,
    subject: &Value,
    descriptor: &Value,
) -> Result<(), String> {
    let value = retained_json(root, descriptor)?;
    selected_files(&entries(&value, "kind")?, subject)
}
