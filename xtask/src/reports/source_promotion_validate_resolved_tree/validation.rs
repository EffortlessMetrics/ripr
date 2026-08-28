fn validate(
    options: &Options,
    state: &mut ValidationState,
    evidence_root: &Path,
) -> Result<(), String> {
    let (preflight, preflight_path) = read_bound_json(
        &options.repo,
        &options.preflight,
        &options.preflight_sha256,
        "preflight",
    )?;
    state.inputs.preflight_path = Some(preflight_path);
    validate_preflight(&preflight, &options.source_parent)?;
    if string_field(&preflight, "swarm_parent")? != options.swarm_parent {
        return Err("preflight swarm parent does not match exact input".to_string());
    }
    let preflight_tree = preflight
        .get("dry_merge")
        .and_then(|value| value.get("reviewed_resolved_tree"))
        .and_then(Value::as_str)
        .ok_or_else(|| "preflight is missing reviewed resolved tree".to_string())?;
    if preflight_tree != options.reviewed_tree {
        return Err("preflight reviewed tree does not match exact input".to_string());
    }
    state.preflight_verified = true;

    let (manifest, resolution_path) = read_bound_json(
        &options.repo,
        &options.resolution_manifest,
        &options.resolution_sha256,
        "resolution manifest",
    )?;
    state.inputs.resolution_path = Some(resolution_path);
    validate_resolution_manifest_contract(&manifest, &preflight, &options.preflight_sha256)?;
    state.resolution_verified = true;

    verify_exact_commit(&options.repo, &options.source_parent, "--source-parent")?;
    verify_exact_commit(&options.repo, &options.swarm_parent, "--swarm-parent")?;
    verify_exact_tree(&options.repo, &options.reviewed_tree)?;

    let live_head = git(&options.repo, &["rev-parse", "HEAD"], &[])?;
    ensure_checker_source_identity(live_head.trim(), &options.source_parent)?;

    let checker = std::env::current_exe()
        .map_err(|error| format!("failed to identify running xtask executable: {error}"))?;
    let checker_metadata = fs::symlink_metadata(&checker).map_err(|error| {
        format!(
            "failed to inspect running xtask executable {}: {error}",
            checker.display()
        )
    })?;
    if checker_metadata.file_type().is_symlink() || !checker_metadata.is_file() {
        return Err(format!(
            "running xtask executable is not a non-symlink regular file: {}",
            checker.display()
        ));
    }
    state.checker_source_sha = Some(options.source_parent.clone());
    state.checker_executable_sha256 = Some(digest_file(&checker)?);

    let refs_before = snapshot_refs(&options.repo)?;
    let worktrees_before = snapshot_worktrees(&options.repo)?;

    let mut materialized = match MaterializedTree::create(options) {
        Ok(materialized) => materialized,
        Err(reason) => {
            return match observe_repository_after(
                &options.repo,
                state,
                &refs_before,
                &worktrees_before,
            ) {
                Ok(()) => Err(reason),
                Err(observation_error) => Err(format!(
                    "{reason}; failed to observe repository state after materialization failure: {observation_error}"
                )),
            };
        }
    };
    state.materialization_created = true;
    state.materialized_tree = Some(options.reviewed_tree.clone());
    state.disposable_commit = Some(materialized.commit.clone());

    let execution_result = validate_materialized_tree(
        options,
        state,
        &checker,
        &materialized.root,
        evidence_root,
    );

    let cleanup = materialized.cleanup();
    state.worktree_remove_succeeded = cleanup.worktree_remove_succeeded;
    state.materialization_directory_removed = cleanup.materialization_directory_removed;
    state.worktree_residue_observed = cleanup.worktree_residue_observed;
    state.cleanup_failure_reason = cleanup.failure_reason.clone();
    if let Some(reason) = cleanup.failure_reason {
        push_failure_once(state, &reason);
    }

    let observation_result =
        observe_repository_after(&options.repo, state, &refs_before, &worktrees_before);
    match (execution_result, observation_result) {
        (Ok(()), Ok(())) => {}
        (Err(reason), Ok(())) => return Err(reason),
        (Ok(()), Err(observation_error)) => return Err(observation_error),
        (Err(reason), Err(observation_error)) => {
            return Err(format!(
                "{reason}; failed to observe repository state after validation: {observation_error}"
            ));
        }
    }
    if state.ref_mutation_observed {
        return Err("repository refs changed during resolved-tree validation".to_string());
    }
    if state.worktree_registry_changed || state.worktree_residue_observed {
        return Err("disposable worktree cleanup did not restore repository state".to_string());
    }
    if state.cleanup_failure_reason.is_some() {
        return Err("resolved-tree materialization cleanup failed".to_string());
    }
    if !commands_are_terminal_green(&state.commands) {
        return Err("one or more required governance commands did not pass".to_string());
    }
    Ok(())
}

fn validate_resolution_manifest_contract(
    manifest: &Value,
    preflight: &Value,
    preflight_sha256: &str,
) -> Result<(), String> {
    validate_manifest(manifest, preflight, preflight_sha256)?;
    validate_resolution_manifest_dispositions(manifest)
}

fn validate_resolution_manifest_dispositions(manifest: &Value) -> Result<(), String> {
    let dispositions = manifest
        .get("dispositions")
        .and_then(Value::as_array)
        .ok_or_else(|| "resolution manifest missing dispositions array".to_string())?;

    for (index, row) in dispositions.iter().enumerate() {
        let disposition = row
            .get("disposition")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("resolution manifest row {index} missing disposition"))?;
        match disposition {
            "source_blob" | "swarm_blob" | "excluded" => {}
            "integrated" => validate_integrated_evidence(row, index)?,
            other => {
                return Err(format!(
                    "resolution manifest row {index} has unknown disposition {other:?}; expected source_blob, swarm_blob, integrated, or excluded"
                ));
            }
        }
    }
    Ok(())
}

fn validate_integrated_evidence(row: &Value, index: usize) -> Result<(), String> {
    let evidence = row
        .get("integration_evidence")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "resolution manifest integrated row {index} requires typed digest-bound integration_evidence"
            )
        })?;
    if evidence.get("type").and_then(Value::as_str) != Some("digest_bound_artifact") {
        return Err(format!(
            "resolution manifest integrated row {index} integration_evidence.type must be digest_bound_artifact"
        ));
    }
    let reference = evidence
        .get("ref")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "resolution manifest integrated row {index} integration_evidence.ref must be non-empty"
            )
        })?;
    if reference.contains('\n') || reference.contains('\r') {
        return Err(format!(
            "resolution manifest integrated row {index} integration_evidence.ref must be one line"
        ));
    }
    let digest = evidence
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "resolution manifest integrated row {index} integration_evidence.sha256 is required"
            )
        })?;
    validate_exact_hex("integration_evidence.sha256", digest, 64)
}

fn validate_materialized_tree(
    options: &Options,
    state: &mut ValidationState,
    checker: &Path,
    root: &Path,
    evidence_root: &Path,
) -> Result<(), String> {
    let candidate_tree = git(root, &["rev-parse", "HEAD^{tree}"], &[])?;
    if candidate_tree.trim() != options.reviewed_tree {
        return Err(format!(
            "materialized checkout tree {} does not equal reviewed tree {}",
            candidate_tree.trim(),
            options.reviewed_tree
        ));
    }
    let candidate_status = git(root, &["status", "--porcelain=v1"], &[])?;
    if !candidate_status.trim().is_empty() {
        return Err(format!(
            "materialized reviewed-tree checkout is not clean before validation: {}",
            candidate_status.trim()
        ));
    }
    state.materialization_clean_before = true;

    let logs_dir = evidence_root.join("commands");
    fs::create_dir(&logs_dir)
        .map_err(|error| format!("failed to create exclusive command evidence directory: {error}"))?;

    let mut prior_failure: Option<String> = None;
    for (index, command) in REQUIRED_COMMANDS.iter().enumerate() {
        if let Some(failed_command) = &prior_failure {
            state.commands[index] = command_receipt(
                command,
                "not_run",
                None,
                None,
                Some(&format!(
                    "not run because prior required command {failed_command} did not pass"
                )),
            );
            continue;
        }

        let receipt = run_required_command(
            checker,
            root,
            command,
            index,
            &logs_dir,
            &options.source_parent,
        );
        let passed = command_receipt_is_terminal_pass(&receipt, command);
        state.commands[index] = receipt;
        if !passed {
            prior_failure = Some((*command).to_string());
            let reason = state.commands[index]
                .get("failure_reason")
                .and_then(Value::as_str)
                .unwrap_or("required command did not pass")
                .to_string();
            state.failure_reasons.push(format!("{command}: {reason}"));
        }
    }

    let final_status = git(root, &["status", "--porcelain=v1"], &[])?;
    if !final_status.trim().is_empty() {
        return Err(format!(
            "governance commands changed the reviewed-tree checkout: {}",
            final_status.trim()
        ));
    }
    state.materialization_clean_after = true;

    if let Some(command) = prior_failure {
        return Err(format!(
            "required governance command {command} did not pass"
        ));
    }
    Ok(())
}

fn ensure_checker_source_identity(observed: &str, expected: &str) -> Result<(), String> {
    if observed == expected {
        return Ok(());
    }
    Err(format!(
        "validator checkout HEAD {observed} does not equal exact source parent {expected}"
    ))
}

fn observe_repository_after(
    repo: &Path,
    state: &mut ValidationState,
    refs_before: &str,
    worktrees_before: &str,
) -> Result<(), String> {
    let refs_after = snapshot_refs(repo)?;
    let worktrees_after = snapshot_worktrees(repo)?;
    state.ref_mutation_observed = refs_before != refs_after;
    state.worktree_registry_changed = worktrees_before != worktrees_after;
    Ok(())
}

#[cfg(test)]
mod manifest_disposition_tests {
    use super::{
        validate_resolution_manifest_contract, validate_resolution_manifest_dispositions,
    };

    fn preflight() -> serde_json::Value {
        serde_json::json!({
            "schema": "ripr.source_promotion_preflight.v1",
            "source_parent": "0".repeat(40),
            "swarm_parent": "1".repeat(40),
            "merge_base": "2".repeat(40),
            "dry_merge": {
                "reviewed_resolved_tree": "3".repeat(40),
                "reviewed_resolved_tree_verified": true,
                "conflicts": []
            },
            "source_survivor_candidates": [],
            "swarm_authority_resolution_candidates": []
        })
    }

    fn bound_manifest(preflight_sha256: &str) -> serde_json::Value {
        serde_json::json!({
            "schema": "ripr.source_promotion_resolution.v1",
            "preflight_sha256": preflight_sha256,
            "source_parent": "0".repeat(40),
            "swarm_parent": "1".repeat(40),
            "merge_base": "2".repeat(40),
            "reviewed_join_tree": "3".repeat(40),
            "dispositions": []
        })
    }

    #[test]
    fn production_manifest_contract_uses_bare_preflight_digest_and_rejects_mismatch(
    ) -> Result<(), String> {
        let digest = "a".repeat(64);
        let preflight = preflight();
        let manifest = bound_manifest(&digest);

        validate_resolution_manifest_contract(&manifest, &preflight, &digest)?;

        for mismatched_digest in [format!("sha256:{digest}"), "b".repeat(64)] {
            let Err(_) = validate_resolution_manifest_contract(
                &manifest,
                &preflight,
                &mismatched_digest,
            ) else {
                return Err(format!(
                    "mismatched preflight digest {mismatched_digest:?} unexpectedly passed"
                ));
            };
        }
        Ok(())
    }

    fn row(disposition: &str) -> serde_json::Value {
        serde_json::json!({
            "kind": "conflict",
            "key": "x",
            "disposition": disposition,
            "rationale": "reviewed",
            "evidence": "review"
        })
    }

    fn manifest(row: serde_json::Value) -> serde_json::Value {
        serde_json::json!({"dispositions": [row]})
    }

    #[test]
    fn disposition_enum_is_exact() -> Result<(), String> {
        for disposition in ["source_blob", "swarm_blob", "excluded"] {
            validate_resolution_manifest_dispositions(&manifest(row(disposition)))?;
        }
        for disposition in ["source", "swarm", "merge", "drop", ""] {
            let Err(_) = validate_resolution_manifest_dispositions(&manifest(row(disposition)))
            else {
                return Err(format!(
                    "invalid resolution disposition {disposition:?} unexpectedly passed"
                ));
            };
        }
        Ok(())
    }

    #[test]
    fn integrated_requires_typed_digest_bound_evidence() -> Result<(), String> {
        let mut integrated = row("integrated");
        let Err(_) =
            validate_resolution_manifest_dispositions(&manifest(integrated.clone()))
        else {
            return Err("integrated disposition without typed evidence unexpectedly passed".to_string());
        };

        integrated["integration_evidence"] = serde_json::json!({
            "type": "digest_bound_artifact",
            "ref": "receipts/network-policy.json",
            "sha256": "a".repeat(64)
        });
        validate_resolution_manifest_dispositions(&manifest(integrated.clone()))?;

        integrated["integration_evidence"]["type"] = serde_json::json!("free_form");
        let Err(_) =
            validate_resolution_manifest_dispositions(&manifest(integrated.clone()))
        else {
            return Err("free-form integration evidence unexpectedly passed".to_string());
        };
        integrated["integration_evidence"]["type"] = serde_json::json!("digest_bound_artifact");
        integrated["integration_evidence"]["sha256"] = serde_json::json!("deadbeef");
        let Err(_) =
            validate_resolution_manifest_dispositions(&manifest(integrated.clone()))
        else {
            return Err("short integration evidence digest unexpectedly passed".to_string());
        };
        integrated["integration_evidence"]["sha256"] = serde_json::json!("a".repeat(64));
        integrated["integration_evidence"]["ref"] = serde_json::json!("   ");
        let Err(_) = validate_resolution_manifest_dispositions(&manifest(integrated)) else {
            return Err("blank integration evidence reference unexpectedly passed".to_string());
        };
        Ok(())
    }
}
