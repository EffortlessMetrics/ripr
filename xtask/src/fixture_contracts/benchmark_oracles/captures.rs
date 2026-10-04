//! Compact historical capture bindings and explicit current custody limits.

use super::*;

#[derive(Default)]
pub(super) struct Custody {
    pub(super) local: usize,
    pub(super) external: usize,
}

pub(super) fn validate(
    root: &Path,
    key: &Value,
    pairing: &Value,
    row: &Value,
    production: &str,
    test: &str,
    passes: bool,
) -> Result<Custody, String> {
    let variant = text(row, "variant")?;
    let capture = retained_json(root, &row["capture"])?;
    let mut observation = row.clone();
    let _ = observation
        .as_object_mut()
        .and_then(|value| value.remove("capture"));
    if capture["kind"].as_str() != Some("retained_native_capture")
        || capture["case_id"] != key["case_id"]
        || capture["observation"] != observation
    {
        return Err(format!(
            "{variant}: missing, swapped or stale native capture"
        ));
    }
    native_inputs::validate(
        root,
        key,
        pairing,
        production,
        test,
        &capture["full_workspace_inputs_before"],
        &capture["full_workspace_inputs_after"],
    )
    .map_err(|error| format!("native input {variant}: {error}"))?;
    let runner = &row["runner"];
    let artifact = &capture["artifact"];
    let selected = &capture["compiler_artifact"];
    let cwd = text(runner, "cwd")?.trim_end_matches('/');
    if !cwd.starts_with('/') {
        return Err(format!(
            "{variant}: capture working directory must be absolute"
        ));
    }
    let manifest = format!("{cwd}/{}", text(key, "package_manifest_path")?);
    let directory = manifest
        .rsplit_once('/')
        .map(|(directory, _)| directory)
        .ok_or_else(|| format!("{variant}: manifest has no directory"))?;
    let library_source = format!("{cwd}/{}", text(key, "library_source_path")?);
    let package = text(key, "package")?;
    let version = text(key, "package_version")?;
    let package_id = text(selected, "package_id")?;
    let source = format!("path+file://{directory}#");
    if package_id != format!("{source}{version}")
        && package_id != format!("{source}{package}@{version}")
    {
        return Err(format!("{variant}: captured package identity differs"));
    }
    if selected["reason"].as_str() != Some("compiler-artifact")
        || selected["manifest_path"].as_str() != Some(manifest.as_str())
        || selected["target"]["kind"] != serde_json::json!(["lib"])
        || selected["target"]["name"] != key["library_target"]
        || selected["target"]["src_path"].as_str() != Some(library_source.as_str())
        || selected["profile"]["test"].as_bool() != Some(true)
        || selected["executable"] != artifact["compiled_path"]
        || !text(artifact, "compiled_path")?.starts_with('/')
        || artifact["sha256"] != runner["artifact_sha256"]
        || artifact["bytes"].as_u64().is_none_or(|bytes| bytes == 0)
    {
        return Err(format!(
            "{variant}: compiler-selected artifact identity differs"
        ));
    }
    let retained = &artifact["retained"];
    if retained["sha256"] != artifact["sha256"] || retained["bytes"] != artifact["bytes"] {
        return Err(format!("{variant}: retained executable identity differs"));
    }
    let verification = retained_json(root, &capture["retained_verification"])?;
    if verification["kind"].as_str() != Some("post_capture_frozen_executable_rehash")
        || verification["artifact"] != *retained
    {
        return Err(format!(
            "{variant}: stale or missing retained-executable measurement"
        ));
    }
    let _ = text(&verification, "observed_utc")?;
    let executable = text(retained, "path")?;
    if !executable.starts_with('/') {
        return Err(format!(
            "{variant}: retained executable capture path must be absolute"
        ));
    }
    let test_id = text(key, "test_id")?;
    let listing = &capture["discovery"];
    validate_terminal(variant, listing, 0)?;
    if listing["argv"] != serde_json::json!([executable, test_id, "--exact", "--list"])
        || retained_text(root, &listing["stdout"])?
            != format!("{test_id}: test\n\n1 test, 0 benchmarks\n")
    {
        return Err(format!(
            "{variant}: compiler artifact does not list the exact nonempty subject"
        ));
    }
    let _ = retained_text(root, &listing["stderr"])?;
    let execution = &capture["artifact_execution"];
    validate_terminal(variant, execution, if passes { 0 } else { 101 })?;
    if execution["argv"] != serde_json::json!([executable, test_id, "--exact"])
        || execution["executable_sha256"] != artifact["sha256"]
    {
        return Err(format!(
            "{variant}: retained artifact execution identity differs"
        ));
    }
    validate_native_output(
        root,
        key,
        variant,
        &execution["stdout"],
        &execution["stderr"],
        passes,
    )?;
    match text(&artifact["custody"], "status")? {
        "local" => {
            let file = &artifact["custody"]["file"];
            verify_file(root, file)?;
            if file["bytes"] != artifact["bytes"] || file["sha256"] != artifact["sha256"] {
                return Err(format!(
                    "{variant}: local artifact bytes differ from captured executable"
                ));
            }
            Ok(Custody {
                local: 1,
                external: 0,
            })
        }
        "external" => {
            let _ = text(&artifact["custody"], "locator")?;
            if artifact["custody"].get("file").is_some() {
                return Err(format!(
                    "{variant}: external custody cannot silently bypass a declared local file"
                ));
            }
            Ok(Custody {
                local: 0,
                external: 1,
            })
        }
        _ => Err(format!("{variant}: unsupported executable custody")),
    }
}

fn validate_terminal(variant: &str, value: &Value, exit: i64) -> Result<(), String> {
    if value["phase"].as_str() != Some("completed")
        || value["timed_out"].as_bool() != Some(false)
        || value["process_failed"].as_bool() != Some(false)
        || value["exit_code"].as_i64() != Some(exit)
    {
        return Err(format!(
            "{variant}: unavailable artifact discovery/execution"
        ));
    }
    Ok(())
}
