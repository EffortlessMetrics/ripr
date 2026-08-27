#[cfg(test)]
mod source_promotion_control_tests {
    use super::*;
    use std::fmt::Debug;
    use std::ops::Deref;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
        cleaned: bool,
    }

    impl TestDir {
        fn new(path: PathBuf) -> Self {
            Self {
                path,
                cleaned: false,
            }
        }

        fn cleanup(mut self) -> Result<(), String> {
            fs::remove_dir_all(&self.path).map_err(|error| {
                format!(
                    "failed to remove test directory {}: {error}",
                    self.path.display()
                )
            })?;
            self.cleaned = true;
            Ok(())
        }

        fn to_path_buf(&self) -> PathBuf {
            self.deref().to_path_buf()
        }
    }

    impl Deref for TestDir {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            if !self.cleaned {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(message.into())
        }
    }

    fn require_equal<T>(actual: T, expected: T, context: &str) -> Result<(), String>
    where
        T: Debug + PartialEq,
    {
        if actual == expected {
            Ok(())
        } else {
            Err(format!(
                "{context}: expected {expected:?}, observed {actual:?}"
            ))
        }
    }

    fn replace_object_field(
        value: &mut Value,
        key: &str,
        replacement: Value,
        context: &str,
    ) -> Result<(), String> {
        let field = value
            .as_object_mut()
            .and_then(|object| object.get_mut(key))
            .ok_or_else(|| format!("{context} is missing object field {key}"))?;
        *field = replacement;
        Ok(())
    }

    fn required_array_field_mut<'a>(
        value: &'a mut Value,
        key: &str,
        context: &str,
    ) -> Result<&'a mut Vec<Value>, String> {
        value
            .as_object_mut()
            .and_then(|object| object.get_mut(key))
            .and_then(Value::as_array_mut)
            .ok_or_else(|| format!("{context} is missing array field {key}"))
    }

    fn normalized_contract_definition(
        root: &Path,
        contract_path: &str,
        start: &str,
        end: &str,
    ) -> Result<String, String> {
        let contract = fs::read_to_string(root.join(contract_path))
            .map_err(|error| format!("read {contract_path}: {error}"))?
            .replace("\r\n", "\n");
        let definition = contract
            .split_once(start)
            .map(|(_, remainder)| remainder)
            .and_then(|remainder| remainder.split_once(end).map(|(body, _)| body))
            .ok_or_else(|| format!("{contract_path} status definition is missing"))?;
        Ok(format!("{start}{definition}")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "))
    }

    fn test_identity() -> PromotionIdentity {
        PromotionIdentity {
            source_parent: "1111111111111111111111111111111111111111".to_string(),
            swarm_parent: "2222222222222222222222222222222222222222".to_string(),
            join_tree: "3333333333333333333333333333333333333333".to_string(),
            preflight_sha256: "4444444444444444444444444444444444444444444444444444444444444444"
                .to_string(),
            resolution_sha256: "5555555555555555555555555555555555555555555555555555555555555555"
                .to_string(),
        }
    }

    fn command_subject_role_for_test(command: &str) -> &'static str {
        if command == "check-command-catalog" {
            "source_parent_trusted_checker_self_health"
        } else {
            "reviewed_tree_source_governance_contract"
        }
    }

    fn valid_resolved_tree_receipt() -> Value {
        let identity = test_identity();
        let commands = [
            "check-network-policy",
            "check-process-policy",
            "check-workflows",
            "check-file-policy",
            "check-dependencies",
            "check-generated-clean",
            "check-executable-files",
            "check-command-catalog",
            "check-spec-format",
            "check-traceability",
            "check-doc-artifacts",
            "check-public-api",
            "check-architecture",
        ]
        .iter()
        .enumerate()
        .map(|(index, command)| {
            serde_json::json!({
                "command": command,
                "subject_role": command_subject_role_for_test(command),
                "state": "passed",
                "exit_code": 0,
                "timeout_bound_ms": 180000,
                "evidence_present": true,
                "stdout_path": format!("commands/{:02}-{command}.stdout.log", index + 1),
                "stdout_bytes": 0,
                "stdout_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "stdout_truncated": false,
                "stderr_path": format!("commands/{:02}-{command}.stderr.log", index + 1),
                "stderr_bytes": 0,
                "stderr_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "stderr_truncated": false,
                "failure_reason": null,
            })
        })
        .collect::<Vec<_>>();
        serde_json::json!({
            "schema": "ripr.source_promotion_resolved_tree_validation.v1",
            "status": "validated",
            "source_parent": identity.source_parent.as_str(),
            "swarm_parent": identity.swarm_parent.as_str(),
            "reviewed_tree": identity.join_tree.as_str(),
            "preflight": {
                "path_role": "source_checkout_regular_file",
                "path": "preflight.json",
                "sha256": identity.preflight_sha256.as_str(),
                "verified": true,
            },
            "resolution_manifest": {
                "path_role": "source_checkout_regular_file",
                "path": "resolution.json",
                "sha256": identity.resolution_sha256.as_str(),
                "verified": true,
            },
            "trusted_checker": {
                "source_sha": "1111111111111111111111111111111111111111",
                "executable_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            },
            "materialization": {
                "reviewed_tree": "3333333333333333333333333333333333333333",
                "disposable_commit": "6666666666666666666666666666666666666666",
                "created": true,
                "clean_before": true,
                "clean_after": true,
                "worktree_remove_succeeded": true,
                "directory_removed": true,
                "worktree_residue_observed": false,
                "cleanup_failure_reason": null,
            },
            "commands": commands,
            "repository_observation": {
                "ref_mutation_observed": false,
                "worktree_registry_changed": false,
            },
            "authoritative_commit_attempted": false,
            "branch_attempted": false,
            "tag_attempted": false,
            "push_attempted": false,
            "ref_mutation_attempted": false,
            "failure_reasons": [],
        })
    }

    fn valid_admission_receipt(identity: &PromotionIdentity) -> Value {
        serde_json::json!({
            "schema": ADMISSION_SCHEMA,
            "status": "admitted",
            "source_parent": identity.source_parent.as_str(),
            "swarm_parent": identity.swarm_parent.as_str(),
            "join_tree": identity.join_tree.as_str(),
            "preflight_sha256": identity.preflight_sha256.as_str(),
            "resolution_manifest_sha256": identity.resolution_sha256.as_str(),
            "swarm_ref": "refs/tags/ripr-release-0.11.0-2222222222222222222222222222222222222222",
            "resolved_tree_packet_index_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "resolved_tree_validation_receipt_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "trusted_builder_packet_index_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "trusted_builder_receipt_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            "integration_index_sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "integration_receipts": {
                "command_catalog_integration": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                "network_policy_integration": "9999999999999999999999999999999999999999999999999999999999999999",
            },
            "checker_executable_sha256": "8888888888888888888888888888888888888888888888888888888888888888",
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
        })
    }

    fn integration_evidence_from_admission(admission: &Value) -> Result<IntegrationEvidence, String> {
        let index_sha256 = json_string(admission, "integration_index_sha256")
            .ok_or_else(|| "admission fixture is missing integration index digest".to_string())?
            .to_string();
        let receipts = admission
            .get("integration_receipts")
            .and_then(Value::as_object)
            .ok_or_else(|| "admission fixture is missing integration receipts".to_string())?;
        let mut receipt_digests = BTreeMap::new();
        for kind in REQUIRED_INTEGRATION_KINDS {
            let digest = receipts
                .get(*kind)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("admission fixture is missing {kind} digest"))?;
            receipt_digests.insert((*kind).to_string(), digest.to_string());
        }
        Ok(IntegrationEvidence {
            index_sha256,
            receipt_digests,
        })
    }

    fn valid_builder_receipt(
        identity: &PromotionIdentity,
        executable_sha256: &str,
        cargo_lock_sha256: &str,
    ) -> Value {
        serde_json::json!({
            "schema": BUILDER_SCHEMA,
            "status": "built",
            "source_parent": identity.source_parent.as_str(),
            "workflow_source_sha": identity.source_parent.as_str(),
            "clean_checkout": true,
            "rust_toolchain": format!("rustc {RUST_TOOLCHAIN} test"),
            "locked_build": true,
            "isolated_cargo_target_dir": true,
            "cargo_lock_sha256": cargo_lock_sha256,
            "executable_sha256": executable_sha256,
            "authoritative_commit_attempted": false,
            "commit_tree_attempts": 0,
            "local_ref_attempts": 0,
            "remote_push_attempts": 0,
            "merge_command_attempts": 0,
            "ref_mutation_attempted": false,
            "push_attempted": false,
            "merge_command": null,
            "failure_reasons": [],
        })
    }

    fn valid_qualification_receipt_for(
        identity: &PromotionIdentity,
        admission: &Value,
        admission_packet: &IndexedPacket,
        admission_receipt_sha256: &str,
        network_policy_receipt_sha256: &str,
    ) -> Result<Value, String> {
        let validation_receipt_sha256 =
            json_string(admission, "resolved_tree_validation_receipt_sha256").ok_or_else(|| {
                "admission fixture is missing resolved-tree validation digest".to_string()
            })?;
        Ok(serde_json::json!({
            "schema": QUALIFICATION_SCHEMA,
            "status": "qualified",
            "source_parent": identity.source_parent.as_str(),
            "swarm_parent": identity.swarm_parent.as_str(),
            "join_tree": identity.join_tree.as_str(),
            "preflight_sha256": identity.preflight_sha256.as_str(),
            "resolution_manifest_sha256": identity.resolution_sha256.as_str(),
            "admission_packet_index_sha256": admission_packet.index_sha256.as_str(),
            "admission_receipt_sha256": admission_receipt_sha256,
            "resolved_tree_validation_receipt_sha256": validation_receipt_sha256,
            "network_policy_receipt_sha256": network_policy_receipt_sha256,
            "promotion_ref_mutation_attempted": false,
            "lanes": REQUIRED_QUALIFICATION_LANES.iter().map(|name| serde_json::json!({
                "name": name,
                "state": "passed",
                "evidence_sha256":
                    "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
            })).collect::<Vec<_>>(),
            "failure_reasons": [],
        }))
    }

    fn valid_construction_evidence(identity: PromotionIdentity) -> ConstructionEvidence {
        ConstructionEvidence {
            identity,
            swarm_ref: "refs/tags/ripr-release-0.11.0-2222222222222222222222222222222222222222"
                .to_string(),
            candidate_ref: "refs/heads/promote/0.11.0-test".to_string(),
            admission_index_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            admission_receipt_sha256:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            validation_index_sha256:
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
            integration_index_sha256:
                "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
            qualification_sha256:
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
            join_commit: "7777777777777777777777777777777777777777".to_string(),
            commit_timestamp: "2026-08-25T00:00:00+00:00".to_string(),
        }
    }

    fn test_temp_dir(label: &str) -> Result<TestDir, String> {
        let token = digest_bytes(
            format!(
                "{label}:{}:{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|error| format!("test clock error: {error}"))?
                    .as_nanos()
            )
            .as_bytes(),
        );
        let suffix = token
            .get(..20)
            .ok_or_else(|| "test directory token is shorter than 20 bytes".to_string())?;
        let path =
            std::env::temp_dir().join(format!("ripr-source-promotion-control-{label}-{suffix}"));
        fs::create_dir(&path)
            .map_err(|error| format!("failed to create test directory: {error}"))?;
        Ok(TestDir::new(path))
    }

    fn git_test(repo: &Path, args: &[&str]) -> Result<String, String> {
        git(repo, args)
    }

    fn init_synthetic_repo(label: &str) -> Result<(TestDir, PromotionIdentity), String> {
        let repo = test_temp_dir(label)?;
        git_test(&repo, &["init", "--initial-branch=main"])?;
        git_test(&repo, &["config", "user.name", "RIPR Test"])?;
        git_test(&repo, &["config", "user.email", "ripr-test@invalid"])?;
        git_test(&repo, &["config", "commit.gpgsign", "false"])?;
        git_test(&repo, &["config", "tag.gpgSign", "false"])?;
        fs::write(repo.join("base.txt"), "base\n")
            .map_err(|error| format!("failed to write base fixture: {error}"))?;
        git_test(&repo, &["add", "base.txt"])?;
        git_test(&repo, &["commit", "-m", "base"])?;
        git_test(&repo, &["checkout", "-b", "swarm"])?;
        fs::write(repo.join("swarm.txt"), "swarm\n")
            .map_err(|error| format!("failed to write swarm fixture: {error}"))?;
        git_test(&repo, &["add", "swarm.txt"])?;
        git_test(&repo, &["commit", "-m", "swarm"])?;
        let swarm_parent = current_head(&repo)?;
        git_test(
            &repo,
            &[
                "tag",
                &format!("ripr-release-0.11.0-{swarm_parent}"),
                &swarm_parent,
            ],
        )?;
        git_test(&repo, &["checkout", "main"])?;
        fs::write(repo.join("source.txt"), "source\n")
            .map_err(|error| format!("failed to write source fixture: {error}"))?;
        git_test(&repo, &["add", "source.txt"])?;
        git_test(&repo, &["commit", "-m", "source"])?;
        let source_parent = current_head(&repo)?;
        let join_tree = commit_tree(&repo, &source_parent)?;
        Ok((
            repo,
            PromotionIdentity {
                source_parent,
                swarm_parent,
                join_tree,
                preflight_sha256:
                    "4444444444444444444444444444444444444444444444444444444444444444".to_string(),
                resolution_sha256:
                    "5555555555555555555555555555555555555555555555555555555555555555".to_string(),
            },
        ))
    }

    struct ConstructionSnapshotFixture {
        repo: TestDir,
        identity: PromotionIdentity,
        options: ConstructionOptions,
        admission_packet: IndexedPacket,
        validation_packet: IndexedPacket,
        integration: IntegrationEvidence,
        qualification_sha256: String,
    }

    struct AdmissionSnapshotFixture {
        repo: TestDir,
        options: AdmissionOptions,
        evidence: AdmissionEvidence,
        swarm_ref: String,
        validation_packet: IndexedPacket,
        builder_packet: IndexedPacket,
        integration: IntegrationEvidence,
        executable_sha256: String,
    }

    fn write_test_json(path: &Path, value: &Value, label: &str) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec_pretty(value)
            .map_err(|error| format!("failed to encode {label}: {error}"))?;
        bytes.push(b'\n');
        fs::write(path, &bytes).map_err(|error| format!("failed to write {label}: {error}"))?;
        Ok(bytes)
    }

    fn write_test_packet(
        root: &Path,
        schema: &str,
        kind: &str,
        status: &str,
        report_name: &str,
        report: &Value,
    ) -> Result<IndexedPacket, String> {
        fs::create_dir_all(root)
            .map_err(|error| format!("failed to create test packet directory: {error}"))?;
        let report_bytes = write_test_json(&root.join(report_name), report, "test packet report")?;
        let index = serde_json::json!({
            "schema": schema,
            "kind": kind,
            "status": status,
            "complete": true,
            "files": [{
                "path": report_name,
                "bytes": report_bytes.len(),
                "sha256": digest_bytes(&report_bytes),
            }],
        });
        write_test_json(&root.join(PACKET_INDEX), &index, "test packet index")?;
        read_indexed_packet(root, schema, Some(kind), Some(status), report_name)
    }

    fn write_test_integration_index(
        root: &Path,
        identity: &PromotionIdentity,
        executable_sha256: &str,
    ) -> Result<(PathBuf, IntegrationEvidence), String> {
        fs::create_dir_all(root)
            .map_err(|error| format!("failed to create integration fixture directory: {error}"))?;
        let mut rows = Vec::new();
        for (kind, path) in [
            ("command_catalog_integration", "command-catalog.json"),
            ("network_policy_integration", "network-policy.json"),
        ] {
            let schema = integration_schema(kind)
                .ok_or_else(|| format!("test integration kind is unsupported: {kind}"))?;
            let receipt = serde_json::json!({
                "schema": schema,
                "status": "integrated",
                "source_parent": identity.source_parent.as_str(),
                "swarm_parent": identity.swarm_parent.as_str(),
                "join_tree": identity.join_tree.as_str(),
                "preflight_sha256": identity.preflight_sha256.as_str(),
                "resolution_manifest_sha256": identity.resolution_sha256.as_str(),
                "producer_source_sha": identity.source_parent.as_str(),
                "producer_executable_sha256": executable_sha256,
                "ref_mutation_attempted": false,
                "failure_reasons": [],
            });
            let bytes = write_test_json(&root.join(path), &receipt, "typed integration receipt")?;
            rows.push(serde_json::json!({
                "kind": kind,
                "path": path,
                "sha256": digest_bytes(&bytes),
            }));
        }
        let index = serde_json::json!({
            "schema": INTEGRATION_INDEX_SCHEMA,
            "status": "complete",
            "source_parent": identity.source_parent.as_str(),
            "swarm_parent": identity.swarm_parent.as_str(),
            "join_tree": identity.join_tree.as_str(),
            "preflight_sha256": identity.preflight_sha256.as_str(),
            "resolution_manifest_sha256": identity.resolution_sha256.as_str(),
            "required_kinds": REQUIRED_INTEGRATION_KINDS,
            "receipts": rows,
            "failure_reasons": [],
        });
        let index_path = root.join("integration-index.json");
        let index_bytes = write_test_json(&index_path, &index, "integration receipt index")?;
        let index_sha256 = digest_bytes(&index_bytes);
        let evidence = validate_integration_index(
            &index_path,
            &index_sha256,
            identity,
            executable_sha256,
        )?;
        Ok((index_path, evidence))
    }

    fn admission_snapshot_fixture(label: &str) -> Result<AdmissionSnapshotFixture, String> {
        let (repo, mut identity) = init_synthetic_repo(label)?;
        fs::write(repo.join("Cargo.lock"), b"# synthetic lockfile\n")
            .map_err(|error| format!("failed to write synthetic Cargo.lock: {error}"))?;
        git_test(&repo, &["add", "Cargo.lock"])?;
        git_test(&repo, &["commit", "-m", "add synthetic lockfile"])?;
        identity.source_parent = current_head(&repo)?;
        identity.join_tree = commit_tree(&repo, &identity.source_parent)?;
        let evidence_root = repo.join(".git/source-promotion-control-test");
        fs::create_dir_all(&evidence_root)
            .map_err(|error| format!("failed to create admission evidence directory: {error}"))?;
        let executable_sha256 = current_executable_sha256()?;
        let swarm_ref = format!(
            "refs/tags/ripr-release-0.11.0-{}",
            identity.swarm_parent.as_str()
        );
        let preflight = serde_json::json!({
            "schema": "ripr.source_promotion_preflight.v1",
            "mode": "two_parent_join",
            "source_parent": identity.source_parent.as_str(),
            "source_main": identity.source_parent.as_str(),
            "swarm_parent": identity.swarm_parent.as_str(),
            "swarm_ref": swarm_ref.as_str(),
            "swarm_ref_sha": identity.swarm_parent.as_str(),
            "merge_base": identity.source_parent.as_str(),
            "source_repository": {
                "common_dir_verified": true,
                "root_verified": true,
                "remote_verified": true,
            },
            "swarm_repository": {
                "common_dir_verified": true,
                "root_verified": true,
                "remote_verified": true,
            },
            "dry_merge": {
                "reviewed_resolved_tree": identity.join_tree.as_str(),
                "reviewed_resolved_tree_verified": true,
                "conflicts": [],
            },
            "source_range": {},
            "swarm_range": {},
            "version_state": { "requested_version": "0.11.0" },
            "invalidation_rules": {},
            "source_survivor_candidates": [],
            "swarm_authority_resolution_candidates": [],
        });
        let preflight_path = evidence_root.join("preflight.json");
        let preflight_bytes = write_test_json(&preflight_path, &preflight, "admission preflight")?;
        identity.preflight_sha256 = digest_bytes(&preflight_bytes);
        let manifest = serde_json::json!({
            "schema": "ripr.source_promotion_resolution.v1",
            "preflight_sha256": identity.preflight_sha256.as_str(),
            "source_parent": identity.source_parent.as_str(),
            "swarm_parent": identity.swarm_parent.as_str(),
            "merge_base": identity.source_parent.as_str(),
            "reviewed_join_tree": identity.join_tree.as_str(),
            "dispositions": [],
        });
        let resolution_path = evidence_root.join("resolution.json");
        let resolution_bytes =
            write_test_json(&resolution_path, &manifest, "admission resolution manifest")?;
        identity.resolution_sha256 = digest_bytes(&resolution_bytes);

        let mut validation = valid_resolved_tree_receipt();
        for (key, value) in [
            ("source_parent", identity.source_parent.as_str()),
            ("swarm_parent", identity.swarm_parent.as_str()),
            ("reviewed_tree", identity.join_tree.as_str()),
        ] {
            replace_object_field(
                &mut validation,
                key,
                Value::String(value.to_string()),
                "admission validation identity",
            )?;
        }
        let validation_preflight = validation
            .get_mut("preflight")
            .ok_or_else(|| "admission validation fixture is missing preflight".to_string())?;
        replace_object_field(
            validation_preflight,
            "sha256",
            Value::String(identity.preflight_sha256.clone()),
            "admission validation preflight digest",
        )?;
        let validation_resolution = validation
            .get_mut("resolution_manifest")
            .ok_or_else(|| "admission validation fixture is missing resolution manifest".to_string())?;
        replace_object_field(
            validation_resolution,
            "sha256",
            Value::String(identity.resolution_sha256.clone()),
            "admission validation resolution digest",
        )?;
        let trusted_checker = validation
            .get_mut("trusted_checker")
            .ok_or_else(|| "admission validation fixture is missing trusted checker".to_string())?;
        replace_object_field(
            trusted_checker,
            "source_sha",
            Value::String(identity.source_parent.clone()),
            "admission validation checker source",
        )?;
        replace_object_field(
            trusted_checker,
            "executable_sha256",
            Value::String(executable_sha256.clone()),
            "admission validation checker executable",
        )?;
        let materialization = validation
            .get_mut("materialization")
            .ok_or_else(|| "admission validation fixture is missing materialization".to_string())?;
        replace_object_field(
            materialization,
            "reviewed_tree",
            Value::String(identity.join_tree.clone()),
            "admission validation materialized tree",
        )?;
        let validation_packet = write_test_packet(
            &evidence_root.join("validation-packet"),
            RESOLVED_TREE_PACKET_SCHEMA,
            "resolved_tree_validation",
            "validated",
            VALIDATION_REPORT,
            &validation,
        )?;

        let cargo_lock_sha256 = file_sha256(&repo.join("Cargo.lock"), "test Cargo.lock")?;
        let builder =
            valid_builder_receipt(&identity, &executable_sha256, &cargo_lock_sha256);
        let builder_packet = write_test_packet(
            &evidence_root.join("builder-packet"),
            CONTROL_PACKET_SCHEMA,
            "trusted_builder",
            "built",
            BUILDER_REPORT,
            &builder,
        )?;
        let (integration_index, integration) = write_test_integration_index(
            &evidence_root.join("integration"),
            &identity,
            &executable_sha256,
        )?;
        let options = AdmissionOptions {
            repo: repo.to_path_buf(),
            identity,
            validation_packet: evidence_root.join("validation-packet"),
            builder_packet: evidence_root.join("builder-packet"),
            integration_index,
            integration_index_sha256: integration.index_sha256.clone(),
            preflight: preflight_path,
            resolution_manifest: resolution_path,
            out: evidence_root.join("unused-output"),
        };
        let evidence = validate_admission(&options)?;
        Ok(AdmissionSnapshotFixture {
            repo,
            options,
            evidence,
            swarm_ref,
            validation_packet,
            builder_packet,
            integration,
            executable_sha256,
        })
    }

    fn construction_snapshot_fixture(label: &str) -> Result<ConstructionSnapshotFixture, String> {
        let (repo, identity) = init_synthetic_repo(label)?;
        let admission_root = repo.join("admission-packet");
        let validation_root = repo.join("validation-packet");
        let mut admission = valid_admission_receipt(&identity);
        let executable_sha256 = admission
            .get("checker_executable_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "test admission receipt is missing executable identity".to_string())?
            .to_string();
        let (integration_index, integration) = write_test_integration_index(
            &repo.join("integration"),
            &identity,
            &executable_sha256,
        )?;
        replace_object_field(
            &mut admission,
            "integration_index_sha256",
            Value::String(integration.index_sha256.clone()),
            "construction admission fixture",
        )?;
        replace_object_field(
            &mut admission,
            "integration_receipts",
            serde_json::to_value(&integration.receipt_digests).map_err(|error| {
                format!("failed to encode construction integration evidence: {error}")
            })?,
            "construction admission fixture",
        )?;
        let admission_packet = write_test_packet(
            &admission_root,
            CONTROL_PACKET_SCHEMA,
            "resolved_tree_admission",
            "admitted",
            ADMISSION_REPORT,
            &admission,
        )?;
        let validation_packet = write_test_packet(
            &validation_root,
            RESOLVED_TREE_PACKET_SCHEMA,
            "resolved_tree_validation",
            "validated",
            VALIDATION_REPORT,
            &valid_resolved_tree_receipt(),
        )?;
        let preflight = repo.join("preflight.json");
        let resolution_manifest = repo.join("resolution.json");
        let qualification_receipt = repo.join("qualification.json");
        fs::write(&preflight, b"preflight\n")
            .map_err(|error| format!("failed to write snapshot preflight: {error}"))?;
        fs::write(&resolution_manifest, b"resolution\n")
            .map_err(|error| format!("failed to write snapshot resolution: {error}"))?;
        fs::write(&qualification_receipt, b"qualification\n")
            .map_err(|error| format!("failed to write snapshot qualification: {error}"))?;
        let qualification_sha256 = file_sha256(
            &qualification_receipt,
            "snapshot qualification receipt",
        )?;
        let options = ConstructionOptions {
            repo: repo.to_path_buf(),
            admission_packet: admission_root,
            validation_packet: validation_root,
            integration_index,
            integration_index_sha256: integration.index_sha256.clone(),
            preflight,
            resolution_manifest,
            qualification_receipt,
            qualification_receipt_sha256: qualification_sha256.clone(),
            source_main_ref: SOURCE_MAIN_REF.to_string(),
            swarm_ref: format!(
                "refs/tags/ripr-release-0.11.0-{}",
                identity.swarm_parent.as_str()
            ),
            candidate_ref: "refs/heads/promote/0.11.0-test".to_string(),
            out: repo.join("unused-output"),
        };
        construction_snapshot(
            &options,
            &identity,
            &admission_packet,
            &validation_packet,
            &integration,
            &qualification_sha256,
        )?;
        Ok(ConstructionSnapshotFixture {
            repo,
            identity,
            options,
            admission_packet,
            validation_packet,
            integration,
            qualification_sha256,
        })
    }

    fn require_snapshot_rejection_without_authority(
        fixture: &ConstructionSnapshotFixture,
        reason: &str,
        refs_before: &str,
    ) -> Result<(), String> {
        require_equal(
            refs_digest(&fixture.repo)?,
            refs_before.to_string(),
            "snapshot rejection must preserve every ref",
        )?;
        let report = construction_rejection_report(
            Some(&fixture.identity),
            Some(&fixture.options.candidate_ref),
            reason,
            false,
        );
        for counter in [
            "commit_tree_attempts",
            "local_ref_attempts",
            "remote_push_attempts",
            "merge_command_attempts",
        ] {
            require_equal(
                report.get(counter).and_then(Value::as_u64),
                Some(0),
                "snapshot rejection attempt counter",
            )?;
        }
        require(
            report.get("merge_command").is_some_and(Value::is_null),
            "snapshot rejection must not emit merge authority",
        )
    }

    fn require_admission_rejection_without_authority(
        fixture: &AdmissionSnapshotFixture,
        reason: &str,
        refs_before: &str,
    ) -> Result<(), String> {
        require_equal(
            refs_digest(&fixture.repo)?,
            refs_before.to_string(),
            "admission rejection must preserve every ref",
        )?;
        let report = admission_rejection_report(Some(&fixture.options.identity), reason);
        for counter in [
            "commit_tree_attempts",
            "local_ref_attempts",
            "remote_push_attempts",
            "merge_command_attempts",
        ] {
            require_equal(
                report.get(counter).and_then(Value::as_u64),
                Some(0),
                "admission rejection attempt counter",
            )?;
        }
        require(
            report.get("merge_command").is_some_and(Value::is_null),
            "admission rejection must not emit merge authority",
        )
    }

    fn require_builder_rejection_without_merge_authority(root: &Path) -> Result<(), String> {
        let packet = read_indexed_packet(
            root,
            CONTROL_PACKET_SCHEMA,
            Some("trusted_builder"),
            Some("rejected"),
            BUILDER_REPORT,
        )?;
        let report = packet_json(&packet, BUILDER_REPORT, "rejected trusted builder receipt")?;
        require(
            report.get("merge_command").is_some_and(Value::is_null),
            "rejected builder receipt must retain a null merge command",
        )?;
        for counter in [
            "commit_tree_attempts",
            "local_ref_attempts",
            "remote_push_attempts",
            "merge_command_attempts",
        ] {
            require_equal(
                report.get(counter).and_then(Value::as_u64),
                Some(0),
                "rejected builder attempt counter",
            )?;
        }
        Ok(())
    }

    #[test]
    fn impossible_validated_fixture_is_explicitly_rejected() -> Result<(), String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/source_promotion_control/impossible-validated.json");
        let (value, _, _) = read_json(&path, "impossible validated fixture")?;
        require_equal(
            json_string(&value, "status"),
            Some("validated"),
            "negative fixture must remain superficially validated",
        )?;
        require(
            !resolved_tree_receipt_is_admissible(&value),
            "a top-level validated string must never earn admission",
        )?;
        Ok(())
    }

    #[test]
    fn resolved_tree_admission_rejects_failed_unavailable_not_run_and_reordered_commands()
    -> Result<(), String> {
        let valid = valid_resolved_tree_receipt();
        require(
            resolved_tree_receipt_is_admissible(&valid),
            "valid resolved-tree receipt should be admissible",
        )?;
        for state in ["failed", "unavailable", "not_run"] {
            let mut candidate = valid.clone();
            let command = required_array_field_mut(
                &mut candidate,
                "commands",
                "resolved-tree receipt fixture",
            )?
                .first_mut()
                .ok_or_else(|| "resolved-tree commands fixture is empty".to_string())?;
            replace_object_field(
                command,
                "state",
                Value::String(state.to_string()),
                "resolved-tree command fixture",
            )?;
            require(
                !resolved_tree_receipt_is_admissible(&candidate),
                format!("resolved-tree receipt with {state} command should reject"),
            )?;
        }

        let mut reordered = valid.clone();
        let commands = required_array_field_mut(
            &mut reordered,
            "commands",
            "resolved-tree receipt fixture",
        )?;
        require(
            commands.len() >= 2,
            "resolved-tree commands fixture requires two entries for reorder",
        )?;
        commands.swap(0, 1);
        require(
            !resolved_tree_receipt_is_admissible(&reordered),
            "resolved-tree receipt with reordered commands should reject",
        )?;

        let mut duplicate = valid;
        let commands = required_array_field_mut(
            &mut duplicate,
            "commands",
            "resolved-tree receipt fixture",
        )?;
        let first = commands
            .first()
            .cloned()
            .ok_or_else(|| "resolved-tree commands fixture is empty".to_string())?;
        commands.push(first);
        require(
            !resolved_tree_receipt_is_admissible(&duplicate),
            "resolved-tree receipt with duplicate commands should reject",
        )?;
        Ok(())
    }

    #[test]
    fn admission_receipt_requires_every_green_authority_bit() -> Result<(), String> {
        let identity = test_identity();
        let valid = valid_admission_receipt(&identity);
        require(
            validate_admission_receipt(&valid, &identity).is_ok(),
            "valid admission receipt should pass",
        )?;

        for key in [
            "all_required_typed_integration_receipts_present",
            "final_identity_reread_passed",
            "constructor_eligible_after_tree_qualification",
        ] {
            let mut candidate = valid.clone();
            replace_object_field(
                &mut candidate,
                key,
                Value::Bool(false),
                "admission receipt fixture",
            )?;
            require(
                validate_admission_receipt(&candidate, &identity).is_err(),
                format!("admission receipt should reject false {key}"),
            )?;
        }

        let mut attempted = valid.clone();
        replace_object_field(
            &mut attempted,
            "ref_mutation_attempted",
            Value::Bool(true),
            "admission receipt fixture",
        )?;
        require(
            validate_admission_receipt(&attempted, &identity).is_err(),
            "admission receipt should reject a ref mutation attempt",
        )?;
        Ok(())
    }

    #[test]
    fn admission_consumer_requires_null_builder_merge_command() -> Result<(), String> {
        let fixture = admission_snapshot_fixture("builder-merge-command-consumer")?;
        let builder = packet_json(
            &fixture.builder_packet,
            BUILDER_REPORT,
            "trusted builder fixture",
        )?;
        let validation = packet_json(
            &fixture.validation_packet,
            VALIDATION_REPORT,
            "resolved-tree validation fixture",
        )?;
        require(
            validate_builder_receipt(&builder, &validation, &fixture.options).is_ok(),
            "builder consumer must accept an explicitly null merge command",
        )?;

        let mut missing = builder.clone();
        let removed = missing
            .as_object_mut()
            .and_then(|object| object.remove("merge_command"));
        require(
            removed.is_some(),
            "builder fixture must contain merge_command before removal",
        )?;
        require(
            validate_builder_receipt(&missing, &validation, &fixture.options).is_err(),
            "builder consumer must reject a missing merge command",
        )?;

        let mut non_null = builder;
        replace_object_field(
            &mut non_null,
            "merge_command",
            Value::String("git merge forbidden".to_string()),
            "trusted builder fixture",
        )?;
        require(
            validate_builder_receipt(&non_null, &validation, &fixture.options).is_err(),
            "builder consumer must reject a non-null merge command",
        )?;
        fixture.repo.cleanup()
    }

    #[test]
    fn builder_rejections_emit_null_merge_command_and_zero_attempts() -> Result<(), String> {
        let root = test_temp_dir("builder-rejection-shape")?;
        let parse_out = root.join("parse-rejection");
        let parse_args = vec![
            SOURCE_PROMOTION_TRUSTED_BUILDER_SUBCOMMAND.to_string(),
            "--out".to_string(),
            parse_out.to_string_lossy().into_owned(),
        ];
        require(
            write_trusted_builder_receipt(&parse_args).is_err(),
            "malformed builder invocation must reject",
        )?;
        require_builder_rejection_without_merge_authority(&parse_out)?;

        let repo = current_repo()?;
        let source_parent = current_head(&repo)?;
        let live_out = root.join("live-validation-rejection");
        let live_args = vec![
            SOURCE_PROMOTION_TRUSTED_BUILDER_SUBCOMMAND.to_string(),
            "--source-parent".to_string(),
            source_parent.clone(),
            "--workflow-source-sha".to_string(),
            source_parent,
            "--executable".to_string(),
            "missing-builder-executable".to_string(),
            "--cargo-target-dir".to_string(),
            root.join("cargo-target").to_string_lossy().into_owned(),
            "--out".to_string(),
            live_out.to_string_lossy().into_owned(),
        ];
        require(
            write_trusted_builder_receipt(&live_args).is_err(),
            "parsed builder invocation must reject during live validation",
        )?;
        require_builder_rejection_without_merge_authority(&live_out)?;
        root.cleanup()
    }

    #[test]
    fn integration_index_rejects_well_shaped_unbound_bytes_before_attempts()
    -> Result<(), String> {
        let directory = test_temp_dir("integration-index-bound-digest")?;
        let identity = test_identity();
        let index_path = directory.join("integration-index.json");
        let hand_authored = serde_json::json!({
            "schema": INTEGRATION_INDEX_SCHEMA,
            "status": "complete",
            "source_parent": identity.source_parent.as_str(),
            "swarm_parent": identity.swarm_parent.as_str(),
            "join_tree": identity.join_tree.as_str(),
            "preflight_sha256": identity.preflight_sha256.as_str(),
            "resolution_manifest_sha256": identity.resolution_sha256.as_str(),
            "required_kinds": REQUIRED_INTEGRATION_KINDS,
            "receipts": [
                {
                    "kind": "command_catalog_integration",
                    "path": "command-catalog.json",
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                },
                {
                    "kind": "network_policy_integration",
                    "path": "network-policy.json",
                    "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                },
            ],
            "failure_reasons": [],
        });
        let bytes = serde_json::to_vec_pretty(&hand_authored)
            .map_err(|error| format!("failed to encode integration index fixture: {error}"))?;
        fs::write(&index_path, &bytes)
            .map_err(|error| format!("failed to write integration index fixture: {error}"))?;
        let actual_digest = digest_bytes(&bytes);
        let bound_digest =
            "0000000000000000000000000000000000000000000000000000000000000000";
        require(
            actual_digest != bound_digest,
            "hand-authored integration index must differ from its caller-bound digest",
        )?;

        let reason = validate_integration_index(
            &index_path,
            bound_digest,
            &identity,
            "8888888888888888888888888888888888888888888888888888888888888888",
        )
        .err()
        .ok_or_else(|| "non-bound integration index unexpectedly admitted".to_string())?;
        require(
            reason.contains("integration receipt index SHA-256 mismatch"),
            "non-bound integration index must reject at the caller-bound digest check",
        )?;
        let report = admission_rejection_report(Some(&identity), &reason);
        for counter in [
            "commit_tree_attempts",
            "local_ref_attempts",
            "remote_push_attempts",
            "merge_command_attempts",
        ] {
            require_equal(
                report.get(counter).and_then(Value::as_u64),
                Some(0),
                "digest mismatch rejection attempt counter",
            )?;
        }
        directory.cleanup()
    }

    #[test]
    fn qualification_rejects_zero_step_failed_and_reordered_lanes() -> Result<(), String> {
        let identity = test_identity();
        let admission = valid_admission_receipt(&identity);
        let integration = integration_evidence_from_admission(&admission)?;
        let packet = IndexedPacket {
            index_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            files: BTreeMap::new(),
        };
        let admission_receipt_sha256 =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let qualification_sha256 =
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        let network_policy_receipt_sha256 = integration
            .receipt_digests
            .get("network_policy_integration")
            .ok_or_else(|| "integration fixture is missing network-policy digest".to_string())?;
        let base = valid_qualification_receipt_for(
            &identity,
            &admission,
            &packet,
            admission_receipt_sha256,
            network_policy_receipt_sha256,
        )?;
        require(
            validate_qualification_receipt(
                &base,
                &identity,
                &admission,
                &packet,
                admission_receipt_sha256,
                &integration,
                qualification_sha256,
            )
            .is_ok(),
            "valid qualification receipt should pass",
        )?;

        let mut zero = base.clone();
        replace_object_field(
            &mut zero,
            "lanes",
            Value::Array(Vec::new()),
            "qualification receipt fixture",
        )?;
        require(
            validate_qualification_receipt(
                &zero,
                &identity,
                &admission,
                &packet,
                admission_receipt_sha256,
                &integration,
                qualification_sha256,
            )
            .is_err(),
            "qualification receipt with zero lanes should reject",
        )?;

        let mut failed = base.clone();
        let lane = required_array_field_mut(
            &mut failed,
            "lanes",
            "qualification receipt fixture",
        )?
            .first_mut()
            .ok_or_else(|| "qualification lanes fixture is empty".to_string())?;
        replace_object_field(
            lane,
            "state",
            Value::String("failed".to_string()),
            "qualification lane fixture",
        )?;
        require(
            validate_qualification_receipt(
                &failed,
                &identity,
                &admission,
                &packet,
                admission_receipt_sha256,
                &integration,
                qualification_sha256,
            )
            .is_err(),
            "qualification receipt with a failed lane should reject",
        )?;

        let root = test_temp_dir("qualification-substitution")?;
        let qualification_path = root.join("tree-qualification.json");
        let qualification_bytes = serde_json::to_vec_pretty(&base)
            .map_err(|error| format!("failed to serialize qualification fixture: {error}"))?;
        fs::write(&qualification_path, qualification_bytes)
            .map_err(|error| format!("failed to write qualification fixture: {error}"))?;
        let wrong_expected_sha256 = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
        let digest_failure = read_bound_json(
            &qualification_path,
            wrong_expected_sha256,
            "substituted tree qualification receipt",
        )
        .err()
        .ok_or_else(|| {
            "well-shaped substituted qualification receipt unexpectedly matched caller-bound digest"
                .to_string()
        })?;
        let rejection = construction_rejection_report(
            Some(&identity),
            Some("refs/heads/promote/0.11.0-test"),
            &digest_failure,
            false,
        );
        require_equal(
            rejection.get("commit_tree_attempts").and_then(Value::as_u64),
            Some(0),
            "substituted qualification receipt commit-tree attempts",
        )?;
        root.cleanup()?;

        let mut reordered = base;
        let lanes = required_array_field_mut(
            &mut reordered,
            "lanes",
            "qualification receipt fixture",
        )?;
        require(
            lanes.len() >= 2,
            "qualification lanes fixture requires two entries for reorder",
        )?;
        lanes.swap(0, 1);
        require(
            validate_qualification_receipt(
                &reordered,
                &identity,
                &admission,
                &packet,
                admission_receipt_sha256,
                &integration,
                qualification_sha256,
            )
            .is_err(),
            "qualification receipt with reordered lanes should reject",
        )?;
        Ok(())
    }

    #[test]
    fn construction_rejects_forged_admission_integration_digest_before_commit_tree()
    -> Result<(), String> {
        let fixture = admission_snapshot_fixture("construction-forged-integration")?;
        let identity = fixture.options.identity.clone();
        let root = fixture.repo.join(".git/forged-construction-evidence");
        let admission_root = root.join("admission-packet");
        let mut admission = admission_success_report(&fixture.evidence);
        let forged_network_digest =
            "0000000000000000000000000000000000000000000000000000000000000000";
        let actual_network_digest = fixture
            .integration
            .receipt_digests
            .get("network_policy_integration")
            .ok_or_else(|| "validated integration evidence is missing network policy".to_string())?;
        require(
            actual_network_digest != forged_network_digest,
            "forged network-policy digest must differ from validated integration evidence",
        )?;
        let admission_receipts = admission
            .get_mut("integration_receipts")
            .ok_or_else(|| "admission success fixture is missing integration receipts".to_string())?;
        replace_object_field(
            admission_receipts,
            "network_policy_integration",
            Value::String(forged_network_digest.to_string()),
            "forged admission integration receipts",
        )?;
        let admission_packet = write_test_packet(
            &admission_root,
            CONTROL_PACKET_SCHEMA,
            "resolved_tree_admission",
            "admitted",
            ADMISSION_REPORT,
            &admission,
        )?;
        let admission_receipt_sha256 =
            packet_file_sha256(&admission_packet, ADMISSION_REPORT)?;
        let qualification = valid_qualification_receipt_for(
            &identity,
            &admission,
            &admission_packet,
            &admission_receipt_sha256,
            forged_network_digest,
        )?;
        let qualification_path = root.join("tree-qualification.json");
        let qualification_bytes = write_test_json(
            &qualification_path,
            &qualification,
            "forged-integration qualification receipt",
        )?;
        let qualification_sha256 = digest_bytes(&qualification_bytes);
        let options = ConstructionOptions {
            repo: fixture.repo.to_path_buf(),
            admission_packet: admission_root,
            validation_packet: fixture.options.validation_packet.clone(),
            integration_index: fixture.options.integration_index.clone(),
            integration_index_sha256: fixture.options.integration_index_sha256.clone(),
            preflight: fixture.options.preflight.clone(),
            resolution_manifest: fixture.options.resolution_manifest.clone(),
            qualification_receipt: qualification_path,
            qualification_receipt_sha256: qualification_sha256,
            source_main_ref: SOURCE_MAIN_REF.to_string(),
            swarm_ref: fixture.swarm_ref.clone(),
            candidate_ref: "refs/heads/promote/0.11.0-forged-integration".to_string(),
            out: root.join("unused-output"),
        };
        let refs_before = refs_digest(&fixture.repo)?;
        let failure = construct_exact_join_inner(&options, None)
            .err()
            .ok_or_else(|| "forged admission integration digest unexpectedly constructed".to_string())?;
        require(
            failure.0.contains("integration") && failure.0.contains("receipt"),
            "construction must reject at the actual integration-receipt binding",
        )?;
        require(
            !failure.2,
            "forged integration receipt must reject before commit-tree",
        )?;
        require_equal(
            refs_digest(&fixture.repo)?,
            refs_before,
            "forged integration rejection must preserve refs",
        )?;
        let report = construction_rejection_report(
            failure.1.as_deref(),
            Some(&options.candidate_ref),
            &failure.0,
            failure.2,
        );
        require_equal(
            report.get("commit_tree_attempts").and_then(Value::as_u64),
            Some(0),
            "forged integration rejection commit-tree attempts",
        )?;
        fixture.repo.cleanup()
    }

    #[test]
    fn qualification_lane_documentation_matches_production_order() -> Result<(), String> {
        // Intentional independent mirror of the normative denominator in
        // RIPR-SPEC-0150; do not derive this oracle from the production constant.
        const NORMATIVE_QUALIFICATION_LANES: &[&str] = &[
            "editor_package_linux",
            "editor_package_windows",
            "rust_product",
            "source_governance",
            "source_survivors",
            "trusted_product_journeys",
            "untrusted_workspace_contract",
            "w7_product",
        ];
        require_equal(
            REQUIRED_QUALIFICATION_LANES,
            NORMATIVE_QUALIFICATION_LANES,
            "production qualification-lane denominator",
        )?;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())?;
        let schema = fs::read_to_string(root.join("docs/OUTPUT_SCHEMA.md"))
            .map_err(|error| format!("read docs/OUTPUT_SCHEMA.md: {error}"))?
            .replace("\r\n", "\n");
        let marker = "Qualification requires exactly these ordered lane names:\n\n";
        let lane_block = schema
            .split_once(marker)
            .map(|(_, remainder)| remainder)
            .and_then(|remainder| remainder.split_once("\n\n").map(|(block, _)| block))
            .ok_or_else(|| "output schema qualification-lane block is missing".to_string())?;
        let expected_block = NORMATIVE_QUALIFICATION_LANES
            .iter()
            .enumerate()
            .map(|(index, lane)| {
                let punctuation = if index + 1 == NORMATIVE_QUALIFICATION_LANES.len() {
                    '.'
                } else {
                    ';'
                };
                format!("- `{lane}`{punctuation}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        require_equal(
            lane_block,
            expected_block.as_str(),
            "output schema qualification-lane block",
        )?;

        Ok(())
    }

    #[test]
    fn output_schema_assigns_evidence_to_the_correct_control_stage() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())?;
        let schema = fs::read_to_string(root.join("docs/OUTPUT_SCHEMA.md"))
            .map_err(|error| format!("read docs/OUTPUT_SCHEMA.md: {error}"))?
            .replace("\r\n", "\n");
        const ADMISSION_OWNERSHIP: &str =
            "Admission consumes the\nproducer-bound `ripr.source_promotion_integration_index.v1` schema";
        const CONSTRUCTION_OWNERSHIP: &str =
            "construction\nconsumes the terminal `ripr.source_promotion_tree_qualification.v1` schema";
        require(
            schema.contains(ADMISSION_OWNERSHIP),
            "OUTPUT_SCHEMA must assign the producer-bound integration index to admission",
        )?;
        require(
            schema.contains(CONSTRUCTION_OWNERSHIP),
            "OUTPUT_SCHEMA must assign terminal qualification to construction",
        )
    }

    #[test]
    fn publication_status_definitions_match_every_contract() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())?;
        for (contract_path, start, end, expected_definition) in [
            (
                "docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md",
                "- `published` means",
                "- `published_but_invalidated`",
                "- `published` means the guarded push's machine-readable status reported an actual update of the exact target ref, the remote target was reread at the exact constructed join, and all post-push authority rereads remained valid;",
            ),
            (
                "docs/SOURCE_PROMOTION.md",
                "- `published` means",
                "- `published_but_invalidated`",
                "- `published` means the guarded push's machine-readable status reported an actual update of the exact target ref, the remote candidate ref was observed at the exact join, and every post-push authority reread remained valid;",
            ),
            (
                "docs/OUTPUT_SCHEMA.md",
                "`published` means",
                "Publication additionally uses",
                "`published` means machine-readable guarded-push status reported an actual update of the exact target ref, the exact join was observed there, and every bound post-push authority remained current.",
            ),
            (
                "policy/output_contracts.txt",
                "source_promotion_control_status|published|",
                "\n",
                "source_promotion_control_status|published|Machine-readable guarded-push status reported an actual update of the exact target ref, the exact candidate ref was observed at the constructed join, and all bound post-push authorities remained current.",
            ),
        ] {
            let normalized_definition =
                normalized_contract_definition(&root, contract_path, start, end)?;
            require_equal(
                normalized_definition.as_str(),
                expected_definition,
                &format!("{contract_path} published definition"),
            )?;
        }
        for (contract_path, start, end, expected_definition) in [
            (
                "docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md",
                "- `published_but_invalidated` means",
                "- `publication_state_unknown`",
                "- `published_but_invalidated` means the remote target was observed at the exact join but a bound source, W7, packet, object, or URL authority invalidated during publication, or the post-push local candidate-ref observation was unavailable; and",
            ),
            (
                "docs/SOURCE_PROMOTION.md",
                "- `published_but_invalidated` means",
                "- `publication_state_unknown`",
                "- `published_but_invalidated` means the exact join reached the remote candidate ref but a bound source, W7, packet, object, or URL identity invalidated during publication, or the post-push local candidate-ref observation was unavailable;",
            ),
            (
                "docs/OUTPUT_SCHEMA.md",
                "`published_but_invalidated` when",
                "Neither status",
                "`published_but_invalidated` when the exact remote candidate ref moved but a bound input invalidated afterward or the post-push local candidate-ref observation was unavailable, and `publication_state_unknown` when a push was attempted but its final remote state could not be observed or the exact join was observed without a machine-readable actual target-update attribution. An exit-zero up-to-date/no-op push is not publication attribution.",
            ),
            (
                "policy/output_contracts.txt",
                "source_promotion_control_status|published_but_invalidated|",
                "\n",
                "source_promotion_control_status|published_but_invalidated|The remote candidate ref reached the constructed join but a bound input invalidated during publication or the post-push local candidate-ref observation was unavailable.",
            ),
        ] {
            let normalized_definition =
                normalized_contract_definition(&root, contract_path, start, end)?;
            require_equal(
                normalized_definition.as_str(),
                expected_definition,
                &format!("{contract_path} published_but_invalidated definition"),
            )?;
        }
        for (contract_path, start, end, expected_definition) in [
            (
                "docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md",
                "- `publication_state_unknown` means",
                "\n\n`rejected`",
                "- `publication_state_unknown` means the final remote state could not be observed, or it equals the join without an actual target-update attribution; an exit-zero up-to-date/no-op push is not publication attribution.",
            ),
            (
                "docs/SOURCE_PROMOTION.md",
                "- `publication_state_unknown` means",
                "- `rejected`",
                "- `publication_state_unknown` means the final remote state could not be read, or it equals the join without an actual target-update attribution; an exit-zero up-to-date/no-op push is not publication attribution; and",
            ),
            (
                "docs/OUTPUT_SCHEMA.md",
                "`publication_state_unknown` when",
                "Neither status",
                "`publication_state_unknown` when a push was attempted but its final remote state could not be observed or the exact join was observed without a machine-readable actual target-update attribution. An exit-zero up-to-date/no-op push is not publication attribution.",
            ),
            (
                "policy/output_contracts.txt",
                "source_promotion_control_status|publication_state_unknown|",
                "\n",
                "source_promotion_control_status|publication_state_unknown|Final remote state was unavailable or equaled the join without machine-readable actual target-update attribution; an exit-zero up-to-date/no-op push is not publication attribution.",
            ),
        ] {
            let normalized_definition =
                normalized_contract_definition(&root, contract_path, start, end)?;
            require_equal(
                normalized_definition.as_str(),
                expected_definition,
                &format!("{contract_path} publication_state_unknown definition"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn control_packet_rejects_partial_digest_mismatch_and_unindexed_files() -> Result<(), String> {
        let temp = test_temp_dir("packet")?;
        let root = temp.join("packet");
        let report = serde_json::json!({
            "schema": ADMISSION_SCHEMA,
            "status": "admitted",
            "failure_reasons": [],
        });
        write_control_packet(
            &root,
            "resolved_tree_admission",
            ADMISSION_REPORT,
            &report,
            "test",
            "test",
        )?;
        let packet = read_indexed_packet(
            &root,
            CONTROL_PACKET_SCHEMA,
            Some("resolved_tree_admission"),
            Some("admitted"),
            ADMISSION_REPORT,
        )?;
        let unchanged_index = fs::read(root.join(PACKET_INDEX))
            .map_err(|error| format!("failed to read packet-index race fixture: {error}"))?;
        require_equal(
            packet_file_sha256(&packet, ADMISSION_REPORT)?.len(),
            64,
            "packet digest width",
        )?;

        fs::write(root.join("unexpected.txt"), "unexpected\n")
            .map_err(|error| format!("failed to write unexpected packet file: {error}"))?;
        require(
            read_indexed_packet(
                &root,
                CONTROL_PACKET_SCHEMA,
                Some("resolved_tree_admission"),
                Some("admitted"),
                ADMISSION_REPORT,
            )
            .is_err(),
            "packet with an unindexed file should reject",
        )?;
        fs::remove_file(root.join("unexpected.txt"))
            .map_err(|error| format!("failed to remove unexpected packet file: {error}"))?;

        fs::write(root.join(ADMISSION_REPORT), "{}\n")
            .map_err(|error| format!("failed to corrupt packet report: {error}"))?;
        require_equal(
            fs::read(root.join(PACKET_INDEX))
                .map_err(|error| format!("failed to reread packet-index race fixture: {error}"))?,
            unchanged_index,
            "member-only race leaves packet-index bytes unchanged",
        )?;
        require(
            read_indexed_packet(
                &root,
                CONTROL_PACKET_SCHEMA,
                Some("resolved_tree_admission"),
                Some("admitted"),
                ADMISSION_REPORT,
            )
            .is_err(),
            "final full-packet reread must reject a changed member behind an unchanged index",
        )?;
        temp.cleanup()
    }

    #[test]
    fn construction_snapshot_rejects_changed_packet_member_behind_unchanged_index()
    -> Result<(), String> {
        let fixture = construction_snapshot_fixture("snapshot-packet-member-race")?;
        let refs_before = refs_digest(&fixture.repo)?;
        let index_before = fs::read(fixture.options.validation_packet.join(PACKET_INDEX))
            .map_err(|error| format!("failed to read validation packet index: {error}"))?;
        let report_path = fixture.options.validation_packet.join(VALIDATION_REPORT);
        let mut changed_report = fs::read(&report_path)
            .map_err(|error| format!("failed to read validation packet member: {error}"))?;
        let final_byte = changed_report
            .last_mut()
            .ok_or_else(|| "validation packet member is unexpectedly empty".to_string())?;
        *final_byte = b' ';
        fs::write(&report_path, changed_report)
            .map_err(|error| format!("failed to mutate validation packet member: {error}"))?;
        require_equal(
            fs::read(fixture.options.validation_packet.join(PACKET_INDEX))
                .map_err(|error| format!("failed to reread validation packet index: {error}"))?,
            index_before,
            "validation packet index bytes remain unchanged",
        )?;

        let reason = construction_snapshot(
            &fixture.options,
            &fixture.identity,
            &fixture.admission_packet,
            &fixture.validation_packet,
            &fixture.integration,
            &fixture.qualification_sha256,
        )
        .err()
        .ok_or_else(|| "changed validation packet member unexpectedly passed snapshot".to_string())?;
        require(
            reason.contains("packet digest mismatch"),
            "production snapshot must reject the changed indexed member",
        )?;
        require_snapshot_rejection_without_authority(&fixture, &reason, &refs_before)?;
        fixture.repo.cleanup()
    }

    #[test]
    fn admission_snapshot_rejects_changed_packet_member_behind_unchanged_index()
    -> Result<(), String> {
        let fixture = admission_snapshot_fixture("admission-snapshot-packet-member-race")?;
        let refs_before = refs_digest(&fixture.repo)?;
        let index_before = fs::read(fixture.options.validation_packet.join(PACKET_INDEX))
            .map_err(|error| format!("failed to read admission validation index: {error}"))?;
        let report_path = fixture.options.validation_packet.join(VALIDATION_REPORT);
        let mut changed_report = fs::read(&report_path)
            .map_err(|error| format!("failed to read admission validation member: {error}"))?;
        let final_byte = changed_report
            .last_mut()
            .ok_or_else(|| "admission validation member is unexpectedly empty".to_string())?;
        *final_byte = b' ';
        fs::write(&report_path, changed_report)
            .map_err(|error| format!("failed to mutate admission validation member: {error}"))?;
        require_equal(
            fs::read(fixture.options.validation_packet.join(PACKET_INDEX)).map_err(|error| {
                format!("failed to reread admission validation index: {error}")
            })?,
            index_before,
            "admission validation index bytes remain unchanged",
        )?;

        let reason = admission_snapshot(
            &fixture.options,
            &fixture.swarm_ref,
            &fixture.validation_packet,
            &fixture.builder_packet,
            &fixture.integration,
            &fixture.executable_sha256,
        )
        .err()
        .ok_or_else(|| "changed admission packet member unexpectedly passed snapshot".to_string())?;
        require(
            reason.contains("packet digest mismatch"),
            "admission snapshot must reject the changed indexed member",
        )?;
        require_admission_rejection_without_authority(&fixture, &reason, &refs_before)?;
        fixture.repo.cleanup()
    }

    #[test]
    fn admission_snapshot_rejects_changed_typed_receipt_behind_unchanged_index()
    -> Result<(), String> {
        let fixture = admission_snapshot_fixture("admission-snapshot-integration-race")?;
        let refs_before = refs_digest(&fixture.repo)?;
        let index_before = fs::read(&fixture.options.integration_index)
            .map_err(|error| format!("failed to read admission integration index: {error}"))?;
        let integration_root = fixture
            .options
            .integration_index
            .parent()
            .ok_or_else(|| "admission integration index has no parent".to_string())?;
        let receipt_path = integration_root.join("command-catalog.json");
        let mut changed_receipt = fs::read(&receipt_path)
            .map_err(|error| format!("failed to read admission integration receipt: {error}"))?;
        changed_receipt.push(b' ');
        fs::write(&receipt_path, changed_receipt)
            .map_err(|error| format!("failed to mutate admission integration receipt: {error}"))?;
        require_equal(
            fs::read(&fixture.options.integration_index).map_err(|error| {
                format!("failed to reread admission integration index: {error}")
            })?,
            index_before,
            "admission integration index bytes remain unchanged",
        )?;

        let reason = admission_snapshot(
            &fixture.options,
            &fixture.swarm_ref,
            &fixture.validation_packet,
            &fixture.builder_packet,
            &fixture.integration,
            &fixture.executable_sha256,
        )
        .err()
        .ok_or_else(|| {
            "changed admission typed integration receipt unexpectedly passed snapshot".to_string()
        })?;
        require(
            reason.contains("integration receipt digest mismatch"),
            "admission snapshot must reject the changed typed integration receipt",
        )?;
        require_admission_rejection_without_authority(&fixture, &reason, &refs_before)?;
        fixture.repo.cleanup()
    }

    #[test]
    fn admission_rejects_stable_wrong_protected_w7_before_attempts() -> Result<(), String> {
        let fixture = admission_snapshot_fixture("admission-stable-wrong-w7")?;
        git_test(
            &fixture.repo,
            &[
                "update-ref",
                &fixture.swarm_ref,
                fixture.options.identity.source_parent.as_str(),
            ],
        )?;
        let refs_before = refs_digest(&fixture.repo)?;
        let reason = validate_admission(&fixture.options)
            .err()
            .ok_or_else(|| "stable wrong protected W7 unexpectedly earned admission".to_string())?;
        require(
            reason.contains("protected W7 ref does not equal admitted SWARM_PARENT"),
            "admission must compare the protected W7 value with the requested identity",
        )?;
        require_admission_rejection_without_authority(&fixture, &reason, &refs_before)?;
        fixture.repo.cleanup()
    }

    #[test]
    fn construction_snapshot_rejects_changed_typed_receipt_behind_unchanged_index()
    -> Result<(), String> {
        let fixture = construction_snapshot_fixture("snapshot-integration-receipt-race")?;
        let refs_before = refs_digest(&fixture.repo)?;
        let index_before = fs::read(&fixture.options.integration_index)
            .map_err(|error| format!("failed to read integration index: {error}"))?;
        let integration_root = fixture
            .options
            .integration_index
            .parent()
            .ok_or_else(|| "integration index fixture has no parent".to_string())?;
        let receipt_path = integration_root.join("command-catalog.json");
        let mut changed_receipt = fs::read(&receipt_path)
            .map_err(|error| format!("failed to read typed integration receipt: {error}"))?;
        changed_receipt.push(b' ');
        fs::write(&receipt_path, changed_receipt)
            .map_err(|error| format!("failed to mutate typed integration receipt: {error}"))?;
        require_equal(
            fs::read(&fixture.options.integration_index)
                .map_err(|error| format!("failed to reread integration index: {error}"))?,
            index_before,
            "integration index bytes remain unchanged",
        )?;

        let reason = construction_snapshot(
            &fixture.options,
            &fixture.identity,
            &fixture.admission_packet,
            &fixture.validation_packet,
            &fixture.integration,
            &fixture.qualification_sha256,
        )
        .err()
        .ok_or_else(|| "changed typed integration receipt unexpectedly passed snapshot".to_string())?;
        require(
            reason.contains("integration receipt digest mismatch"),
            "production snapshot must reject the changed typed integration receipt",
        )?;
        require_snapshot_rejection_without_authority(&fixture, &reason, &refs_before)?;
        fixture.repo.cleanup()
    }

    #[test]
    fn candidate_ref_and_remote_row_validation_fail_closed() -> Result<(), String> {
        require(
            validate_candidate_ref("refs/heads/promote/0.11.0-w7").is_ok(),
            "valid candidate ref should pass",
        )?;
        for reference in [
            "main",
            "refs/heads/main",
            "refs/heads/promote/0.12.0-w7",
            "refs/heads/promote/0.11.0-../escape",
            "refs/heads/promote/0.11.0-w7 bad",
            "refs/heads/promote/0.11.0-w7\u{0007}",
            "refs/heads/promote/0.11.0-w7@{1}",
            "refs/heads/promote/0.11.0-w7/.hidden",
            "refs/heads/promote/0.11.0-w7.",
            "refs/heads/promote/0.11.0-w7.lock",
            "refs/heads/promote/0.11.0-w7.lock/child",
        ] {
            require(
                validate_candidate_ref(reference).is_err(),
                format!("unsafe candidate ref should reject: {reference}"),
            )?;
        }
        require_equal(
            parse_remote_ref("", "refs/heads/promote/0.11.0-w7")
                .ok()
                .flatten(),
            None,
            "empty remote response",
        )?;
        require(
            parse_remote_ref(
                "1111111111111111111111111111111111111111 refs/heads/other\n",
                "refs/heads/promote/0.11.0-w7",
            )
            .is_err(),
            "remote row naming a different ref should reject",
        )?;
        require_equal(
            parse_guarded_push_porcelain(
                "=\t1111111111111111111111111111111111111111:refs/heads/promote/0.11.0-w7\t[up to date]\n",
                "refs/heads/promote/0.11.0-w7",
            )?,
            false,
            "up-to-date guarded push is not an attributable update",
        )?;
        require_equal(
            parse_guarded_push_porcelain(
                "*\t1111111111111111111111111111111111111111:refs/heads/promote/0.11.0-w7\t[new reference]\n",
                "refs/heads/promote/0.11.0-w7",
            )?,
            true,
            "new-reference guarded push is an attributable update",
        )?;
        require(
            parse_guarded_push_porcelain(
                "*\t1111111111111111111111111111111111111111:refs/heads/promote/0.11.0-other\t[new reference]\n",
                "refs/heads/promote/0.11.0-w7",
            )
            .is_err(),
            "guarded push naming another target should reject",
        )?;
        require(
            parse_guarded_push_porcelain(
                "* malformed status\n",
                "refs/heads/promote/0.11.0-w7",
            )
            .is_err(),
            "malformed guarded-push porcelain should reject",
        )?;
        require(
            parse_guarded_push_porcelain(
                "unexpected output\n*\t1111111111111111111111111111111111111111:refs/heads/promote/0.11.0-w7\t[new reference]\n",
                "refs/heads/promote/0.11.0-w7",
            )
            .is_err(),
            "unknown guarded-push output must not be ignored beside a valid row",
        )?;
        Ok(())
    }

    #[test]
    fn local_candidate_ref_io_rejects_symbolic_and_broken_refs() -> Result<(), String> {
        let (repo, identity) = init_synthetic_repo("candidate-ref-kind")?;
        let candidate_ref = "refs/heads/promote/0.11.0-ref-kind";
        let source_before = current_head(&repo)?;
        git_test(&repo, &["symbolic-ref", candidate_ref, SOURCE_MAIN_REF])?;
        require(
            read_optional_local_ref(&repo, candidate_ref).is_err(),
            "symbolic candidate ref should reject",
        )?;
        update_local_ref(
            &repo,
            candidate_ref,
            Some(identity.swarm_parent.as_str()),
            Some(source_before.as_str()),
        )?;
        require_equal(
            current_head(&repo)?,
            source_before,
            "no-deref candidate update preserves the symbolic referent",
        )?;
        require_equal(
            read_optional_local_ref(&repo, candidate_ref)?,
            Some(identity.swarm_parent.clone()),
            "no-deref update replaces only the candidate ref itself",
        )?;
        update_local_ref(
            &repo,
            candidate_ref,
            None,
            Some(identity.swarm_parent.as_str()),
        )?;

        let broken_ref = "refs/heads/promote/0.11.0-broken";
        let broken_path = repo.join(".git").join(broken_ref);
        let broken_parent = broken_path
            .parent()
            .ok_or_else(|| "broken-ref fixture has no parent".to_string())?;
        fs::create_dir_all(broken_parent)
            .map_err(|error| format!("failed to create broken-ref fixture parent: {error}"))?;
        fs::write(&broken_path, "not-an-object-id\n")
            .map_err(|error| format!("failed to write broken-ref fixture: {error}"))?;
        require(
            read_optional_local_ref(&repo, broken_ref).is_err(),
            "broken direct candidate ref should reject instead of reading as absent",
        )?;
        repo.cleanup()
    }

    #[test]
    fn rejected_and_prepublication_receipts_never_emit_merge_command() -> Result<(), String> {
        let identity = test_identity();
        let construction = construction_rejection_report(
            Some(&identity),
            Some("refs/heads/promote/0.11.0-test"),
            "rejected",
            false,
        );
        let publication =
            publication_rejection_report(None, None, "rejected", &PublicationState::default());
        let admitted = valid_admission_receipt(&identity);
        require(
            construction
                .get("merge_command")
                .is_some_and(Value::is_null),
            "rejected construction should not emit a merge command",
        )?;
        require(
            publication.get("merge_command").is_some_and(Value::is_null),
            "rejected publication should not emit a merge command",
        )?;
        require(
            admitted.get("merge_command").is_some_and(Value::is_null),
            "admission should not emit a merge command",
        )?;
        Ok(())
    }

    #[test]
    fn source_promotion_constructor_preserves_parent_order_tree_and_refs() -> Result<(), String> {
        let (repo, identity) = init_synthetic_repo("construct")?;
        let refs_before = refs_digest(&repo)?;
        let join = create_exact_join_object(&repo, &identity)?;
        let repeated = create_exact_join_object(&repo, &identity)?;
        require_equal(
            join.clone(),
            repeated,
            "same admitted inputs must produce the same exact-J object",
        )?;
        verify_constructed_join(&repo, &join, &identity)?;
        require_equal(
            refs_digest(&repo)?,
            refs_before,
            "exact-J construction must preserve refs",
        )?;

        let mut reversed = identity.clone();
        std::mem::swap(&mut reversed.source_parent, &mut reversed.swarm_parent);
        require(
            verify_constructed_join(&repo, &join, &reversed).is_err(),
            "reversed exact-J parents should reject",
        )?;

        let mut wrong_tree = identity;
        wrong_tree.join_tree = commit_tree(&repo, &wrong_tree.swarm_parent)?;
        require(
            verify_constructed_join(&repo, &join, &wrong_tree).is_err(),
            "wrong exact-J tree should reject",
        )?;
        repo.cleanup()
    }

    #[test]
    fn source_promotion_constructor_rejects_mismatched_journal_before_commit_tree()
    -> Result<(), String> {
        let (repo, identity) = init_synthetic_repo("construct-journal")?;
        let refs_before = refs_digest(&repo)?;
        let admission_bytes = b"test admission receipt".to_vec();
        let admission_receipt_sha256 = digest_bytes(&admission_bytes);
        let mut admission_files = BTreeMap::new();
        admission_files.insert(
            ADMISSION_REPORT.to_string(),
            IndexedFile {
                sha256: admission_receipt_sha256.clone(),
                contents: admission_bytes,
            },
        );
        let admission_packet = IndexedPacket {
            index_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            files: admission_files,
        };
        let validation_packet = IndexedPacket {
            index_sha256:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            files: BTreeMap::new(),
        };
        let integration_index_sha256 =
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string();
        let qualification_sha256 =
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                .to_string();
        let options = ConstructionOptions {
            repo: repo.to_path_buf(),
            admission_packet: repo.join("unused-admission"),
            validation_packet: repo.join("unused-validation"),
            integration_index: repo.join("unused-integration"),
            integration_index_sha256: integration_index_sha256.clone(),
            preflight: repo.join("unused-preflight"),
            resolution_manifest: repo.join("unused-resolution"),
            qualification_receipt: repo.join("unused-qualification"),
            qualification_receipt_sha256: qualification_sha256.clone(),
            source_main_ref: SOURCE_MAIN_REF.to_string(),
            swarm_ref: format!(
                "refs/tags/ripr-release-0.11.0-{}",
                identity.swarm_parent.as_str()
            ),
            candidate_ref: "refs/heads/promote/0.11.0-test".to_string(),
            out: repo.join("unused-output"),
        };
        let commit_timestamp = canonical_join_timestamp(&repo, &identity)?;
        let mut mismatched_context = construction_reconciliation_value(
            &options,
            &identity,
            &admission_packet,
            &validation_packet,
            &integration_index_sha256,
            &qualification_sha256,
            &commit_timestamp,
        )?;
        let join_tree = mismatched_context
            .as_object_mut()
            .and_then(|object| object.get_mut("join_tree"))
            .ok_or_else(|| "construction context is missing join_tree".to_string())?;
        *join_tree = Value::String(identity.source_parent.clone());

        let failure = construct_validated_join(
            &options,
            ValidatedConstructionInputs {
                identity: Box::new(identity),
                admission_packet,
                admission_receipt_sha256,
                validation_packet,
                integration_index_sha256,
                qualification_sha256,
            },
            Some(&mismatched_context),
        )
        .err()
        .ok_or_else(|| "mismatched construction journal unexpectedly constructed".to_string())?;
        require(
            !failure.2,
            "mismatched construction journal must report zero commit-tree attempts",
        )?;
        require_equal(
            refs_digest(&repo)?,
            refs_before,
            "mismatched construction journal must preserve every ref",
        )?;
        repo.cleanup()
    }

    #[test]
    fn source_promotion_publication_uses_expected_absent_guard_without_merge_authority()
    -> Result<(), String> {
        let (repo, identity) = init_synthetic_repo("publish")?;
        let join = create_exact_join_object(&repo, &identity)?;
        verify_constructed_join(&repo, &join, &identity)?;
        let mut evidence = valid_construction_evidence(identity.clone());
        evidence.join_commit = join.clone();
        evidence.commit_timestamp = canonical_join_timestamp(&repo, &identity)?;
        evidence.swarm_ref = format!(
            "refs/tags/ripr-release-0.11.0-{}",
            identity.swarm_parent.as_str()
        );
        let packet_root = repo.join("control-construction");
        let report = construction_success_report(&evidence);
        write_control_packet(
            &packet_root,
            "exact_join_construction",
            CONSTRUCTION_REPORT,
            &report,
            "test construction",
            "test",
        )?;

        let remote_root = test_temp_dir("remote")?;
        let remote = remote_root.join("remote.git");
        let remote_text = remote.to_string_lossy().into_owned();
        git_test(&repo, &["init", "--bare", &remote_text])?;
        git_test(&repo, &["remote", "add", "origin", &remote_text])?;
        git_test(
            &repo,
            &[
                "push",
                "origin",
                SOURCE_MAIN_REF,
                evidence.swarm_ref.as_str(),
            ],
        )?;
        let out = repo.join("publication-output");
        let options = PublicationOptions {
            repo: repo.to_path_buf(),
            construction_packet: packet_root,
            source_main_ref: SOURCE_MAIN_REF.to_string(),
            remote: "origin".to_string(),
            source_remote_url: remote_text.clone(),
            swarm_remote_url: remote_text,
            target_ref: evidence.candidate_ref.clone(),
            expected_old: None,
            expected_absent: true,
            out,
        };
        validate_source_main_authority(&options, &evidence)?;
        validate_swarm_parent_authority(&options, &evidence)?;
        git_test(
            &repo,
            &[
                "--git-dir",
                options.source_remote_url.as_str(),
                "update-ref",
                SOURCE_MAIN_REF,
                identity.swarm_parent.as_str(),
                identity.source_parent.as_str(),
            ],
        )?;
        require(
            validate_source_main_authority(&options, &evidence).is_err(),
            "remote-only source-main movement must invalidate the source authority field",
        )?;
        require(
            validate_swarm_parent_authority(&options, &evidence).is_ok(),
            "remote-only source-main movement must not invalidate W7 authority",
        )?;
        git_test(
            &repo,
            &[
                "--git-dir",
                options.source_remote_url.as_str(),
                "update-ref",
                SOURCE_MAIN_REF,
                identity.source_parent.as_str(),
                identity.swarm_parent.as_str(),
            ],
        )?;
        git_test(
            &repo,
            &[
                "update-ref",
                evidence.swarm_ref.as_str(),
                identity.source_parent.as_str(),
                identity.swarm_parent.as_str(),
            ],
        )?;
        require(
            validate_swarm_parent_authority(&options, &evidence).is_err(),
            "local-only W7 movement must invalidate the W7 authority field",
        )?;
        require(
            validate_source_main_authority(&options, &evidence).is_ok(),
            "local-only W7 movement must not invalidate source-main authority",
        )?;
        git_test(
            &repo,
            &[
                "update-ref",
                evidence.swarm_ref.as_str(),
                identity.swarm_parent.as_str(),
                identity.source_parent.as_str(),
            ],
        )?;
        update_local_ref(
            &repo,
            &evidence.candidate_ref,
            Some(identity.source_parent.as_str()),
            None,
        )?;
        let mismatched_local = publish_candidate_ref_inner(&options, None);
        require(
            mismatched_local.as_ref().is_err_and(|failure| {
                failure.2.local_ref_attempts == 0 && failure.2.remote_push_attempts == 0
            }),
            "mismatched pre-existing local candidate ref must reject before mutation",
        )?;
        update_local_ref(
            &repo,
            &evidence.candidate_ref,
            None,
            Some(identity.source_parent.as_str()),
        )?;
        let context = publication_reconciliation_context(&options)?;
        let indexed_construction = read_indexed_packet(
            &options.construction_packet,
            CONTROL_PACKET_SCHEMA,
            Some("exact_join_construction"),
            Some("constructed"),
            CONSTRUCTION_REPORT,
        )?;
        require_equal(
            json_string(&context, "join_commit"),
            Some(join.as_str()),
            "publication reconciliation intended join",
        )?;
        require_equal(
            json_string(&context, "construction_packet_index_sha256"),
            Some(indexed_construction.index_sha256.as_str()),
            "publication reconciliation construction packet identity",
        )?;
        require(
            json_string(&context, "join_commit") != Some(identity.source_parent.as_str()),
            "publication reconciliation must distinguish the intended join from an unrelated ref state",
        )?;
        let mut unrelated_context = context.clone();
        let unrelated_join = unrelated_context
            .as_object_mut()
            .and_then(|object| object.get_mut("join_commit"))
            .ok_or_else(|| "publication context is missing join_commit".to_string())?;
        *unrelated_join = Value::String(identity.source_parent.clone());
        require(
            require_reconciliation_context_unchanged(
                &context,
                Ok(unrelated_context.clone()),
                "publication",
            )
            .is_err(),
            "reconciliation must reject an unrelated observed join identity",
        )?;
        let mismatched_journal = publish_candidate_ref_inner(&options, Some(&unrelated_context));
        require(
            mismatched_journal.as_ref().is_err_and(|failure| {
                failure.2.local_ref_attempts == 0 && failure.2.remote_push_attempts == 0
            }),
            "mismatched journal identity must reject before ref or push mutation",
        )?;
        let unavailable_observation = publish_candidate_ref_inner_with_final_remote_reader(
            &options,
            Some(&context),
            |_repo, _remote, _reference| {
                Err("injected post-push remote observation failure".to_string())
            },
        )
        .err()
        .ok_or_else(|| "unavailable remote observation unexpectedly published".to_string())?;
        require(
            unavailable_observation
                .0
                .contains("remote candidate-ref state unavailable after push attempt"),
            "post-push observation failure should remain explicit",
        )?;
        require_equal(
            unavailable_observation.2.local_ref_attempts,
            2,
            "post-push observation local update plus rollback attempts",
        )?;
        require_equal(
            unavailable_observation.2.local_ref_rollback_succeeded,
            Some(true),
            "post-push observation local rollback disposition",
        )?;
        require_equal(
            read_optional_local_ref(&repo, &evidence.candidate_ref)?,
            None,
            "post-push observation failure restores the absent local ref",
        )?;
        git_test(
            &repo,
            &["push", "origin", &format!(":{}", evidence.candidate_ref)],
        )?;
        let failed_push_observed_join = publish_candidate_ref_inner_with_publication_runners(
            &options,
            Some(&context),
            |_options, _lease, _refspec| {
                Ok((false, false, "injected push failure".to_string()))
            },
            |_repo, _remote, _reference| Ok(Some(join.clone())),
            read_optional_local_ref,
        )
        .err()
        .ok_or_else(|| "failed push with observed join unexpectedly published".to_string())?;
        require(
            failed_push_observed_join
                .0
                .contains("guarded push process did not report success"),
            "failed push with observed join should remain explicitly unattributed",
        )?;
        require_equal(
            failed_push_observed_join.2.local_ref_attempts,
            2,
            "failed push with observed join local update plus rollback attempts",
        )?;
        require_equal(
            failed_push_observed_join.2.local_ref_rollback_succeeded,
            Some(true),
            "failed push with observed join local rollback disposition",
        )?;
        require_equal(
            read_optional_local_ref(&repo, &evidence.candidate_ref)?,
            None,
            "failed push with observed join restores the absent local ref",
        )?;
        let raced_repo = repo.to_path_buf();
        let raced_remote = options.remote.clone();
        let raced_join = join.clone();
        let raced_target = evidence.candidate_ref.clone();
        let no_op_race = publish_candidate_ref_inner_with_publication_runners(
            &options,
            Some(&context),
            move |runner_options, lease, refspec| {
                git_test(
                    &raced_repo,
                    &[
                        "push",
                        raced_remote.as_str(),
                        &format!("{raced_join}:{raced_target}"),
                    ],
                )?;
                run_guarded_candidate_push(runner_options, lease, refspec)
            },
            read_remote_ref,
            read_optional_local_ref,
        )
        .err()
        .ok_or_else(|| "up-to-date race unexpectedly received publication attribution".to_string())?;
        require(
            no_op_race
                .0
                .contains("guarded push reported no actual target update"),
            "up-to-date race must remain explicitly unattributed",
        )?;
        require_equal(
            no_op_race.2.push_process_succeeded,
            Some(true),
            "up-to-date race process outcome",
        )?;
        require_equal(
            no_op_race.2.target_ref_updated,
            Some(false),
            "up-to-date race target-update attribution",
        )?;
        require_equal(
            no_op_race.2.local_ref_rollback_succeeded,
            Some(true),
            "up-to-date race local rollback disposition",
        )?;
        let no_op_report = publication_rejection_report(
            no_op_race.1.as_deref(),
            Some(&evidence.candidate_ref),
            &no_op_race.0,
            &no_op_race.2,
        );
        require_equal(
            json_string(&no_op_report, "status"),
            Some("publication_state_unknown"),
            "up-to-date race publication status",
        )?;
        require_equal(
            json_bool(&no_op_report, "push_process_succeeded"),
            Some(true),
            "up-to-date race receipt process truth",
        )?;
        require_equal(
            json_bool(&no_op_report, "target_ref_updated"),
            Some(false),
            "up-to-date race receipt update truth",
        )?;
        require_equal(
            read_optional_local_ref(&repo, &evidence.candidate_ref)?,
            None,
            "up-to-date race restores the absent local candidate ref",
        )?;
        require_equal(
            read_remote_ref(&repo, "origin", &evidence.candidate_ref)?,
            Some(join.clone()),
            "up-to-date race leaves the coincidental remote join untouched",
        )?;
        git_test(
            &repo,
            &["push", "origin", &format!(":{}", evidence.candidate_ref)],
        )?;
        let divergent_repo = repo.to_path_buf();
        let divergent_remote = options.remote.clone();
        let divergent_target = evidence.candidate_ref.clone();
        let divergent_commit = identity.source_parent.clone();
        let attributed_then_moved = publish_candidate_ref_inner_with_publication_runners(
            &options,
            Some(&context),
            move |runner_options, lease, refspec| {
                let attributed = run_guarded_candidate_push(runner_options, lease, refspec)?;
                require_equal(
                    (attributed.0, attributed.1),
                    (true, true),
                    "guarded push must attribute the target update before the injected race",
                )?;
                git_test(
                    &divergent_repo,
                    &[
                        "push",
                        "--force",
                        divergent_remote.as_str(),
                        &format!("{divergent_commit}:{divergent_target}"),
                    ],
                )?;
                Ok(attributed)
            },
            read_remote_ref,
            read_optional_local_ref,
        )
        .err()
        .ok_or_else(|| "attributed update followed by remote movement unexpectedly published".to_string())?;
        require_equal(
            attributed_then_moved.2.push_process_succeeded,
            Some(true),
            "remote-movement race push process outcome",
        )?;
        require_equal(
            attributed_then_moved.2.target_ref_updated,
            Some(true),
            "remote-movement race update attribution",
        )?;
        require_equal(
            attributed_then_moved.2.observed_final_ref.as_deref(),
            Some(identity.source_parent.as_str()),
            "remote-movement race final remote observation",
        )?;
        require_equal(
            attributed_then_moved.2.local_ref_attempts,
            2,
            "remote-movement race local update plus rollback attempts",
        )?;
        require_equal(
            attributed_then_moved.2.local_ref_rollback_succeeded,
            Some(true),
            "remote-movement race rollback disposition",
        )?;
        require_equal(
            read_optional_local_ref(&repo, &evidence.candidate_ref)?,
            None,
            "remote-movement race restores the absent local candidate ref",
        )?;
        require_equal(
            read_remote_ref(&repo, "origin", &evidence.candidate_ref)?,
            Some(identity.source_parent.clone()),
            "remote-movement race leaves the divergent remote value untouched",
        )?;
        let attributed_then_moved_report = publication_rejection_report(
            attributed_then_moved.1.as_deref(),
            Some(&evidence.candidate_ref),
            &attributed_then_moved.0,
            &attributed_then_moved.2,
        );
        require_equal(
            json_string(&attributed_then_moved_report, "status"),
            Some("rejected"),
            "remote-movement race publication status",
        )?;
        for field in [
            "push_process_succeeded",
            "target_ref_updated",
            "atomic_push",
            "expected_state_guard_passed",
        ] {
            require_equal(
                json_bool(&attributed_then_moved_report, field),
                Some(true),
                "remote-movement race operation fact",
            )?;
        }
        git_test(
            &repo,
            &["push", "origin", &format!(":{}", evidence.candidate_ref)],
        )?;
        let unavailable_local = publish_candidate_ref_inner_with_publication_runners(
            &options,
            Some(&context),
            run_guarded_candidate_push,
            read_remote_ref,
            |_repo, _reference| Err("injected post-push local observation failure".to_string()),
        )
        .err()
        .ok_or_else(|| "unavailable local observation unexpectedly published".to_string())?;
        require(
            unavailable_local
                .0
                .contains("post-push local candidate state was unavailable"),
            "post-push local observation failure should remain explicit",
        )?;
        require_equal(
            unavailable_local.2.local_ref_after.as_deref(),
            None,
            "unavailable post-push local state must not masquerade as an observation",
        )?;
        let unavailable_local_report = publication_rejection_report(
            unavailable_local.1.as_deref(),
            Some(&evidence.candidate_ref),
            &unavailable_local.0,
            &unavailable_local.2,
        );
        require_equal(
            json_string(&unavailable_local_report, "status"),
            Some("published_but_invalidated"),
            "unavailable post-push local state publication status",
        )?;
        require_equal(
            read_remote_ref(&repo, "origin", &evidence.candidate_ref)?,
            Some(join.clone()),
            "unavailable local observation still reconciles the exact remote join",
        )?;
        update_local_ref(
            &repo,
            &evidence.candidate_ref,
            None,
            Some(join.as_str()),
        )?;
        git_test(
            &repo,
            &["push", "origin", &format!(":{}", evidence.candidate_ref)],
        )?;
        let raced_commit = identity.source_parent.clone();
        let raced_join = join.clone();
        let raced_target = evidence.candidate_ref.clone();
        let immutable_push = publish_candidate_ref_inner_with_publication_runners(
            &options,
            Some(&context),
            |runner_options, lease, refspec| {
                let expected_refspec = format!("{raced_join}:{raced_target}");
                require_equal(
                    refspec,
                    expected_refspec.as_str(),
                    "guarded push immutable join refspec",
                )?;
                update_local_ref(
                    &runner_options.repo,
                    &raced_target,
                    Some(&raced_commit),
                    Some(&raced_join),
                )?;
                run_guarded_candidate_push(runner_options, lease, refspec)
            },
            read_remote_ref,
            read_optional_local_ref,
        )
        .map_err(|failure| failure.0)?;
        require_equal(
            read_remote_ref(&repo, "origin", &evidence.candidate_ref)?,
            Some(join.clone()),
            "immutable push publishes the constructed join despite a raced local ref",
        )?;
        require_equal(
            read_optional_local_ref(&repo, &evidence.candidate_ref)?,
            Some(identity.source_parent.clone()),
            "immutable push does not overwrite the raced local ref",
        )?;
        require_equal(
            immutable_push.1.local_ref_after,
            Some(identity.source_parent.clone()),
            "immutable push receipt reports the raced local ref",
        )?;
        update_local_ref(
            &repo,
            &evidence.candidate_ref,
            None,
            Some(identity.source_parent.as_str()),
        )?;
        git_test(
            &repo,
            &["push", "origin", &format!(":{}", evidence.candidate_ref)],
        )?;
        let (published, publication_state) =
            publish_candidate_ref_inner(&options, Some(&context)).map_err(|failure| failure.0)?;
        require_equal(
            published.join_commit.as_str(),
            join.as_str(),
            "published join identity",
        )?;
        require_equal(
            read_remote_ref(&repo, "origin", &evidence.candidate_ref)?,
            Some(join.clone()),
            "published remote ref identity",
        )?;
        let success = publication_success_report(&published, &options, &publication_state);
        require_equal(
            json_bool(&success, "push_process_succeeded"),
            Some(true),
            "successful publication push-process result",
        )?;
        require_equal(
            json_bool(&success, "target_ref_updated"),
            Some(true),
            "successful publication target-update attribution",
        )?;
        require_equal(
            success.get("merge_command").is_some_and(Value::is_null),
            true,
            "successful publication must not emit merge authority",
        )?;
        require_equal(
            success
                .get("merge_command_attempts")
                .and_then(Value::as_u64),
            Some(0),
            "successful publication merge-command attempts",
        )?;

        fs::write(repo.join("moved-main.txt"), "moved\n")
            .map_err(|error| format!("failed to write moved-main fixture: {error}"))?;
        git_test(&repo, &["add", "moved-main.txt"])?;
        git_test(&repo, &["commit", "-m", "move source main"])?;
        let moved_main_options = PublicationOptions {
            expected_old: Some(join.clone()),
            expected_absent: false,
            ..options.clone()
        };
        let moved_main = publish_candidate_ref_inner(&moved_main_options, None);
        require(
            moved_main.as_ref().is_err_and(|failure| {
                failure.0.contains("source main moved")
                    && failure.2.local_ref_attempts == 0
                    && failure.2.remote_push_attempts == 0
            }),
            "actual refs/heads/main movement must reject before publication mutation",
        )?;

        let second = publish_candidate_ref_inner(&options, None);
        require(
            second
                .as_ref()
                .is_err_and(|failure| failure.2.remote_push_attempts == 0),
            "unexpected existing candidate ref must fail before push",
        )?;
        repo.cleanup()?;
        remote_root.cleanup()
    }

    #[test]
    fn source_promotion_publication_rolls_back_local_ref_on_remote_rejection() -> Result<(), String>
    {
        let (repo, identity) = init_synthetic_repo("publish-rollback")?;
        let join = create_exact_join_object(&repo, &identity)?;
        let mut evidence = valid_construction_evidence(identity.clone());
        evidence.join_commit = join.clone();
        evidence.commit_timestamp = canonical_join_timestamp(&repo, &identity)?;
        evidence.swarm_ref = format!(
            "refs/tags/ripr-release-0.11.0-{}",
            identity.swarm_parent.as_str()
        );
        let packet_root = repo.join("control-construction");
        write_control_packet(
            &packet_root,
            "exact_join_construction",
            CONSTRUCTION_REPORT,
            &construction_success_report(&evidence),
            "test construction",
            "test",
        )?;

        git_test(&repo, &["checkout", "-b", "blocking"])?;
        fs::write(repo.join("blocking.txt"), "blocking\n")
            .map_err(|error| format!("failed to write blocking fixture: {error}"))?;
        git_test(&repo, &["add", "blocking.txt"])?;
        git_test(&repo, &["commit", "-m", "blocking"])?;
        let expected_old = current_head(&repo)?;
        git_test(&repo, &["checkout", "main"])?;

        let remote_root = test_temp_dir("rollback-remote")?;
        let remote = remote_root.join("remote.git");
        let remote_text = remote.to_string_lossy().into_owned();
        git_test(&repo, &["init", "--bare", &remote_text])?;
        git_test(&repo, &["remote", "add", "origin", &remote_text])?;
        git_test(
            &repo,
            &[
                "push",
                "origin",
                SOURCE_MAIN_REF,
                evidence.swarm_ref.as_str(),
                &format!("{expected_old}:{}", evidence.candidate_ref),
            ],
        )?;
        git_test(
            &repo,
            &[
                "--git-dir",
                &remote_text,
                "config",
                "receive.denyNonFastForwards",
                "true",
            ],
        )?;
        update_local_ref(&repo, &evidence.candidate_ref, Some(&expected_old), None)?;

        let options = PublicationOptions {
            repo: repo.to_path_buf(),
            construction_packet: packet_root,
            source_main_ref: SOURCE_MAIN_REF.to_string(),
            remote: "origin".to_string(),
            source_remote_url: remote_text.clone(),
            swarm_remote_url: remote_text,
            target_ref: evidence.candidate_ref.clone(),
            expected_old: Some(expected_old.clone()),
            expected_absent: false,
            out: repo.join("publication-output"),
        };
        let failure = publish_candidate_ref_inner(&options, None)
            .err()
            .ok_or_else(|| "non-fast-forward fixture unexpectedly published".to_string())?;
        let (reason, rejected_evidence, state) = failure;
        require(
            reason.contains("did not publish the exact join"),
            "remote rejection should be reported after reconciliation",
        )?;
        require(
            rejected_evidence.is_some(),
            "rejection should retain construction evidence",
        )?;
        require_equal(state.remote_push_attempts, 1, "remote push attempts")?;
        require_equal(
            state.local_ref_attempts,
            2,
            "local update plus rollback attempts",
        )?;
        require_equal(
            state.local_ref_rollback_succeeded,
            Some(true),
            "local rollback disposition",
        )?;
        require_equal(
            read_optional_local_ref(&repo, &evidence.candidate_ref)?,
            Some(expected_old.clone()),
            "local candidate ref after rollback",
        )?;
        require_equal(
            read_remote_ref(&repo, "origin", &evidence.candidate_ref)?,
            Some(expected_old),
            "remote candidate ref after rejected push",
        )?;
        let rejected = publication_rejection_report(
            Some(&evidence),
            Some(&evidence.candidate_ref),
            &reason,
            &state,
        );
        require_equal(json_string(&rejected, "status"), Some("rejected"), "status")?;
        require_equal(
            rejected
                .get("merge_command_attempts")
                .and_then(Value::as_u64),
            Some(0),
            "rejected publication merge-command attempts",
        )?;
        repo.cleanup()?;
        remote_root.cleanup()
    }

    #[test]
    fn construction_receipt_rejects_stale_or_reversed_claims() -> Result<(), String> {
        let identity = test_identity();
        let evidence = valid_construction_evidence(identity.clone());
        let valid = construction_success_report(&evidence);
        require(
            validate_construction_receipt(&valid, &evidence).is_ok(),
            "valid construction receipt should pass",
        )?;

        let mut reversed = valid.clone();
        replace_object_field(
            &mut reversed,
            "ordered_parents",
            serde_json::json!([
                identity.swarm_parent.as_str(),
                identity.source_parent.as_str()
            ]),
            "construction receipt fixture",
        )?;
        require(
            validate_construction_receipt(&reversed, &evidence).is_err(),
            "construction receipt with reversed parents should reject",
        )?;

        let mut premature_merge = valid;
        replace_object_field(
            &mut premature_merge,
            "merge_command",
            Value::String("git merge --no-ff refs/heads/promote/0.11.0-test".to_string()),
            "construction receipt fixture",
        )?;
        require(
            validate_construction_receipt(&premature_merge, &evidence).is_err(),
            "construction receipt with a premature merge command should reject",
        )?;
        Ok(())
    }

    #[test]
    fn command_parser_rejects_duplicate_unknown_and_conflicting_expected_state()
    -> Result<(), String> {
        let duplicate = vec![
            SOURCE_PROMOTION_PUBLISH_CANDIDATE_REF_SUBCOMMAND.to_string(),
            "--target-ref".to_string(),
            "refs/heads/promote/0.11.0-a".to_string(),
            "--target-ref".to_string(),
            "refs/heads/promote/0.11.0-b".to_string(),
        ];
        require(
            parse_command_args(
                &duplicate,
                SOURCE_PROMOTION_PUBLISH_CANDIDATE_REF_SUBCOMMAND,
                &["--target-ref"],
                &[],
            )
            .is_err(),
            "duplicate command option should reject",
        )?;

        let unknown = vec![
            SOURCE_PROMOTION_ADMIT_RESOLVED_TREE_SUBCOMMAND.to_string(),
            "--unknown".to_string(),
            "x".to_string(),
        ];
        require(
            parse_command_args(
                &unknown,
                SOURCE_PROMOTION_ADMIT_RESOLVED_TREE_SUBCOMMAND,
                &[],
                &[],
            )
            .is_err(),
            "unknown command option should reject",
        )?;

        let publication_args = |expected_state: &[&str]| {
            let mut args = vec![
                SOURCE_PROMOTION_PUBLISH_CANDIDATE_REF_SUBCOMMAND.to_string(),
                "--construction-packet".to_string(),
                "target/packet".to_string(),
                "--source-main-ref".to_string(),
                SOURCE_MAIN_REF.to_string(),
                "--remote".to_string(),
                "origin".to_string(),
                "--target-ref".to_string(),
                "refs/heads/promote/0.11.0-test".to_string(),
            ];
            args.extend(expected_state.iter().map(|value| (*value).to_string()));
            args
        };
        require(
            parse_publication_options(&publication_args(&[
                "--expected-absent",
                "--expected-old",
                "1111111111111111111111111111111111111111",
            ]))
            .is_err(),
            "both expected-state options should reject",
        )?;
        require(
            parse_publication_options(&publication_args(&[])).is_err(),
            "missing expected-state option should reject",
        )?;
        Ok(())
    }

    #[test]
    fn source_main_authority_rejects_caller_selected_aliases() -> Result<(), String> {
        require(
            validate_source_main_ref(SOURCE_MAIN_REF).is_ok(),
            "the exact protected source main ref should pass",
        )?;
        require(
            validate_source_main_ref("refs/heads/source").is_err(),
            "a caller-selected source alias should reject",
        )
    }

    #[test]
    fn publication_authority_requires_successful_guarded_push_process() -> Result<(), String> {
        let evidence = valid_construction_evidence(test_identity());
        let mut state = PublicationState {
            push_process_succeeded: Some(false),
            target_ref_updated: Some(false),
            remote_state_observed: true,
            observed_final_ref: Some(evidence.join_commit.clone()),
            remote_push_attempts: 1,
            ..PublicationState::default()
        };
        require(
            !authoritative_publication_observed(&state, &evidence),
            "a coincidental remote join after a failed push must not grant publication authority",
        )?;
        let report = publication_rejection_report(
            Some(&evidence),
            Some(&evidence.candidate_ref),
            "guarded push failed",
            &state,
        );
        require_equal(
            json_string(&report, "status"),
            Some("publication_state_unknown"),
            "failed-push coincidental remote state",
        )?;
        require_equal(
            report.get("atomic_push"),
            Some(&Value::Null),
            "failed-push atomic authority",
        )?;
        state.push_process_succeeded = Some(true);
        state.target_ref_updated = Some(true);
        state.source_main_unchanged = Some(true);
        state.swarm_parent_unchanged = Some(true);
        state.construction_packet_unchanged = Some(true);
        state.remote_authority_unchanged = Some(true);
        require(
            authoritative_publication_observed(&state, &evidence),
            "a successful guarded push plus exact remote observation should be authoritative",
        )?;
        state.source_main_unchanged = Some(false);
        require(
            !authoritative_publication_observed(&state, &evidence),
            "a known false post-push authority bit must block publication authority",
        )?;
        let invalidated = publication_rejection_report(
            Some(&evidence),
            Some(&evidence.candidate_ref),
            "source main moved after push",
            &state,
        );
        require_equal(
            json_string(&invalidated, "status"),
            Some("published_but_invalidated"),
            "successful push with invalidated authority",
        )
    }

    #[test]
    fn control_packet_reservation_is_exclusive_and_journaled_before_side_effects()
    -> Result<(), String> {
        let root = test_temp_dir("packet-reservation")?;
        let out = root.join("packet");
        let context = serde_json::json!({"target_ref": "refs/heads/test"});
        let reservation = reserve_control_packet_output(&out, "test_control", &context)?;
        require(
            out.join("control-attempt.json").is_file(),
            "reservation should synchronously write the attempt journal",
        )?;
        let journal_bytes = read_regular_file(
            &out.join("control-attempt.json"),
            "reserved control attempt journal",
        )?;
        let journal: Value = serde_json::from_slice(&journal_bytes)
            .map_err(|error| format!("malformed reserved attempt journal: {error}"))?;
        require_equal(
            journal.get("reconciliation_context"),
            Some(&context),
            "reserved journal must preserve the exact reconciliation context",
        )?;
        require(
            reserve_control_packet_output(&out, "test_control", &context).is_err(),
            "a second writer must not claim the reserved output",
        )?;
        let report = serde_json::json!({"status": "rejected"});
        write_reserved_control_packet(
            &reservation,
            "test_control",
            "test-control.json",
            &report,
            "Test control",
            "Test-only packet.",
        )?;
        let packet = read_indexed_packet(
            &out,
            CONTROL_PACKET_SCHEMA,
            Some("test_control"),
            Some("rejected"),
            "test-control.json",
        )?;
        require(
            packet.files.contains_key("control-attempt.json"),
            "completed packet index should retain the pre-side-effect attempt journal",
        )?;
        root.cleanup()
    }

    #[test]
    fn control_packet_finalization_failure_retains_incomplete_attempt_journal() -> Result<(), String>
    {
        let root = test_temp_dir("packet-finalization-failure")?;
        let out = root.join("packet");
        let reservation =
            reserve_control_packet_output(&out, "test_control", &serde_json::json!({}))?;
        fs::create_dir(out.join("test-control.json"))
            .map_err(|error| format!("failed to inject report-path collision: {error}"))?;
        let report = serde_json::json!({"status": "rejected"});
        require(
            write_reserved_control_packet(
                &reservation,
                "test_control",
                "test-control.json",
                &report,
                "Test control",
                "Test-only packet.",
            )
            .is_err(),
            "injected finalization failure should be reported",
        )?;
        require(
            out.join("control-attempt.json").is_file(),
            "failed finalization should retain the pre-side-effect journal",
        )?;
        require(
            !out.join(PACKET_INDEX).exists(),
            "failed finalization must not publish a complete packet index",
        )?;
        root.cleanup()
    }
}
