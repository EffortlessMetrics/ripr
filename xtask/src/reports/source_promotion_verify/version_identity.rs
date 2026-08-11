use serde_json::Value as JsonValue;
use std::path::Path;
use toml::Value as TomlValue;

use super::git_bytes;

pub(super) const RELEASE_METADATA_SURFACES: &[&str] = &[
    "Cargo.toml",
    "crates/ripr/Cargo.toml",
    "Cargo.lock",
    "editors/vscode/package.json",
    "editors/vscode/package-lock.json",
    "CHANGELOG.md",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseVersionIdentity {
    crate_version: String,
    cargo_lock_ripr_version: String,
    extension_version: String,
    npm_lock_root_version: String,
}

pub(super) fn verify_release_metadata_identity(
    repo: &Path,
    join: &str,
    source: &str,
) -> Result<(), String> {
    let source_identity = read_identity(repo, source)?;
    let join_identity = read_identity(repo, join)?;
    compare_identities(&source_identity, &join_identity)?;

    let source_changelog = git_bytes(repo, &["show", &format!("{source}:CHANGELOG.md")])?;
    let join_changelog = git_bytes(repo, &["show", &format!("{join}:CHANGELOG.md")])?;
    if source_changelog != join_changelog {
        return Err(
            "release metadata identity failed: CHANGELOG.md differs from source parent"
                .to_string(),
        );
    }
    Ok(())
}

fn read_identity(repo: &Path, commit: &str) -> Result<ReleaseVersionIdentity, String> {
    identity_from_documents(
        &read_utf8(repo, commit, "Cargo.toml")?,
        &read_utf8(repo, commit, "crates/ripr/Cargo.toml")?,
        &read_utf8(repo, commit, "Cargo.lock")?,
        &read_utf8(repo, commit, "editors/vscode/package.json")?,
        &read_utf8(repo, commit, "editors/vscode/package-lock.json")?,
    )
}

fn read_utf8(repo: &Path, commit: &str, path: &str) -> Result<String, String> {
    let bytes = git_bytes(repo, &["show", &format!("{commit}:{path}")])?;
    String::from_utf8(bytes).map_err(|error| format!("{path} at {commit} is not UTF-8: {error}"))
}

fn identity_from_documents(
    workspace_manifest: &str,
    crate_manifest: &str,
    cargo_lock: &str,
    package_json: &str,
    package_lock: &str,
) -> Result<ReleaseVersionIdentity, String> {
    let workspace: TomlValue = toml::from_str(workspace_manifest)
        .map_err(|error| format!("Cargo.toml is malformed: {error}"))?;
    let crate_value: TomlValue = toml::from_str(crate_manifest)
        .map_err(|error| format!("crates/ripr/Cargo.toml is malformed: {error}"))?;
    let lock_value: TomlValue =
        toml::from_str(cargo_lock).map_err(|error| format!("Cargo.lock is malformed: {error}"))?;
    let package: JsonValue = serde_json::from_str(package_json)
        .map_err(|error| format!("editors/vscode/package.json is malformed: {error}"))?;
    let npm_lock: JsonValue = serde_json::from_str(package_lock)
        .map_err(|error| format!("editors/vscode/package-lock.json is malformed: {error}"))?;

    Ok(ReleaseVersionIdentity {
        crate_version: effective_crate_version(&workspace, &crate_value)?,
        cargo_lock_ripr_version: cargo_lock_ripr_version(&lock_value)?,
        extension_version: json_version(&package, "editors/vscode/package.json")?,
        npm_lock_root_version: npm_lock_root_version(&npm_lock)?,
    })
}

fn effective_crate_version(
    workspace: &TomlValue,
    crate_manifest: &TomlValue,
) -> Result<String, String> {
    let package = crate_manifest
        .get("package")
        .and_then(TomlValue::as_table)
        .ok_or_else(|| "crates/ripr/Cargo.toml is missing [package]".to_string())?;
    let version = package
        .get("version")
        .ok_or_else(|| "crates/ripr/Cargo.toml is missing package.version".to_string())?;
    if let Some(value) = version.as_str() {
        return nonempty_version(value, "crates/ripr/Cargo.toml package.version");
    }
    let inherits = version
        .as_table()
        .and_then(|table| table.get("workspace"))
        .and_then(TomlValue::as_bool);
    if inherits != Some(true) {
        return Err(
            "crates/ripr/Cargo.toml package.version must be a string or workspace = true"
                .to_string(),
        );
    }
    let workspace_version = workspace
        .get("workspace")
        .and_then(TomlValue::as_table)
        .and_then(|table| table.get("package"))
        .and_then(TomlValue::as_table)
        .and_then(|table| table.get("version"))
        .and_then(TomlValue::as_str)
        .ok_or_else(|| {
            "Cargo.toml is missing workspace.package.version required by version.workspace"
                .to_string()
        })?;
    nonempty_version(workspace_version, "Cargo.toml workspace.package.version")
}

fn cargo_lock_ripr_version(lock: &TomlValue) -> Result<String, String> {
    let packages = lock
        .get("package")
        .and_then(TomlValue::as_array)
        .ok_or_else(|| "Cargo.lock is missing [[package]] entries".to_string())?;
    let mut versions = packages.iter().filter_map(|package| {
        let table = package.as_table()?;
        if table.get("name").and_then(TomlValue::as_str) != Some("ripr") {
            return None;
        }
        table
            .get("version")
            .and_then(TomlValue::as_str)
            .map(str::to_string)
    });
    let first = versions
        .next()
        .ok_or_else(|| "Cargo.lock is missing the ripr package version".to_string())?;
    if versions.next().is_some() {
        return Err("Cargo.lock contains multiple ripr package versions".to_string());
    }
    nonempty_version(&first, "Cargo.lock ripr package version")
}

fn json_version(value: &JsonValue, path: &str) -> Result<String, String> {
    let version = value
        .get("version")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{path} is missing string field version"))?;
    nonempty_version(version, &format!("{path} version"))
}

fn npm_lock_root_version(value: &JsonValue) -> Result<String, String> {
    let root = json_version(value, "editors/vscode/package-lock.json root")?;
    let package_root = value
        .get("packages")
        .and_then(JsonValue::as_object)
        .and_then(|packages| packages.get(""))
        .and_then(JsonValue::as_object)
        .and_then(|package| package.get("version"))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            "editors/vscode/package-lock.json is missing packages[\"\"] version".to_string()
        })?;
    let package_root = nonempty_version(
        package_root,
        "editors/vscode/package-lock.json packages[\"\"] version",
    )?;
    if root != package_root {
        return Err(format!(
            "editors/vscode/package-lock.json root version {root} disagrees with packages[\"\"] version {package_root}"
        ));
    }
    Ok(root)
}

fn nonempty_version(value: &str, field: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err(format!("{field} is empty"))
    } else {
        Ok(value.to_string())
    }
}

fn compare_identities(
    source: &ReleaseVersionIdentity,
    join: &ReleaseVersionIdentity,
) -> Result<(), String> {
    let fields = [
        (
            "effective ripr crate version",
            &source.crate_version,
            &join.crate_version,
        ),
        (
            "Cargo.lock ripr package version",
            &source.cargo_lock_ripr_version,
            &join.cargo_lock_ripr_version,
        ),
        (
            "VS Code package version",
            &source.extension_version,
            &join.extension_version,
        ),
        (
            "npm lock root version",
            &source.npm_lock_root_version,
            &join.npm_lock_root_version,
        ),
    ];
    for (field, expected, actual) in fields {
        if expected != actual {
            return Err(format!(
                "release metadata identity failed: {field} changed from {expected} to {actual}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPLICIT_WORKSPACE: &str = "[workspace]\nmembers = [\"crates/ripr\"]\n";
    const INHERITED_WORKSPACE: &str =
        "[workspace]\nmembers = [\"crates/ripr\"]\n[workspace.package]\nversion = \"0.10.1\"\n";
    const EXPLICIT_CRATE: &str =
        "[package]\nname = \"ripr\"\nversion = \"0.10.1\"\n[dependencies]\nserde = \"1\"\n";
    const INHERITED_CRATE: &str =
        "[package]\nname = \"ripr\"\nversion.workspace = true\n[dependencies]\nrayon = \"1\"\n";
    const LOCK: &str = "version = 3\n[[package]]\nname = \"ripr\"\nversion = \"0.10.1\"\ndependencies = [\"rayon\"]\n";
    const PACKAGE: &str =
        "{\"name\":\"ripr\",\"version\":\"0.10.1\",\"scripts\":{\"compile\":\"tsc\"}}";
    const PACKAGE_LOCK: &str =
        "{\"name\":\"ripr\",\"version\":\"0.10.1\",\"packages\":{\"\":{\"name\":\"ripr\",\"version\":\"0.10.1\",\"dependencies\":{\"vscode-languageclient\":\"^9\"}}}}";

    #[test]
    fn dependency_and_workspace_layout_changes_preserve_version_identity() -> Result<(), String> {
        let explicit = identity_from_documents(
            EXPLICIT_WORKSPACE,
            EXPLICIT_CRATE,
            LOCK,
            PACKAGE,
            PACKAGE_LOCK,
        )?;
        let inherited = identity_from_documents(
            INHERITED_WORKSPACE,
            INHERITED_CRATE,
            LOCK,
            PACKAGE,
            PACKAGE_LOCK,
        )?;
        if explicit != inherited {
            return Err(format!(
                "equivalent explicit/workspace version identities diverged: {explicit:?} vs {inherited:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn each_governed_version_field_is_compared() -> Result<(), String> {
        let source = identity_from_documents(
            EXPLICIT_WORKSPACE,
            EXPLICIT_CRATE,
            LOCK,
            PACKAGE,
            PACKAGE_LOCK,
        )?;
        let mut variants = Vec::new();
        let mut crate_version = source.clone();
        crate_version.crate_version = "0.10.2".to_string();
        variants.push(("effective ripr crate version", crate_version));
        let mut lock_version = source.clone();
        lock_version.cargo_lock_ripr_version = "0.10.2".to_string();
        variants.push(("Cargo.lock ripr package version", lock_version));
        let mut extension = source.clone();
        extension.extension_version = "0.10.2".to_string();
        variants.push(("VS Code package version", extension));
        let mut npm = source.clone();
        npm.npm_lock_root_version = "0.10.2".to_string();
        variants.push(("npm lock root version", npm));

        for (field, variant) in variants {
            let error = compare_identities(&source, &variant)
                .err()
                .ok_or_else(|| format!("{field} mutation was accepted"))?;
            if !error.contains(field) {
                return Err(format!("{field} mismatch did not name its field: {error}"));
            }
        }
        Ok(())
    }

    #[test]
    fn npm_lock_root_and_package_root_must_agree() -> Result<(), String> {
        let mismatched =
            "{\"name\":\"ripr\",\"version\":\"0.10.1\",\"packages\":{\"\":{\"name\":\"ripr\",\"version\":\"0.10.2\"}}}";
        let error = identity_from_documents(
            EXPLICIT_WORKSPACE,
            EXPLICIT_CRATE,
            LOCK,
            PACKAGE,
            mismatched,
        )
        .err()
        .ok_or_else(|| "mismatched npm lock root versions were accepted".to_string())?;
        if !error.contains("disagrees") {
            return Err(format!("npm mismatch reason was not specific: {error}"));
        }
        Ok(())
    }

    #[test]
    fn cargo_lock_requires_one_ripr_package_version() -> Result<(), String> {
        let missing = "version = 3\n[[package]]\nname = \"other\"\nversion = \"1.0.0\"\n";
        if identity_from_documents(
            EXPLICIT_WORKSPACE,
            EXPLICIT_CRATE,
            missing,
            PACKAGE,
            PACKAGE_LOCK,
        )
        .is_ok()
        {
            return Err("Cargo.lock without ripr was accepted".to_string());
        }
        let duplicate = format!("{LOCK}[[package]]\nname = \"ripr\"\nversion = \"0.10.1\"\n");
        if identity_from_documents(
            EXPLICIT_WORKSPACE,
            EXPLICIT_CRATE,
            &duplicate,
            PACKAGE,
            PACKAGE_LOCK,
        )
        .is_ok()
        {
            return Err("Cargo.lock with duplicate ripr versions was accepted".to_string());
        }
        Ok(())
    }
}
