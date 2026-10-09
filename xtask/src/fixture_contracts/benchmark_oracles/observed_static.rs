//! Check retained historical static capture identities without prescribing a class.

use super::*;

fn same_bytes(left: &Value, right: &Value) -> bool {
    left["bytes"].as_u64().is_some()
        && left["bytes"] == right["bytes"]
        && left["sha256"].as_str().is_some()
        && left["sha256"] == right["sha256"]
}

pub(super) fn validate(
    root: &Path,
    key: &Value,
    pairing: &Value,
    descriptor: &Value,
) -> Result<(), String> {
    let capture = retained_json(root, descriptor)?;
    if capture["kind"].as_str() != Some("historical_exact_ripr_observation")
        || capture["status"].as_str() != Some("THREE_COMMANDS_COMPLETED")
        || capture["case_id"] != key["case_id"]
        || capture["normative_static_expectation"].as_bool() != Some(false)
        || capture["test_source_path"] != key["test_source_path"]
    {
        return Err(
            "historical static capture has a wrong case, scope or test identity".to_string(),
        );
    }
    let _ = text(&capture, "transforms")?;
    let production_path = text(&capture, "production_source_path")?;
    if !local_path(production_path)
        || capture["production_source_path"] != key["production_source_path"]
    {
        return Err("historical production path must be contained".to_string());
    }
    for field in [
        "subject_alignment",
        "corrected_summary",
        "original_summary_before_correction",
    ] {
        if let Some(descriptor) = capture.get(field)
            && !retained_json(root, descriptor)?.is_object()
        {
            return Err(format!(
                "historical static {field} must retain a JSON object"
            ));
        }
    }
    if let Some(attempt) = capture.get("old_attempt")
        && (!attempt.is_object() || !retained_json(root, &attempt["receipt"])?.is_object())
    {
        return Err("historical static old_attempt must retain its JSON receipt".to_string());
    }
    let producer = retained_json(root, &capture["producer_receipt"])?;
    let execution = retained_json(root, &capture["execution_receipt"])?;
    verify_file(root, &capture["diff"])?;
    let executable = &capture["executable"];
    if producer["head"] != capture["producer_head"]
        || producer["tree"] != capture["producer_tree"]
        || producer["artifacts"]["ripr"]["head"] != capture["producer_head"]
        || producer["artifacts"]["ripr"]["tree"] != capture["producer_tree"]
        || producer["artifacts"]["ripr"]["compiler_artifact"]["target"]["name"].as_str()
            != Some("ripr")
        || producer["artifacts"]["ripr"]["compiler_artifact"]["target"]["kind"]
            != serde_json::json!(["bin"])
        || execution["producer_head"] != capture["producer_head"]
        || execution["status"].as_str() != Some("THREE_COMMANDS_TERMINAL")
        || execution["compiler_artifact"] != producer["artifacts"]["ripr"]["compiler_artifact"]
        || producer["artifacts"]["ripr"]["compiler_artifact"]["executable"] != executable["path"]
    {
        return Err("historical static producer identity differs".to_string());
    }
    let _ = text(&capture, "producer_head")?;
    let _ = text(&capture, "producer_tree")?;
    for (value, field, length) in [
        (&capture, "producer_head", 40),
        (&capture, "producer_tree", 40),
        (executable, "sha256", 64),
    ] {
        let digest = text(value, field)?;
        if digest.len() != length || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("historical static producer digest is malformed".to_string());
        }
    }
    if executable["bytes"].as_u64().is_none_or(|bytes| bytes == 0)
        || !text(executable, "path")?.starts_with('/')
    {
        return Err("historical static executable identity is incomplete".to_string());
    }
    let producer_files = producer["artifacts"]["ripr"]["files"]
        .as_array()
        .ok_or_else(|| "missing producer artifact files".to_string())?;
    if !producer_files.iter().any(|file| {
        file["path"] == executable["path"]
            && file["sha256"] == executable["sha256"]
            && file["metadata"]["size"] == executable["bytes"]
    }) {
        return Err(
            "historical static executable is not the producer-selected artifact".to_string(),
        );
    }
    for identity in [&execution["identity_before"], &execution["identity_after"]] {
        if identity["executable"] != *executable
            || !same_bytes(&identity["producer_receipt"], &capture["producer_receipt"])
        {
            return Err("historical static execution has a stale producer fence".to_string());
        }
    }
    let rows = capture["observations"]
        .as_array()
        .filter(|rows| rows.len() == 3)
        .ok_or_else(|| "historical static capture needs three variants".to_string())?;
    let runs = execution["observations"]
        .as_array()
        .filter(|rows| rows.len() == 3)
        .ok_or_else(|| "historical static execution needs three variants".to_string())?;
    let native = pairing["observations"]
        .as_array()
        .ok_or_else(|| "missing native observations".to_string())?;
    let mut seen = BTreeSet::new();
    for row in rows {
        let variant = text(row, "variant")?;
        if !matches!(variant, "corrected" | "original" | "weak") || !seen.insert(variant) {
            return Err("historical static variant is wrong or repeated".to_string());
        }
        let matches = runs
            .iter()
            .filter(|run| run["variant"].as_str() == Some(variant))
            .collect::<Vec<_>>();
        let [run] = matches.as_slice() else {
            return Err("historical static execution variant is missing or repeated".to_string());
        };
        if run["timed_out"].as_bool() != Some(false)
            || !run["resource_stop"].is_null()
            || run["parsed_json"].as_bool() != Some(true)
            || run["exit_code"].as_i64().is_none()
            || run["exit_code"] != row["native_exit"]
            || run["patch_before"] != run["patch_after"]
            || !same_bytes(&run["patch_before"], &capture["diff"])
        {
            return Err("historical static command is unavailable or inconsistent".to_string());
        }
        for identity in [&run["identity_before"], &run["identity_after"]] {
            if identity["executable"] != *executable
                || !same_bytes(&identity["producer_receipt"], &capture["producer_receipt"])
            {
                return Err("historical static command has a wrong executable/producer".to_string());
            }
        }
        let expected_argv = serde_json::json!([
            text(executable, "path")?,
            "check",
            "--root",
            text(run, "root")?,
            "--diff",
            text(&run["patch_before"], "path")?,
            "--format",
            "json"
        ]);
        if run["argv"] != expected_argv {
            return Err("historical static command/root/diff differs".to_string());
        }
        for (field, retained) in [
            ("stdout", "json"),
            ("stderr", "stderr"),
            ("inputs_before", "inputs_before"),
            ("inputs_after", "inputs_after"),
        ] {
            verify_file(root, &row[retained])?;
            if !same_bytes(&run[field], &row[retained]) {
                return Err(format!(
                    "historical static {variant} {field} identity differs"
                ));
            }
        }
        let report = retained_json(root, &row["json"])?;
        if report["root"] != run["root"]
            || !report["summary"].is_object()
            || report["analysis_outcome"]["analysis_complete"]
                .as_bool()
                .is_none()
            || !report["findings"].is_array()
        {
            return Err("historical static report root or structure differs".to_string());
        }
        for (field, reported) in [
            ("classification", "classification"),
            ("oracle_kind", "oracle_kind"),
            ("oracle_strength", "oracle_strength"),
            ("reported_related_tests_total", "related_tests_total"),
        ] {
            if row.get(field).is_some() && row[field] != report["findings"][0][reported] {
                return Err(
                    "historical static summary differs from its retained report".to_string()
                );
            }
        }
        let inputs = retained_json(root, &row["inputs_before"])?;
        let after = retained_json(root, &row["inputs_after"])?;
        if inputs != after {
            return Err("historical static input fence changed".to_string());
        }
        let native_variant = format!("fixed_{variant}");
        let native_row = native
            .iter()
            .find(|row| row["variant"].as_str() == Some(native_variant.as_str()))
            .ok_or_else(|| "historical static input has no matching native pair".to_string())?;
        let native_capture = retained_json(root, &native_row["capture"])?;
        if inputs != retained_json(root, &native_capture["full_workspace_inputs_before"])?
            || inputs != retained_json(root, &native_capture["full_workspace_inputs_after"])?
        {
            return Err(
                "historical static full inputs differ from the exact native variant".to_string(),
            );
        }
        let entries = inputs
            .as_array()
            .ok_or_else(|| "historical static input inventory must be an array".to_string())?;
        for (path, expected) in [
            (production_path, &key["sources"]["fixed"]),
            (text(key, "test_source_path")?, &key["tests"][variant]),
            ("Cargo.lock", &pairing["lock"]),
        ] {
            let matches = entries
                .iter()
                .filter(|entry| entry["path"].as_str() == Some(path))
                .collect::<Vec<_>>();
            if matches.len() != 1 || !same_bytes(matches[0], expected) {
                return Err(format!(
                    "historical static {variant} has wrong input {path}"
                ));
            }
        }
    }
    Ok(())
}
