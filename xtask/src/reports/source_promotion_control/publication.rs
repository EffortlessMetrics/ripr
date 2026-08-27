fn parse_publication_options(args: &[String]) -> Result<PublicationOptions, String> {
    let parsed = parse_command_args(
        args,
        SOURCE_PROMOTION_PUBLISH_CANDIDATE_REF_SUBCOMMAND,
        &[
            "--construction-packet",
            "--source-main-ref",
            "--remote",
            "--target-ref",
            "--expected-old",
            "--out",
        ],
        &["--expected-absent"],
    )?;
    let repo = current_repo()?;
    let source_main_ref = parsed.required("--source-main-ref")?;
    let remote = parsed.required("--remote")?;
    let target_ref = parsed.required("--target-ref")?;
    validate_source_main_ref(&source_main_ref)?;
    validate_candidate_ref(&target_ref)?;
    if remote != "origin" {
        return Err("candidate-ref publisher requires the exact remote name origin".to_string());
    }
    let expected_old = parsed.optional("--expected-old");
    if let Some(value) = &expected_old {
        validate_exact_hex("--expected-old", value, 40)?;
    }
    let expected_absent = parsed.has_flag("--expected-absent");
    if expected_absent == expected_old.is_some() {
        return Err("specify exactly one of --expected-absent or --expected-old <sha>".to_string());
    }
    Ok(PublicationOptions {
        repo: repo.clone(),
        construction_packet: resolve_candidate_path(
            &repo,
            &PathBuf::from(parsed.required("--construction-packet")?),
        ),
        source_main_ref,
        remote,
        source_remote_url: SOURCE_REPOSITORY_URL.to_string(),
        swarm_remote_url: SWARM_REPOSITORY_URL.to_string(),
        target_ref,
        expected_old,
        expected_absent,
        out: parsed
            .optional("--out")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_PUBLICATION_OUT)),
    })
}

fn publish_candidate_ref(args: &[String]) -> Result<(), String> {
    let out = control_out_from_args(args, DEFAULT_PUBLICATION_OUT)?;
    let options = match parse_publication_options(args) {
        Ok(options) => options,
        Err(reason) => {
            let repo = current_repo()?;
            let owned_roots = supplied_protected_roots(
                &repo,
                args,
                &[("--construction-packet", "exact-join construction packet", false)],
            );
            let protected_roots = borrowed_protected_roots(&owned_roots);
            let report =
                publication_rejection_report(None, None, &reason, &PublicationState::default());
            return write_rejection_or_combine_protected(
                &repo,
                &out,
                &protected_roots,
                &ControlPacketWrite {
                    kind: "candidate_ref_publication",
                    report_name: PUBLICATION_REPORT,
                    report: &report,
                    title: "Candidate-ref publication",
                    claim_boundary: "A non-success publication packet records zero merge-command authority and truthfully preserves any observed or unknown remote mutation state. It grants no source integration or release authority.",
                },
                reason,
            );
        }
    };
    let protected_roots: [(&Path, &str); 1] = [(
        options.construction_packet.as_path(),
        "exact-join construction packet",
    )];
    reject_control_packet_output_overlap(&options.repo, &options.out, &protected_roots)?;

    let reconciliation_context = match publication_reconciliation_context(&options) {
        Ok(context) => context,
        Err(reason) => {
            let report = publication_rejection_report(
                None,
                Some(&options.target_ref),
                &reason,
                &PublicationState::default(),
            );
            return write_rejection_or_combine_protected(
                &options.repo,
                &options.out,
                &protected_roots,
                &ControlPacketWrite {
                    kind: "candidate_ref_publication",
                    report_name: PUBLICATION_REPORT,
                    report: &report,
                    title: "Candidate-ref publication",
                    claim_boundary: "A rejected publication packet emits no merge command and grants no source, release, or publication authority.",
                },
                reason,
            );
        }
    };

    let reservation = reserve_control_packet_output_protected(
        &options.repo,
        &options.out,
        &protected_roots,
        "candidate_ref_publication",
        &reconciliation_context,
    )?;
    if let Err(reason) = require_reconciliation_context_unchanged(
        &reconciliation_context,
        publication_reconciliation_context(&options),
        "publication",
    ) {
        let report = publication_rejection_report(
            None,
            Some(&options.target_ref),
            &reason,
            &PublicationState::default(),
        );
        return write_reserved_rejection_or_combine(
            &reservation,
            "candidate_ref_publication",
            PUBLICATION_REPORT,
            &report,
            "Candidate-ref publication",
            "A rejected publication packet emits no merge command and grants no source, release, or publication authority.",
            reason,
        );
    }

    match publish_candidate_ref_inner(&options, Some(&reconciliation_context)) {
        Ok((evidence, state)) => {
            let report = publication_success_report(&evidence, &options, &state);
            write_reserved_control_packet(
                &reservation,
                "candidate_ref_publication",
                PUBLICATION_REPORT,
                &report,
                "Candidate-ref publication",
                "This packet proves one exact candidate ref was atomically created or updated behind the requested expected-state guard. It is not source integration or release authorization.",
            )
        }
        Err(failure) => {
            let (reason, evidence, state) = failure;
            let report = publication_rejection_report(
                evidence.as_deref(),
                Some(&options.target_ref),
                &reason,
                &state,
            );
            write_reserved_rejection_or_combine(
                &reservation,
                "candidate_ref_publication",
                PUBLICATION_REPORT,
                &report,
                "Candidate-ref publication",
                "A rejected publication packet emits no merge command and grants no source, release, or publication authority.",
                reason,
            )
        }
    }
}

fn publication_reconciliation_context(options: &PublicationOptions) -> Result<Value, String> {
    let packet = read_indexed_packet(
        &options.construction_packet,
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
    let evidence = construction_evidence_from_receipt(&receipt)?;
    validate_construction_receipt(&receipt, &evidence)?;
    if evidence.candidate_ref != options.target_ref {
        return Err(
            "publisher target ref differs from construction-bound candidate ref".to_string(),
        );
    }
    verify_constructed_join(&options.repo, &evidence.join_commit, &evidence.identity)?;
    validate_publication_inputs(options, &evidence, &packet)?;
    let urls = read_remote_urls(&options.repo, &options.remote)?;
    if urls
        != (
            options.source_remote_url.clone(),
            options.source_remote_url.clone(),
        )
    {
        return Err("source remote authority moved before publication".to_string());
    }
    let expected = if options.expected_absent {
        None
    } else {
        options.expected_old.clone()
    };
    if read_remote_ref(
        &options.repo,
        &options.source_remote_url,
        &options.target_ref,
    )? != expected
        || read_optional_local_ref(&options.repo, &options.target_ref)? != expected
    {
        return Err("candidate ref expected-state guard failed before reservation".to_string());
    }
    publication_reconciliation_value(options, &evidence, &packet, &expected)
}

fn publication_reconciliation_value(
    options: &PublicationOptions,
    evidence: &ConstructionEvidence,
    packet: &IndexedPacket,
    expected: &Option<String>,
) -> Result<Value, String> {
    Ok(serde_json::json!({
        "source_parent": evidence.identity.source_parent,
        "swarm_parent": evidence.identity.swarm_parent,
        "join_tree": evidence.identity.join_tree,
        "join_commit": evidence.join_commit,
        "swarm_ref": evidence.swarm_ref,
        "source_main_ref": options.source_main_ref,
        "remote": options.remote,
        "source_repository_url": options.source_remote_url,
        "swarm_repository_url": options.swarm_remote_url,
        "target_ref": options.target_ref,
        "construction_packet_index_sha256": packet.index_sha256,
        "construction_receipt_sha256": packet_file_sha256(packet, CONSTRUCTION_REPORT)?,
        "expected_state": expected.clone().map(Value::String).unwrap_or_else(|| Value::String("absent".to_string())),
        "maximum_commit_tree_attempts": 0,
        "maximum_local_ref_attempts": 2,
        "maximum_remote_push_attempts": 1,
        "maximum_merge_command_attempts": 0,
    }))
}

fn publish_candidate_ref_inner(
    options: &PublicationOptions,
    expected_reconciliation_context: Option<&Value>,
) -> Result<(ConstructionEvidence, PublicationState), PublicationFailure> {
    publish_candidate_ref_inner_with_publication_runners(
        options,
        expected_reconciliation_context,
        run_guarded_candidate_push,
        read_remote_ref,
        read_optional_local_ref,
    )
}

#[cfg(test)]
fn publish_candidate_ref_inner_with_final_remote_reader<F>(
    options: &PublicationOptions,
    expected_reconciliation_context: Option<&Value>,
    final_remote_reader: F,
) -> Result<(ConstructionEvidence, PublicationState), PublicationFailure>
where
    F: Fn(&Path, &str, &str) -> Result<Option<String>, String>,
{
    publish_candidate_ref_inner_with_publication_runners(
        options,
        expected_reconciliation_context,
        run_guarded_candidate_push,
        final_remote_reader,
        read_optional_local_ref,
    )
}

fn run_guarded_candidate_push(
    options: &PublicationOptions,
    lease: &str,
    refspec: &str,
) -> Result<(bool, Option<bool>, String), String> {
    let output = Command::new("git")
        .current_dir(&options.repo)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .args([
            "push",
            "--porcelain",
            "--atomic",
            "--no-verify",
            lease,
            options.source_remote_url.as_str(),
            refspec,
        ])
        .output()
        .map_err(|error| format!("failed to start guarded candidate-ref push: {error}"))?;
    let process_succeeded = output.status.success();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Ok(classify_guarded_push_output(
        process_succeeded,
        output.stdout,
        &stderr,
        &options.target_ref,
    ))
}

fn classify_guarded_push_output(
    process_succeeded: bool,
    stdout: Vec<u8>,
    stderr: &str,
    target_ref: &str,
) -> (bool, Option<bool>, String) {
    let stdout = match String::from_utf8(stdout)
        .map_err(|error| format!("guarded push output was not UTF-8: {error}"))
    {
        Ok(stdout) => stdout,
        Err(reason) => {
            let attribution_diagnostic =
                format!("target-update attribution unavailable: {reason}");
            let detail = [stderr, attribution_diagnostic.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" | ");
            let target_updated = if process_succeeded { None } else { Some(false) };
            return (process_succeeded, target_updated, detail);
        }
    };
    let detail = [stdout.trim(), stderr]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    if !process_succeeded {
        return (false, Some(false), detail);
    }
    match parse_guarded_push_porcelain(&stdout, target_ref) {
        Ok(target_updated) => (true, Some(target_updated), detail),
        Err(reason) => {
            let attribution_diagnostic =
                format!("target-update attribution unavailable: {reason}");
            let detail = [detail.as_str(), attribution_diagnostic.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" | ");
            (true, None, detail)
        }
    }
}

fn parse_guarded_push_porcelain(output: &str, target_ref: &str) -> Result<bool, String> {
    let mut rows = Vec::new();
    for line in output.lines() {
        if line.is_empty() || line.starts_with("To ") || line == "Done" {
            continue;
        }
        let mut columns = line.splitn(3, '\t');
        let flag = columns
            .next()
            .ok_or_else(|| "guarded push porcelain status flag is missing".to_string())?;
        let refs = columns
            .next()
            .ok_or_else(|| "guarded push porcelain ref-status row is malformed".to_string())?;
        let summary = columns
            .next()
            .ok_or_else(|| "guarded push porcelain summary is missing".to_string())?;
        rows.push((flag, refs, summary));
    }
    if rows.len() != 1 {
        return Err(format!(
            "guarded push porcelain returned {} ref-status rows for one target",
            rows.len()
        ));
    }
    let (flag, refs, _) = rows
        .first()
        .ok_or_else(|| "guarded push porcelain status row disappeared".to_string())?;
    let (_, destination) = refs
        .rsplit_once(':')
        .ok_or_else(|| "guarded push porcelain ref-status row is malformed".to_string())?;
    if destination != target_ref {
        return Err("guarded push porcelain named a different target ref".to_string());
    }
    match *flag {
        " " | "+" | "*" => Ok(true),
        "=" => Ok(false),
        other => Err(format!(
            "guarded push porcelain returned non-publishing status flag {other:?}"
        )),
    }
}

fn publish_candidate_ref_inner_with_publication_runners<P, F, L>(
    options: &PublicationOptions,
    expected_reconciliation_context: Option<&Value>,
    push_runner: P,
    final_remote_reader: F,
    post_push_local_reader: L,
) -> Result<(ConstructionEvidence, PublicationState), PublicationFailure>
where
    P: Fn(&PublicationOptions, &str, &str) -> Result<(bool, Option<bool>, String), String>,
    F: Fn(&Path, &str, &str) -> Result<Option<String>, String>,
    L: Fn(&Path, &str) -> Result<Option<String>, String>,
{
    let mut state = Box::<PublicationState>::default();
    let packet = read_indexed_packet(
        &options.construction_packet,
        CONTROL_PACKET_SCHEMA,
        Some("exact_join_construction"),
        Some("constructed"),
        CONSTRUCTION_REPORT,
    )
    .map_err(|reason| (reason, None, state.clone()))?;
    let receipt = packet_json(
        &packet,
        CONSTRUCTION_REPORT,
        "exact-join construction receipt",
    )
    .map_err(|reason| (reason, None, state.clone()))?;
    let evidence = Box::new(
        construction_evidence_from_receipt(&receipt)
            .map_err(|reason| (reason, None, state.clone()))?,
    );
    validate_construction_receipt(&receipt, &evidence)
        .map_err(|reason| (reason, Some(evidence.clone()), state.clone()))?;

    if evidence.candidate_ref != options.target_ref {
        return Err((
            "publisher target ref differs from construction-bound candidate ref".to_string(),
            Some(evidence),
            state,
        ));
    }
    verify_constructed_join(&options.repo, &evidence.join_commit, &evidence.identity)
        .map_err(|reason| (reason, Some(evidence.clone()), state.clone()))?;

    let expected = if options.expected_absent {
        None
    } else {
        options.expected_old.clone()
    };
    let (fetch_url, push_url) = read_remote_urls(&options.repo, &options.remote)
        .map_err(|reason| (reason, Some(evidence.clone()), state.clone()))?;
    if fetch_url != options.source_remote_url || push_url != options.source_remote_url {
        return Err((
            "origin fetch/push URLs do not equal the bound source repository URL".to_string(),
            Some(evidence),
            state,
        ));
    }
    let observed = read_remote_ref(
        &options.repo,
        &options.source_remote_url,
        &options.target_ref,
    )
    .map_err(|reason| (reason, Some(evidence.clone()), state.clone()))?;
    if observed != expected {
        return Err((
            format!(
                "candidate ref expected-state guard failed: expected {expected:?}, observed {observed:?}"
            ),
            Some(evidence),
            state,
        ));
    }
    let local_before = read_optional_local_ref(&options.repo, &options.target_ref)
        .map_err(|reason| (reason, Some(evidence.clone()), state.clone()))?;
    state.local_ref_before = local_before.clone();
    if local_before != expected {
        return Err((
            format!(
                "local candidate-ref expected-state guard failed: expected {expected:?}, observed {local_before:?}"
            ),
            Some(evidence),
            state,
        ));
    }

    validate_publication_inputs(options, &evidence, &packet)
        .map_err(|reason| (reason, Some(evidence.clone()), state.clone()))?;
    if let Some(expected_context) = expected_reconciliation_context {
        require_reconciliation_context_unchanged(
            expected_context,
            publication_reconciliation_value(options, &evidence, &packet, &expected),
            "publication",
        )
        .map_err(|reason| (reason, Some(evidence.clone()), state.clone()))?;
    }

    let lease = match &expected {
        Some(old) => format!("--force-with-lease={}:{}", options.target_ref, old),
        None => format!("--force-with-lease={}:", options.target_ref),
    };
    state.local_ref_attempts = 1;
    update_local_ref(
        &options.repo,
        &options.target_ref,
        Some(&evidence.join_commit),
        expected.as_deref(),
    )
    .map_err(|reason| (reason, Some(evidence.clone()), state.clone()))?;
    state.local_ref_after = Some(evidence.join_commit.clone());

    if let Err(reason) = validate_publication_inputs(options, &evidence, &packet) {
        rollback_local_candidate(options, &evidence, &expected, &mut state);
        return Err((reason, Some(evidence), state));
    }

    let refspec = format!("{}:{}", evidence.join_commit, options.target_ref);
    state.remote_push_attempts = 1;
    let push = push_runner(options, lease.as_str(), refspec.as_str());
    state.push_process_succeeded = push.as_ref().ok().map(|output| output.0);
    state.target_ref_updated = push.as_ref().ok().and_then(|output| output.1);
    let post_push_local = post_push_local_reader(&options.repo, &options.target_ref);
    state.local_ref_after = post_push_local.as_ref().ok().cloned().flatten();

    let final_remote = final_remote_reader(
        &options.repo,
        &options.source_remote_url,
        &options.target_ref,
    );
    match final_remote {
        Ok(value) => {
            state.remote_state_observed = true;
            state.observed_final_ref = value;
        }
        Err(reason) => {
            rollback_local_candidate(options, &evidence, &expected, &mut state);
            return Err((
                format!("remote candidate-ref state unavailable after push attempt: {reason}"),
                Some(evidence),
                state,
            ));
        }
    }

    let source_main_result = validate_source_main_authority(options, &evidence);
    state.source_main_unchanged = Some(source_main_result.is_ok());
    let swarm_parent_result = validate_swarm_parent_authority(options, &evidence);
    state.swarm_parent_unchanged = Some(swarm_parent_result.is_ok());
    let construction_packet_result = validate_construction_packet(options, &packet);
    state.construction_packet_unchanged = Some(construction_packet_result.is_ok());
    let remote_authority_result = validate_remote_authority(options);
    state.remote_authority_unchanged = Some(remote_authority_result.is_ok());
    let join_result = verify_constructed_join(
        &options.repo,
        &evidence.join_commit,
        &evidence.identity,
    );
    let input_result = source_main_result
        .and(swarm_parent_result)
        .and(construction_packet_result)
        .and(remote_authority_result)
        .and(join_result);

    if state.observed_final_ref.as_deref() == Some(evidence.join_commit.as_str()) {
        if state.push_process_succeeded != Some(true) {
            rollback_local_candidate(options, &evidence, &expected, &mut state);
            return Err((
                "remote candidate ref equals the join, but the guarded push process did not report success"
                    .to_string(),
                Some(evidence),
                state,
            ));
        }
        if state.target_ref_updated != Some(true) {
            rollback_local_candidate(options, &evidence, &expected, &mut state);
            let reason = if state.target_ref_updated == Some(false) {
                "remote candidate ref equals the join, but the guarded push reported no actual target update"
                    .to_string()
            } else {
                let detail = push
                    .as_ref()
                    .ok()
                    .map(|output| output.2.as_str())
                    .filter(|detail| !detail.is_empty())
                    .unwrap_or("guarded push target-update attribution was unavailable");
                format!(
                    "remote candidate ref equals the join, but actual target-update attribution was unavailable: {detail}"
                )
            };
            return Err((
                reason,
                Some(evidence),
                state,
            ));
        }
        if let Err(reason) = post_push_local {
            return Err((
                format!(
                    "candidate ref published but post-push local candidate state was unavailable: {reason}"
                ),
                Some(evidence),
                state,
            ));
        }
        if let Err(reason) = input_result {
            return Err((
                format!(
                    "candidate ref published but inputs invalidated during publication: {reason}"
                ),
                Some(evidence),
                state,
            ));
        }
        if !authoritative_publication_observed(&state, &evidence) {
            return Err((
                "candidate ref published but one or more post-push authority rereads failed"
                    .to_string(),
                Some(evidence),
                state,
            ));
        }
        return Ok((*evidence, *state));
    }

    rollback_local_candidate(options, &evidence, &expected, &mut state);
    let push_reason = match push {
        Ok(output) => format!(
            "guarded candidate-ref push did not publish the exact join: {}",
            output.2
        ),
        Err(reason) => reason,
    };
    Err((push_reason, Some(evidence), state))
}

fn authoritative_publication_observed(
    state: &PublicationState,
    evidence: &ConstructionEvidence,
) -> bool {
    state.push_process_succeeded == Some(true)
        && state.target_ref_updated == Some(true)
        && state.remote_state_observed
        && state.observed_final_ref.as_deref() == Some(evidence.join_commit.as_str())
        && post_publication_authority_current(state)
}

fn post_publication_authority_current(state: &PublicationState) -> bool {
    state.source_main_unchanged == Some(true)
        && state.swarm_parent_unchanged == Some(true)
        && state.construction_packet_unchanged == Some(true)
        && state.remote_authority_unchanged == Some(true)
}

fn validate_publication_inputs(
    options: &PublicationOptions,
    evidence: &ConstructionEvidence,
    packet: &IndexedPacket,
) -> Result<(), String> {
    validate_source_main_authority(options, evidence)?;
    validate_swarm_parent_authority(options, evidence)?;
    validate_construction_packet(options, packet)?;
    validate_remote_authority(options)?;
    verify_constructed_join(&options.repo, &evidence.join_commit, &evidence.identity)
}

fn validate_source_main_authority(
    options: &PublicationOptions,
    evidence: &ConstructionEvidence,
) -> Result<(), String> {
    let source_local = read_commit_ref(&options.repo, &options.source_main_ref, "source main ref")?;
    let source_remote = read_remote_ref(
        &options.repo,
        &options.source_remote_url,
        &options.source_main_ref,
    )?;
    if source_local != evidence.identity.source_parent
        || source_remote.as_deref() != Some(evidence.identity.source_parent.as_str())
    {
        return Err("local or remote source main moved after exact-join construction".to_string());
    }
    Ok(())
}

fn validate_swarm_parent_authority(
    options: &PublicationOptions,
    evidence: &ConstructionEvidence,
) -> Result<(), String> {
    let swarm_local = read_commit_ref(&options.repo, &evidence.swarm_ref, "protected W7 ref")?;
    let swarm_remote = read_remote_ref(
        &options.repo,
        &options.swarm_remote_url,
        &evidence.swarm_ref,
    )?;
    if swarm_local != evidence.identity.swarm_parent
        || swarm_remote.as_deref() != Some(evidence.identity.swarm_parent.as_str())
    {
        return Err(
            "local or remote protected W7 ref moved after exact-join construction".to_string(),
        );
    }
    Ok(())
}

fn validate_construction_packet(
    options: &PublicationOptions,
    packet: &IndexedPacket,
) -> Result<(), String> {
    let current_packet = read_indexed_packet(
        &options.construction_packet,
        CONTROL_PACKET_SCHEMA,
        Some("exact_join_construction"),
        Some("constructed"),
        CONSTRUCTION_REPORT,
    )?;
    if current_packet != *packet {
        return Err("construction packet bytes or inventory moved before publication".to_string());
    }
    Ok(())
}

fn validate_remote_authority(options: &PublicationOptions) -> Result<(), String> {
    let urls = read_remote_urls(&options.repo, &options.remote)?;
    if urls
        != (
            options.source_remote_url.clone(),
            options.source_remote_url.clone(),
        )
    {
        return Err("source remote authority moved before publication".to_string());
    }
    Ok(())
}

fn rollback_local_candidate(
    options: &PublicationOptions,
    evidence: &ConstructionEvidence,
    expected: &Option<String>,
    state: &mut PublicationState,
) {
    state.local_ref_attempts += 1;
    let result = update_local_ref(
        &options.repo,
        &options.target_ref,
        expected.as_deref(),
        Some(&evidence.join_commit),
    );
    state.local_ref_rollback_succeeded = Some(result.is_ok());
    state.local_ref_after = read_optional_local_ref(&options.repo, &options.target_ref)
        .ok()
        .flatten();
}

fn construction_evidence_from_receipt(receipt: &Value) -> Result<ConstructionEvidence, String> {
    let identity = PromotionIdentity {
        source_parent: required_receipt_string(receipt, "source_parent")?,
        swarm_parent: required_receipt_string(receipt, "swarm_parent")?,
        join_tree: required_receipt_string(receipt, "join_tree")?,
        preflight_sha256: required_receipt_string(receipt, "preflight_sha256")?,
        resolution_sha256: required_receipt_string(receipt, "resolution_manifest_sha256")?,
    };
    validate_exact_hex("source parent", &identity.source_parent, 40)?;
    validate_exact_hex("swarm parent", &identity.swarm_parent, 40)?;
    validate_exact_hex("join tree", &identity.join_tree, 40)?;
    validate_exact_hex("preflight digest", &identity.preflight_sha256, 64)?;
    validate_exact_hex("resolution digest", &identity.resolution_sha256, 64)?;
    let evidence = ConstructionEvidence {
        identity,
        swarm_ref: required_receipt_string(receipt, "swarm_ref")?,
        candidate_ref: required_receipt_string(receipt, "candidate_ref")?,
        admission_index_sha256: required_receipt_string(receipt, "admission_packet_index_sha256")?,
        admission_receipt_sha256: required_receipt_string(receipt, "admission_receipt_sha256")?,
        validation_index_sha256: required_receipt_string(
            receipt,
            "resolved_tree_packet_index_sha256",
        )?,
        integration_index_sha256: required_receipt_string(receipt, "integration_index_sha256")?,
        qualification_sha256: required_receipt_string(
            receipt,
            "tree_qualification_receipt_sha256",
        )?,
        join_commit: required_receipt_string(receipt, "join_commit")?,
        commit_timestamp: required_receipt_string(receipt, "commit_timestamp")?,
    };
    validate_full_ref(&evidence.swarm_ref, "construction W7 ref")?;
    validate_candidate_ref(&evidence.candidate_ref)?;
    for (label, digest) in [
        (
            "admission packet index digest",
            evidence.admission_index_sha256.as_str(),
        ),
        (
            "admission receipt digest",
            evidence.admission_receipt_sha256.as_str(),
        ),
        (
            "resolved-tree packet index digest",
            evidence.validation_index_sha256.as_str(),
        ),
        (
            "integration index digest",
            evidence.integration_index_sha256.as_str(),
        ),
        (
            "qualification receipt digest",
            evidence.qualification_sha256.as_str(),
        ),
    ] {
        validate_exact_hex(label, digest, 64)?;
    }
    validate_exact_hex("constructed join commit", &evidence.join_commit, 40)?;
    if evidence.commit_timestamp.trim().is_empty()
        || evidence.commit_timestamp.contains('\n')
        || evidence.commit_timestamp.contains('\0')
    {
        return Err("construction receipt has invalid commit timestamp".to_string());
    }
    Ok(evidence)
}

fn required_receipt_string(value: &Value, key: &str) -> Result<String, String> {
    json_string(value, key)
        .map(ToString::to_string)
        .ok_or_else(|| format!("receipt is missing {key}"))
}

fn validate_construction_receipt(
    receipt: &Value,
    evidence: &ConstructionEvidence,
) -> Result<(), String> {
    let expected_message_sha256 = digest_bytes(JOIN_MESSAGE.as_bytes());
    if json_string(receipt, "schema") != Some(CONSTRUCTION_SCHEMA)
        || json_string(receipt, "status") != Some("constructed")
        || !evidence.identity.matches_json(receipt)
        || json_bool(receipt, "final_identity_reread_passed") != Some(true)
        || json_bool(receipt, "refs_unchanged") != Some(true)
        || json_bool(receipt, "authoritative_commit_attempted") != Some(true)
        || receipt.get("commit_tree_attempts").and_then(Value::as_u64) != Some(1)
        || receipt.get("local_ref_attempts").and_then(Value::as_u64) != Some(0)
        || receipt.get("remote_push_attempts").and_then(Value::as_u64) != Some(0)
        || receipt
            .get("merge_command_attempts")
            .and_then(Value::as_u64)
            != Some(0)
        || json_bool(receipt, "unreferenced_exact_join_constructed") != Some(true)
        || json_bool(receipt, "ref_mutation_attempted") != Some(false)
        || json_bool(receipt, "push_attempted") != Some(false)
        || json_string(receipt, "commit_author_name") != Some(JOIN_AUTHOR_NAME)
        || json_string(receipt, "commit_author_email") != Some(JOIN_AUTHOR_EMAIL)
        || json_string(receipt, "commit_timestamp") != Some(evidence.commit_timestamp.as_str())
        || json_string(receipt, "commit_message_sha256") != Some(expected_message_sha256.as_str())
        || !receipt.get("merge_command").is_some_and(Value::is_null)
        || !empty_failure_reasons(receipt)
    {
        return Err("construction receipt did not earn publication eligibility".to_string());
    }
    let parents = receipt
        .get("ordered_parents")
        .and_then(Value::as_array)
        .ok_or_else(|| "construction receipt is missing ordered_parents".to_string())?;
    if parents.len() != 2
        || parents.first().and_then(Value::as_str) != Some(evidence.identity.source_parent.as_str())
        || parents.get(1).and_then(Value::as_str) != Some(evidence.identity.swarm_parent.as_str())
    {
        return Err("construction receipt has wrong or reversed parent order".to_string());
    }
    Ok(())
}

fn publication_success_report(
    evidence: &ConstructionEvidence,
    options: &PublicationOptions,
    state: &PublicationState,
) -> Value {
    serde_json::json!({
        "schema": PUBLICATION_SCHEMA,
        "status": "published",
        "source_parent": evidence.identity.source_parent.as_str(),
        "swarm_parent": evidence.identity.swarm_parent.as_str(),
        "join_tree": evidence.identity.join_tree.as_str(),
        "preflight_sha256": evidence.identity.preflight_sha256.as_str(),
        "resolution_manifest_sha256": evidence.identity.resolution_sha256.as_str(),
        "join_commit": evidence.join_commit.as_str(),
        "swarm_ref": evidence.swarm_ref.as_str(),
        "candidate_ref": options.target_ref,
        "remote": options.remote,
        "expected_state": if options.expected_absent {
            Value::String("absent".to_string())
        } else {
            options.expected_old.clone().map(Value::String).unwrap_or(Value::Null)
        },
        "observed_final_ref": state.observed_final_ref,
        "atomic_push": true,
        "push_process_succeeded": state.push_process_succeeded,
        "target_ref_updated": state.target_ref_updated,
        "expected_state_guard_passed": true,
        "source_main_unchanged": state.source_main_unchanged,
        "swarm_parent_unchanged": state.swarm_parent_unchanged,
        "construction_packet_unchanged": state.construction_packet_unchanged,
        "remote_authority_unchanged": state.remote_authority_unchanged,
        "local_ref_before": state.local_ref_before,
        "local_ref_after": state.local_ref_after,
        "local_ref_rollback_succeeded": state.local_ref_rollback_succeeded,
        "ref_mutation_attempted": true,
        "push_attempted": true,
        "commit_tree_attempts": 0,
        "local_ref_attempts": state.local_ref_attempts,
        "remote_push_attempts": state.remote_push_attempts,
        "merge_command_attempts": state.merge_command_attempts,
        "merge_command": null,
        "failure_reasons": [],
        "invalidation_rules": [
            "Moving source main, W7, the construction packet, exact join object, remote candidate ref, or expected-state value invalidates this publication receipt.",
            "The authoritative merge command remains owned by the later source integration control and is not emitted here.",
        ],
        "non_claims": [
            "Candidate-ref publication is not source integration.",
            "No release tag, crate, GitHub Release, asset, marketplace, signing, secret, or K back-sync operation was authorized.",
        ],
    })
}

fn publication_rejection_report(
    evidence: Option<&ConstructionEvidence>,
    target_ref: Option<&str>,
    reason: &str,
    state: &PublicationState,
) -> Value {
    let published_but_invalidated = evidence.is_some_and(|value| {
        state.push_process_succeeded == Some(true)
            && state.target_ref_updated == Some(true)
            && state.remote_state_observed
            && state.observed_final_ref.as_deref() == Some(value.join_commit.as_str())
    });
    let observed_join_without_attributed_update = evidence.is_some()
        && (state.push_process_succeeded != Some(true) || state.target_ref_updated != Some(true))
        && state.remote_state_observed
        && state.observed_final_ref.as_deref() == evidence.map(|value| value.join_commit.as_str());
    let status = if published_but_invalidated {
        "published_but_invalidated"
    } else if observed_join_without_attributed_update
        || (state.remote_push_attempts > 0 && !state.remote_state_observed)
    {
        "publication_state_unknown"
    } else {
        "rejected"
    };
    let attributed_update = state.push_process_succeeded == Some(true)
        && state.target_ref_updated == Some(true);
    let operation_fact = if attributed_update {
        Some(true)
    } else if state.remote_push_attempts > 0 {
        None
    } else {
        Some(false)
    };
    serde_json::json!({
        "schema": PUBLICATION_SCHEMA,
        "status": status,
        "source_parent": evidence.map(|value| value.identity.source_parent.as_str()),
        "swarm_parent": evidence.map(|value| value.identity.swarm_parent.as_str()),
        "join_tree": evidence.map(|value| value.identity.join_tree.as_str()),
        "join_commit": evidence.map(|value| value.join_commit.as_str()),
        "candidate_ref": target_ref,
        "observed_final_ref": state.observed_final_ref,
        "remote_state_observed": state.remote_state_observed,
        "atomic_push": operation_fact,
        "push_process_succeeded": state.push_process_succeeded,
        "target_ref_updated": state.target_ref_updated,
        "expected_state_guard_passed": operation_fact,
        "source_main_unchanged": state.source_main_unchanged,
        "swarm_parent_unchanged": state.swarm_parent_unchanged,
        "construction_packet_unchanged": state.construction_packet_unchanged,
        "remote_authority_unchanged": state.remote_authority_unchanged,
        "local_ref_before": state.local_ref_before,
        "local_ref_after": state.local_ref_after,
        "local_ref_rollback_succeeded": state.local_ref_rollback_succeeded,
        "ref_mutation_attempted": state.local_ref_attempts > 0,
        "push_attempted": state.remote_push_attempts > 0,
        "commit_tree_attempts": 0,
        "local_ref_attempts": state.local_ref_attempts,
        "remote_push_attempts": state.remote_push_attempts,
        "merge_command_attempts": state.merge_command_attempts,
        "merge_command": null,
        "failure_reasons": [reason],
        "non_claims": [
            "A rejected publication packet emits no merge command.",
            "This receipt grants no source integration, release, marketplace, crate, asset, signing, secret, or merge authority, including when it records a remote mutation that was later invalidated.",
        ],
    })
}
