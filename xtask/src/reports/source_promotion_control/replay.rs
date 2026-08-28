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
        ("integration index digest", input.integration_index_sha256, 64),
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
    validate_control_packet(
        &builder_packet,
        BUILDER_REPORT,
        &builder,
        "trusted builder",
    )?;
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
    let validation_receipt_sha256 =
        packet_file_sha256(&validation_packet, VALIDATION_REPORT)?;
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
            let (_, qualification_sha256) = qualification.as_ref().ok_or_else(|| {
                "constructor replay requires qualification evidence".to_string()
            })?;
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
            let indexed = packet.files.get(&expected_path).ok_or_else(|| {
                format!("resolved-tree {name} {stream} evidence is not indexed")
            })?;
            if command.get(format!("{stream}_bytes")).and_then(Value::as_u64)
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
        _ => return Err(format!("unsupported admitted control report: {report_name}")),
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
