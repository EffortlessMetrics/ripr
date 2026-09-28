fn parse_builder_options(args: &[String]) -> Result<BuilderOptions, String> {
    let parsed = parse_command_args(
        args,
        SOURCE_PROMOTION_TRUSTED_BUILDER_SUBCOMMAND,
        &[
            "--source-parent",
            "--workflow-source-sha",
            "--executable",
            "--cargo-target-dir",
            "--out",
        ],
        &["--locked-build", "--isolated-target-dir"],
    )?;
    let source_parent = parsed.required("--source-parent")?;
    let workflow_source_sha = parsed.required("--workflow-source-sha")?;
    validate_exact_hex("--source-parent", &source_parent, 40)?;
    validate_exact_hex("--workflow-source-sha", &workflow_source_sha, 40)?;
    let repo = current_repo()?;
    let executable =
        resolve_candidate_path(&repo, &PathBuf::from(parsed.required("--executable")?));
    let cargo_target_dir = resolve_candidate_path(
        &repo,
        &PathBuf::from(parsed.required("--cargo-target-dir")?),
    );
    let out = parsed
        .optional("--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_BUILDER_OUT));
    Ok(BuilderOptions {
        repo,
        source_parent,
        workflow_source_sha,
        executable,
        cargo_target_dir,
        out,
        locked_build: parsed.has_flag("--locked-build"),
        isolated_target_dir: parsed.has_flag("--isolated-target-dir"),
    })
}

fn write_trusted_builder_receipt(args: &[String]) -> Result<(), String> {
    let out = control_out_from_args(args, DEFAULT_BUILDER_OUT)?;
    let options = match parse_builder_options(args) {
        Ok(options) => options,
        Err(reason) => {
            let repo = current_repo()?;
            let mut owned_roots = supplied_protected_roots(
                &repo,
                args,
                &[
                    ("--cargo-target-dir", "isolated Cargo target directory", false),
                    ("--executable", "built xtask executable", false),
                ],
            );
            owned_roots.push(OwnedProtectedRoot {
                path: repo.join("Cargo.lock"),
                label: "Cargo.lock",
            });
            let protected_roots = borrowed_protected_roots(&owned_roots);
            let report = serde_json::json!({
                "schema": BUILDER_SCHEMA,
                "status": "rejected",
                "source_parent": null,
                "workflow_source_sha": null,
                "clean_checkout": false,
                "rust_toolchain": null,
                "cargo_lock_sha256": null,
                "locked_build": false,
                "isolated_cargo_target_dir": false,
                "executable_sha256": null,
                "failure_reasons": [reason.as_str()],
                "authoritative_commit_attempted": false,
                "commit_tree_attempts": 0,
                "local_ref_attempts": 0,
                "remote_push_attempts": 0,
                "merge_command_attempts": 0,
                "merge_command": null,
                "ref_mutation_attempted": false,
                "push_attempted": false,
            });
            return write_rejection_or_combine_protected(
                &repo,
                &out,
                &protected_roots,
                &ControlPacketWrite {
                    kind: "trusted_builder",
                    report_name: BUILDER_REPORT,
                    report: &report,
                    title: "Trusted source-promotion builder",
                    claim_boundary: "A rejected builder packet grants no validation, construction, ref, merge, or publication authority.",
                },
                reason,
            );
        }
    };

    let validation = validate_live_builder(&options);
    let (status, cargo_lock_sha256, executable_sha256, rust_toolchain, clean, failures) =
        match validation {
            Ok((lock, executable, rust, clean_checkout)) => (
                "built",
                Some(lock),
                Some(executable),
                Some(rust),
                clean_checkout,
                Vec::<String>::new(),
            ),
            Err(reason) => ("rejected", None, None, None, false, vec![reason]),
        };
    let report = serde_json::json!({
        "schema": BUILDER_SCHEMA,
        "status": status,
        "source_parent": options.source_parent,
        "workflow_source_sha": options.workflow_source_sha,
        "clean_checkout": clean,
        "rust_toolchain": rust_toolchain,
        "cargo_lock_sha256": cargo_lock_sha256,
        "locked_build": options.locked_build,
        "isolated_cargo_target_dir": options.isolated_target_dir,
        "executable_sha256": executable_sha256,
        "failure_reasons": failures,
        "authoritative_commit_attempted": false,
        "commit_tree_attempts": 0,
        "local_ref_attempts": 0,
        "remote_push_attempts": 0,
        "merge_command_attempts": 0,
        "merge_command": null,
        "ref_mutation_attempted": false,
        "push_attempted": false,
        "non_claims": [
            "This packet records build provenance for the exact executable; it does not validate a candidate tree.",
            "The locked-build and isolated-target-dir facts are accepted only because the source-owned hosted workflow supplies those explicit flags and is separately contract-checked.",
        ],
    });
    let cargo_lock = options.repo.join("Cargo.lock");
    let protected_roots = [
        (
            options.cargo_target_dir.as_path(),
            "isolated Cargo target directory",
        ),
        (options.executable.as_path(), "built xtask executable"),
        (cargo_lock.as_path(), "Cargo.lock"),
    ];
    reject_control_packet_output_overlap(&options.repo, &options.out, &protected_roots)?;
    let write_result = write_control_packet_protected(
        &options.repo,
        &options.out,
        &protected_roots,
        &ControlPacketWrite {
            kind: "trusted_builder",
            report_name: BUILDER_REPORT,
            report: &report,
            title: BUILDER_TITLE,
            claim_boundary: BUILDER_SUCCESS_CLAIM,
        },
    );
    if status == "built" {
        write_result
    } else {
        match write_result {
            Ok(()) => Err(report
                .get("failure_reasons")
                .and_then(Value::as_array)
                .and_then(|reasons| reasons.first())
                .and_then(Value::as_str)
                .unwrap_or("trusted builder rejected")
                .to_string()),
            Err(write_error) => Err(write_error),
        }
    }
}

fn validate_live_builder(
    options: &BuilderOptions,
) -> Result<(String, String, String, bool), String> {
    if !options.locked_build {
        return Err("trusted builder requires --locked-build".to_string());
    }
    if !options.isolated_target_dir {
        return Err("trusted builder requires --isolated-target-dir".to_string());
    }
    if options.workflow_source_sha != options.source_parent {
        return Err("builder workflow source SHA must equal SOURCE_PARENT".to_string());
    }
    if current_head(&options.repo)? != options.source_parent {
        return Err("builder checkout HEAD does not equal SOURCE_PARENT".to_string());
    }
    let clean = clean_checkout(&options.repo)?;
    if !clean {
        return Err("builder source checkout is not clean".to_string());
    }

    let rust = command_output(&options.repo, "rustc", &["--version"])?;
    let rust = rust.trim().to_string();
    if !rust.starts_with(&format!("rustc {RUST_TOOLCHAIN} ")) {
        return Err(format!(
            "builder Rust toolchain must be {RUST_TOOLCHAIN}, observed {rust}"
        ));
    }

    let canonical_repo = options
        .repo
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize source checkout: {error}"))?;
    let target_metadata = fs::symlink_metadata(&options.cargo_target_dir).map_err(|error| {
        format!(
            "failed to inspect isolated CARGO_TARGET_DIR {}: {error}",
            options.cargo_target_dir.display()
        )
    })?;
    if target_metadata.file_type().is_symlink() || !target_metadata.is_dir() {
        return Err("CARGO_TARGET_DIR must be a non-symlink directory".to_string());
    }
    let canonical_target = options
        .cargo_target_dir
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize CARGO_TARGET_DIR: {error}"))?;
    if canonical_target.starts_with(&canonical_repo) {
        return Err("CARGO_TARGET_DIR must be outside the source checkout".to_string());
    }

    let executable_metadata = fs::symlink_metadata(&options.executable).map_err(|error| {
        format!(
            "failed to inspect built xtask executable {}: {error}",
            options.executable.display()
        )
    })?;
    if executable_metadata.file_type().is_symlink() || !executable_metadata.is_file() {
        return Err("built xtask executable must be a non-symlink regular file".to_string());
    }
    let canonical_executable = options
        .executable
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize built xtask executable: {error}"))?;
    if !canonical_executable.starts_with(&canonical_target) {
        return Err("built xtask executable is outside the isolated target directory".to_string());
    }
    let running = std::env::current_exe()
        .map_err(|error| format!("failed to locate running xtask executable: {error}"))?
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize running xtask executable: {error}"))?;
    if running != canonical_executable {
        return Err("builder receipt must be emitted by the exact built executable".to_string());
    }

    let cargo_lock_sha256 = file_sha256(&options.repo.join("Cargo.lock"), "Cargo.lock")?;
    let executable_sha256 = file_sha256(&canonical_executable, "built xtask executable")?;
    Ok((cargo_lock_sha256, executable_sha256, rust, clean))
}

fn parse_admission_options(args: &[String]) -> Result<AdmissionOptions, String> {
    let parsed = parse_command_args(
        args,
        SOURCE_PROMOTION_ADMIT_RESOLVED_TREE_SUBCOMMAND,
        &[
            "--source-parent",
            "--swarm-parent",
            "--join-tree",
            "--preflight",
            "--preflight-sha256",
            "--resolution-manifest",
            "--resolution-sha256",
            "--validation-packet",
            "--builder-packet",
            "--integration-index",
            "--integration-index-sha256",
            "--out",
        ],
        &[],
    )?;
    let repo = current_repo()?;
    let identity = PromotionIdentity::from_values(&parsed)?;
    let resolve = |key: &str| -> Result<PathBuf, String> {
        Ok(resolve_candidate_path(
            &repo,
            &PathBuf::from(parsed.required(key)?),
        ))
    };
    let validation_packet = resolve("--validation-packet")?;
    let builder_packet = resolve("--builder-packet")?;
    let integration_index = resolve("--integration-index")?;
    let integration_index_sha256 = parsed.required("--integration-index-sha256")?;
    validate_exact_hex(
        "--integration-index-sha256",
        &integration_index_sha256,
        64,
    )?;
    let preflight = resolve("--preflight")?;
    let resolution_manifest = resolve("--resolution-manifest")?;
    Ok(AdmissionOptions {
        repo,
        identity,
        validation_packet,
        builder_packet,
        integration_index,
        integration_index_sha256,
        preflight,
        resolution_manifest,
        out: parsed
            .optional("--out")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_ADMISSION_OUT)),
    })
}

fn admit_resolved_tree(args: &[String]) -> Result<(), String> {
    let out = control_out_from_args(args, DEFAULT_ADMISSION_OUT)?;
    let options = match parse_admission_options(args) {
        Ok(options) => options,
        Err(reason) => {
            let repo = current_repo()?;
            let owned_roots = supplied_protected_roots(
                &repo,
                args,
                &[
                    ("--validation-packet", "resolved-tree validation packet", false),
                    ("--builder-packet", "trusted-builder packet", false),
                    ("--integration-index", "integration receipt sidecar directory", true),
                    ("--preflight", "finalized P1 preflight", false),
                    ("--resolution-manifest", "complete resolution manifest", false),
                ],
            );
            let protected_roots = borrowed_protected_roots(&owned_roots);
            let report = admission_rejection_report(None, &reason);
            return write_rejection_or_combine_protected(
                &repo,
                &out,
                &protected_roots,
                &ControlPacketWrite {
                    kind: "resolved_tree_admission",
                    report_name: ADMISSION_REPORT,
                    report: &report,
                    title: "Resolved-tree admission",
                    claim_boundary: "A rejected admission packet is not construction eligibility and grants no object, ref, merge, release, or publication authority.",
                },
                reason,
            );
        }
    };

    let integration_root = options.integration_index.parent().ok_or_else(|| {
        "integration receipt index has no protected parent directory".to_string()
    })?;
    let protected_roots: [(&Path, &str); 6] = [
        (
            options.validation_packet.as_path(),
            "resolved-tree validation packet",
        ),
        (
            options.builder_packet.as_path(),
            "trusted-builder packet",
        ),
        (integration_root, "integration receipt sidecar directory"),
        (
            options.integration_index.as_path(),
            "integration receipt index",
        ),
        (options.preflight.as_path(), "finalized P1 preflight"),
        (
            options.resolution_manifest.as_path(),
            "complete resolution manifest",
        ),
    ];
    reject_control_packet_output_overlap(&options.repo, &options.out, &protected_roots)?;

    match validate_admission(&options) {
        Ok(evidence) => {
            let report = admission_success_report(&evidence);
            write_control_packet_protected(
                &options.repo,
                &options.out,
                &protected_roots,
                &ControlPacketWrite {
                    kind: "resolved_tree_admission",
                    report_name: ADMISSION_REPORT,
                    report: &report,
                    title: ADMISSION_TITLE,
                    claim_boundary: ADMISSION_SUCCESS_CLAIM,
                },
            )
        }
        Err(reason) => {
            let report = admission_rejection_report(Some(&options.identity), &reason);
            write_rejection_or_combine_protected(
                &options.repo,
                &options.out,
                &protected_roots,
                &ControlPacketWrite {
                    kind: "resolved_tree_admission",
                    report_name: ADMISSION_REPORT,
                    report: &report,
                    title: "Resolved-tree admission",
                    claim_boundary: "A rejected admission packet is not construction eligibility and grants no object, ref, merge, release, or publication authority.",
                },
                reason,
            )
        }
    }
}

fn validate_admission(options: &AdmissionOptions) -> Result<AdmissionEvidence, String> {
    validate_admission_with_snapshot_reader(options, admission_snapshot)
}

fn validate_admission_with_snapshot_reader<F>(
    options: &AdmissionOptions,
    mut snapshot_reader: F,
) -> Result<AdmissionEvidence, String>
where
    F: FnMut(
        &AdmissionOptions,
        &str,
        &IndexedPacket,
        &IndexedPacket,
        &IntegrationEvidence,
        &str,
    ) -> Result<AdmissionSnapshot, String>,
{
    let validation_packet = read_indexed_packet(
        &options.validation_packet,
        RESOLVED_TREE_PACKET_SCHEMA,
        None,
        Some("validated"),
        VALIDATION_REPORT,
    )?;
    let validation_receipt = packet_json(
        &validation_packet,
        VALIDATION_REPORT,
        "resolved-tree validation receipt",
    )?;
    validate_resolved_tree_binding(&validation_receipt, &options.identity)?;

    let (preflight, preflight_bytes) = read_bound_json(
        &options.preflight,
        &options.identity.preflight_sha256,
        "finalized P1 preflight",
    )?;
    validate_preflight(&preflight, &options.identity.source_parent)?;
    validate_preflight_identity(&preflight, &options.identity)?;
    let (manifest, _manifest_bytes) = read_bound_json(
        &options.resolution_manifest,
        &options.identity.resolution_sha256,
        "complete resolution manifest",
    )?;
    validate_manifest(&manifest, &preflight, &digest_bytes(&preflight_bytes))?;
    validate_manifest_identity(&manifest, &options.identity)?;

    let builder_packet = read_indexed_packet(
        &options.builder_packet,
        CONTROL_PACKET_SCHEMA,
        Some("trusted_builder"),
        Some("built"),
        BUILDER_REPORT,
    )?;
    let builder_receipt = packet_json(&builder_packet, BUILDER_REPORT, "trusted builder receipt")?;
    let executable_sha256 =
        validate_builder_receipt(&builder_receipt, &validation_receipt, options)?;

    let integration = validate_integration_index(
        &options.integration_index,
        &options.integration_index_sha256,
        &options.identity,
        &executable_sha256,
    )?;
    let swarm_ref = json_string(&preflight, "swarm_ref")
        .ok_or_else(|| "preflight is missing swarm_ref".to_string())?
        .to_string();
    validate_full_ref(&swarm_ref, "protected W7 ref")?;

    let before = snapshot_reader(
        options,
        &swarm_ref,
        &validation_packet,
        &builder_packet,
        &integration,
        &executable_sha256,
    )?;
    validate_admission_snapshot_identity(&before, &options.identity)?;
    if !clean_checkout(&options.repo)? {
        return Err("source checkout is not clean at admission".to_string());
    }
    let after = snapshot_reader(
        options,
        &swarm_ref,
        &validation_packet,
        &builder_packet,
        &integration,
        &executable_sha256,
    )?;
    validate_admission_snapshot_identity(&after, &options.identity)?;
    if before != after {
        return Err("admission inputs moved during final identity reread".to_string());
    }

    let validation_receipt_sha256 = packet_file_sha256(&validation_packet, VALIDATION_REPORT)?;
    let builder_receipt_sha256 = packet_file_sha256(&builder_packet, BUILDER_REPORT)?;
    Ok(AdmissionEvidence {
        identity: options.identity.clone(),
        swarm_ref,
        validation_index_sha256: validation_packet.index_sha256,
        validation_receipt_sha256,
        builder_index_sha256: builder_packet.index_sha256,
        builder_receipt_sha256,
        integration,
        executable_sha256,
    })
}

fn validate_admission_snapshot_identity(
    snapshot: &AdmissionSnapshot,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    for (actual, expected, reason) in [
        (
            snapshot.source_head.as_str(),
            identity.source_parent.as_str(),
            "source checkout HEAD moved from admitted SOURCE_PARENT",
        ),
        (
            snapshot.swarm_head.as_str(),
            identity.swarm_parent.as_str(),
            "protected W7 ref does not equal admitted SWARM_PARENT",
        ),
        (
            snapshot.join_tree.as_str(),
            identity.join_tree.as_str(),
            "reviewed join tree moved from admitted JOIN_TREE",
        ),
        (
            snapshot.preflight_sha256.as_str(),
            identity.preflight_sha256.as_str(),
            "finalized P1 preflight digest moved from exact admission input",
        ),
        (
            snapshot.resolution_sha256.as_str(),
            identity.resolution_sha256.as_str(),
            "complete resolution manifest digest moved from exact admission input",
        ),
    ] {
        if actual != expected {
            return Err(reason.to_string());
        }
    }
    Ok(())
}

fn validate_resolved_tree_binding(
    receipt: &Value,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    if !resolved_tree_receipt_is_admissible(receipt) {
        return Err(
            "resolved-tree receipt did not earn admission; a top-level validated string is insufficient"
                .to_string(),
        );
    }
    validate_resolved_tree_identity_binding(receipt, identity)
}

fn validate_resolved_tree_identity_binding(
    receipt: &Value,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    if json_string(receipt, "source_parent") != Some(identity.source_parent.as_str())
        || json_string(receipt, "swarm_parent") != Some(identity.swarm_parent.as_str())
        || json_string(receipt, "reviewed_tree") != Some(identity.join_tree.as_str())
    {
        return Err(
            "resolved-tree receipt identity differs from requested transaction".to_string(),
        );
    }
    let preflight = receipt
        .get("preflight")
        .ok_or_else(|| "resolved-tree receipt is missing preflight binding".to_string())?;
    let resolution = receipt
        .get("resolution_manifest")
        .ok_or_else(|| "resolved-tree receipt is missing resolution binding".to_string())?;
    if json_string(preflight, "sha256") != Some(identity.preflight_sha256.as_str())
        || json_string(resolution, "sha256") != Some(identity.resolution_sha256.as_str())
    {
        return Err("resolved-tree receipt sidecar digest differs from exact inputs".to_string());
    }
    Ok(())
}

fn validate_preflight_identity(
    preflight: &Value,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    if json_string(preflight, "source_parent") != Some(identity.source_parent.as_str())
        || json_string(preflight, "swarm_parent") != Some(identity.swarm_parent.as_str())
        || preflight
            .get("dry_merge")
            .and_then(|value| value.get("reviewed_resolved_tree"))
            .and_then(Value::as_str)
            != Some(identity.join_tree.as_str())
    {
        return Err("preflight identities differ from exact admission inputs".to_string());
    }
    Ok(())
}

fn validate_manifest_identity(
    manifest: &Value,
    identity: &PromotionIdentity,
) -> Result<(), String> {
    if json_string(manifest, "source_parent") != Some(identity.source_parent.as_str())
        || json_string(manifest, "swarm_parent") != Some(identity.swarm_parent.as_str())
        || json_string(manifest, "reviewed_join_tree") != Some(identity.join_tree.as_str())
        || json_string(manifest, "preflight_sha256") != Some(identity.preflight_sha256.as_str())
    {
        return Err(
            "resolution manifest identities differ from exact admission inputs".to_string(),
        );
    }
    Ok(())
}

fn validate_builder_receipt(
    builder: &Value,
    validation: &Value,
    options: &AdmissionOptions,
) -> Result<String, String> {
    let executable =
        validate_builder_receipt_contract(builder, validation, &options.identity)?;
    let expected_lock = file_sha256(&options.repo.join("Cargo.lock"), "Cargo.lock")?;
    if json_string(builder, "cargo_lock_sha256") != Some(expected_lock.as_str()) {
        return Err(
            "trusted builder Cargo.lock digest differs from live source checkout".to_string(),
        );
    }
    if current_executable_sha256()? != executable {
        return Err(
            "running admission executable differs from trusted builder receipt".to_string(),
        );
    }
    Ok(executable)
}

fn validate_builder_receipt_contract(
    builder: &Value,
    validation: &Value,
    identity: &PromotionIdentity,
) -> Result<String, String> {
    validate_exact_json_fields(
        builder,
        "trusted builder receipt",
        &[
            "schema", "status", "source_parent", "workflow_source_sha", "clean_checkout",
            "rust_toolchain", "cargo_lock_sha256", "locked_build", "isolated_cargo_target_dir",
            "executable_sha256", "failure_reasons", "authoritative_commit_attempted",
            "commit_tree_attempts", "local_ref_attempts", "remote_push_attempts",
            "merge_command_attempts", "merge_command", "ref_mutation_attempted", "push_attempted",
        ],
        &["non_claims"],
    )?;
    if json_string(builder, "schema") != Some(BUILDER_SCHEMA)
        || json_string(builder, "status") != Some("built")
        || json_string(builder, "source_parent") != Some(identity.source_parent.as_str())
        || json_string(builder, "workflow_source_sha")
            != Some(identity.source_parent.as_str())
        || json_bool(builder, "clean_checkout") != Some(true)
        || json_string(builder, "rust_toolchain")
            .is_none_or(|value| !value.starts_with(&format!("rustc {RUST_TOOLCHAIN} ")))
        || json_bool(builder, "locked_build") != Some(true)
        || json_bool(builder, "isolated_cargo_target_dir") != Some(true)
        || !empty_failure_reasons(builder)
    {
        return Err(
            "trusted builder receipt is incomplete or does not bind SOURCE_PARENT".to_string(),
        );
    }
    for key in [
        "authoritative_commit_attempted",
        "ref_mutation_attempted",
        "push_attempted",
    ] {
        if json_bool(builder, key) != Some(false) {
            return Err(format!("trusted builder receipt reports forbidden {key}"));
        }
    }
    for key in [
        "commit_tree_attempts",
        "local_ref_attempts",
        "remote_push_attempts",
        "merge_command_attempts",
    ] {
        if builder.get(key).and_then(Value::as_u64) != Some(0) {
            return Err(format!("trusted builder receipt reports forbidden {key}"));
        }
    }
    if !builder.get("merge_command").is_some_and(Value::is_null) {
        return Err("trusted builder receipt must not contain a merge command".to_string());
    }

    json_string(builder, "cargo_lock_sha256")
        .filter(|value| is_exact_lower_hex(value, 64))
        .ok_or_else(|| "trusted builder receipt has invalid Cargo.lock digest".to_string())?;
    let executable = json_string(builder, "executable_sha256")
        .filter(|value| is_exact_lower_hex(value, 64))
        .ok_or_else(|| "trusted builder receipt has invalid executable digest".to_string())?
        .to_string();
    let validation_executable = validation
        .get("trusted_checker")
        .and_then(|value| value.get("executable_sha256"))
        .and_then(Value::as_str);
    if validation_executable != Some(executable.as_str()) {
        return Err(
            "trusted builder executable digest differs from resolved-tree validator receipt"
                .to_string(),
        );
    }
    Ok(executable)
}

fn integration_schema(kind: &str) -> Option<&'static str> {
    match kind {
        "command_catalog_integration" => {
            Some("ripr.source_promotion_command_catalog_integration.v1")
        }
        "network_policy_integration" => Some("ripr.source_promotion_network_policy_integration.v1"),
        _ => None,
    }
}

fn validate_integration_index(
    index_path: &Path,
    expected_index_sha256: &str,
    identity: &PromotionIdentity,
    trusted_executable_sha256: &str,
) -> Result<IntegrationEvidence, String> {
    let (index, _) = read_bound_json(
        index_path,
        expected_index_sha256,
        "integration receipt index",
    )?;
    if json_string(&index, "schema") != Some(INTEGRATION_INDEX_SCHEMA)
        || json_string(&index, "status") != Some("complete")
        || !identity.matches_json(&index)
        || !empty_failure_reasons(&index)
    {
        return Err("integration receipt index is incomplete or identity-mismatched".to_string());
    }
    let required = index
        .get("required_kinds")
        .and_then(Value::as_array)
        .ok_or_else(|| "integration index is missing required_kinds".to_string())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| "integration required kind is not a string".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_required = REQUIRED_INTEGRATION_KINDS
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if required != expected_required {
        return Err(format!(
            "integration required_kinds must be exactly {expected_required:?}"
        ));
    }

    let rows = index
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| "integration index is missing receipts".to_string())?;
    if rows.len() != REQUIRED_INTEGRATION_KINDS.len() {
        return Err("integration index does not contain every required receipt".to_string());
    }
    let parent = index_path
        .parent()
        .ok_or_else(|| "integration index has no parent directory".to_string())?;
    let mut receipt_digests = BTreeMap::new();
    let mut prior: Option<String> = None;
    for row in rows {
        let kind = json_string(row, "kind")
            .ok_or_else(|| "integration receipt row is missing kind".to_string())?;
        if prior.as_deref().is_some_and(|value| value >= kind) {
            return Err(
                "integration receipt rows must be strictly sorted and unique by kind".to_string(),
            );
        }
        prior = Some(kind.to_string());
        let schema = integration_schema(kind)
            .ok_or_else(|| format!("unsupported integration receipt kind {kind}"))?;
        let path_text = json_string(row, "path")
            .ok_or_else(|| "integration receipt row is missing path".to_string())?;
        let relative = safe_packet_relative_path(path_text, "integration receipt path")?;
        let expected_digest = json_string(row, "sha256")
            .filter(|value| is_exact_lower_hex(value, 64))
            .ok_or_else(|| "integration receipt row has invalid sha256".to_string())?;
        let receipt_path = parent.join(relative);
        let (receipt, _, actual_digest) = read_json(&receipt_path, "integration receipt")?;
        if actual_digest != expected_digest {
            return Err(format!("integration receipt digest mismatch for {kind}"));
        }
        if json_string(&receipt, "schema") != Some(schema)
            || json_string(&receipt, "status") != Some("integrated")
            || !identity.matches_json(&receipt)
            || json_string(&receipt, "producer_source_sha") != Some(identity.source_parent.as_str())
            || json_string(&receipt, "producer_executable_sha256")
                != Some(trusted_executable_sha256)
            || json_bool(&receipt, "ref_mutation_attempted") != Some(false)
            || !empty_failure_reasons(&receipt)
        {
            return Err(format!(
                "integration receipt {kind} is incomplete or identity-mismatched"
            ));
        }
        if receipt_digests
            .insert(kind.to_string(), actual_digest)
            .is_some()
        {
            return Err(format!("duplicate integration receipt kind {kind}"));
        }
    }
    if !REQUIRED_INTEGRATION_KINDS
        .iter()
        .all(|kind| receipt_digests.contains_key(*kind))
    {
        return Err("integration index omitted a required typed receipt".to_string());
    }
    Ok(IntegrationEvidence {
        index_sha256: expected_index_sha256.to_string(),
        receipt_digests,
    })
}

fn admission_snapshot(
    options: &AdmissionOptions,
    swarm_ref: &str,
    validation_packet: &IndexedPacket,
    builder_packet: &IndexedPacket,
    integration: &IntegrationEvidence,
    executable_sha256: &str,
) -> Result<AdmissionSnapshot, String> {
    let source_head = current_head(&options.repo)?;
    let swarm_head = read_commit_ref(&options.repo, swarm_ref, "protected W7 ref")?;
    let join_tree = read_tree_identity(&options.repo, &options.identity.join_tree)?;
    let preflight_sha256 = file_sha256(&options.preflight, "finalized P1 preflight")?;
    let resolution_sha256 =
        file_sha256(&options.resolution_manifest, "complete resolution manifest")?;
    let observed_validation_packet = read_indexed_packet(
        &options.validation_packet,
        RESOLVED_TREE_PACKET_SCHEMA,
        None,
        Some("validated"),
        VALIDATION_REPORT,
    )?;
    let observed_builder_packet = read_indexed_packet(
        &options.builder_packet,
        CONTROL_PACKET_SCHEMA,
        Some("trusted_builder"),
        Some("built"),
        BUILDER_REPORT,
    )?;
    let observed_integration = validate_integration_index(
        &options.integration_index,
        &options.integration_index_sha256,
        &options.identity,
        executable_sha256,
    )?;
    if observed_validation_packet != *validation_packet
        || observed_builder_packet != *builder_packet
        || observed_integration != *integration
    {
        return Err("indexed admission evidence moved after validation".to_string());
    }
    let observed_executable = current_executable_sha256()?;
    if observed_executable != executable_sha256 {
        return Err("running checker executable moved during admission".to_string());
    }
    Ok(AdmissionSnapshot {
        source_head,
        swarm_head,
        join_tree,
        preflight_sha256,
        resolution_sha256,
        validation_index_sha256: observed_validation_packet.index_sha256,
        builder_index_sha256: observed_builder_packet.index_sha256,
        integration_index_sha256: observed_integration.index_sha256,
        executable_sha256: observed_executable,
    })
}

fn admission_success_report(evidence: &AdmissionEvidence) -> Value {
    serde_json::json!({
        "schema": ADMISSION_SCHEMA,
        "status": "admitted",
        "source_parent": evidence.identity.source_parent.as_str(),
        "swarm_parent": evidence.identity.swarm_parent.as_str(),
        "join_tree": evidence.identity.join_tree.as_str(),
        "swarm_ref": evidence.swarm_ref.as_str(),
        "preflight_sha256": evidence.identity.preflight_sha256.as_str(),
        "resolution_manifest_sha256": evidence.identity.resolution_sha256.as_str(),
        "resolved_tree_packet_index_sha256": evidence.validation_index_sha256,
        "resolved_tree_validation_receipt_sha256": evidence.validation_receipt_sha256,
        "trusted_builder_packet_index_sha256": evidence.builder_index_sha256,
        "trusted_builder_receipt_sha256": evidence.builder_receipt_sha256,
        "integration_index_sha256": evidence.integration.index_sha256,
        "integration_receipts": evidence.integration.receipt_digests,
        "checker_executable_sha256": evidence.executable_sha256,
        "all_required_typed_integration_receipts_present": true,
        "final_identity_reread_passed": true,
        "constructor_eligible_after_tree_qualification": true,
        "authoritative_commit_attempted": false,
        "commit_tree_attempts": 0,
        "local_ref_attempts": 0,
        "remote_push_attempts": 0,
        "merge_command_attempts": 0,
        "ref_mutation_attempted": false,
        "push_attempted": false,
        "merge_command": null,
        "failure_reasons": [],
        "invalidation_rules": [
            "Any movement in SOURCE_PARENT, W7 ref, JOIN_TREE, finalized P1 bytes, complete manifest bytes, validator packet/index, builder packet/index, integration receipts, or checker executable invalidates this admission.",
            "A later constructor must also consume a terminal exact TREE_QUALIFICATION receipt before git commit-tree.",
        ],
        "non_claims": [
            "No exact join object was constructed.",
            "No candidate ref, source ref, tag, release, marketplace, or publication channel was changed.",
            "Admission is not product/editor qualification and is not release authorization.",
        ],
    })
}

fn admission_rejection_report(identity: Option<&PromotionIdentity>, reason: &str) -> Value {
    let identity_json = identity.map(PromotionIdentity::as_json);
    serde_json::json!({
        "schema": ADMISSION_SCHEMA,
        "status": "rejected",
        "identity": identity_json,
        "source_parent": identity.map(|value| value.source_parent.as_str()),
        "swarm_parent": identity.map(|value| value.swarm_parent.as_str()),
        "join_tree": identity.map(|value| value.join_tree.as_str()),
        "preflight_sha256": identity.map(|value| value.preflight_sha256.as_str()),
        "resolution_manifest_sha256": identity.map(|value| value.resolution_sha256.as_str()),
        "all_required_typed_integration_receipts_present": false,
        "final_identity_reread_passed": false,
        "constructor_eligible_after_tree_qualification": false,
        "authoritative_commit_attempted": false,
        "commit_tree_attempts": 0,
        "local_ref_attempts": 0,
        "remote_push_attempts": 0,
        "merge_command_attempts": 0,
        "ref_mutation_attempted": false,
        "push_attempted": false,
        "merge_command": null,
        "failure_reasons": [reason],
        "non_claims": [
            "A rejected admission is not construction eligibility.",
            "No exact join object, candidate ref, merge command, release, or publication authority was created.",
        ],
    })
}
