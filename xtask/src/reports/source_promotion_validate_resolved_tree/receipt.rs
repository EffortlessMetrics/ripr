fn command_receipt(
    command: &str,
    state: &str,
    exit_code: Option<i32>,
    evidence: Option<&CommandEvidence>,
    failure_reason: Option<&str>,
) -> Value {
    serde_json::json!({
        "command": command,
        "subject_role": command_subject_role(command),
        "state": state,
        "exit_code": exit_code,
        "timeout_bound_ms": duration_ms(COMMAND_TIMEOUT),
        "evidence_present": evidence.is_some(),
        "stdout_path": evidence.map(|value| value.stdout_path.as_str()),
        "stdout_bytes": evidence.map(|value| value.stdout_bytes),
        "stdout_sha256": evidence.map(|value| value.stdout_sha256.as_str()),
        "stdout_truncated": evidence.map(|value| value.stdout_truncated),
        "stderr_path": evidence.map(|value| value.stderr_path.as_str()),
        "stderr_bytes": evidence.map(|value| value.stderr_bytes),
        "stderr_sha256": evidence.map(|value| value.stderr_sha256.as_str()),
        "stderr_truncated": evidence.map(|value| value.stderr_truncated),
        "failure_reason": failure_reason,
    })
}

const COMMAND_RECEIPT_FIELDS: &[&str] = &[
    "command",
    "subject_role",
    "state",
    "exit_code",
    "timeout_bound_ms",
    "evidence_present",
    "stdout_path",
    "stdout_bytes",
    "stdout_sha256",
    "stdout_truncated",
    "stderr_path",
    "stderr_bytes",
    "stderr_sha256",
    "stderr_truncated",
    "failure_reason",
];

const RECEIPT_FIELDS: &[&str] = &[
    "schema",
    "tool_version",
    "status",
    "source_parent",
    "swarm_parent",
    "reviewed_tree",
    "preflight",
    "resolution_manifest",
    "trusted_checker",
    "materialization",
    "required_command_catalog",
    "commands",
    "repository_observation",
    "packet_contract",
    "disposable_git_object_write_attempted",
    "authoritative_commit_attempted",
    "branch_attempted",
    "tag_attempted",
    "push_attempted",
    "ref_mutation_attempted",
    "failure_reasons",
    "invalidation_rules",
    "non_claims",
];
const BOUND_INPUT_FIELDS: &[&str] = &["path_role", "path", "sha256", "verified"];
const TRUSTED_CHECKER_FIELDS: &[&str] = &["selection", "source_sha", "executable_sha256"];
const MATERIALIZATION_FIELDS: &[&str] = &[
    "path_role",
    "reviewed_tree",
    "disposable_commit",
    "created",
    "clean_before",
    "clean_after",
    "worktree_remove_succeeded",
    "directory_removed",
    "worktree_residue_observed",
    "cleanup_failure_reason",
    "authoritative",
];
const REPOSITORY_OBSERVATION_FIELDS: &[&str] =
    &["ref_mutation_observed", "worktree_registry_changed"];
const PACKET_CONTRACT_FIELDS: &[&str] = &[
    "runner_owned_exclusive_staging",
    "create_new_files",
    "index_written_last",
    "atomic_directory_publish",
    "index",
];
pub(crate) const INVALIDATION_RULES: &[&str] = &[
    "Changing the exact source parent, W7 parent, reviewed tree, preflight bytes, resolution-manifest bytes, running checker identity, required-command catalog, or receipt schema invalidates this validation.",
    "A failed, unavailable, or not_run required command rejects construction eligibility.",
    "Any observed or attempted authoritative ref mutation, failed exact worktree removal, or retained worktree residue rejects construction eligibility.",
    "Moving or replacing any packet file, command evidence bytes, or packet index invalidates this validation packet.",
];
pub(crate) const NON_CLAIMS: &[&str] = &[
    "The disposable commit is an unreferenced materialization object only; it is not J, a release object, a branch, or publication authority.",
    "The checker claim is bounded to the running executable selected from a checkout whose HEAD equals the exact source parent and whose executable digest is recorded.",
    "check-command-catalog is trusted-checker self-health; it does not prove candidate-tree xtask command compatibility or candidate command authority.",
    "Candidate command authority is earned later by the reviewed-tree integration and qualification gates owned by #1478 and #1507.",
    "This receipt proves only the named source-governed repository contracts on one exact reviewed tree.",
    "It does not prove product correctness, editor journeys, release readiness, merge eligibility beyond the named contracts, or publication authority.",
];

fn object_has_exact_fields(value: &Value, fields: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
    })
}

fn string_catalog_is_exact(value: &Value, expected: &[&str]) -> bool {
    value.as_array().is_some_and(|catalog| {
        catalog.len() == expected.len()
            && catalog
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.as_str() == Some(expected))
    })
}

pub(crate) fn validate_resolved_tree_receipt_contract(
    report: &Value,
    expected_status: &str,
) -> Result<(), String> {
    if !matches!(expected_status, "validated" | "rejected") {
        return Err(format!(
            "unsupported resolved-tree receipt disposition: {expected_status}"
        ));
    }
    if !object_has_exact_fields(report, RECEIPT_FIELDS) {
        return Err("resolved-tree receipt has a non-canonical top-level field set".to_string());
    }
    for (field, fields) in [
        ("preflight", BOUND_INPUT_FIELDS),
        ("resolution_manifest", BOUND_INPUT_FIELDS),
        ("trusted_checker", TRUSTED_CHECKER_FIELDS),
        ("materialization", MATERIALIZATION_FIELDS),
        ("repository_observation", REPOSITORY_OBSERVATION_FIELDS),
        ("packet_contract", PACKET_CONTRACT_FIELDS),
    ] {
        if !report
            .get(field)
            .is_some_and(|value| object_has_exact_fields(value, fields))
        {
            return Err(format!(
                "resolved-tree receipt has a non-canonical {field} field set"
            ));
        }
    }
    if report.get("schema").and_then(Value::as_str) != Some(RECEIPT_SCHEMA)
        || report.get("tool_version").and_then(Value::as_str) != Some(env!("CARGO_PKG_VERSION"))
        || report.get("status").and_then(Value::as_str) != Some(expected_status)
    {
        return Err("resolved-tree receipt schema, version, or status is incompatible".to_string());
    }
    if !report
        .get("required_command_catalog")
        .is_some_and(|value| string_catalog_is_exact(value, REQUIRED_COMMANDS))
    {
        return Err("resolved-tree receipt has a non-canonical required command catalog".to_string());
    }
    if !report
        .get("invalidation_rules")
        .is_some_and(|value| string_catalog_is_exact(value, INVALIDATION_RULES))
        || !report
            .get("non_claims")
            .is_some_and(|value| string_catalog_is_exact(value, NON_CLAIMS))
    {
        return Err("resolved-tree receipt has non-canonical claim catalogs".to_string());
    }
    let preflight = &report["preflight"];
    let resolution = &report["resolution_manifest"];
    let checker = &report["trusted_checker"];
    let materialization = &report["materialization"];
    let observation = &report["repository_observation"];
    let packet_contract = &report["packet_contract"];
    let source = report.get("source_parent").and_then(Value::as_str);
    let swarm = report.get("swarm_parent").and_then(Value::as_str);
    let tree = report.get("reviewed_tree").and_then(Value::as_str);
    if !source.is_some_and(|value| is_exact_lower_hex(value, 40))
        || !swarm.is_some_and(|value| is_exact_lower_hex(value, 40))
        || !tree.is_some_and(|value| is_exact_lower_hex(value, 40))
        || preflight.get("path_role").and_then(Value::as_str)
        != Some("source_checkout_regular_file")
        || resolution.get("path_role").and_then(Value::as_str)
            != Some("source_checkout_regular_file")
        || preflight.get("verified").and_then(Value::as_bool) != Some(true)
        || resolution.get("verified").and_then(Value::as_bool) != Some(true)
        || preflight
            .get("path")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || resolution
            .get("path")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || !preflight
            .get("sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        || !resolution
            .get("sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        || checker.get("selection").and_then(Value::as_str)
            != Some(
                "running xtask executable from checkout whose HEAD equals source_parent",
            )
        || checker.get("source_sha").and_then(Value::as_str) != source
        || !checker
            .get("executable_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        || materialization.get("path_role").and_then(Value::as_str)
            != Some("os_temp_disposable_checkout")
        || materialization.get("reviewed_tree").and_then(Value::as_str) != tree
        || !materialization
            .get("disposable_commit")
            .and_then(Value::as_str)
            .is_some_and(|value| is_exact_lower_hex(value, 40))
        || materialization.get("created").and_then(Value::as_bool) != Some(true)
        || materialization.get("clean_before").and_then(Value::as_bool) != Some(true)
        || materialization.get("clean_after").and_then(Value::as_bool) != Some(true)
        || materialization
            .get("worktree_remove_succeeded")
            .and_then(Value::as_bool)
            != Some(true)
        || materialization.get("directory_removed").and_then(Value::as_bool) != Some(true)
        || materialization
            .get("worktree_residue_observed")
            .and_then(Value::as_bool)
            != Some(false)
        || !materialization
            .get("cleanup_failure_reason")
            .is_some_and(Value::is_null)
        || materialization.get("authoritative").and_then(Value::as_bool) != Some(false)
        || observation
            .get("ref_mutation_observed")
            .and_then(Value::as_bool)
            != Some(false)
        || observation
            .get("worktree_registry_changed")
            .and_then(Value::as_bool)
            != Some(false)
        || packet_contract
            .get("runner_owned_exclusive_staging")
            .and_then(Value::as_bool)
            != Some(true)
        || packet_contract.get("create_new_files").and_then(Value::as_bool) != Some(true)
        || packet_contract.get("index_written_last").and_then(Value::as_bool) != Some(true)
        || packet_contract
            .get("atomic_directory_publish")
            .and_then(Value::as_bool)
            != Some(true)
        || packet_contract.get("index").and_then(Value::as_str) != Some(PACKET_INDEX)
        || !report
            .get("commands")
            .is_some_and(|value| value.is_array())
        || !report
            .get("failure_reasons")
            .is_some_and(|value| value.is_array())
        || !report
            .get("invalidation_rules")
            .is_some_and(|value| value.is_array())
        || !report.get("non_claims").is_some_and(|value| value.is_array())
        || !report
            .get("disposable_git_object_write_attempted")
            .is_some_and(Value::is_boolean)
    {
        return Err("resolved-tree receipt has an incompatible canonical shape".to_string());
    }
    if report
        .get("disposable_git_object_write_attempted")
        .and_then(Value::as_bool)
        != materialization.get("created").and_then(Value::as_bool)
    {
        return Err(
            "resolved-tree receipt disposable object attempt differs from materialization"
                .to_string(),
        );
    }
    for field in [
        "authoritative_commit_attempted",
        "branch_attempted",
        "tag_attempted",
        "push_attempted",
        "ref_mutation_attempted",
    ] {
        if report.get(field).and_then(Value::as_bool) != Some(false) {
            return Err(format!(
                "resolved-tree receipt reports forbidden authority attempt {field}"
            ));
        }
    }
    let failure_reasons = report["failure_reasons"]
        .as_array()
        .ok_or_else(|| "resolved-tree receipt failure reasons are not an array".to_string())?;
    let reasons_are_valid = failure_reasons.iter().all(|reason| {
        reason
            .as_str()
            .is_some_and(|value| !value.trim().is_empty())
    });
    if !reasons_are_valid
        || (expected_status == "validated" && !failure_reasons.is_empty())
        || (expected_status == "rejected" && failure_reasons.is_empty())
    {
        return Err(
            "resolved-tree receipt failure reasons differ from its disposition".to_string(),
        );
    }
    Ok(())
}

fn command_receipt_has_exact_fields(receipt: &Value) -> bool {
    object_has_exact_fields(receipt, COMMAND_RECEIPT_FIELDS)
}

pub(crate) fn command_receipt_is_terminal_pass(receipt: &Value, expected: &str) -> bool {
    command_receipt_has_exact_fields(receipt)
        && receipt.get("command").and_then(Value::as_str) == Some(expected)
        && receipt.get("subject_role").and_then(Value::as_str) == Some(command_subject_role(expected))
        && receipt.get("state").and_then(Value::as_str) == Some("passed")
        && receipt.get("exit_code").and_then(Value::as_i64) == Some(0)
        && receipt.get("timeout_bound_ms").and_then(Value::as_u64)
            == Some(duration_ms(COMMAND_TIMEOUT))
        && receipt.get("evidence_present").and_then(Value::as_bool) == Some(true)
        && receipt.get("stdout_path").and_then(Value::as_str).is_some()
        && receipt.get("stdout_bytes").and_then(Value::as_u64).is_some()
        && receipt
            .get("stdout_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        && receipt.get("stdout_truncated").and_then(Value::as_bool).is_some()
        && receipt.get("stderr_path").and_then(Value::as_str).is_some()
        && receipt.get("stderr_bytes").and_then(Value::as_u64).is_some()
        && receipt
            .get("stderr_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        && receipt.get("stderr_truncated").and_then(Value::as_bool).is_some()
        && receipt.get("failure_reason").is_some_and(Value::is_null)
}

pub(crate) fn command_receipt_is_j5_failed(receipt: &Value, expected: &str) -> bool {
    command_receipt_has_exact_fields(receipt)
        && receipt.get("command").and_then(Value::as_str) == Some(expected)
        && receipt.get("subject_role").and_then(Value::as_str)
            == Some(command_subject_role(expected))
        && receipt.get("state").and_then(Value::as_str) == Some("failed")
        && receipt
            .get("exit_code")
            .and_then(Value::as_i64)
            .is_some_and(|exit_code| exit_code != 0)
        && receipt.get("timeout_bound_ms").and_then(Value::as_u64)
            == Some(duration_ms(COMMAND_TIMEOUT))
        && receipt.get("evidence_present").and_then(Value::as_bool) == Some(true)
        && receipt.get("stdout_path").and_then(Value::as_str).is_some()
        && receipt.get("stdout_bytes").and_then(Value::as_u64).is_some()
        && receipt
            .get("stdout_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        && receipt.get("stdout_truncated").and_then(Value::as_bool).is_some()
        && receipt.get("stderr_path").and_then(Value::as_str).is_some()
        && receipt.get("stderr_bytes").and_then(Value::as_u64).is_some()
        && receipt
            .get("stderr_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        && receipt.get("stderr_truncated").and_then(Value::as_bool).is_some()
        && receipt
            .get("failure_reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| !reason.trim().is_empty())
}

pub(crate) fn command_receipt_is_j5_not_run(receipt: &Value, expected: &str) -> bool {
    command_receipt_has_exact_fields(receipt)
        && receipt.get("command").and_then(Value::as_str) == Some(expected)
        && receipt.get("subject_role").and_then(Value::as_str)
            == Some(command_subject_role(expected))
        && receipt.get("state").and_then(Value::as_str) == Some("not_run")
        && receipt.get("exit_code").is_some_and(Value::is_null)
        && receipt.get("timeout_bound_ms").and_then(Value::as_u64)
            == Some(duration_ms(COMMAND_TIMEOUT))
        && receipt.get("evidence_present").and_then(Value::as_bool) == Some(false)
        && [
            "stdout_path",
            "stdout_bytes",
            "stdout_sha256",
            "stdout_truncated",
            "stderr_path",
            "stderr_bytes",
            "stderr_sha256",
            "stderr_truncated",
        ]
        .into_iter()
        .all(|field| receipt.get(field).is_some_and(Value::is_null))
        && receipt
            .get("failure_reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| !reason.trim().is_empty())
}

fn commands_are_terminal_green(commands: &[Value]) -> bool {
    commands.len() == REQUIRED_COMMANDS.len()
        && commands
            .iter()
            .zip(REQUIRED_COMMANDS)
            .all(|(receipt, expected)| command_receipt_is_terminal_pass(receipt, expected))
}

fn state_earns_validated(state: &ValidationState) -> bool {
    let source = state.inputs.source_parent.as_deref();
    let swarm = state.inputs.swarm_parent.as_deref();
    let tree = state.inputs.reviewed_tree.as_deref();
    source.is_some_and(|value| is_exact_lower_hex(value, 40))
        && swarm.is_some_and(|value| is_exact_lower_hex(value, 40))
        && tree.is_some_and(|value| is_exact_lower_hex(value, 40))
        && state
            .inputs
            .preflight_sha256
            .as_deref()
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        && state
            .inputs
            .resolution_sha256
            .as_deref()
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        && state.inputs.preflight_path.as_deref().is_some_and(|value| !value.is_empty())
        && state.inputs.resolution_path.as_deref().is_some_and(|value| !value.is_empty())
        && state.preflight_verified
        && state.resolution_verified
        && state.checker_source_sha.as_deref() == source
        && state
            .checker_executable_sha256
            .as_deref()
            .is_some_and(|value| is_exact_lower_hex(value, 64))
        && state.materialized_tree.as_deref() == tree
        && state
            .disposable_commit
            .as_deref()
            .is_some_and(|value| is_exact_lower_hex(value, 40))
        && state.materialization_created
        && state.materialization_clean_before
        && state.materialization_clean_after
        && state.worktree_remove_succeeded
        && state.materialization_directory_removed
        && !state.worktree_residue_observed
        && state.cleanup_failure_reason.is_none()
        && !state.ref_mutation_observed
        && !state.worktree_registry_changed
        && commands_are_terminal_green(&state.commands)
        && state.failure_reasons.is_empty()
}

fn report_value(state: &ValidationState) -> Value {
    let status = if state_earns_validated(state) {
        "validated"
    } else {
        "rejected"
    };
    serde_json::json!({
        "schema": RECEIPT_SCHEMA,
        "tool_version": env!("CARGO_PKG_VERSION"),
        "status": status,
        "source_parent": &state.inputs.source_parent,
        "swarm_parent": &state.inputs.swarm_parent,
        "reviewed_tree": &state.inputs.reviewed_tree,
        "preflight": {
            "path_role": "source_checkout_regular_file",
            "path": &state.inputs.preflight_path,
            "sha256": &state.inputs.preflight_sha256,
            "verified": state.preflight_verified,
        },
        "resolution_manifest": {
            "path_role": "source_checkout_regular_file",
            "path": &state.inputs.resolution_path,
            "sha256": &state.inputs.resolution_sha256,
            "verified": state.resolution_verified,
        },
        "trusted_checker": {
            "selection": "running xtask executable from checkout whose HEAD equals source_parent",
            "source_sha": &state.checker_source_sha,
            "executable_sha256": &state.checker_executable_sha256,
        },
        "materialization": {
            "path_role": "os_temp_disposable_checkout",
            "reviewed_tree": &state.materialized_tree,
            "disposable_commit": &state.disposable_commit,
            "created": state.materialization_created,
            "clean_before": state.materialization_clean_before,
            "clean_after": state.materialization_clean_after,
            "worktree_remove_succeeded": state.worktree_remove_succeeded,
            "directory_removed": state.materialization_directory_removed,
            "worktree_residue_observed": state.worktree_residue_observed,
            "cleanup_failure_reason": &state.cleanup_failure_reason,
            "authoritative": false,
        },
        "required_command_catalog": REQUIRED_COMMANDS,
        "commands": &state.commands,
        "repository_observation": {
            "ref_mutation_observed": state.ref_mutation_observed,
            "worktree_registry_changed": state.worktree_registry_changed,
        },
        "packet_contract": {
            "runner_owned_exclusive_staging": true,
            "create_new_files": true,
            "index_written_last": true,
            "atomic_directory_publish": true,
            "index": PACKET_INDEX,
        },
        "disposable_git_object_write_attempted": state.materialization_created,
        "authoritative_commit_attempted": false,
        "branch_attempted": false,
        "tag_attempted": false,
        "push_attempted": false,
        "ref_mutation_attempted": false,
        "failure_reasons": &state.failure_reasons,
        "invalidation_rules": INVALIDATION_RULES,
        "non_claims": NON_CLAIMS,
    })
}

pub(crate) fn resolved_tree_receipt_is_admissible(report: &Value) -> bool {
    if validate_resolved_tree_receipt_contract(report, "validated").is_err() {
        return false;
    }
    let commands = match report.get("commands").and_then(Value::as_array) {
        Some(commands) => commands,
        None => return false,
    };
    if !commands_are_terminal_green(commands) {
        return false;
    }
    true
}
