#[derive(Debug)]
pub(super) struct AdmissionClosureReplayInput<'a> {
    pub(super) validation_packet: &'a Path,
    pub(super) builder_packet: &'a Path,
    pub(super) admission_packet: &'a Path,
    pub(super) integration_index: &'a Path,
    pub(super) preflight: &'a Path,
    pub(super) resolution_manifest: &'a Path,
    pub(super) qualification_receipt: Option<&'a Path>,
    pub(super) construction_packet: Option<&'a Path>,
    pub(super) source_parent: &'a str,
    pub(super) swarm_parent: &'a str,
    pub(super) join_tree: &'a str,
    pub(super) protected_w7_ref: &'a str,
    pub(super) preflight_sha256: &'a str,
    pub(super) resolution_sha256: &'a str,
    pub(super) integration_index_sha256: &'a str,
    pub(super) qualification_sha256: Option<&'a str>,
}

#[derive(Debug)]
pub(super) struct ReplayedControllerReceipts {
    pub(super) admission: Value,
    pub(super) construction: Option<Value>,
}

pub(super) fn replay_rejected_builder_packet(
    root: &Path,
    source_parent: &str,
    workflow_source_sha: &str,
) -> Result<Value, String> {
    let receipt = read_rejected_control_packet(root, "trusted_builder", BUILDER_REPORT)?;
    validate_exact_json_fields(
        &receipt,
        "rejected builder receipt",
        &[
            "schema", "status", "source_parent", "workflow_source_sha", "clean_checkout",
            "rust_toolchain", "cargo_lock_sha256", "locked_build", "isolated_cargo_target_dir",
            "executable_sha256", "failure_reasons", "authoritative_commit_attempted",
            "commit_tree_attempts", "local_ref_attempts", "remote_push_attempts",
            "merge_command_attempts", "merge_command", "ref_mutation_attempted", "push_attempted",
        ],
        &["non_claims"],
    )?;
    if json_string(&receipt, "schema") != Some(BUILDER_SCHEMA)
        || optional_exact_string(&receipt, "source_parent", source_parent).is_err()
        || optional_exact_string(&receipt, "workflow_source_sha", workflow_source_sha).is_err()
    {
        return Err("rejected builder receipt has invalid provenance".to_string());
    }
    validate_rejected_no_authority(&receipt, false)?;
    Ok(receipt)
}

pub(super) fn replay_rejected_admission_closure(
    input: &AdmissionClosureReplayInput<'_>,
    require_exact_identity: bool,
) -> Result<Value, String> {
    let identity = PromotionIdentity {
        source_parent: input.source_parent.to_string(),
        swarm_parent: input.swarm_parent.to_string(),
        join_tree: input.join_tree.to_string(),
        preflight_sha256: input.preflight_sha256.to_string(),
        resolution_sha256: input.resolution_sha256.to_string(),
    };
    validate_rejected_admission_prefix(input, &identity)?;
    let receipt = read_rejected_control_packet(
        input.admission_packet,
        "resolved_tree_admission",
        ADMISSION_REPORT,
    )?;
    validate_exact_json_fields(
        &receipt,
        "rejected admission receipt",
        &[
            "schema", "status", "identity", "source_parent", "swarm_parent", "join_tree",
            "preflight_sha256", "resolution_manifest_sha256",
            "all_required_typed_integration_receipts_present", "final_identity_reread_passed",
            "constructor_eligible_after_tree_qualification", "authoritative_commit_attempted",
            "commit_tree_attempts", "local_ref_attempts", "remote_push_attempts",
            "merge_command_attempts", "ref_mutation_attempted", "push_attempted", "merge_command",
            "failure_reasons",
        ],
        &["non_claims"],
    )?;
    if json_string(&receipt, "schema") != Some(ADMISSION_SCHEMA) {
        return Err("rejected admission receipt uses an unsupported schema".to_string());
    }
    let has_identity = [
        "source_parent",
        "swarm_parent",
        "join_tree",
        "preflight_sha256",
        "resolution_manifest_sha256",
    ]
    .into_iter()
    .any(|key| !receipt[key].is_null());
    if (require_exact_identity || has_identity) && !identity.matches_json(&receipt) {
        return Err("rejected admission receipt differs from exact workflow identity".to_string());
    }
    if receipt
        .get("identity")
        .is_some_and(|value| !value.is_null())
    {
        let embedded = receipt
            .get("identity")
            .ok_or_else(|| "rejected admission receipt is missing identity".to_string())?;
        validate_exact_json_fields(
            embedded,
            "rejected admission embedded identity",
            &[
                "source_parent",
                "swarm_parent",
                "join_tree",
                "preflight_sha256",
                "resolution_manifest_sha256",
            ],
            &[],
        )?;
        if embedded != &identity.as_json() {
            return Err(
                "rejected admission embedded identity differs from workflow identity".to_string(),
            );
        }
    }
    for key in [
        "all_required_typed_integration_receipts_present",
        "final_identity_reread_passed",
        "constructor_eligible_after_tree_qualification",
    ] {
        if json_bool(&receipt, key) != Some(false) {
            return Err(format!(
                "rejected admission receipt reports forbidden {key}"
            ));
        }
    }
    validate_rejected_no_authority(&receipt, false)?;
    Ok(receipt)
}

pub(super) fn replay_rejected_construction_packet(
    root: &Path,
    input: &AdmissionClosureReplayInput<'_>,
) -> Result<Value, String> {
    let receipt =
        read_rejected_control_packet(root, "exact_join_construction", CONSTRUCTION_REPORT)?;
    validate_exact_json_fields(
        &receipt,
        "rejected construction receipt",
        &[
            "schema", "status", "source_parent", "swarm_parent", "join_tree",
            "preflight_sha256", "resolution_manifest_sha256", "candidate_ref", "join_commit",
            "ordered_parents", "final_identity_reread_passed", "refs_unchanged",
            "authoritative_commit_attempted", "commit_tree_attempts", "local_ref_attempts",
            "remote_push_attempts", "merge_command_attempts", "unreferenced_exact_join_constructed",
            "ref_mutation_attempted", "push_attempted", "merge_command", "failure_reasons",
        ],
        &["non_claims"],
    )?;
    if json_string(&receipt, "schema") != Some(CONSTRUCTION_SCHEMA) {
        return Err("rejected construction receipt uses an unsupported schema".to_string());
    }
    let attempted = json_bool(&receipt, "authoritative_commit_attempted") == Some(true);
    for (key, expected) in [
        ("source_parent", input.source_parent),
        ("swarm_parent", input.swarm_parent),
        ("join_tree", input.join_tree),
        ("preflight_sha256", input.preflight_sha256),
        ("resolution_manifest_sha256", input.resolution_sha256),
    ] {
        if attempted {
            if json_string(&receipt, key) != Some(expected) {
                return Err(format!(
                    "attempted rejected construction receipt has missing or mismatched {key}"
                ));
            }
        } else {
            optional_exact_string(&receipt, key, expected)?;
        }
    }
    if !receipt["join_commit"].is_null()
        || !receipt["refs_unchanged"].is_null()
        || receipt["ordered_parents"]
            .as_array()
            .is_none_or(|parents| !parents.is_empty())
        || json_bool(&receipt, "final_identity_reread_passed") != Some(false)
        || json_bool(&receipt, "unreferenced_exact_join_constructed") != Some(false)
    {
        return Err("rejected construction receipt claims constructed join authority".to_string());
    }
    validate_rejected_no_authority(&receipt, true)?;
    Ok(receipt)
}

#[cfg(test)]
pub(super) fn render_rejected_control_markdown(
    report_name: &str,
    report: &Value,
) -> Result<String, String> {
    let (title, claim) = match report_name {
        BUILDER_REPORT => (
            BUILDER_TITLE,
            "A rejected builder packet grants no validation, construction, ref, merge, or publication authority.",
        ),
        ADMISSION_REPORT => (
            ADMISSION_TITLE,
            "A rejected admission packet is not construction eligibility and grants no object, ref, merge, release, or publication authority.",
        ),
        CONSTRUCTION_REPORT => (
            CONSTRUCTION_TITLE,
            "A rejected construction packet grants no ref, merge, release, or publication authority.",
        ),
        _ => {
            return Err(format!(
                "unsupported rejected control report: {report_name}"
            ));
        }
    };
    render_control_markdown(title, report, claim)
}

fn read_rejected_control_packet(
    root: &Path,
    kind: &str,
    report_name: &str,
) -> Result<Value, String> {
    let packet = read_indexed_packet(
        root,
        CONTROL_PACKET_SCHEMA,
        Some(kind),
        Some("rejected"),
        report_name,
    )?;
    let receipt = packet_json(&packet, report_name, "rejected controller receipt")?;
    if json_string(&receipt, "status") != Some("rejected") {
        return Err("rejected controller index differs from receipt status".to_string());
    }
    validate_exact_packet_inventory(
        &packet,
        &BTreeSet::from([
            "control-attempt.json".to_string(),
            report_name.to_string(),
            markdown_sibling(report_name)?,
        ]),
        "rejected controller",
    )?;
    let claims: &[&str] = match report_name {
        BUILDER_REPORT => &[
            BUILDER_SUCCESS_CLAIM,
            "A rejected builder packet grants no validation, construction, ref, merge, or publication authority.",
        ],
        ADMISSION_REPORT => &[
            "A rejected admission packet is not construction eligibility and grants no object, ref, merge, release, or publication authority.",
        ],
        CONSTRUCTION_REPORT => &[
            "A rejected construction packet grants no ref, merge, release, or publication authority.",
        ],
        _ => {
            return Err(format!(
                "unsupported rejected control report: {report_name}"
            ));
        }
    };
    let title = match report_name {
        BUILDER_REPORT => BUILDER_TITLE,
        ADMISSION_REPORT => ADMISSION_TITLE,
        CONSTRUCTION_REPORT => CONSTRUCTION_TITLE,
        _ => {
            return Err(format!(
                "unsupported rejected control report: {report_name}"
            ));
        }
    };
    let canonical = claims
        .iter()
        .map(|claim| render_control_markdown(title, &receipt, claim))
        .collect::<Result<Vec<_>, _>>()?;
    let markdown = packet
        .files
        .get(&markdown_sibling(report_name)?)
        .ok_or_else(|| "rejected controller packet is missing Markdown".to_string())?;
    if canonical
        .iter()
        .all(|expected| markdown.contents != expected.as_bytes())
    {
        return Err("rejected controller Markdown differs from canonical receipt".to_string());
    }
    Ok(receipt)
}

fn validate_rejected_no_authority(receipt: &Value, allow_commit_tree: bool) -> Result<(), String> {
    receipt
        .get("failure_reasons")
        .and_then(Value::as_array)
        .filter(|reasons| {
            !reasons.is_empty()
                && reasons.iter().all(|reason| {
                    reason.as_str().is_some_and(|value| {
                        !value.trim().is_empty() && !value.contains(['\n', '\r', '\0'])
                    })
                })
        })
        .ok_or_else(|| "rejected controller receipt has no failure reason".to_string())?;
    for key in ["ref_mutation_attempted", "push_attempted"] {
        if json_bool(receipt, key) != Some(false) {
            return Err(format!(
                "rejected controller receipt reports forbidden {key}"
            ));
        }
    }
    let commit_tree_attempts = receipt
        .get("commit_tree_attempts")
        .and_then(Value::as_u64)
        .ok_or_else(|| "rejected controller receipt is missing commit_tree_attempts".to_string())?;
    let authoritative = json_bool(receipt, "authoritative_commit_attempted").ok_or_else(|| {
        "rejected controller receipt is missing authoritative attempt state".to_string()
    })?;
    if (!allow_commit_tree && (authoritative || commit_tree_attempts != 0))
        || (allow_commit_tree
            && (commit_tree_attempts > 1 || authoritative != (commit_tree_attempts == 1)))
    {
        return Err(
            "rejected controller receipt has inconsistent commit-tree authority".to_string(),
        );
    }
    for key in [
        "local_ref_attempts",
        "remote_push_attempts",
        "merge_command_attempts",
    ] {
        if receipt.get(key).and_then(Value::as_u64) != Some(0) {
            return Err(format!(
                "rejected controller receipt reports forbidden {key}"
            ));
        }
    }
    if !receipt["merge_command"].is_null() {
        return Err("rejected controller receipt contains a merge command".to_string());
    }
    Ok(())
}

fn optional_exact_string(receipt: &Value, key: &str, expected: &str) -> Result<(), String> {
    if !receipt[key].is_null() && json_string(receipt, key) != Some(expected) {
        return Err(format!("rejected controller receipt has mismatched {key}"));
    }
    Ok(())
}

fn validate_rejected_admission_prefix(
    input: &AdmissionClosureReplayInput<'_>,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    let validation_packet = read_indexed_packet(
        input.validation_packet,
        RESOLVED_TREE_PACKET_SCHEMA,
        None,
        Some("validated"),
        VALIDATION_REPORT,
    )?;
    let validation = packet_json(
        &validation_packet,
        VALIDATION_REPORT,
        "resolved-tree validation receipt",
    )?;
    validate_packet_markdown(
        &validation_packet,
        VALIDATION_REPORT,
        &super::source_promotion_validate_resolved_tree::render_markdown(&validation)?,
        "resolved-tree validation",
    )?;
    validate_resolved_tree_binding(&validation, identity)?;
    validate_validation_command_evidence(&validation_packet, &validation)?;
    let (preflight, preflight_bytes) = read_bound_json(
        input.preflight,
        input.preflight_sha256,
        "finalized P1 preflight",
    )?;
    validate_preflight(&preflight, input.source_parent)?;
    validate_preflight_identity(&preflight, identity)?;
    if json_string(&preflight, "swarm_ref") != Some(input.protected_w7_ref) {
        return Err("preflight protected W7 ref differs from workflow identity".to_string());
    }
    let (manifest, _) = read_bound_json(
        input.resolution_manifest,
        input.resolution_sha256,
        "complete resolution manifest",
    )?;
    validate_manifest(&manifest, &preflight, &digest_bytes(&preflight_bytes))?;
    validate_manifest_identity(&manifest, identity)?;
    let builder_packet = read_indexed_packet(
        input.builder_packet,
        CONTROL_PACKET_SCHEMA,
        Some("trusted_builder"),
        Some("built"),
        BUILDER_REPORT,
    )?;
    let builder = packet_json(&builder_packet, BUILDER_REPORT, "trusted builder receipt")?;
    validate_control_packet(&builder_packet, BUILDER_REPORT, &builder, "trusted builder")?;
    let executable = validate_builder_receipt_contract(&builder, &validation, identity)?;
    validate_integration_index(
        input.integration_index,
        input.integration_index_sha256,
        identity,
        &executable,
    )?;
    Ok(())
}

pub(super) fn replay_admitted_closure(
    input: &AdmissionClosureReplayInput<'_>,
) -> Result<ReplayedControllerReceipts, String> {
    let identity = PromotionIdentity {
        source_parent: input.source_parent.to_string(),
        swarm_parent: input.swarm_parent.to_string(),
        join_tree: input.join_tree.to_string(),
        preflight_sha256: input.preflight_sha256.to_string(),
        resolution_sha256: input.resolution_sha256.to_string(),
    };
    for (label, value, width) in [
        ("source parent", identity.source_parent.as_str(), 40),
        ("swarm parent", identity.swarm_parent.as_str(), 40),
        ("join tree", identity.join_tree.as_str(), 40),
        ("preflight digest", identity.preflight_sha256.as_str(), 64),
        ("resolution digest", identity.resolution_sha256.as_str(), 64),
        (
            "integration index digest",
            input.integration_index_sha256,
            64,
        ),
    ] {
        validate_exact_hex(label, value, width)?;
    }
    validate_full_ref(input.protected_w7_ref, "protected W7 ref")?;

    let validation_packet = read_indexed_packet(
        input.validation_packet,
        RESOLVED_TREE_PACKET_SCHEMA,
        None,
        Some("validated"),
        VALIDATION_REPORT,
    )?;
    let validation = packet_json(
        &validation_packet,
        VALIDATION_REPORT,
        "resolved-tree validation receipt",
    )?;
    validate_packet_markdown(
        &validation_packet,
        VALIDATION_REPORT,
        &super::source_promotion_validate_resolved_tree::render_markdown(&validation)?,
        "resolved-tree validation",
    )?;
    validate_resolved_tree_binding(&validation, &identity)?;
    validate_validation_command_evidence(&validation_packet, &validation)?;

    let (preflight, preflight_bytes) = read_bound_json(
        input.preflight,
        input.preflight_sha256,
        "finalized P1 preflight",
    )?;
    validate_preflight(&preflight, input.source_parent)?;
    validate_preflight_identity(&preflight, &identity)?;
    if json_string(&preflight, "swarm_ref") != Some(input.protected_w7_ref) {
        return Err("preflight protected W7 ref differs from workflow identity".to_string());
    }
    let (manifest, _) = read_bound_json(
        input.resolution_manifest,
        input.resolution_sha256,
        "complete resolution manifest",
    )?;
    validate_manifest(&manifest, &preflight, &digest_bytes(&preflight_bytes))?;
    validate_manifest_identity(&manifest, &identity)?;

    let builder_packet = read_indexed_packet(
        input.builder_packet,
        CONTROL_PACKET_SCHEMA,
        Some("trusted_builder"),
        Some("built"),
        BUILDER_REPORT,
    )?;
    let builder = packet_json(&builder_packet, BUILDER_REPORT, "trusted builder receipt")?;
    validate_control_packet(&builder_packet, BUILDER_REPORT, &builder, "trusted builder")?;
    let executable = validate_builder_receipt_contract(&builder, &validation, &identity)?;
    let integration = validate_integration_index(
        input.integration_index,
        input.integration_index_sha256,
        &identity,
        &executable,
    )?;

    let admission_packet = read_indexed_packet(
        input.admission_packet,
        CONTROL_PACKET_SCHEMA,
        Some("resolved_tree_admission"),
        Some("admitted"),
        ADMISSION_REPORT,
    )?;
    let admission = packet_json(
        &admission_packet,
        ADMISSION_REPORT,
        "resolved-tree admission receipt",
    )?;
    validate_control_packet(
        &admission_packet,
        ADMISSION_REPORT,
        &admission,
        "resolved-tree admission",
    )?;
    validate_admission_receipt(&admission, &identity)?;
    let validation_receipt_sha256 = packet_file_sha256(&validation_packet, VALIDATION_REPORT)?;
    let builder_receipt_sha256 = packet_file_sha256(&builder_packet, BUILDER_REPORT)?;
    let integration_receipts = serde_json::to_value(&integration.receipt_digests)
        .map_err(|error| format!("could not serialize integration receipts: {error}"))?;
    if json_string(&admission, "swarm_ref") != Some(input.protected_w7_ref)
        || json_string(&admission, "resolved_tree_packet_index_sha256")
            != Some(validation_packet.index_sha256.as_str())
        || json_string(&admission, "resolved_tree_validation_receipt_sha256")
            != Some(validation_receipt_sha256.as_str())
        || json_string(&admission, "trusted_builder_packet_index_sha256")
            != Some(builder_packet.index_sha256.as_str())
        || json_string(&admission, "trusted_builder_receipt_sha256")
            != Some(builder_receipt_sha256.as_str())
        || json_string(&admission, "integration_index_sha256")
            != Some(integration.index_sha256.as_str())
        || admission.get("integration_receipts") != Some(&integration_receipts)
        || json_string(&admission, "checker_executable_sha256") != Some(executable.as_str())
    {
        return Err("admission receipt differs from replayed controller evidence".to_string());
    }

    let admission_receipt_sha256 = packet_file_sha256(&admission_packet, ADMISSION_REPORT)?;
    let qualification = match input.qualification_receipt {
        Some(qualification_path) => {
            let qualification_sha256 = input.qualification_sha256.ok_or_else(|| {
                "qualification replay is missing the caller-bound digest".to_string()
            })?;
            let (receipt, _) = read_bound_json(
                qualification_path,
                qualification_sha256,
                "tree qualification receipt",
            )?;
            validate_qualification_receipt(
                &receipt,
                &identity,
                &admission,
                &admission_packet,
                &admission_receipt_sha256,
                &integration,
                qualification_sha256,
            )?;
            Some((receipt, qualification_sha256))
        }
        None if input.qualification_sha256.is_none() => None,
        None => {
            return Err("qualification digest has no replayable receipt".to_string());
        }
    };
    let construction = match input.construction_packet {
        Some(construction_root) => {
            let (_, qualification_sha256) = qualification
                .as_ref()
                .ok_or_else(|| "constructor replay requires qualification evidence".to_string())?;
            let packet = read_indexed_packet(
                construction_root,
                CONTROL_PACKET_SCHEMA,
                Some("exact_join_construction"),
                Some("constructed"),
                CONSTRUCTION_REPORT,
            )?;
            let receipt = packet_json(
                &packet,
                CONSTRUCTION_REPORT,
                "exact-join construction receipt",
            )?;
            validate_control_packet(
                &packet,
                CONSTRUCTION_REPORT,
                &receipt,
                "exact-join construction",
            )?;
            let evidence = construction_evidence_from_receipt(&receipt)?;
            validate_construction_receipt(&receipt, &evidence)?;
            if evidence.identity != identity
                || evidence.swarm_ref != input.protected_w7_ref
                || evidence.admission_index_sha256 != admission_packet.index_sha256
                || evidence.admission_receipt_sha256 != admission_receipt_sha256
                || evidence.validation_index_sha256 != validation_packet.index_sha256
                || evidence.integration_index_sha256 != integration.index_sha256
                || evidence.qualification_sha256 != *qualification_sha256
            {
                return Err(
                    "construction receipt differs from replayed controller evidence".to_string(),
                );
            }
            Some(receipt)
        }
        None => None,
    };

    Ok(ReplayedControllerReceipts {
        admission,
        construction,
    })
}

fn validate_validation_command_evidence(
    packet: &IndexedPacket,
    validation: &Value,
) -> Result<(), String> {
    let commands = validation
        .get("commands")
        .and_then(Value::as_array)
        .ok_or_else(|| "resolved-tree validation receipt is missing commands".to_string())?;
    let mut expected_files = BTreeSet::from([
        VALIDATION_REPORT.to_string(),
        markdown_sibling(VALIDATION_REPORT)?,
    ]);
    for (index, command) in commands.iter().enumerate() {
        let name = json_string(command, "command")
            .ok_or_else(|| "resolved-tree command receipt is missing command".to_string())?;
        for stream in ["stdout", "stderr"] {
            let expected_path = format!("commands/{:02}-{name}.{stream}.log", index + 1);
            expected_files.insert(expected_path.clone());
            if json_string(command, &format!("{stream}_path")) != Some(expected_path.as_str()) {
                return Err(format!(
                    "resolved-tree {name} {stream} path differs from canonical indexed evidence"
                ));
            }
            let indexed = packet
                .files
                .get(&expected_path)
                .ok_or_else(|| format!("resolved-tree {name} {stream} evidence is not indexed"))?;
            if command
                .get(format!("{stream}_bytes"))
                .and_then(Value::as_u64)
                != Some(indexed.contents.len() as u64)
                || json_string(command, &format!("{stream}_sha256"))
                    != Some(indexed.sha256.as_str())
            {
                return Err(format!(
                    "resolved-tree {name} {stream} receipt differs from indexed evidence bytes"
                ));
            }
        }
    }
    validate_exact_packet_inventory(packet, &expected_files, "resolved-tree validation")?;
    Ok(())
}

fn validate_control_packet(
    packet: &IndexedPacket,
    report_name: &str,
    report: &Value,
    label: &str,
) -> Result<(), String> {
    let expected = BTreeSet::from([
        "control-attempt.json".to_string(),
        report_name.to_string(),
        markdown_sibling(report_name)?,
    ]);
    validate_exact_packet_inventory(packet, &expected, label)?;
    validate_packet_markdown(
        packet,
        report_name,
        &render_admitted_control_markdown(report_name, report)?,
        label,
    )
}

pub(super) fn render_admitted_control_markdown(
    report_name: &str,
    report: &Value,
) -> Result<String, String> {
    let (title, claim_boundary) = match report_name {
        BUILDER_REPORT => (BUILDER_TITLE, BUILDER_SUCCESS_CLAIM),
        ADMISSION_REPORT => (ADMISSION_TITLE, ADMISSION_SUCCESS_CLAIM),
        CONSTRUCTION_REPORT => (CONSTRUCTION_TITLE, CONSTRUCTION_SUCCESS_CLAIM),
        _ => {
            return Err(format!(
                "unsupported admitted control report: {report_name}"
            ));
        }
    };
    render_control_markdown(title, report, claim_boundary)
}

fn markdown_sibling(report_name: &str) -> Result<String, String> {
    report_name
        .strip_suffix(".json")
        .map(|stem| format!("{stem}.md"))
        .ok_or_else(|| format!("packet report name must end in .json: {report_name}"))
}

fn validate_exact_packet_inventory(
    packet: &IndexedPacket,
    expected: &BTreeSet<String>,
    label: &str,
) -> Result<(), String> {
    let observed = packet.files.keys().cloned().collect::<BTreeSet<_>>();
    if &observed != expected {
        return Err(format!(
            "{label} packet inventory differs from its exact contract: expected {expected:?}, observed {observed:?}"
        ));
    }
    Ok(())
}

fn validate_packet_markdown(
    packet: &IndexedPacket,
    report_name: &str,
    expected: &str,
    label: &str,
) -> Result<(), String> {
    let markdown = markdown_sibling(report_name)?;
    let observed = packet
        .files
        .get(&markdown)
        .ok_or_else(|| format!("{label} packet is missing canonical Markdown"))?;
    if observed.contents != expected.as_bytes() {
        return Err(format!(
            "{label} Markdown differs from its canonical JSON rendering"
        ));
    }
    Ok(())
}
