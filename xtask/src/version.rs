use std::fs;
use std::path::Path;
use std::process::Command;

const WORKSPACE_MANIFEST: &str = "Cargo.toml";
const EXTENSION_MANIFEST: &str = "editors/vscode/package.json";
const EXTENSION_LOCKFILE: &str = "editors/vscode/package-lock.json";

pub(crate) fn bump_version(args: &[String]) -> Result<(), String> {
    let [version] = args else {
        return Err("Usage: cargo xtask bump-version <version>".to_string());
    };
    validate_version_input(version)?;

    let workspace = read_file(WORKSPACE_MANIFEST)?;
    let extension = read_file(EXTENSION_MANIFEST)?;
    let lockfile = read_file(EXTENSION_LOCKFILE)?;
    let update = prepare_update(&workspace, &extension, &lockfile, version)?;

    let paths = [
        Path::new(WORKSPACE_MANIFEST),
        Path::new(EXTENSION_MANIFEST),
        Path::new(EXTENSION_LOCKFILE),
    ];
    let originals = [&workspace, &extension, &lockfile];
    let updated = [&update.workspace, &update.extension, &update.lockfile];

    for (index, path) in paths.iter().enumerate() {
        if let Err(error) = fs::write(path, updated[index]) {
            let rollback = restore_files(&paths, &originals);
            return Err(format_write_failure(path, error, rollback));
        }
    }

    if let Err(error) = cargo_metadata_validation() {
        let rollback = restore_files(&paths, &originals);
        return Err(format!(
            "version bump was rolled back because Cargo validation failed: {error}{rollback}"
        ));
    }

    println!(
        "updated release version {} -> {} in {WORKSPACE_MANIFEST}, {EXTENSION_MANIFEST}, and {EXTENSION_LOCKFILE}",
        update.old_version, version
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct VersionUpdate {
    old_version: String,
    workspace: String,
    extension: String,
    lockfile: String,
}

fn prepare_update(
    workspace: &str,
    extension: &str,
    lockfile: &str,
    new_version: &str,
) -> Result<VersionUpdate, String> {
    let current_workspace = section_version(workspace, "workspace.package")?;
    let current_extension = json_root_version(extension, EXTENSION_MANIFEST)?;
    let current_lockfile = lockfile_versions(lockfile)?;

    if current_extension != current_workspace || current_lockfile != current_workspace {
        return Err(format!(
            "release version drift detected before bump: workspace={current_workspace}, package.json={current_extension}, package-lock.json={current_lockfile}"
        ));
    }
    if new_version == current_workspace {
        return Err(format!("release version is already {current_workspace}"));
    }

    let updated_workspace = replace_section_version(workspace, "workspace.package", new_version)?;
    let updated_extension =
        replace_json_version_lines(extension, &current_workspace, new_version, 1)?;
    let updated_lockfile =
        replace_json_version_lines(lockfile, &current_workspace, new_version, 2)?;

    verify_updated_json(&updated_extension, EXTENSION_MANIFEST, new_version, 1)?;
    verify_updated_lockfile(&updated_lockfile, new_version)?;

    Ok(VersionUpdate {
        old_version: current_workspace,
        workspace: updated_workspace,
        extension: updated_extension,
        lockfile: updated_lockfile,
    })
}

fn validate_version_input(version: &str) -> Result<(), String> {
    if version.is_empty()
        || version.trim() != version
        || version.chars().any(char::is_whitespace)
        || version
            .chars()
            .any(|character| matches!(character, '"' | '\'' | '\r' | '\n'))
    {
        return Err(format!(
            "invalid release version {version:?}; provide a valid Cargo-compatible version such as 0.11.0"
        ));
    }
    Ok(())
}

fn section_version(text: &str, section: &str) -> Result<String, String> {
    let header = format!("[{section}]");
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = strip_toml_comment(line).trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == header;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() != "version" {
            continue;
        }
        let value = unquote(value.trim())
            .ok_or_else(|| format!("{section}.version must be a quoted string, got {value:?}"))?;
        if value.is_empty() {
            return Err(format!("{section}.version must not be empty"));
        }
        return Ok(value.to_string());
    }
    Err(format!("[{section}] has no usable version field"))
}

fn replace_section_version(text: &str, section: &str, new_version: &str) -> Result<String, String> {
    let header = format!("[{section}]");
    let mut in_section = false;
    let mut replaced = false;
    let mut output = String::with_capacity(text.len() + new_version.len());

    for line in text.split_inclusive('\n') {
        let trimmed = strip_toml_comment(line).trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == header;
        }
        if in_section && !replaced && trimmed.starts_with("version") {
            let Some((key, value)) = trimmed.split_once('=') else {
                return Err(format!("malformed {section}.version line"));
            };
            if key.trim() == "version" {
                let value_start = line
                    .find(value.trim())
                    .ok_or_else(|| format!("could not locate {section}.version value in source"))?;
                let quoted = value.trim();
                let old = unquote(quoted).ok_or_else(|| {
                    format!("{section}.version must be a quoted string, got {quoted:?}")
                })?;
                let quote_start = value_start
                    + quoted.find(old).ok_or_else(|| {
                        format!("could not locate {section}.version string in source")
                    })?;
                let quote_end = quote_start + old.len();
                output.push_str(&line[..quote_start]);
                output.push_str(new_version);
                output.push_str(&line[quote_end..]);
                replaced = true;
                continue;
            }
        }
        output.push_str(line);
    }

    if !replaced {
        return Err(format!("[{section}] version field was not replaced"));
    }
    Ok(output)
}

fn json_root_version(text: &str, path: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("{path} is not valid JSON: {error}"))?;
    value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|version| !version.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{path} has no usable root version field"))
}

fn lockfile_versions(text: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("{EXTENSION_LOCKFILE} is not valid JSON: {error}"))?;
    let top = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| format!("{EXTENSION_LOCKFILE} has no usable root version field"))?;
    let package = value
        .get("packages")
        .and_then(|packages| packages.get(""))
        .and_then(|root| root.get("version"))
        .and_then(serde_json::Value::as_str)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| format!("{EXTENSION_LOCKFILE} has no usable packages[\"\"].version"))?;
    if top != package {
        return Err(format!(
            "{EXTENSION_LOCKFILE} root and packages[\"\"] versions differ: {top} != {package}"
        ));
    }
    Ok(top.to_string())
}

fn replace_json_version_lines(
    text: &str,
    old_version: &str,
    new_version: &str,
    expected_count: usize,
) -> Result<String, String> {
    let mut output = String::with_capacity(text.len() + expected_count * new_version.len());
    let mut replaced = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if replaced < expected_count && trimmed.starts_with("\"version\"") {
            let colon = line
                .find(':')
                .ok_or_else(|| "version field line has no colon in JSON manifest".to_string())?;
            let value_start = line[colon + 1..]
                .find('"')
                .map(|offset| colon + 1 + offset + 1)
                .ok_or_else(|| "version field line has no quoted value".to_string())?;
            let value_end = line[value_start..]
                .find('"')
                .map(|offset| value_start + offset)
                .ok_or_else(|| "version field line has no closing quote".to_string())?;
            if &line[value_start..value_end] != old_version {
                return Err(format!(
                    "expected JSON version {old_version:?}, found {:?}",
                    &line[value_start..value_end]
                ));
            }
            output.push_str(&line[..value_start]);
            output.push_str(new_version);
            output.push_str(&line[value_end..]);
            replaced += 1;
        } else {
            output.push_str(line);
        }
    }
    if replaced != expected_count {
        return Err(format!(
            "expected {expected_count} JSON version fields, replaced {replaced}"
        ));
    }
    Ok(output)
}

fn verify_updated_json(
    text: &str,
    path: &str,
    expected: &str,
    version_fields: usize,
) -> Result<(), String> {
    let actual = json_root_version(text, path)?;
    if actual != expected {
        return Err(format!("{path} version did not update to {expected}"));
    }
    let count = text
        .lines()
        .filter(|line| line.trim_start().starts_with("\"version\""))
        .count();
    if count < version_fields {
        return Err(format!(
            "{path} has fewer than {version_fields} version fields"
        ));
    }
    Ok(())
}

fn verify_updated_lockfile(text: &str, expected: &str) -> Result<(), String> {
    let actual = lockfile_versions(text)?;
    if actual != expected {
        return Err(format!("{EXTENSION_LOCKFILE} did not update to {expected}"));
    }
    Ok(())
}

fn cargo_metadata_validation() -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .map_err(|error| format!("spawn cargo metadata: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(stderr.trim().to_string())
}

fn read_file(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("read {path}: {error}"))
}

fn restore_files(paths: &[&Path], originals: &[&String]) -> String {
    let mut failures = Vec::new();
    for (path, original) in paths.iter().zip(originals.iter()) {
        if let Err(error) = fs::write(path, original) {
            failures.push(format!(" {}: {error}", path.display()));
        }
    }
    if failures.is_empty() {
        String::new()
    } else {
        format!("; rollback also failed for{}", failures.join(","))
    }
}

fn format_write_failure(path: &Path, error: std::io::Error, rollback: String) -> String {
    format!("write {} failed: {error}{rollback}", path.display())
}

fn strip_toml_comment(line: &str) -> &str {
    let mut quote = None;
    for (index, character) in line.char_indices() {
        match (quote, character) {
            (None, '"' | '\'') => quote = Some(character),
            (Some(open), _) if open == character => quote = None,
            (None, '#') => return &line[..index],
            _ => {}
        }
    }
    line
}

fn unquote(value: &str) -> Option<&str> {
    for quote in ['"', '\''] {
        if let Some(value) = value.strip_prefix(quote) {
            return value.strip_suffix(quote);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        EXTENSION_LOCKFILE, EXTENSION_MANIFEST, WORKSPACE_MANIFEST, bump_version,
        cargo_metadata_validation, format_write_failure, json_root_version, lockfile_versions,
        prepare_update, replace_json_version_lines, replace_section_version, restore_files,
        section_version, validate_version_input, verify_updated_json, verify_updated_lockfile,
    };
    use std::fs;
    use std::io;
    use std::path::Path;

    #[test]
    fn prepares_all_release_version_surfaces_without_reformatting_json() -> Result<(), String> {
        let workspace = "[workspace.package]\nversion = \"0.10.0\"\nedition = \"2024\"\n";
        let extension = "{\n  \"name\": \"ripr\",\n  \"version\": \"0.10.0\",\n  \"publisher\": \"EffortlessMetrics\"\n}\n";
        let lockfile = "{\n  \"name\": \"ripr\",\n  \"version\": \"0.10.0\",\n  \"packages\": {\n    \"\": {\n      \"name\": \"ripr\",\n      \"version\": \"0.10.0\"\n    },\n    \"node_modules/example\": {\n      \"version\": \"1.0.0\"\n    }\n  }\n}\n";

        let update = prepare_update(workspace, extension, lockfile, "0.11.0")?;
        if update.old_version != "0.10.0" {
            return Err(format!("unexpected old version: {}", update.old_version));
        }
        if !update.workspace.contains("version = \"0.11.0\"")
            || !update.extension.contains("\"version\": \"0.11.0\"")
            || update.lockfile.matches("\"version\": \"0.11.0\"").count() != 2
        {
            return Err("not all release version surfaces were updated".to_string());
        }
        if !update.lockfile.contains("1.0.0") {
            return Err("dependency version was unexpectedly changed".to_string());
        }
        Ok(())
    }

    #[test]
    fn rejects_version_drift_before_writing() -> Result<(), String> {
        let result = prepare_update(
            "[workspace.package]\nversion = \"0.10.0\"\n",
            "{\"version\":\"0.9.0\"}",
            "{\"version\":\"0.10.0\",\"packages\":{\"\":{\"version\":\"0.10.0\"}}}",
            "0.11.0",
        );
        match result {
            Ok(_) => return Err("version drift must fail closed".to_string()),
            Err(error) => {
                let expected = "release version drift detected before bump: workspace=0.10.0, package.json=0.9.0, package-lock.json=0.10.0";
                if error != expected {
                    return Err(format!("expected error {expected:?}, got {error:?}"));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn reads_owned_version_fields() -> Result<(), String> {
        let workspace = "[workspace.package]\nversion = \"0.10.0\"\n";
        let lockfile = "{\"version\":\"0.10.0\",\"packages\":{\"\":{\"version\":\"0.10.0\"}}}";
        if section_version(workspace, "workspace.package")? != "0.10.0"
            || lockfile_versions(lockfile)? != "0.10.0"
            || json_root_version("{\"version\":\"0.10.0\"}", "package.json")? != "0.10.0"
        {
            return Err("owned version fields were not read".to_string());
        }
        Ok(())
    }

    #[test]
    fn bumps_fixture_release_files_and_validates_cargo_metadata() -> Result<(), String> {
        crate::tests::with_temp_cwd("version-bump", |root| {
            fs::write(
                root.join("Cargo.toml"),
                "[workspace]\nresolver = \"2\"\n\n[workspace.package]\nversion = \"0.10.0\"\n",
            )
            .map_err(|error| error.to_string())?;
            fs::create_dir_all(root.join("editors/vscode")).map_err(|error| error.to_string())?;
            fs::write(
                root.join(EXTENSION_MANIFEST),
                "{\n  \"name\": \"ripr\",\n  \"version\": \"0.10.0\"\n}\n",
            )
            .map_err(|error| error.to_string())?;
            fs::write(
                root.join(EXTENSION_LOCKFILE),
                "{\n  \"version\": \"0.10.0\",\n  \"packages\": {\n    \"\": {\n      \"version\": \"0.10.0\"\n    }\n  }\n}\n",
            )
            .map_err(|error| error.to_string())?;

            bump_version(&["0.11.0".to_string()])?;
            let workspace = fs::read_to_string(root.join(WORKSPACE_MANIFEST))
                .map_err(|error| error.to_string())?;
            let extension = fs::read_to_string(root.join(EXTENSION_MANIFEST))
                .map_err(|error| error.to_string())?;
            let lockfile = fs::read_to_string(root.join(EXTENSION_LOCKFILE))
                .map_err(|error| error.to_string())?;
            if !workspace.contains("version = \"0.11.0\"")
                || !extension.contains("\"version\": \"0.11.0\"")
                || lockfile.matches("\"version\": \"0.11.0\"").count() != 2
            {
                return Err("fixture release files did not receive the new version".to_string());
            }
            Ok(())
        })
    }

    #[test]
    fn rejects_invalid_version_inputs() -> Result<(), String> {
        for version in ["", " 0.11.0", "0.11.0 ", "0. 11.0", "0\".11.0", "0\n11.0"] {
            if validate_version_input(version).is_ok() {
                return Err(format!("invalid version was accepted: {version:?}"));
            }
        }
        validate_version_input("0.11.0")
    }

    #[test]
    fn rejects_malformed_or_drifting_json_version_surfaces() -> Result<(), String> {
        for (text, expected) in [
            ("not json", "not valid JSON"),
            ("{}", "no usable root version"),
            ("{\"version\":\"\"}", "no usable root version"),
        ] {
            let error = json_root_version(text, "package.json")
                .err()
                .ok_or_else(|| format!("malformed JSON fixture was accepted: {text}"))?;
            if !error.contains(expected) {
                return Err(format!("unexpected JSON error: {error}"));
            }
        }

        for (text, expected) in [
            ("{\"version\":\"0.10.0\"}", "packages[\"\"]"),
            (
                "{\"version\":\"0.10.0\",\"packages\":{\"\":{\"version\":\"0.9.0\"}}}",
                "differ",
            ),
            ("not json", "not valid JSON"),
        ] {
            let error = lockfile_versions(text)
                .err()
                .ok_or_else(|| format!("malformed lockfile fixture was accepted: {text}"))?;
            if !error.contains(expected) {
                return Err(format!("unexpected lockfile error: {error}"));
            }
        }
        Ok(())
    }

    #[test]
    fn replacement_and_updated_surface_guards_reject_bad_shapes() -> Result<(), String> {
        let malformed_workspace = replace_section_version(
            "[workspace.package]\nversion\n",
            "workspace.package",
            "0.11.0",
        )
        .err()
        .ok_or_else(|| "malformed workspace version line was accepted".to_string())?;
        if !malformed_workspace.contains("malformed") {
            return Err(format!("unexpected workspace error: {malformed_workspace}"));
        }

        let malformed_json =
            replace_json_version_lines("{\n  \"version\": \"0.9.0\"\n}\n", "0.10.0", "0.11.0", 1)
                .err()
                .ok_or_else(|| "JSON version drift was accepted".to_string())?;
        if !malformed_json.contains("expected JSON version") {
            return Err(format!(
                "unexpected JSON replacement error: {malformed_json}"
            ));
        }

        let count_error =
            replace_json_version_lines("{\n  \"version\": \"0.10.0\"\n}\n", "0.10.0", "0.11.0", 2)
                .err()
                .ok_or_else(|| "missing JSON version field was accepted".to_string())?;
        if !count_error.contains("expected 2 JSON version fields") {
            return Err(format!("unexpected JSON count error: {count_error}"));
        }

        let json_error = verify_updated_json("{}", "package.json", "0.11.0", 1)
            .err()
            .ok_or_else(|| "updated JSON without a version was accepted".to_string())?;
        if !json_error.contains("no usable root version") {
            return Err(format!("unexpected updated JSON error: {json_error}"));
        }
        let lock_error = verify_updated_lockfile(
            "{\"version\":\"0.11.0\",\"packages\":{\"\":{\"version\":\"0.10.0\"}}}",
            "0.11.0",
        )
        .err()
        .ok_or_else(|| "drifting updated lockfile was accepted".to_string())?;
        if !lock_error.contains("differ") {
            return Err(format!("unexpected updated lockfile error: {lock_error}"));
        }
        Ok(())
    }

    #[test]
    fn replacement_helpers_preserve_comments_and_unquote_values() -> Result<(), String> {
        let workspace = replace_section_version(
            "[workspace.package]\nversion = '0.10.0' # release\n",
            "workspace.package",
            "0.11.0",
        )?;
        if !workspace.contains("version = '0.11.0' # release") {
            return Err("TOML comment or quote style was not preserved".to_string());
        }
        Ok(())
    }

    #[test]
    fn rejects_missing_empty_and_repeated_release_version_inputs() -> Result<(), String> {
        if bump_version(&[]).is_ok() {
            return Err("missing bump-version argument was accepted".to_string());
        }
        if prepare_update(
            "[workspace.package]\nversion = \"0.10.0\"\n",
            "{\"version\":\"0.10.0\"}",
            "{\"version\":\"0.10.0\",\"packages\":{\"\":{\"version\":\"0.10.0\"}}}",
            "0.10.0",
        )
        .is_ok()
        {
            return Err("same release version was accepted".to_string());
        }

        for workspace in [
            "[workspace.package]\nversion = \"\"\n",
            "[workspace]\nname = \"missing\"\n",
        ] {
            if section_version(workspace, "workspace.package").is_ok() {
                return Err(format!(
                    "invalid workspace version was accepted: {workspace:?}"
                ));
            }
        }
        for workspace in [
            "[workspace.package]\nversion\n",
            "[workspace.package]\nversion = bare\n",
        ] {
            if replace_section_version(workspace, "workspace.package", "0.11.0").is_ok() {
                return Err(format!(
                    "invalid workspace replacement was accepted: {workspace:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn reports_updated_surface_mismatches_and_rollback_failures() -> Result<(), String> {
        for (result, expected) in [
            (
                verify_updated_json("{\"version\":\"0.10.0\"}", "package.json", "0.11.0", 1),
                "did not update",
            ),
            (
                verify_updated_json("{\"version\":\"0.11.0\"}", "package.json", "0.11.0", 2),
                "fewer than 2",
            ),
            (
                verify_updated_lockfile(
                    "{\"version\":\"0.10.0\",\"packages\":{\"\":{\"version\":\"0.10.0\"}}}",
                    "0.11.0",
                ),
                "did not update",
            ),
        ] {
            let error = result
                .err()
                .ok_or_else(|| format!("mismatch was accepted; expected {expected}"))?;
            if !error.contains(expected) {
                return Err(format!("unexpected mismatch error: {error}"));
            }
        }

        let root = crate::tests::temp_dir("version-rollback");
        let existing = root.join("existing.txt");
        fs::write(&existing, "new").map_err(|error| error.to_string())?;
        let original = "old".to_string();
        if !restore_files(&[existing.as_path()], &[&original]).is_empty() {
            return Err("successful rollback reported a failure".to_string());
        }
        if fs::read_to_string(&existing).map_err(|error| error.to_string())? != "old" {
            return Err("successful rollback did not restore the original".to_string());
        }

        let missing_path = root.join("missing").join("file.txt");
        let rollback_error = restore_files(&[missing_path.as_path()], &[&original]);
        if rollback_error.is_empty() {
            return Err("failed rollback did not report an error".to_string());
        }
        let write_error = format_write_failure(
            Path::new("Cargo.toml"),
            io::Error::other("synthetic write failure"),
            rollback_error,
        );
        if !write_error.contains("synthetic write failure") {
            return Err(format!("write failure omitted its cause: {write_error}"));
        }
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn reports_cargo_metadata_failure_after_spawn() -> Result<(), String> {
        crate::tests::with_temp_cwd("version-metadata-failure", |root| {
            fs::write(root.join("Cargo.toml"), "[workspace\n")
                .map_err(|error| error.to_string())?;
            let error = cargo_metadata_validation()
                .err()
                .ok_or_else(|| "invalid Cargo manifest was accepted".to_string())?;
            if error.is_empty() {
                return Err("Cargo metadata failure had no diagnostic".to_string());
            }
            Ok(())
        })
    }
}
