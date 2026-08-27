#[cfg(test)]
mod source_promotion_control_tests {
    use super::*;
    use std::fmt::Debug;
    use std::ops::Deref;

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
        fs::write(repo.join("base.txt"), "base\n")
            .map_err(|error| format!("failed to write base fixture: {error}"))?;
        git_test(&repo, &["add", "base.txt"])?;
        git_test(&repo, &["commit", "-m", "base"])?;
        git_test(&repo, &["checkout", "-b", "source"])?;
        fs::write(repo.join("source.txt"), "source\n")
            .map_err(|error| format!("failed to write source fixture: {error}"))?;
        git_test(&repo, &["add", "source.txt"])?;
        git_test(&repo, &["commit", "-m", "source"])?;
        let source_parent = current_head(&repo)?;
        git_test(&repo, &["checkout", "main"])?;
        fs::write(repo.join("swarm.txt"), "swarm\n")
            .map_err(|error| format!("failed to write swarm fixture: {error}"))?;
        git_test(&repo, &["add", "swarm.txt"])?;
        git_test(&repo, &["commit", "-m", "swarm"])?;
        let swarm_parent = current_head(&repo)?;
        git_test(&repo, &["checkout", "source"])?;
        let join_tree = commit_tree(&repo, &source_parent)?;
        git_test(
            &repo,
            &[
                "tag",
                &format!("ripr-release-0.11.0-{swarm_parent}"),
                &swarm_parent,
            ],
        )?;
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
            if let Some(command) = candidate
                .get_mut("commands")
                .and_then(Value::as_array_mut)
                .and_then(|commands| commands.first_mut())
            {
                command["state"] = Value::String(state.to_string());
            }
            require(
                !resolved_tree_receipt_is_admissible(&candidate),
                format!("resolved-tree receipt with {state} command should reject"),
            )?;
        }

        let mut reordered = valid.clone();
        if let Some(commands) = reordered.get_mut("commands").and_then(Value::as_array_mut) {
            commands.swap(0, 1);
        }
        require(
            !resolved_tree_receipt_is_admissible(&reordered),
            "resolved-tree receipt with reordered commands should reject",
        )?;

        let mut duplicate = valid;
        if let Some(commands) = duplicate.get_mut("commands").and_then(Value::as_array_mut)
            && let Some(first) = commands.first().cloned()
        {
            commands.push(first);
        }
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
            candidate[key] = Value::Bool(false);
            require(
                validate_admission_receipt(&candidate, &identity).is_err(),
                format!("admission receipt should reject false {key}"),
            )?;
        }

        let mut attempted = valid.clone();
        attempted["ref_mutation_attempted"] = Value::Bool(true);
        require(
            validate_admission_receipt(&attempted, &identity).is_err(),
            "admission receipt should reject a ref mutation attempt",
        )?;
        Ok(())
    }

    #[test]
    fn qualification_rejects_zero_step_failed_and_reordered_lanes() -> Result<(), String> {
        let identity = test_identity();
        let admission = valid_admission_receipt(&identity);
        let packet = IndexedPacket {
            index_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            files: BTreeMap::new(),
        };
        let admission_receipt_sha256 =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let qualification_sha256 =
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        let base = serde_json::json!({
            "schema": QUALIFICATION_SCHEMA,
            "status": "qualified",
            "source_parent": identity.source_parent.as_str(),
            "swarm_parent": identity.swarm_parent.as_str(),
            "join_tree": identity.join_tree.as_str(),
            "preflight_sha256": identity.preflight_sha256.as_str(),
            "resolution_manifest_sha256": identity.resolution_sha256.as_str(),
            "admission_packet_index_sha256": packet.index_sha256.as_str(),
            "admission_receipt_sha256": admission_receipt_sha256,
            "resolved_tree_validation_receipt_sha256":
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "network_policy_receipt_sha256":
                "9999999999999999999999999999999999999999999999999999999999999999",
            "promotion_ref_mutation_attempted": false,
            "lanes": REQUIRED_QUALIFICATION_LANES.iter().map(|name| serde_json::json!({
                "name": name,
                "state": "passed",
                "evidence_sha256":
                    "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
            })).collect::<Vec<_>>(),
            "failure_reasons": [],
        });
        require(
            validate_qualification_receipt(
                &base,
                &identity,
                &admission,
                &packet,
                admission_receipt_sha256,
                qualification_sha256,
            )
            .is_ok(),
            "valid qualification receipt should pass",
        )?;

        let mut zero = base.clone();
        zero["lanes"] = Value::Array(Vec::new());
        require(
            validate_qualification_receipt(
                &zero,
                &identity,
                &admission,
                &packet,
                admission_receipt_sha256,
                qualification_sha256,
            )
            .is_err(),
            "qualification receipt with zero lanes should reject",
        )?;

        let mut failed = base.clone();
        if let Some(lane) = failed
            .get_mut("lanes")
            .and_then(Value::as_array_mut)
            .and_then(|lanes| lanes.first_mut())
        {
            lane["state"] = Value::String("failed".to_string());
        }
        require(
            validate_qualification_receipt(
                &failed,
                &identity,
                &admission,
                &packet,
                admission_receipt_sha256,
                qualification_sha256,
            )
            .is_err(),
            "qualification receipt with a failed lane should reject",
        )?;

        let mut reordered = base;
        if let Some(lanes) = reordered.get_mut("lanes").and_then(Value::as_array_mut) {
            lanes.swap(0, 1);
        }
        require(
            validate_qualification_receipt(
                &reordered,
                &identity,
                &admission,
                &packet,
                admission_receipt_sha256,
                qualification_sha256,
            )
            .is_err(),
            "qualification receipt with reordered lanes should reject",
        )?;
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
        require(
            read_indexed_packet(
                &root,
                CONTROL_PACKET_SCHEMA,
                Some("resolved_tree_admission"),
                Some("admitted"),
                ADMISSION_REPORT,
            )
            .is_err(),
            "packet with a digest mismatch should reject",
        )?;
        temp.cleanup()
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
        Ok(())
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
                "refs/heads/source",
                evidence.swarm_ref.as_str(),
            ],
        )?;
        let out = repo.join("publication-output");
        let options = PublicationOptions {
            repo: repo.to_path_buf(),
            construction_packet: packet_root,
            source_main_ref: "refs/heads/source".to_string(),
            remote: "origin".to_string(),
            source_remote_url: remote_text.clone(),
            swarm_remote_url: remote_text,
            target_ref: evidence.candidate_ref.clone(),
            expected_old: None,
            expected_absent: true,
            out,
        };
        update_local_ref(
            &repo,
            &evidence.candidate_ref,
            Some(identity.source_parent.as_str()),
            None,
        )?;
        let mismatched_local = publish_candidate_ref_inner(&options);
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
        let (published, publication_state) =
            publish_candidate_ref_inner(&options).map_err(|failure| failure.0)?;
        require_equal(
            published.join_commit.as_str(),
            join.as_str(),
            "published join identity",
        )?;
        require_equal(
            read_remote_ref(&repo, "origin", &evidence.candidate_ref)?,
            Some(join),
            "published remote ref identity",
        )?;
        let success = publication_success_report(&published, &options, &publication_state);
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

        let second = publish_candidate_ref_inner(&options);
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
        git_test(&repo, &["checkout", "source"])?;

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
                "refs/heads/source",
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
            source_main_ref: "refs/heads/source".to_string(),
            remote: "origin".to_string(),
            source_remote_url: remote_text.clone(),
            swarm_remote_url: remote_text,
            target_ref: evidence.candidate_ref.clone(),
            expected_old: Some(expected_old.clone()),
            expected_absent: false,
            out: repo.join("publication-output"),
        };
        let failure = publish_candidate_ref_inner(&options)
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
        reversed["ordered_parents"] = serde_json::json!([
            identity.swarm_parent.as_str(),
            identity.source_parent.as_str()
        ]);
        require(
            validate_construction_receipt(&reversed, &evidence).is_err(),
            "construction receipt with reversed parents should reject",
        )?;

        let mut premature_merge = valid;
        premature_merge["merge_command"] =
            Value::String("git merge --no-ff refs/heads/promote/0.11.0-test".to_string());
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
        Ok(())
    }
}
