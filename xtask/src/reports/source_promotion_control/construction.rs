fn construction_out_from_args(args: &[String]) -> PathBuf {
    args.windows(2)
        .find(|pair| {
            pair.first().is_some_and(|value| value == "--out")
                && pair.get(1).is_some_and(|value| !value.trim().is_empty())
        })
        .and_then(|pair| pair.get(1).map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONSTRUCTION_OUT))
}

fn parse_construction_options(args: &[String]) -> Result<ConstructionOptions, String> {
    let parsed = parse_command_args(
        args,
        SOURCE_PROMOTION_CONSTRUCT_EXACT_JOIN_SUBCOMMAND,
        &[
            "--admission-packet",
            "--validation-packet",
            "--integration-index",
            "--integration-index-sha256",
            "--preflight",
            "--resolution-manifest",
            "--qualification-receipt",
            "--qualification-receipt-sha256",
            "--source-main-ref",
            "--swarm-ref",
            "--candidate-ref",
            "--out",
        ],
        &[],
    )?;
    let repo = current_repo()?;
    let resolve = |key: &str| -> Result<PathBuf, String> {
        Ok(resolve_candidate_path(
            &repo,
            &PathBuf::from(parsed.required(key)?),
        ))
    };
    let source_main_ref = parsed.required("--source-main-ref")?;
    let swarm_ref = parsed.required("--swarm-ref")?;
    let candidate_ref = parsed.required("--candidate-ref")?;
    validate_source_main_ref(&source_main_ref)?;
    validate_full_ref(&swarm_ref, "protected W7 ref")?;
    validate_candidate_ref(&candidate_ref)?;
    let admission_packet = resolve("--admission-packet")?;
    let validation_packet = resolve("--validation-packet")?;
    let integration_index = resolve("--integration-index")?;
    let integration_index_sha256 = parsed.required("--integration-index-sha256")?;
    validate_exact_hex(
        "--integration-index-sha256",
        &integration_index_sha256,
        64,
    )?;
    let preflight = resolve("--preflight")?;
    let resolution_manifest = resolve("--resolution-manifest")?;
    let qualification_receipt = resolve("--qualification-receipt")?;
    let qualification_receipt_sha256 = parsed.required("--qualification-receipt-sha256")?;
    validate_exact_hex(
        "tree qualification receipt SHA-256",
        &qualification_receipt_sha256,
        64,
    )?;
    Ok(ConstructionOptions {
        repo,
        admission_packet,
        validation_packet,
        integration_index,
        integration_index_sha256,
        preflight,
        resolution_manifest,
        qualification_receipt,
        qualification_receipt_sha256,
        source_main_ref,
        swarm_ref,
        candidate_ref,
        out: parsed
            .optional("--out")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONSTRUCTION_OUT)),
    })
}

fn construct_exact_join(args: &[String]) -> Result<(), String> {
    let out = construction_out_from_args(args);
    let options = match parse_construction_options(args) {
        Ok(options) => options,
        Err(reason) => {
            let report = construction_rejection_report(None, None, &reason, false);
            return write_rejection_or_combine(
                &out,
                "exact_join_construction",
                CONSTRUCTION_REPORT,
                &report,
                "Exact-join construction",
                "A rejected construction packet grants no ref, merge, release, or publication authority.",
                reason,
            );
        }
    };

    let reconciliation_context = match construction_reconciliation_context(&options) {
        Ok(context) => context,
        Err(reason) => {
            let report =
                construction_rejection_report(None, Some(&options.candidate_ref), &reason, false);
            return write_rejection_or_combine(
                &options.out,
                "exact_join_construction",
                CONSTRUCTION_REPORT,
                &report,
                "Exact-join construction",
                "A rejected construction packet grants no ref, merge, release, or publication authority.",
                reason,
            );
        }
    };

    let reservation = reserve_control_packet_output(
        &options.out,
        "exact_join_construction",
        &reconciliation_context,
    )?;
    if let Err(reason) = require_reconciliation_context_unchanged(
        &reconciliation_context,
        construction_reconciliation_context(&options),
        "construction",
    ) {
        let report =
            construction_rejection_report(None, Some(&options.candidate_ref), &reason, false);
        return write_reserved_rejection_or_combine(
            &reservation,
            "exact_join_construction",
            CONSTRUCTION_REPORT,
            &report,
            "Exact-join construction",
            "A rejected construction packet grants no ref, merge, release, or publication authority.",
            reason,
        );
    }

    match construct_exact_join_inner(&options, Some(&reconciliation_context)) {
        Ok(evidence) => {
            let report = construction_success_report(&evidence);
            write_reserved_control_packet(
                &reservation,
                "exact_join_construction",
                CONSTRUCTION_REPORT,
                &report,
                "Exact-join construction",
                "This packet records one exact unreferenced direct two-parent object after terminal tree qualification. It moves no ref and grants no release or publication authority.",
            )
        }
        Err(failure) => {
            let (reason, identity, attempted) = failure;
            let report = construction_rejection_report(
                identity.as_deref(),
                Some(&options.candidate_ref),
                &reason,
                attempted,
            );
            write_reserved_rejection_or_combine(
                &reservation,
                "exact_join_construction",
                CONSTRUCTION_REPORT,
                &report,
                "Exact-join construction",
                "A rejected construction packet grants no ref, merge, release, or publication authority.",
                reason,
            )
        }
    }
}

fn construction_reconciliation_context(options: &ConstructionOptions) -> Result<Value, String> {
    let admission_packet = read_indexed_packet(
        &options.admission_packet,
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
    let identity = identity_from_admission(&admission)?;
    validate_admission_receipt(&admission, &identity)?;
    if json_string(&admission, "swarm_ref") != Some(options.swarm_ref.as_str()) {
        return Err("constructor W7 ref differs from admitted protected ref".to_string());
    }
    let validation_packet = read_indexed_packet(
        &options.validation_packet,
        RESOLVED_TREE_PACKET_SCHEMA,
        None,
        Some("validated"),
        VALIDATION_REPORT,
    )?;
    let source_head = read_commit_ref(&options.repo, &options.source_main_ref, "source main ref")?;
    let swarm_head = read_commit_ref(&options.repo, &options.swarm_ref, "protected W7 ref")?;
    if source_head != identity.source_parent || swarm_head != identity.swarm_parent {
        return Err("source or W7 ref differs from the admitted construction identity".to_string());
    }
    let commit_timestamp = canonical_join_timestamp(&options.repo, &identity)?;
    let qualification_sha256 = file_sha256(
        &options.qualification_receipt,
        "tree qualification receipt",
    )?;
    require_expected_qualification_sha256(
        &options.qualification_receipt_sha256,
        &qualification_sha256,
    )?;
    let integration_index_sha256 = file_sha256(&options.integration_index, "integration index")?;
    if integration_index_sha256 != options.integration_index_sha256 {
        return Err(format!(
            "integration index SHA-256 mismatch: expected {}, observed {integration_index_sha256}",
            options.integration_index_sha256
        ));
    }
    construction_reconciliation_value(
        options,
        &identity,
        &admission_packet,
        &validation_packet,
        &integration_index_sha256,
        &qualification_sha256,
        &commit_timestamp,
    )
}

fn require_expected_qualification_sha256(
    expected_sha256: &str,
    actual_sha256: &str,
) -> Result<(), String> {
    if actual_sha256 != expected_sha256 {
        return Err(format!(
            "tree qualification receipt SHA-256 differs from caller-bound digest: expected {}, observed {actual_sha256}",
            expected_sha256
        ));
    }
    Ok(())
}

fn construction_reconciliation_value(
    options: &ConstructionOptions,
    identity: &PromotionIdentity,
    admission_packet: &IndexedPacket,
    validation_packet: &IndexedPacket,
    integration_index_sha256: &str,
    qualification_sha256: &str,
    commit_timestamp: &str,
) -> Result<Value, String> {
    Ok(serde_json::json!({
        "source_parent": identity.source_parent,
        "swarm_parent": identity.swarm_parent,
        "join_tree": identity.join_tree,
        "preflight_sha256": identity.preflight_sha256,
        "resolution_manifest_sha256": identity.resolution_sha256,
        "source_main_ref": options.source_main_ref,
        "swarm_ref": options.swarm_ref,
        "candidate_ref": options.candidate_ref,
        "admission_packet_index_sha256": admission_packet.index_sha256,
        "admission_receipt_sha256": packet_file_sha256(admission_packet, ADMISSION_REPORT)?,
        "resolved_tree_packet_index_sha256": validation_packet.index_sha256,
        "integration_index_sha256": integration_index_sha256,
        "tree_qualification_receipt_sha256": qualification_sha256,
        "commit_author_name": JOIN_AUTHOR_NAME,
        "commit_author_email": JOIN_AUTHOR_EMAIL,
        "commit_timestamp": commit_timestamp,
        "commit_message_sha256": digest_bytes(JOIN_MESSAGE.as_bytes()),
        "maximum_commit_tree_attempts": 1,
        "maximum_local_ref_attempts": 0,
        "maximum_remote_push_attempts": 0,
        "maximum_merge_command_attempts": 0,
    }))
}

fn construct_exact_join_inner(
    options: &ConstructionOptions,
    expected_reconciliation_context: Option<&Value>,
) -> Result<ConstructionEvidence, ConstructionFailure> {
    let admission_packet = read_indexed_packet(
        &options.admission_packet,
        CONTROL_PACKET_SCHEMA,
        Some("resolved_tree_admission"),
        Some("admitted"),
        ADMISSION_REPORT,
    )
    .map_err(|reason| (reason, None, false))?;
    let admission = packet_json(
        &admission_packet,
        ADMISSION_REPORT,
        "resolved-tree admission receipt",
    )
    .map_err(|reason| (reason, None, false))?;
    let identity =
        Box::new(identity_from_admission(&admission).map_err(|reason| (reason, None, false))?);
    validate_admission_receipt(&admission, &identity)
        .map_err(|reason| (reason, Some(identity.clone()), false))?;

    if json_string(&admission, "swarm_ref") != Some(options.swarm_ref.as_str()) {
        return Err((
            "constructor W7 ref differs from admitted protected ref".to_string(),
            Some(identity),
            false,
        ));
    }

    let validation_packet = read_indexed_packet(
        &options.validation_packet,
        RESOLVED_TREE_PACKET_SCHEMA,
        None,
        Some("validated"),
        VALIDATION_REPORT,
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;
    if validation_packet.index_sha256
        != json_string(&admission, "resolved_tree_packet_index_sha256").unwrap_or_default()
    {
        return Err((
            "resolved-tree packet index moved after admission".to_string(),
            Some(identity),
            false,
        ));
    }
    let validation = packet_json(
        &validation_packet,
        VALIDATION_REPORT,
        "resolved-tree validation receipt",
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;
    validate_resolved_tree_binding(&validation, &identity)
        .map_err(|reason| (reason, Some(identity.clone()), false))?;
    let validation_receipt_sha256 = packet_file_sha256(&validation_packet, VALIDATION_REPORT)
        .map_err(|reason| (reason, Some(identity.clone()), false))?;
    if Some(validation_receipt_sha256.as_str())
        != json_string(&admission, "resolved_tree_validation_receipt_sha256")
    {
        return Err((
            "resolved-tree validation receipt moved after admission".to_string(),
            Some(identity),
            false,
        ));
    }

    let (preflight, preflight_bytes) = read_bound_json(
        &options.preflight,
        &identity.preflight_sha256,
        "finalized P1 preflight",
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;
    validate_preflight(&preflight, &identity.source_parent)
        .and_then(|()| validate_preflight_identity(&preflight, &identity))
        .map_err(|reason| (reason, Some(identity.clone()), false))?;
    let (manifest, _) = read_bound_json(
        &options.resolution_manifest,
        &identity.resolution_sha256,
        "complete resolution manifest",
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;
    validate_manifest(&manifest, &preflight, &digest_bytes(&preflight_bytes))
        .and_then(|()| validate_manifest_identity(&manifest, &identity))
        .map_err(|reason| (reason, Some(identity.clone()), false))?;

    let trusted_executable_sha256 = json_string(&admission, "checker_executable_sha256")
        .ok_or_else(|| {
            (
                "admission receipt is missing checker executable identity".to_string(),
                Some(identity.clone()),
                false,
            )
        })?;
    let integration = validate_integration_index(
        &options.integration_index,
        &options.integration_index_sha256,
        &identity,
        trusted_executable_sha256,
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;
    if Some(integration.index_sha256.as_str())
        != json_string(&admission, "integration_index_sha256")
        || integration.index_sha256 != options.integration_index_sha256
    {
        return Err((
            "integration index moved after admission".to_string(),
            Some(identity),
            false,
        ));
    }

    let (qualification, _) = read_bound_json(
        &options.qualification_receipt,
        &options.qualification_receipt_sha256,
        "tree qualification receipt",
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;
    let qualification_sha256 = options.qualification_receipt_sha256.clone();
    let admission_receipt_sha256 = packet_file_sha256(&admission_packet, ADMISSION_REPORT)
        .map_err(|reason| (reason, Some(identity.clone()), false))?;
    validate_qualification_receipt(
        &qualification,
        &identity,
        &admission,
        &admission_packet,
        &admission_receipt_sha256,
        &qualification_sha256,
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;

    let before = construction_snapshot(
        options,
        &identity,
        &admission_packet,
        &validation_packet,
        &integration,
        &qualification_sha256,
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;
    validate_construction_snapshot(options, &identity, &before)
        .map_err(|reason| (reason, Some(identity.clone()), false))?;

    let final_read = construction_snapshot(
        options,
        &identity,
        &admission_packet,
        &validation_packet,
        &integration,
        &qualification_sha256,
    )
    .map_err(|reason| (reason, Some(identity.clone()), false))?;
    if before != final_read {
        return Err((
            "construction inputs moved during immediate final reread".to_string(),
            Some(identity),
            false,
        ));
    }
    construct_validated_join(
        options,
        ValidatedConstructionInputs {
            identity,
            admission_packet,
            admission_receipt_sha256,
            validation_packet,
            integration_index_sha256: integration.index_sha256,
            qualification_sha256,
        },
        expected_reconciliation_context,
    )
}

struct ValidatedConstructionInputs {
    identity: Box<PromotionIdentity>,
    admission_packet: IndexedPacket,
    admission_receipt_sha256: String,
    validation_packet: IndexedPacket,
    integration_index_sha256: String,
    qualification_sha256: String,
}

fn construct_validated_join(
    options: &ConstructionOptions,
    inputs: ValidatedConstructionInputs,
    expected_reconciliation_context: Option<&Value>,
) -> Result<ConstructionEvidence, ConstructionFailure> {
    let ValidatedConstructionInputs {
        identity,
        admission_packet,
        admission_receipt_sha256,
        validation_packet,
        integration_index_sha256,
        qualification_sha256,
    } = inputs;
    let commit_timestamp = canonical_join_timestamp(&options.repo, &identity)
        .map_err(|reason| (reason, Some(identity.clone()), false))?;
    if let Some(expected_context) = expected_reconciliation_context {
        require_reconciliation_context_unchanged(
            expected_context,
            construction_reconciliation_value(
                options,
                &identity,
                &admission_packet,
                &validation_packet,
                &integration_index_sha256,
                &qualification_sha256,
                &commit_timestamp,
            ),
            "construction",
        )
        .map_err(|reason| (reason, Some(identity.clone()), false))?;
    }

    let refs_before =
        refs_digest(&options.repo).map_err(|reason| (reason, Some(identity.clone()), false))?;
    let join_commit = create_exact_join_object(&options.repo, &identity)
        .map_err(|reason| (reason, Some(identity.clone()), true))?;
    verify_constructed_join(&options.repo, &join_commit, &identity)
        .map_err(|reason| (reason, Some(identity.clone()), true))?;
    let refs_after =
        refs_digest(&options.repo).map_err(|reason| (reason, Some(identity.clone()), true))?;
    if refs_before != refs_after {
        return Err((
            "Git refs changed while constructing the unreferenced exact join".to_string(),
            Some(identity),
            true,
        ));
    }
    if read_commit_ref(&options.repo, &options.source_main_ref, "source main ref")
        .map_err(|reason| (reason, Some(identity.clone()), true))?
        != identity.source_parent.as_str()
        || read_commit_ref(&options.repo, &options.swarm_ref, "protected W7 ref")
            .map_err(|reason| (reason, Some(identity.clone()), true))?
            != identity.swarm_parent.as_str()
    {
        return Err((
            "source or W7 ref moved after exact-join construction".to_string(),
            Some(identity),
            true,
        ));
    }

    Ok(ConstructionEvidence {
        identity: *identity,
        swarm_ref: options.swarm_ref.clone(),
        candidate_ref: options.candidate_ref.clone(),
        admission_index_sha256: admission_packet.index_sha256,
        admission_receipt_sha256,
        validation_index_sha256: validation_packet.index_sha256,
        integration_index_sha256,
        qualification_sha256,
        join_commit,
        commit_timestamp,
    })
}

fn identity_from_admission(admission: &Value) -> Result<PromotionIdentity, String> {
    let required = |key: &str| {
        json_string(admission, key)
            .map(ToString::to_string)
            .ok_or_else(|| format!("admission receipt is missing {key}"))
    };
    let identity = PromotionIdentity {
        source_parent: required("source_parent")?,
        swarm_parent: required("swarm_parent")?,
        join_tree: required("join_tree")?,
        preflight_sha256: required("preflight_sha256")?,
        resolution_sha256: required("resolution_manifest_sha256")?,
    };
    validate_exact_hex("admitted source parent", &identity.source_parent, 40)?;
    validate_exact_hex("admitted swarm parent", &identity.swarm_parent, 40)?;
    validate_exact_hex("admitted join tree", &identity.join_tree, 40)?;
    validate_exact_hex("admitted preflight digest", &identity.preflight_sha256, 64)?;
    validate_exact_hex(
        "admitted resolution digest",
        &identity.resolution_sha256,
        64,
    )?;
    Ok(identity)
}

fn validate_admission_receipt(
    admission: &Value,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    if json_string(admission, "schema") != Some(ADMISSION_SCHEMA)
        || json_string(admission, "status") != Some("admitted")
        || !identity.matches_json(admission)
        || json_bool(admission, "all_required_typed_integration_receipts_present") != Some(true)
        || json_bool(admission, "final_identity_reread_passed") != Some(true)
        || json_bool(admission, "constructor_eligible_after_tree_qualification") != Some(true)
        || !empty_failure_reasons(admission)
    {
        return Err("admission receipt did not earn exact-join eligibility".to_string());
    }
    for key in [
        "authoritative_commit_attempted",
        "ref_mutation_attempted",
        "push_attempted",
    ] {
        if json_bool(admission, key) != Some(false) {
            return Err(format!("admission receipt reports forbidden {key}"));
        }
    }
    for key in [
        "commit_tree_attempts",
        "local_ref_attempts",
        "remote_push_attempts",
        "merge_command_attempts",
    ] {
        if admission.get(key).and_then(Value::as_u64) != Some(0) {
            return Err(format!("admission receipt reports forbidden {key}"));
        }
    }
    if !admission.get("merge_command").is_some_and(Value::is_null) {
        return Err("admission receipt must not contain a merge command".to_string());
    }
    Ok(())
}

fn validate_qualification_receipt(
    qualification: &Value,
    identity: &PromotionIdentity,
    admission: &Value,
    admission_packet: &IndexedPacket,
    admission_receipt_sha256: &str,
    qualification_sha256: &str,
) -> Result<(), String> {
    validate_exact_hex(
        "tree qualification receipt digest",
        qualification_sha256,
        64,
    )?;
    if json_string(qualification, "schema") != Some(QUALIFICATION_SCHEMA)
        || json_string(qualification, "status") != Some("qualified")
        || !identity.matches_json(qualification)
        || json_bool(qualification, "promotion_ref_mutation_attempted") != Some(false)
        || !empty_failure_reasons(qualification)
    {
        return Err("TREE_QUALIFICATION receipt is not terminal qualified and exact".to_string());
    }
    if json_string(qualification, "admission_packet_index_sha256")
        != Some(admission_packet.index_sha256.as_str())
        || json_string(qualification, "admission_receipt_sha256") != Some(admission_receipt_sha256)
        || json_string(qualification, "resolved_tree_validation_receipt_sha256")
            != json_string(admission, "resolved_tree_validation_receipt_sha256")
        || json_string(qualification, "network_policy_receipt_sha256")
            != admission
                .get("integration_receipts")
                .and_then(|value| value.get("network_policy_integration"))
                .and_then(Value::as_str)
    {
        return Err(
            "TREE_QUALIFICATION receipt is bound to different admission evidence".to_string(),
        );
    }
    let lanes = qualification
        .get("lanes")
        .and_then(Value::as_array)
        .ok_or_else(|| "TREE_QUALIFICATION receipt is missing lanes".to_string())?;
    if lanes.len() != REQUIRED_QUALIFICATION_LANES.len() {
        return Err(format!(
            "TREE_QUALIFICATION receipt must contain exactly {} required lanes",
            REQUIRED_QUALIFICATION_LANES.len()
        ));
    }
    for (lane, required_name) in lanes.iter().zip(REQUIRED_QUALIFICATION_LANES) {
        let name = json_string(lane, "name")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "qualification lane is missing name".to_string())?;
        if name != *required_name {
            return Err(format!(
                "qualification lane denominator mismatch: expected {required_name}, observed {name}"
            ));
        }
        if json_string(lane, "state") != Some("passed")
            || !json_string(lane, "evidence_sha256")
                .is_some_and(|value| is_exact_lower_hex(value, 64))
        {
            return Err(format!(
                "qualification lane {name} is not terminal passed with evidence"
            ));
        }
    }
    Ok(())
}

fn construction_snapshot(
    options: &ConstructionOptions,
    identity: &PromotionIdentity,
    admission_packet: &IndexedPacket,
    validation_packet: &IndexedPacket,
    integration: &IntegrationEvidence,
    qualification_sha256: &str,
) -> Result<ConstructionSnapshot, String> {
    let source_head = read_commit_ref(&options.repo, &options.source_main_ref, "source main ref")?;
    let swarm_head = read_commit_ref(&options.repo, &options.swarm_ref, "protected W7 ref")?;
    let join_tree = read_tree_identity(&options.repo, &identity.join_tree)?;
    let preflight_sha256 = file_sha256(&options.preflight, "finalized P1 preflight")?;
    let resolution_sha256 =
        file_sha256(&options.resolution_manifest, "complete resolution manifest")?;
    let validation_index_sha256 = file_sha256(
        &options.validation_packet.join(PACKET_INDEX),
        "resolved-tree packet index",
    )?;
    let admission_index_sha256 = file_sha256(
        &options.admission_packet.join(PACKET_INDEX),
        "admission packet index",
    )?;
    let integration_index_sha256 = file_sha256(&options.integration_index, "integration index")?;
    let observed_qualification =
        file_sha256(&options.qualification_receipt, "tree qualification receipt")?;
    require_expected_qualification_sha256(
        &options.qualification_receipt_sha256,
        &observed_qualification,
    )?;
    if admission_index_sha256 != admission_packet.index_sha256
        || validation_index_sha256 != validation_packet.index_sha256
        || integration_index_sha256 != options.integration_index_sha256
        || integration_index_sha256 != integration.index_sha256
        || observed_qualification != qualification_sha256
    {
        return Err("construction sidecar moved after validation".to_string());
    }
    Ok(ConstructionSnapshot {
        source_head,
        swarm_head,
        join_tree,
        preflight_sha256,
        resolution_sha256,
        validation_index_sha256,
        admission_index_sha256,
        integration_index_sha256,
        qualification_sha256: observed_qualification,
    })
}

fn validate_construction_snapshot(
    options: &ConstructionOptions,
    identity: &PromotionIdentity,
    snapshot: &ConstructionSnapshot,
) -> Result<(), String> {
    if snapshot.source_head != identity.source_parent.as_str() {
        return Err("source main no longer equals admitted SOURCE_PARENT".to_string());
    }
    if snapshot.swarm_head != identity.swarm_parent.as_str() {
        return Err("protected W7 ref no longer equals admitted SWARM_PARENT".to_string());
    }
    if snapshot.join_tree != identity.join_tree.as_str() {
        return Err("JOIN_TREE moved after admission".to_string());
    }
    if snapshot.preflight_sha256 != identity.preflight_sha256.as_str() {
        return Err("finalized P1 bytes moved after admission".to_string());
    }
    if snapshot.resolution_sha256 != identity.resolution_sha256.as_str() {
        return Err("complete resolution manifest moved after admission".to_string());
    }
    if json_string(
        &packet_json(
            &read_indexed_packet(
                &options.admission_packet,
                CONTROL_PACKET_SCHEMA,
                Some("resolved_tree_admission"),
                Some("admitted"),
                ADMISSION_REPORT,
            )?,
            ADMISSION_REPORT,
            "admission receipt",
        )?,
        "swarm_ref",
    ) != Some(options.swarm_ref.as_str())
    {
        return Err("admission packet no longer binds the requested W7 ref".to_string());
    }
    Ok(())
}

fn canonical_join_timestamp(repo: &Path, identity: &PromotionIdentity) -> Result<String, String> {
    let value = git(
        repo,
        &[
            "show",
            "-s",
            "--format=%cI",
            identity.source_parent.as_str(),
        ],
    )?;
    let value = value.trim().to_string();
    if value.is_empty() || value.contains('\n') || value.contains('\0') {
        return Err("source parent has an invalid canonical commit timestamp".to_string());
    }
    Ok(value)
}

fn create_exact_join_object(repo: &Path, identity: &PromotionIdentity) -> Result<String, String> {
    let timestamp = canonical_join_timestamp(repo, identity)?;
    let mut child = Command::new("git")
        .current_dir(repo)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_AUTHOR_NAME", JOIN_AUTHOR_NAME)
        .env("GIT_AUTHOR_EMAIL", JOIN_AUTHOR_EMAIL)
        .env("GIT_AUTHOR_DATE", timestamp.as_str())
        .env("GIT_COMMITTER_NAME", JOIN_AUTHOR_NAME)
        .env("GIT_COMMITTER_EMAIL", JOIN_AUTHOR_EMAIL)
        .env("GIT_COMMITTER_DATE", timestamp.as_str())
        .args([
            "commit-tree",
            identity.join_tree.as_str(),
            "-p",
            identity.source_parent.as_str(),
            "-p",
            identity.swarm_parent.as_str(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start guarded git commit-tree: {error}"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "guarded git commit-tree did not expose stdin".to_string())?;
        stdin
            .write_all(format!("{JOIN_MESSAGE}\n").as_bytes())
            .map_err(|error| format!("failed to write exact-join commit message: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for guarded git commit-tree: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "guarded git commit-tree failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("git commit-tree output was not UTF-8: {error}"))?;
    let lines = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err("git commit-tree did not return exactly one object identity".to_string());
    }
    let join_commit = lines
        .first()
        .ok_or_else(|| "commit-tree identity disappeared after cardinality validation".to_string())?;
    validate_exact_hex("constructed join commit", join_commit, 40)?;
    Ok((*join_commit).to_string())
}

fn verify_join_metadata(
    repo: &Path,
    join_commit: &str,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    let fields = git(
        repo,
        &[
            "show",
            "-s",
            "--format=%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI%x00%B",
            join_commit,
        ],
    )?;
    let mut parts = fields.splitn(7, '\0');
    let author_name = parts.next().unwrap_or_default();
    let author_email = parts.next().unwrap_or_default();
    let author_date = parts.next().unwrap_or_default();
    let committer_name = parts.next().unwrap_or_default();
    let committer_email = parts.next().unwrap_or_default();
    let committer_date = parts.next().unwrap_or_default();
    let message = parts.next().unwrap_or_default().trim_end_matches('\n');
    let expected_date = canonical_join_timestamp(repo, identity)?;
    if author_name != JOIN_AUTHOR_NAME
        || author_email != JOIN_AUTHOR_EMAIL
        || author_date != expected_date
        || committer_name != JOIN_AUTHOR_NAME
        || committer_email != JOIN_AUTHOR_EMAIL
        || committer_date != expected_date
        || message != JOIN_MESSAGE
    {
        return Err(
            "constructed join metadata differs from the canonical exact-J contract".to_string(),
        );
    }
    Ok(())
}

fn verify_constructed_join(
    repo: &Path,
    join_commit: &str,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    exact_commit(repo, join_commit, "constructed join commit")?;
    let parents = commit_parents(repo, join_commit)?;
    if parents
        != vec![
            identity.source_parent.clone(),
            identity.swarm_parent.clone(),
        ]
    {
        return Err("constructed join has wrong, reversed, or extra parents".to_string());
    }
    if commit_tree(repo, join_commit)? != identity.join_tree.as_str() {
        return Err("constructed join has a tree different from admitted JOIN_TREE".to_string());
    }
    verify_join_metadata(repo, join_commit, identity)
}

fn validate_candidate_ref(reference: &str) -> Result<(), String> {
    validate_full_ref(reference, "candidate ref")?;
    if !reference.starts_with("refs/heads/promote/0.11.0-") {
        return Err(
            "candidate ref must be an exact refs/heads/promote/0.11.0-* branch".to_string(),
        );
    }
    Ok(())
}

fn construction_success_report(evidence: &ConstructionEvidence) -> Value {
    serde_json::json!({
        "schema": CONSTRUCTION_SCHEMA,
        "status": "constructed",
        "source_parent": evidence.identity.source_parent.as_str(),
        "swarm_parent": evidence.identity.swarm_parent.as_str(),
        "join_tree": evidence.identity.join_tree.as_str(),
        "preflight_sha256": evidence.identity.preflight_sha256.as_str(),
        "resolution_manifest_sha256": evidence.identity.resolution_sha256.as_str(),
        "swarm_ref": evidence.swarm_ref.as_str(),
        "candidate_ref": evidence.candidate_ref.as_str(),
        "join_commit": evidence.join_commit.as_str(),
        "ordered_parents": [
            evidence.identity.source_parent.as_str(),
            evidence.identity.swarm_parent.as_str(),
        ],
        "commit_author_name": JOIN_AUTHOR_NAME,
        "commit_author_email": JOIN_AUTHOR_EMAIL,
        "commit_timestamp": evidence.commit_timestamp.as_str(),
        "commit_message_sha256": digest_bytes(JOIN_MESSAGE.as_bytes()),
        "admission_packet_index_sha256": evidence.admission_index_sha256,
        "admission_receipt_sha256": evidence.admission_receipt_sha256,
        "resolved_tree_packet_index_sha256": evidence.validation_index_sha256,
        "integration_index_sha256": evidence.integration_index_sha256,
        "tree_qualification_receipt_sha256": evidence.qualification_sha256,
        "final_identity_reread_passed": true,
        "refs_unchanged": true,
        "authoritative_commit_attempted": true,
        "commit_tree_attempts": 1,
        "local_ref_attempts": 0,
        "remote_push_attempts": 0,
        "merge_command_attempts": 0,
        "unreferenced_exact_join_constructed": true,
        "ref_mutation_attempted": false,
        "push_attempted": false,
        "merge_command": null,
        "failure_reasons": [],
        "invalidation_rules": [
            "Any movement in source main, protected W7, JOIN_TREE, admission packet/index, P1, manifest, integration index, qualification receipt, or constructed object invalidates this construction receipt.",
            "Publication must independently re-read the source and expected candidate-ref state before an atomic guarded push.",
        ],
        "non_claims": [
            "The exact join object is unreferenced and is not source integration, a release tag, or publication.",
            "No candidate branch, source branch, tag, marketplace, crate, release asset, or other public channel moved.",
            "No merge command is emitted until guarded candidate-ref publication succeeds.",
        ],
    })
}

fn construction_rejection_report(
    identity: Option<&PromotionIdentity>,
    candidate_ref: Option<&str>,
    reason: &str,
    attempted: bool,
) -> Value {
    serde_json::json!({
        "schema": CONSTRUCTION_SCHEMA,
        "status": "rejected",
        "source_parent": identity.map(|value| value.source_parent.as_str()),
        "swarm_parent": identity.map(|value| value.swarm_parent.as_str()),
        "join_tree": identity.map(|value| value.join_tree.as_str()),
        "preflight_sha256": identity.map(|value| value.preflight_sha256.as_str()),
        "resolution_manifest_sha256": identity.map(|value| value.resolution_sha256.as_str()),
        "candidate_ref": candidate_ref,
        "join_commit": null,
        "ordered_parents": [],
        "final_identity_reread_passed": false,
        "refs_unchanged": null,
        "authoritative_commit_attempted": attempted,
        "commit_tree_attempts": usize::from(attempted),
        "local_ref_attempts": 0,
        "remote_push_attempts": 0,
        "merge_command_attempts": 0,
        "unreferenced_exact_join_constructed": false,
        "ref_mutation_attempted": false,
        "push_attempted": false,
        "merge_command": null,
        "failure_reasons": [reason],
        "non_claims": [
            "A rejected construction receipt cannot be published.",
            "No candidate ref, merge command, source integration, release, or public channel authority was created.",
        ],
    })
}
