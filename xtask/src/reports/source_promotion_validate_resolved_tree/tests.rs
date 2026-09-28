#[cfg(test)]
mod tests {
    use super::{
        CommandEvidence, PACKET_INDEX, PacketWorkspace, REPORT_JSON, REQUIRED_COMMANDS,
        ValidationState, command_receipt, commands_are_terminal_green, create_exclusive_temp_dir,
        ensure_checker_source_identity, git, input_echo, packet_entries, parse_args, read_bound_json,
        reject_parent_components, render_markdown, report_value, resolved_tree_receipt_is_admissible,
        snapshot_refs, snapshot_worktrees, validate_exact_hex,
        validate_resolved_tree_receipt_contract, verify_exact_commit,
        verify_exact_tree, worktree_listing_contains_path, write_new_file,
    };
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn create(label: &str) -> Result<Self, String> {
            create_exclusive_temp_dir("ripr-resolved-tree-test", label).map(Self)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn valid_args() -> Vec<String> {
        vec![
            "validate-resolved-tree".to_string(),
            "--source-parent".to_string(),
            "a".repeat(40),
            "--swarm-parent".to_string(),
            "b".repeat(40),
            "--reviewed-tree".to_string(),
            "c".repeat(40),
            "--preflight".to_string(),
            "preflight.json".to_string(),
            "--preflight-sha256".to_string(),
            "d".repeat(64),
            "--resolution-manifest".to_string(),
            "resolution.json".to_string(),
            "--resolution-sha256".to_string(),
            "e".repeat(64),
        ]
    }

    fn valid_fixture_state() -> ValidationState {
        let empty_digest = format!("{:x}", Sha256::digest(b""));
        let mut state = ValidationState::new(super::InputEcho {
            source_parent: Some("a".repeat(40)),
            swarm_parent: Some("b".repeat(40)),
            reviewed_tree: Some("c".repeat(40)),
            preflight_path: Some("docs/release/source-promotion/preflight.json".to_string()),
            preflight_sha256: Some("d".repeat(64)),
            resolution_path: Some(
                "docs/release/source-promotion/resolution-manifest.json".to_string(),
            ),
            resolution_sha256: Some("e".repeat(64)),
        });
        state.preflight_verified = true;
        state.resolution_verified = true;
        state.checker_source_sha = Some("a".repeat(40));
        state.checker_executable_sha256 = Some("f".repeat(64));
        state.materialized_tree = Some("c".repeat(40));
        state.disposable_commit = Some("1".repeat(40));
        state.materialization_created = true;
        state.materialization_clean_before = true;
        state.materialization_clean_after = true;
        state.worktree_remove_succeeded = true;
        state.materialization_directory_removed = true;
        state.commands = REQUIRED_COMMANDS
            .iter()
            .enumerate()
            .map(|(index, command)| {
                let evidence = CommandEvidence {
                    stdout_path: format!("commands/{:02}-{command}.stdout.log", index + 1),
                    stdout_bytes: 0,
                    stdout_sha256: empty_digest.clone(),
                    stdout_truncated: false,
                    stderr_path: format!("commands/{:02}-{command}.stderr.log", index + 1),
                    stderr_bytes: 0,
                    stderr_sha256: empty_digest.clone(),
                    stderr_truncated: false,
                };
                command_receipt(command, "passed", Some(0), Some(&evidence), None)
            })
            .collect();
        state
    }

    #[test]
    fn exact_identity_rejects_abbreviated_and_uppercase_values() -> Result<(), String> {
        let Err(_) = validate_exact_hex("sha", "abc123", 40) else {
            return Err("abbreviated identity unexpectedly passed".to_string());
        };
        let Err(_) =
            validate_exact_hex("sha", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", 40)
        else {
            return Err("uppercase identity unexpectedly passed".to_string());
        };
        validate_exact_hex("sha", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 40)
    }

    #[test]
    fn required_command_catalog_is_complete_and_stable() {
        assert_eq!(REQUIRED_COMMANDS.len(), 13);
        assert_eq!(REQUIRED_COMMANDS.first().copied(), Some("check-network-policy"));
        assert_eq!(REQUIRED_COMMANDS.last().copied(), Some("check-architecture"));
    }

    #[test]
    fn option_value_that_is_another_flag_is_rejected_as_a_value() -> Result<(), String> {
        let args = vec![
            "validate-resolved-tree".to_string(),
            "--source-parent".to_string(),
            "--swarm-parent".to_string(),
        ];
        let echo = input_echo(&args);
        assert_eq!(echo.source_parent, None);
        let Err(_) = parse_args(&args) else {
            return Err("flag-shaped option value unexpectedly parsed".to_string());
        };
        Ok(())
    }

    #[test]
    fn duplicate_and_unknown_options_fail_closed() -> Result<(), String> {
        let mut duplicate = valid_args();
        duplicate.extend(["--source-parent".to_string(), "f".repeat(40)]);
        let Err(_) = parse_args(&duplicate) else {
            return Err("duplicate option unexpectedly parsed".to_string());
        };

        let mut unknown = valid_args();
        unknown.extend(["--unknown".to_string(), "value".to_string()]);
        let Err(_) = parse_args(&unknown) else {
            return Err("unknown option unexpectedly parsed".to_string());
        };
        Ok(())
    }

    #[test]
    fn parent_escape_and_digest_mismatch_fail_closed() -> Result<(), String> {
        let Err(_) = reject_parent_components(Path::new("../escape.json"), "fixture") else {
            return Err("parent path escape unexpectedly passed".to_string());
        };
        let root = TempRoot::create("digest-mismatch")?;
        fs::write(root.path().join("input.json"), b"{}\n")
            .map_err(|error| format!("write digest fixture: {error}"))?;
        let error = read_bound_json(
            root.path(),
            Path::new("input.json"),
            &"0".repeat(64),
            "fixture",
        )
        .err()
        .ok_or_else(|| "digest mismatch unexpectedly passed".to_string())?;
        assert!(error.contains("SHA-256 mismatch"));
        Ok(())
    }

    #[test]
    fn checker_source_identity_rejects_non_source_checkout() -> Result<(), String> {
        ensure_checker_source_identity(&"a".repeat(40), &"a".repeat(40))?;
        let Err(_) = ensure_checker_source_identity(&"a".repeat(40), &"b".repeat(40)) else {
            return Err("non-source checker identity unexpectedly passed".to_string());
        };
        Ok(())
    }

    #[test]
    fn command_catalog_requires_exact_order_terminal_passes_and_evidence() {
        let passed = valid_fixture_state().commands;
        assert!(commands_are_terminal_green(&passed));

        for state in ["failed", "not_run", "unavailable"] {
            let mut control = passed.clone();
            control[0]["state"] = Value::String(state.to_string());
            assert!(!commands_are_terminal_green(&control));
        }
        let mut missing_evidence = passed.clone();
        missing_evidence[0]["evidence_present"] = Value::Bool(false);
        assert!(!commands_are_terminal_green(&missing_evidence));
        let mut missing = passed.clone();
        let _ = missing.pop();
        assert!(!commands_are_terminal_green(&missing));
        let mut reordered = passed;
        reordered.swap(0, 1);
        assert!(!commands_are_terminal_green(&reordered));
    }

    #[test]
    fn validated_status_is_earned_and_top_level_tampering_has_no_authority() {
        let validated = report_value(&valid_fixture_state());
        assert_eq!(validated["status"], "validated");
        assert!(resolved_tree_receipt_is_admissible(&validated));

        let rejected = report_value(&ValidationState::new(Default::default()));
        assert_eq!(rejected["status"], "rejected");
        let mut forged = rejected;
        forged["status"] = Value::String("validated".to_string());
        assert!(!resolved_tree_receipt_is_admissible(&forged));
    }

    #[test]
    fn receipt_contract_rejects_catalog_and_unknown_field_tampering() -> Result<(), String> {
        let validated = report_value(&valid_fixture_state());
        validate_resolved_tree_receipt_contract(&validated, "validated")?;
        let mut rejected_state = valid_fixture_state();
        rejected_state
            .failure_reasons
            .push("required command failed".to_string());
        let rejected = report_value(&rejected_state);
        validate_resolved_tree_receipt_contract(&rejected, "rejected")?;

        let mut removed_catalog = validated.clone();
        let catalog = removed_catalog["required_command_catalog"]
            .as_array_mut()
            .ok_or_else(|| "fixture command catalog is not an array".to_string())?;
        if catalog.is_empty() {
            return Err("fixture command catalog is unexpectedly empty".to_string());
        }
        let _ = catalog.remove(0);
        if validate_resolved_tree_receipt_contract(&removed_catalog, "validated").is_ok() {
            return Err("receipt contract accepted a removed catalog row".to_string());
        }

        let mut changed_catalog = validated.clone();
        let first = changed_catalog["required_command_catalog"]
            .as_array_mut()
            .and_then(|catalog| catalog.first_mut())
            .ok_or_else(|| "fixture command catalog has no first row".to_string())?;
        *first = Value::String("check-forged-policy".to_string());
        if validate_resolved_tree_receipt_contract(&changed_catalog, "validated").is_ok() {
            return Err("receipt contract accepted a changed catalog row".to_string());
        }

        let mut unknown_top_level = validated.clone();
        unknown_top_level["merge_authorized"] = Value::Bool(true);
        if validate_resolved_tree_receipt_contract(&unknown_top_level, "validated").is_ok() {
            return Err("receipt contract accepted an unknown top-level authority field".to_string());
        }

        let mut unknown_nested = validated;
        unknown_nested["trusted_checker"]["merge_authorized"] = Value::Bool(true);
        if validate_resolved_tree_receipt_contract(&unknown_nested, "validated").is_ok() {
            return Err("receipt contract accepted an unknown nested authority field".to_string());
        }

        let mut changed_claim = report_value(&valid_fixture_state());
        let claim = changed_claim["non_claims"]
            .as_array_mut()
            .and_then(|claims| claims.first_mut())
            .ok_or_else(|| "fixture has no first non-claim".to_string())?;
        *claim = Value::String("This receipt grants release authority.".to_string());
        if validate_resolved_tree_receipt_contract(&changed_claim, "validated").is_ok() {
            return Err("receipt contract accepted a changed non-claim".to_string());
        }
        Ok(())
    }

    #[test]
    fn receipt_contract_rejects_shared_semantic_tampering() -> Result<(), String> {
        let validated = report_value(&valid_fixture_state());
        for (pointer, replacement) in [
            ("/preflight/verified", Value::Bool(false)),
            ("/trusted_checker/source_sha", Value::String("2".repeat(40))),
            (
                "/trusted_checker/executable_sha256",
                Value::String("short".to_string()),
            ),
            ("/materialization/authoritative", Value::Bool(true)),
            (
                "/repository_observation/ref_mutation_observed",
                Value::Bool(true),
            ),
            (
                "/packet_contract/atomic_directory_publish",
                Value::Bool(false),
            ),
            ("/authoritative_commit_attempted", Value::Bool(true)),
        ] {
            let mut candidate = validated.clone();
            let target = candidate
                .pointer_mut(pointer)
                .ok_or_else(|| format!("fixture has no {pointer}"))?;
            *target = replacement;
            if validate_resolved_tree_receipt_contract(&candidate, "validated").is_ok() {
                return Err(format!("receipt contract accepted semantic tampering at {pointer}"));
            }
        }

        let mut rejected_state = valid_fixture_state();
        rejected_state
            .failure_reasons
            .push("required command failed".to_string());
        let mut rejected = report_value(&rejected_state);
        rejected["failure_reasons"] = Value::Array(Vec::new());
        if validate_resolved_tree_receipt_contract(&rejected, "rejected").is_ok() {
            return Err("receipt contract accepted rejected status without a reason".to_string());
        }
        Ok(())
    }

    #[test]
    fn command_catalog_subject_role_does_not_claim_candidate_authority() -> Result<(), String> {
        let commands = valid_fixture_state().commands;
        let catalog = commands
            .iter()
            .find(|receipt| receipt["command"] == "check-command-catalog")
            .ok_or_else(|| "catalog receipt must exist".to_string())?;
        assert_eq!(
            catalog["subject_role"],
            "source_parent_trusted_checker_self_health"
        );
        Ok(())
    }

    #[test]
    fn worktree_listing_matches_exact_normalized_paths() {
        let listing = "worktree /private/var/tmp/ripr-tree\nHEAD deadbeef\n\n";
        assert!(worktree_listing_contains_path(listing, "/private/var/tmp/ripr-tree"));
        assert!(!worktree_listing_contains_path(listing, "/var/tmp/ripr-tree"));
        assert!(!worktree_listing_contains_path(listing, "/private/var/tmp/ripr"));
    }

    #[test]
    fn exact_object_helpers_reject_wrong_git_object_kinds() -> Result<(), String> {
        let root = TempRoot::create("object-kinds")?;
        git(root.path(), &["init", "--quiet"], &[])?;
        fs::write(root.path().join("value.txt"), "value\n")
            .map_err(|error| format!("write object-kind fixture: {error}"))?;
        git(root.path(), &["add", "value.txt"], &[])?;
        git(
            root.path(),
            &["commit", "--quiet", "-m", "fixture"],
            &[
                ("GIT_AUTHOR_NAME", "ripr test"),
                ("GIT_AUTHOR_EMAIL", "ripr-test@invalid"),
                ("GIT_COMMITTER_NAME", "ripr test"),
                ("GIT_COMMITTER_EMAIL", "ripr-test@invalid"),
            ],
        )?;
        let commit = git(root.path(), &["rev-parse", "HEAD"], &[])?;
        let tree = git(root.path(), &["rev-parse", "HEAD^{tree}"], &[])?;
        verify_exact_commit(root.path(), commit.trim(), "commit")?;
        verify_exact_tree(root.path(), tree.trim())?;
        let Err(_) = verify_exact_commit(root.path(), tree.trim(), "tree") else {
            return Err("tree object unexpectedly passed commit verification".to_string());
        };
        let Err(_) = verify_exact_tree(root.path(), commit.trim()) else {
            return Err("commit object unexpectedly passed tree verification".to_string());
        };
        Ok(())
    }

    #[test]
    fn repository_observation_detects_ref_mutation_without_canonical_ambient_hashes() -> Result<(), String> {
        let root = TempRoot::create("ref-mutation")?;
        git(root.path(), &["init", "--quiet"], &[])?;
        fs::write(root.path().join("value.txt"), "value\n")
            .map_err(|error| format!("write ref fixture: {error}"))?;
        git(root.path(), &["add", "value.txt"], &[])?;
        git(
            root.path(),
            &["commit", "--quiet", "-m", "fixture"],
            &[
                ("GIT_AUTHOR_NAME", "ripr test"),
                ("GIT_AUTHOR_EMAIL", "ripr-test@invalid"),
                ("GIT_COMMITTER_NAME", "ripr test"),
                ("GIT_COMMITTER_EMAIL", "ripr-test@invalid"),
            ],
        )?;
        let before_refs = snapshot_refs(root.path())?;
        let before_worktrees = snapshot_worktrees(root.path())?;
        let head = git(root.path(), &["rev-parse", "HEAD"], &[])?;
        git(
            root.path(),
            &["update-ref", "refs/heads/mutated-control", head.trim()],
            &[],
        )?;
        let mut state = ValidationState::new(Default::default());
        super::observe_repository_after(root.path(), &mut state, &before_refs, &before_worktrees)?;
        assert!(state.ref_mutation_observed);
        let report = report_value(&state);
        let observation = report["repository_observation"]
            .as_object()
            .ok_or_else(|| "repository observation missing".to_string())?;
        assert!(!observation.contains_key("refs_before_sha256"));
        assert!(!observation.contains_key("worktrees_before_sha256"));
        Ok(())
    }

    #[test]
    fn git_object_view_ignores_replacement_refs() -> Result<(), String> {
        let root = TempRoot::create("object-view")?;
        git(root.path(), &["init", "--quiet"], &[])?;
        fs::write(root.path().join("value.txt"), "original\n")
            .map_err(|error| format!("write original fixture: {error}"))?;
        git(root.path(), &["add", "value.txt"], &[])?;
        git(
            root.path(),
            &["commit", "--quiet", "-m", "original"],
            &[
                ("GIT_AUTHOR_NAME", "ripr test"),
                ("GIT_AUTHOR_EMAIL", "ripr-test@invalid"),
                ("GIT_COMMITTER_NAME", "ripr test"),
                ("GIT_COMMITTER_EMAIL", "ripr-test@invalid"),
            ],
        )?;
        let original = git(root.path(), &["rev-parse", "HEAD"], &[])?;
        fs::write(root.path().join("value.txt"), "replacement\n")
            .map_err(|error| format!("write replacement fixture: {error}"))?;
        git(root.path(), &["add", "value.txt"], &[])?;
        git(
            root.path(),
            &["commit", "--quiet", "-m", "replacement"],
            &[
                ("GIT_AUTHOR_NAME", "ripr test"),
                ("GIT_AUTHOR_EMAIL", "ripr-test@invalid"),
                ("GIT_COMMITTER_NAME", "ripr test"),
                ("GIT_COMMITTER_EMAIL", "ripr-test@invalid"),
            ],
        )?;
        let replacement = git(root.path(), &["rev-parse", "HEAD"], &[])?;
        git(root.path(), &["replace", original.trim(), replacement.trim()], &[])?;
        let observed = git(
            root.path(),
            &["show", &format!("{}:value.txt", original.trim())],
            &[],
        )?;
        assert_eq!(observed, "original\n");
        Ok(())
    }

    #[test]
    fn packet_workspace_is_create_new_index_last_and_atomically_published() -> Result<(), String> {
        let root = TempRoot::create("packet")?;
        let final_out = root.path().join("packet-out");
        let mut packet = PacketWorkspace::create(&final_out, "packet-test")?;
        let command_dir = packet.root().join("commands");
        fs::create_dir(&command_dir).map_err(|error| error.to_string())?;
        write_new_file(&command_dir.join("evidence.log"), b"evidence\n")?;
        let Err(_) = write_new_file(&command_dir.join("evidence.log"), b"replace\n") else {
            return Err("create-new evidence write unexpectedly replaced a file".to_string());
        };
        packet.publish(&report_value(&ValidationState::new(Default::default())))?;
        assert!(final_out.join(PACKET_INDEX).is_file());
        assert!(packet_entries(&final_out)?
            .iter()
            .any(|entry| entry["path"] == REPORT_JSON));
        Ok(())
    }

    #[test]
    fn packet_workspace_rejects_existing_output_and_partial_publish_collision() -> Result<(), String> {
        let root = TempRoot::create("packet-collision")?;
        let existing = root.path().join("existing");
        fs::write(&existing, b"occupied").map_err(|error| error.to_string())?;
        let Err(_) = PacketWorkspace::create(&existing, "existing") else {
            return Err("existing output path unexpectedly admitted".to_string());
        };

        let final_out = root.path().join("late-collision");
        let mut packet = PacketWorkspace::create(&final_out, "late")?;
        fs::create_dir(&final_out).map_err(|error| error.to_string())?;
        let Err(_) = packet.publish(&report_value(&ValidationState::new(Default::default()))) else {
            return Err("partial publish collision unexpectedly succeeded".to_string());
        };
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn packet_workspace_rejects_output_and_parent_symlinks() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let root = TempRoot::create("packet-symlink")?;
        let target = root.path().join("target");
        fs::create_dir(&target).map_err(|error| error.to_string())?;
        let output_link = root.path().join("output-link");
        symlink(&target, &output_link).map_err(|error| error.to_string())?;
        let Err(_) = PacketWorkspace::create(&output_link, "output-link") else {
            return Err("symlink output path unexpectedly admitted".to_string());
        };

        let parent_link = root.path().join("parent-link");
        symlink(&target, &parent_link).map_err(|error| error.to_string())?;
        let Err(_) = PacketWorkspace::create(&parent_link.join("packet"), "parent-link") else {
            return Err("symlink parent path unexpectedly admitted".to_string());
        };
        Ok(())
    }

    #[test]
    fn canonical_receipt_fixtures_are_byte_stable_and_semantically_distinct() -> Result<(), String> {
        for (state, expected_json, expected_markdown) in [
            (
                ValidationState::new(Default::default()),
                include_str!(
                    "../../../../fixtures/source_promotion_resolved_tree/expected/rejected.json"
                ),
                include_str!(
                    "../../../../fixtures/source_promotion_resolved_tree/expected/rejected.md"
                ),
            ),
            (
                valid_fixture_state(),
                include_str!(
                    "../../../../fixtures/source_promotion_resolved_tree/expected/validated.json"
                ),
                include_str!(
                    "../../../../fixtures/source_promotion_resolved_tree/expected/validated.md"
                ),
            ),
        ] {
            let report = report_value(&state);
            let json = format!(
                "{}\n",
                serde_json::to_string_pretty(&report)
                    .map_err(|error| format!("serialize fixture receipt: {error}"))?
            );
            assert_eq!(json, expected_json);
            assert_eq!(render_markdown(&report)?, expected_markdown);
        }
        Ok(())
    }

    #[test]
    fn rejected_and_validated_receipts_share_the_same_top_level_keys() {
        let rejected = report_value(&ValidationState::new(Default::default()));
        let validated = report_value(&valid_fixture_state());
        let rejected_keys = rejected
            .as_object()
            .map(|object| object.keys().collect::<Vec<_>>());
        let validated_keys = validated
            .as_object()
            .map(|object| object.keys().collect::<Vec<_>>());
        assert_eq!(rejected_keys, validated_keys);
    }
}
