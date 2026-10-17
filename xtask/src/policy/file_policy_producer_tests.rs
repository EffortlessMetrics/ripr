use super::*;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git_ok(root: &Path, args: &[&str]) -> Result<String, String> {
    successful(Path::new("git"), args, root)
}

fn fixture() -> Result<Fixture, String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-enumeration-{}-{nonce} space α",
        std::process::id()
    ));
    fs::create_dir(&root).map_err(|error| error.to_string())?;
    let fixture = Fixture(root);
    let root = &fixture.0;
    fs::create_dir(root.join("src")).map_err(|error| error.to_string())?;
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"enumeration_subject\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n").map_err(|error| error.to_string())?;
    fs::write(root.join("src/lib.rs"), "pub fn root() -> &'static str { env!(\"CARGO_MANIFEST_DIR\") }\n#[test]\nfn discriminator() { assert!(!root().is_empty()); }\n#[test]\nfn other_filter() {}\n").map_err(|error| error.to_string())?;
    fs::write(
        root.join("rust-toolchain.toml"),
        "[toolchain]\nchannel = \"1.95.0\"\n",
    )
    .map_err(|error| error.to_string())?;
    fs::create_dir(root.join("tests")).map_err(|error| error.to_string())?;
    for name in ["causal_delta_fixture", "source_role_corpus_package"] {
        fs::write(
            root.join("tests").join(format!("{name}.rs")),
            "#[test]\nfn standalone() {}\n",
        )
        .map_err(|error| error.to_string())?;
    }
    fs::write(
        root.join(".gitignore"),
        "/target/\n/evidence*/\n/materialized/\n",
    )
    .map_err(|error| error.to_string())?;
    successful(
        Path::new(env!("CARGO")),
        &["generate-lockfile", "--offline"],
        root,
    )?;
    git_ok(root, &["init", "--quiet"])?;
    git_ok(root, &["add", "."])?;
    git_ok(
        root,
        &[
            "-c",
            "user.name=producer fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "source-bound producer fixture",
        ],
    )?;
    Ok(fixture)
}

fn commands() -> Vec<(usize, String)> {
    // Eleven actual invocations; whole integration targets have no filter.
    (1..=11)
        .map(|line| {
            let selection = match line {
                6 => "--test causal_delta_fixture",
                8 => "--test source_role_corpus_package",
                _ => "--lib discriminator",
            };
            (
                line,
                format!("cargo test -p enumeration_subject {selection}"),
            )
        })
        .collect()
}

fn checked(
    output: &Path,
    subject: &Path,
    root: &Path,
    head: &str,
    commands: &[(usize, String)],
) -> Result<Value, String> {
    verify_with_anchor(
        output,
        subject,
        head,
        &{
            static CHECKER: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            if let Some(value) = CHECKER.get() {
                value.clone()
            } else {
                let value = file_sha(&std::env::current_exe().map_err(|error| error.to_string())?)?;
                let _ = CHECKER.set(value.clone());
                value
            }
        },
        commands,
        Some(root),
        Instant::now() + Duration::from_secs(30),
    )
}

#[test]
fn actual_eleven_cargo_producers_bind_current_source_and_reject_stale_evidence()
-> Result<(), String> {
    let fixture = fixture()?;
    let root = &fixture.0;
    let head = git_ok(root, &["rev-parse", "HEAD"])?;
    let commands = commands();
    let subject = root.join("materialized");
    git_ok(
        root,
        &[
            "clone",
            "--quiet",
            "--no-hardlinks",
            root.to_str().ok_or("root is not UTF-8")?,
            subject.to_str().ok_or("subject is not UTF-8")?,
        ],
    )?;
    git_ok(
        &subject,
        &[
            "-c",
            "user.name=producer fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "disposable same-tree subject",
        ],
    )?;
    assert_ne!(git_ok(&subject, &["rev-parse", "HEAD"])?, head);
    let legacy = root.join("evidence-legacy");
    fs::create_dir(&legacy).map_err(|error| error.to_string())?;
    let legacy_output = legacy.join("file-policy-test-producer.json");
    // The old physical-root strategy really executes all11 calls, but cannot
    // masquerade as the selected same-source producer.
    produce(&subject, &head, None, &legacy_output, &commands)?;
    assert!(checked(&legacy_output, &subject, root, &head, &commands).is_err());
    let evidence = root.join("evidence");
    fs::create_dir(&evidence).map_err(|error| error.to_string())?;
    let output = evidence.join("file-policy-test-producer.json");
    assert!(checked(&output, &subject, root, &head, &commands).is_err());
    produce(&subject, &head, Some(root), &output, &commands)?;
    let summary = checked(&output, &subject, root, &head, &commands)?;
    assert_eq!(summary["mode"], "exact_source_parent_checkout");
    assert_eq!(summary["reference_count"], 11);
    let bytes = fs::read(&output).map_err(|error| error.to_string())?;
    let original: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    for (pointer, replacement) in [
        (
            "/references/0/actual_args/1",
            serde_json::json!("--release"),
        ),
        ("/references/0/selected_tests", serde_json::json!(0)),
        (
            "/references/0/compiler_graph_sha256",
            serde_json::json!("0".repeat(64)),
        ),
        (
            "/metadata/target_directory",
            serde_json::json!("/another-target"),
        ),
        ("/cargo_version", serde_json::json!("another Cargo")),
        ("/inputs/lock_sha256", serde_json::json!("0".repeat(64))),
        ("/subject_inputs/head", serde_json::json!("0".repeat(40))),
        ("/complete", serde_json::json!(false)),
        ("/references/0/exit_code", serde_json::json!(1)),
        ("/references/0/timed_out", serde_json::json!(true)),
        ("/references/0/stdout_truncated", serde_json::json!(true)),
    ] {
        let mut altered = original.clone();
        *altered
            .pointer_mut(pointer)
            .ok_or("missing mutation pointer")? = replacement;
        fs::write(
            &output,
            serde_json::to_vec(&altered).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        assert!(
            checked(&output, &subject, root, &head, &commands).is_err(),
            "accepted {pointer}"
        );
    }
    let mut missing = original.clone();
    missing["references"]
        .as_array_mut()
        .ok_or("missing references")?
        .pop();
    fs::write(
        &output,
        serde_json::to_vec(&missing).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    assert!(checked(&output, &subject, root, &head, &commands).is_err());
    fs::write(&output, &bytes).map_err(|error| error.to_string())?;
    let stdout_name = original["references"][0]["stdout_path"]
        .as_str()
        .ok_or("missing stdout")?;
    let stdout =
        fs::read_to_string(evidence.join(stdout_name)).map_err(|error| error.to_string())?;
    let metadata = &original["metadata"];
    for wrong_command in [
        "cargo test -p another_package --lib discriminator",
        "cargo test -p enumeration_subject --test causal_delta_fixture",
        "cargo test -p enumeration_subject --lib other_filter",
    ] {
        assert!(
            artifacts(
                &stdout,
                metadata,
                root,
                wrong_command,
                &original["test_profile"],
                None
            )
            .is_err(),
            "accepted {wrong_command}"
        );
    }
    for field in ["features", "profile"] {
        let altered = stdout
            .lines()
            .map(|line| {
                let Ok(mut record) = serde_json::from_str::<Value>(line) else {
                    return line.to_string();
                };
                if record["reason"] == "compiler-artifact" && record["profile"]["test"] == true {
                    if field == "features" {
                        record["features"] = serde_json::json!(["invented-feature"]);
                    } else {
                        record["profile"]["opt_level"] = serde_json::json!("3");
                    }
                }
                record.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            artifacts(
                &altered,
                metadata,
                root,
                &commands[0].1,
                &original["test_profile"],
                None
            )
            .is_err(),
            "accepted altered {field}"
        );
    }
    let missing_build = stdout
        .lines()
        .filter(|line| !line.contains("build-finished"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        artifacts(
            &missing_build,
            metadata,
            root,
            &commands[0].1,
            &original["test_profile"],
            None
        )
        .is_err()
    );
    assert!(
        artifacts(
            &format!("{stdout}malformed producer record\n"),
            metadata,
            root,
            &commands[0].1,
            &original["test_profile"],
            None
        )
        .is_err()
    );
    let executable = Path::new(
        original["references"][0]["artifacts"][0]["executable"]
            .as_str()
            .ok_or("missing executable")?,
    );
    let depfile = executable.with_extension("d");
    let dependency_bytes = fs::read_to_string(&depfile).map_err(|error| error.to_string())?;
    let changed = dependency_bytes
        .lines()
        .map(|line| {
            if line.starts_with("# env-dep:CARGO_MANIFEST_DIR=") {
                "# env-dep:CARGO_MANIFEST_DIR=/another-source-root"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(changed, dependency_bytes);
    fs::write(&depfile, changed).map_err(|error| error.to_string())?;
    assert!(
        artifacts(
            &stdout,
            metadata,
            root,
            &commands[0].1,
            &original["test_profile"],
            None
        )
        .is_err()
    );
    fs::write(&depfile, dependency_bytes).map_err(|error| error.to_string())?;
    let exe_bytes = fs::read(executable).map_err(|error| error.to_string())?;
    OpenOptions::new()
        .append(true)
        .open(executable)
        .and_then(|mut file| file.write_all(b"moved"))
        .map_err(|error| error.to_string())?;
    assert!(checked(&output, &subject, root, &head, &commands).is_err());
    fs::write(executable, exe_bytes).map_err(|error| error.to_string())?;
    fs::write(root.join("Cargo.lock"), "moved lock").map_err(|error| error.to_string())?;
    assert!(checked(&output, &subject, root, &head, &commands).is_err());
    git_ok(root, &["restore", "Cargo.lock"])?;
    git_ok(
        root,
        &[
            "-c",
            "user.name=producer fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "same-tree HEAD movement",
        ],
    )?;
    assert!(checked(&output, &subject, root, &head, &commands).is_err());
    Ok(())
}

#[test]
fn real_zero_denominator_and_wrong_filter_are_rejected_with_retained_logs() -> Result<(), String> {
    let fixture = fixture()?;
    let root = &fixture.0;
    let head = git_ok(root, &["rev-parse", "HEAD"])?;
    let evidence = root.join("evidence-zero");
    fs::create_dir(&evidence).map_err(|error| error.to_string())?;
    let output = evidence.join("file-policy-test-producer.json");
    let fallback = root.join("evidence-fallback");
    fs::create_dir(&fallback).map_err(|error| error.to_string())?;
    let fallback_output = fallback.join("file-policy-test-producer.json");
    let positive = vec![(
        1,
        "cargo test -p enumeration_subject --lib discriminator".to_string(),
    )];
    produce(root, &head, None, &fallback_output, &positive)?;
    let checked = verify_with_anchor(
        &fallback_output,
        root,
        &head,
        &file_sha(&std::env::current_exe().map_err(|error| error.to_string())?)?,
        &positive,
        None,
        Instant::now() + Duration::from_secs(30),
    )?;
    assert_eq!(checked["mode"], "reviewed_tree_worktree");
    let commands = vec![(
        1,
        "cargo test -p enumeration_subject --lib absent_discriminator".to_string(),
    )];
    assert!(produce(root, &head, Some(root), &output, &commands).is_err());
    let report: Report =
        serde_json::from_slice(&fs::read(&output).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    assert!(!report.complete);
    assert!(report.failed_invocation.is_some());
    assert!(evidence.join("file-policy-test-01.stdout.log").is_file());
    assert!(evidence.join("file-policy-test-01.stderr.log").is_file());
    Ok(())
}

#[test]
fn source_selection_rejects_dirty_anchor_and_preserves_materialized_fallback() -> Result<(), String>
{
    let fixture = fixture()?;
    let root = &fixture.0;
    let head = git_ok(root, &["rev-parse", "HEAD"])?;
    assert_eq!(
        choose_root(root, &head, Some(root))?.0,
        "exact_source_parent_checkout"
    );
    assert_eq!(
        choose_root(root, &"0".repeat(40), Some(root))?.0,
        "reviewed_tree_worktree"
    );
    assert_eq!(choose_root(root, &head, None)?.0, "reviewed_tree_worktree");
    assert!(
        verify_with_anchor(
            &root.join("missing"),
            root,
            &head,
            "",
            &commands(),
            Some(root),
            Instant::now()
        )
        .is_err()
    );
    fs::write(root.join("untracked-input"), "changed").map_err(|error| error.to_string())?;
    assert!(choose_root(root, &head, Some(root)).is_err());
    Ok(())
}

pub(super) fn retained_fixture(receipt: &Value) -> Result<(Value, Vec<(String, Vec<u8>)>), String> {
    let root = std::env::temp_dir().join("test-only-retained-cargo-producer");
    let target = serde_json::json!({"name": "fixture_package", "kind": ["lib"], "src_path": root.join("src/lib.rs"), "test": true});
    let profile = serde_json::json!({"opt_level": "0", "debuginfo": 2, "debug_assertions": true, "overflow_checks": true, "test": true});
    let executable = root.join("target/debug/deps/fixture_package");
    let record = serde_json::json!({"reason": "compiler-artifact", "package_id": "fixture-package-id", "manifest_path": root.join("Cargo.toml"), "target": target,
        "profile": profile, "features": [], "executable": executable, "fresh": true});
    let stdout = format!(
        "{}\n{}\nfixture::discriminator: test\n",
        record,
        serde_json::json!({"reason": "build-finished", "success": true})
    );
    let source_parent = receipt["source_parent"]
        .as_str()
        .ok_or("fixture lacks source")?
        .to_string();
    let tree = receipt["reviewed_tree"]
        .as_str()
        .ok_or("fixture lacks tree")?
        .to_string();
    let inputs = Inputs {
        root: root.clone(),
        head: receipt["materialization"]["disposable_commit"]
            .as_str()
            .ok_or("fixture lacks materialized commit")?
            .to_string(),
        tree: tree.clone(),
        lock_sha256: "0".repeat(64),
        cargo_sha256: "1".repeat(64),
        rustc_sha256: "2".repeat(64),
        config_sha256: BTreeMap::new(),
        environment_sha256: "3".repeat(64),
    };
    let mut references = Vec::new();
    let mut files = Vec::new();
    let mut commands = Vec::new();
    for line in 1..=11 {
        let command = "cargo test -p fixture_package --lib discriminator".to_string();
        commands.push((line, command.clone()));
        let stdout_path = format!("file-policy-test-{line:02}.stdout.log");
        let stderr_path = format!("file-policy-test-{line:02}.stderr.log");
        files.push((
            format!("commands/{stdout_path}"),
            stdout.as_bytes().to_vec(),
        ));
        files.push((format!("commands/{stderr_path}"), Vec::new()));
        references.push(Reference {
            line,
            actual_args: actual_args(&command),
            command,
            selected_tests: 1,
            exit_code: 0,
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
            duration_ms: 0,
            stdout_path,
            stdout_sha256: sha(stdout.as_bytes()),
            stderr_path,
            stderr_sha256: sha(b""),
            compiler_graph_sha256: json_sha(&vec![record.clone()])?,
            artifacts: vec![Artifact {
                compiler_record: record.clone(),
                executable: executable.clone(),
                executable_sha256: "4".repeat(64),
                depfile: executable.with_extension("d"),
                depfile_sha256: "5".repeat(64),
            }],
        });
    }
    let report = Report {
        schema: SCHEMA.to_string(),
        mode: "reviewed_tree_worktree".to_string(),
        source_parent,
        subject_tree: tree,
        checker_sha256: receipt["trusted_checker"]["executable_sha256"]
            .as_str()
            .ok_or("fixture lacks checker")?
            .to_string(),
        references_sha256: json_sha(&commands)?,
        inputs: inputs.clone(),
        subject_inputs: inputs,
        cargo_version: "test-only Cargo fixture".to_string(),
        rustc_version: "test-only rustc fixture".to_string(),
        metadata: serde_json::json!({"packages": [{"id": "fixture-package-id", "name": "fixture_package", "manifest_path": root.join("Cargo.toml"), "features": {}, "targets": [target]}], "target_directory": root.join("target")}),
        test_profile: profile,
        references,
        complete: true,
        failure_reason: None,
        failed_invocation: None,
    };
    let bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    let summary = producer_summary(&report, &bytes);
    files.push(("commands/file-policy-test-producer.json".to_string(), bytes));
    Ok((summary, files))
}

#[test]
fn indexed_producer_contract_rejects_malformed_missing_and_rebound_evidence() -> Result<(), String>
{
    let receipt = serde_json::json!({"source_parent": "a".repeat(40), "reviewed_tree": "b".repeat(40), "materialization": {"disposable_commit": "c".repeat(40)}, "trusted_checker": {"executable_sha256": "d".repeat(64)}});
    let (summary, files) = retained_fixture(&receipt)?;
    let files = files.into_iter().collect::<BTreeMap<_, _>>();
    let bytes = files
        .get("commands/file-policy-test-producer.json")
        .ok_or("fixture omitted producer")?;
    let read = |path: &str| {
        files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("missing indexed {path}"))
    };
    verify_retained(&summary, bytes, read)?;
    assert!(verify_retained(&summary, b"indexed garbage", read).is_err());
    let original: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    for (pointer, replacement) in [
        ("/complete", serde_json::json!(false)),
        ("/references/0/selected_tests", serde_json::json!(0)),
        (
            "/references/0/artifacts/0/compiler_record/features",
            serde_json::json!(["invented-feature"]),
        ),
        (
            "/references/0/artifacts/0/compiler_record/profile/opt_level",
            serde_json::json!("3"),
        ),
        (
            "/inputs/root",
            serde_json::json!(std::env::temp_dir().join("another-producer")),
        ),
    ] {
        let mut altered = original.clone();
        *altered
            .pointer_mut(pointer)
            .ok_or("missing retained mutation pointer")? = replacement;
        let bytes = serde_json::to_vec(&altered).map_err(|error| error.to_string())?;
        let mut rebound = summary.clone();
        rebound["sha256"] = serde_json::json!(sha(&bytes));
        assert!(
            verify_retained(&rebound, &bytes, read).is_err(),
            "accepted retained {pointer}"
        );
    }
    let mut missing = original;
    missing["references"]
        .as_array_mut()
        .ok_or("missing rows")?
        .pop();
    let bytes = serde_json::to_vec(&missing).map_err(|error| error.to_string())?;
    let mut rebound = summary.clone();
    rebound["sha256"] = serde_json::json!(sha(&bytes));
    assert!(verify_retained(&rebound, &bytes, read).is_err());
    assert!(
        verify_retained(
            &summary,
            files
                .get("commands/file-policy-test-producer.json")
                .ok_or("missing sidecar")?,
            |_| Err("missing indexed log".to_string())
        )
        .is_err()
    );
    Ok(())
}
