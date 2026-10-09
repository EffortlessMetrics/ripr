//! The declared matcher witness has its own subject and external custody.

use super::*;

fn artifact(value: &Value) -> Result<(), String> {
    let digest = text(value, "sha256")?;
    if !text(value, "path")?.starts_with('/')
        || value["bytes"].as_u64().is_none_or(|bytes| bytes == 0)
        || digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("matcher external artifact identity is incomplete".to_string());
    }
    Ok(())
}

fn ranges(value: &Value) -> Result<String, String> {
    let mut result = Vec::new();
    for range in value.as_array().ok_or("matcher ranges must be an array")? {
        let pair = range
            .as_array()
            .filter(|pair| pair.len() == 2)
            .ok_or("matcher range must contain two offsets")?;
        let start = pair[0].as_u64().ok_or("matcher range start is missing")?;
        let end = pair[1].as_u64().ok_or("matcher range end is missing")?;
        if end < start {
            return Err("matcher range is reversed".to_string());
        }
        result.push(format!("{start}..{end}"));
    }
    Ok(format!("[{}]", result.join(", ")))
}

fn observed_output(values: &Value) -> Result<String, String> {
    let word_empty = values["word_boundary_empty_haystack"]
        .as_bool()
        .ok_or("matcher word-boundary observation is missing")?;
    let negated_empty = values["negated_word_boundary_empty_haystack"]
        .as_bool()
        .ok_or("matcher negated-boundary observation is missing")?;
    let word_a = ranges(&values["word_boundary_a_ranges"])?;
    let negated_a = ranges(&values["negated_word_boundary_a_ranges"])?;
    Ok(format!(
        "wb_empty={word_empty}; notwb_empty={negated_empty}; wb_a={word_a}; notwb_a={negated_a}\nmatcher_witness: 4 assertions passed\n"
    ))
}

pub(super) fn validate(root: &Path, key: &Value, pairing: &Value) -> Result<(), String> {
    let basis = key["basis"].as_array().ok_or("matcher basis is missing")?;
    for entry in basis {
        if entry["kind"].as_str() == Some("separate_matcher_semantic_witness") {
            validate_witness(root, key, pairing, entry, basis)
                .map_err(|error| format!("semantic oracle matcher witness: {error}"))?;
        }
    }
    Ok(())
}

fn validate_witness(
    root: &Path,
    key: &Value,
    pairing: &Value,
    basis_entry: &Value,
    basis: &[Value],
) -> Result<(), String> {
    let witness = retained_json(root, &basis_entry["artifact"])?;
    if witness["kind"].as_str() != Some("separate_matcher_semantic_witness")
        || witness["same_hir_predicate_not_consumed_by_matcher"].as_bool() != Some(true)
    {
        return Err("matcher witness kind or declared independence differs".to_string());
    }
    let source = &witness["source_of_nonconsumption_claim"];
    verify_file(root, source)?;
    if !basis.iter().any(|entry| {
        entry["kind"].as_str() == Some("source_document") && entry["artifact"] == *source
    }) {
        return Err("matcher nonconsumption source is not a declared source document".to_string());
    }
    let test = text(basis_entry, "native_test_variant")?;
    if !matches!(test, "corrected" | "original" | "weak") {
        return Err("matcher native test variant is unsupported".to_string());
    }
    for field in ["package_manifest_path", "library_source_path"] {
        if !local_path(text(basis_entry, field)?) {
            return Err(format!("matcher {field} must be a contained logical path"));
        }
    }
    let observations = witness["observations"]
        .as_array()
        .filter(|rows| rows.len() == 2)
        .ok_or("matcher needs fixed and broken observations")?;
    let native = pairing["observations"]
        .as_array()
        .ok_or("native observations are missing")?;
    let mut seen = BTreeSet::new();
    let mut programs = std::collections::BTreeMap::new();
    for observation in observations {
        let production = text(observation, "production")?;
        if !matches!(production, "fixed" | "broken") || !seen.insert(production) {
            return Err("matcher production role is unknown or repeated".to_string());
        }
        let variant = format!("{production}_{test}");
        let row = native
            .iter()
            .find(|row| row["variant"].as_str() == Some(variant.as_str()))
            .ok_or("matcher has no matching native observation")?;
        let capture = retained_json(root, &row["capture"])?;
        native_inputs::validate(
            root,
            key,
            pairing,
            production,
            test,
            &observation["input_inventory_before"],
            &observation["input_inventory_after"],
        )?;
        for (matcher, native) in [
            ("input_inventory_before", "full_workspace_inputs_before"),
            ("input_inventory_after", "full_workspace_inputs_after"),
        ] {
            if retained_json(root, &observation[matcher])? != retained_json(root, &capture[native])?
            {
                return Err("matcher inputs differ from the declared native variant".to_string());
            }
        }
        native_inputs::validate_selected_subject(
            root,
            basis_entry,
            &observation["input_inventory_before"],
        )?;
        validate_library(basis_entry, observation, &row["runner"])?;
        for role in ["original", "observed"] {
            let run = &observation[role];
            let program = &run["program"];
            if retained_text(root, program)?.trim().is_empty() {
                return Err("matcher program is empty".to_string());
            }
            if let Some(previous) = programs.insert(role, program.clone())
                && previous != *program
            {
                return Err(
                    "matcher program identity changes between production variants".to_string(),
                );
            }
            artifact(&run["artifact"])?;
            if run["exit_code"].as_i64() != Some(0)
                || run["argv"] != serde_json::json!([text(&run["artifact"], "path")?])
                || text(run, "custody")?.split(';').next().map(str::trim) != Some("external")
            {
                return Err("matcher command, exit or external custody differs".to_string());
            }
            let output = retained_text(root, &run["stdout"])?;
            let _ = retained_text(root, &run["stderr"])?;
            if role == "observed" && output != observed_output(&observation["observed_values"])? {
                return Err("matcher observed values differ from the retained output".to_string());
            }
        }
    }
    Ok(())
}

fn validate_library(basis: &Value, observation: &Value, runner: &Value) -> Result<(), String> {
    let cwd = text(runner, "cwd")?.trim_end_matches('/');
    let manifest = format!("{cwd}/{}", text(basis, "package_manifest_path")?);
    let directory = manifest
        .rsplit_once('/')
        .map(|(path, _)| path)
        .ok_or("matcher manifest has no directory")?;
    let library_source = format!("{cwd}/{}", text(basis, "library_source_path")?);
    let package = text(basis, "package")?;
    let version = text(basis, "package_version")?;
    let target = text(basis, "library_target")?;
    let selected = &observation["compiler_artifact"];
    let package_id = text(selected, "package_id")?;
    if package_id != format!("path+file://{directory}#{package}@{version}")
        && package_id != format!("path+file://{directory}#{version}")
    {
        return Err("matcher compiler package identity differs".to_string());
    }
    let linked = &observation["linked_library"];
    artifact(linked)?;
    if !text(linked, "path")?.ends_with(".rlib") {
        return Err(
            "matcher linked library must be a selected Rust rlib, not metadata".to_string(),
        );
    }
    if selected["reason"].as_str() != Some("compiler-artifact")
        || selected["manifest_path"].as_str() != Some(manifest.as_str())
        || selected["target"]["kind"] != serde_json::json!(["lib"])
        || selected["target"]["name"].as_str() != Some(target)
        || selected["target"]["src_path"].as_str() != Some(library_source.as_str())
        || selected["profile"]["test"].as_bool() != Some(false)
        || !selected["executable"].is_null()
        || !selected["filenames"]
            .as_array()
            .is_some_and(|files| files.contains(&linked["path"]))
    {
        return Err("matcher selected library identity differs".to_string());
    }
    Ok(())
}
