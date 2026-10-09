use super::*;
use crate::python_judged_panel::parse_json_without_duplicate_keys;

const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
const SHA_D: &str = "dddddddddddddddddddddddddddddddddddddddd";
const SHA_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const SHA_F: &str = "ffffffffffffffffffffffffffffffffffffffff";
const SHA_0: &str = "1010101010101010101010101010101010101010";
const SHA_1: &str = "1111111111111111111111111111111111111111";
const DIGEST_ONE: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const DIGEST_TWO: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const SOURCE_SHA: &str = "9999aaaabbbbccccddddeeeeffff000011112222";
const BINARY_BYTES_V1: &str = "test-binary-bytes-v1";
const BINARY_BYTES_V2: &str = "test-binary-bytes-v2";

/// The per-subject tree identity, recorded identically by the sandbox
/// manifest and the candidate rows (the validator binds the two copies).
/// With every per-subject identity bound, the currentness happy path is
/// genuinely `current`; a subject with an unbound identity is disclosed
/// unverifiable and can never read `current`.
fn tree_digest_for(id: &str) -> String {
    sha256_hex(format!("tree-for-{id}\n").as_bytes())
}

/// A data-driven eight-subject manifest sandbox whose synthetic diffs
/// exist as real files under `<dir>/diffs/<id>.diff`, so the currentness
/// input recomputation reads real bytes.
struct TestSandbox {
    dir: PathBuf,
}

impl TestSandbox {
    fn new(label: &str) -> Result<Self, String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-evalsweep-report-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("diffs"))
            .map_err(|error| format!("create sandbox: {error}"))?;
        Ok(Self { dir })
    }

    fn path(&self, relative: &str) -> PathBuf {
        let mut path = self.dir.clone();
        for component in relative.split('/') {
            path.push(component);
        }
        path
    }

    fn manifest_value(&self) -> Value {
        let ids = [
            ("alpha", SHA_A, "pytest_library", "MIT"),
            ("bravo", SHA_B, "unittest_library", "Apache-2.0"),
            ("charlie", SHA_C, "click_typer", "BSD-3-Clause"),
            ("delta", SHA_D, "pytest_library", "MIT"),
            ("echo", SHA_E, "flask_web", "MIT"),
            ("foxtrot", SHA_F, "fastapi_web", "MIT OR Apache-2.0"),
            ("golf", SHA_0, "pytest_library", "ISC"),
            ("hotel", SHA_1, "pytest_library", "BSD-2-Clause"),
        ];
        let mut repos = Vec::new();
        for (id, sha, shape, license) in ids {
            repos.push(json!({
                "id": id,
                "url": format!("https://example.com/{id}"),
                "sha": sha,
                "license": license,
                "shape": shape,
                "tree_digest": tree_digest_for(id),
                "synthetic_diff": format!("diffs/{id}.diff"),
            }));
        }
        json!({
            "schema_version": "0.1",
            "kind": "python_eval_sweep_manifest",
            "spec": SPEC,
            "tier": TIER,
            "description": "report-route sandbox manifest",
            "repos": repos,
        })
    }

    /// Writes the manifest and per-subject diff files; returns the
    /// manifest digest over the exact file bytes.
    fn write_manifest(&self) -> Result<String, String> {
        let value = self.manifest_value();
        let text = serde_json::to_string_pretty(&value)
            .map_err(|error| format!("serialize manifest: {error}"))?;
        let bytes = text.as_bytes();
        std::fs::write(self.path("manifest.json"), bytes)
            .map_err(|error| format!("write manifest: {error}"))?;
        for id in [
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
        ] {
            std::fs::write(
                self.path(&format!("diffs/{id}.diff")),
                format!("diff-for-{id}\n"),
            )
            .map_err(|error| format!("write diff: {error}"))?;
        }
        Ok(sha256_hex(bytes))
    }

    fn manifest_path_string(&self) -> String {
        self.path("manifest.json").to_string_lossy().to_string()
    }

    fn state_dir(&self) -> String {
        self.path("accepted").to_string_lossy().to_string()
    }

    fn dispositions_path(&self) -> Result<String, String> {
        let path = self.path("dispositions.json");
        write_json(&path, &dispositions_value())?;
        Ok(path.to_string_lossy().to_string())
    }
}

/// The test binary whose bytes hash to `binary_digest_v1()`; a second,
/// distinct binary is `binary_bytes_v2`.
fn binary_digest_v1() -> String {
    sha256_hex(BINARY_BYTES_V1.as_bytes())
}

/// A valid schema-0.3 candidate for the sandbox manifest. Statuses cover
/// the full vocabulary (row 0 complete, then partial/parse-failed/
/// timed-out/crashed/unsupported/tempfail/stale); the summary is derived
/// honestly exactly as the validator requires. `runtime_base` lets a test
/// build a second, distinct-but-valid candidate.
fn candidate_value(
    sandbox: &TestSandbox,
    manifest_sha: &str,
    binary_digest: &str,
    runtime_base: u64,
) -> Value {
    let statuses = [
        "complete",
        "partial",
        "parse-failed",
        "timed-out",
        "crashed",
        "unsupported",
        "tempfail",
        "stale",
    ];
    let executions = [
        "executed",
        "executed",
        "executed",
        "timed-out",
        "failed",
        "not-executed",
        "not-executed",
        "unknown",
    ];
    let manifest = sandbox.manifest_value();
    let repos = manifest.get("repos").and_then(Value::as_array);
    let mut rows = Vec::new();
    for (index, repo) in repos.into_iter().flatten().enumerate() {
        let id = repo.get("id").and_then(Value::as_str).unwrap_or_default();
        let sha = repo.get("sha").and_then(Value::as_str).unwrap_or_default();
        let license = repo
            .get("license")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = statuses[index];
        let execution = executions[index];
        let ran = matches!(
            status,
            "complete" | "partial" | "parse-failed" | "timed-out" | "crashed"
        );
        let mut row = json!({
            "id": id,
            "status": status,
            "repository": {
                "url": format!("https://example.com/{id}"),
                "sha": sha,
            },
            "license": license,
            "selected_root": format!("subjects/{id}"),
            "layout": ["pytest_library"],
            "tree_digest": tree_digest_for(id),
            "binary": {
                "digest": binary_digest,
                "version": "ripr 0.11.0",
                "features": ["python"],
                "build_profile": "debug",
            },
            "config": {
                "profile": "default",
                "input": format!("diffs/{id}.diff"),
            },
            "input_digest": sha256_hex(format!("diff-for-{id}\n").as_bytes()),
            "materialization": if ran { "materialized" } else { "absent" },
            "detection": if ran { "detected" } else { "absent" },
            "corpus_selection": {
                "state": if ran { "selected" } else { "absent" },
                "source_files": 10,
                "test_files": 5,
                "generated_files": 0,
                "vendor_files": 0,
            },
            "execution": execution,
            "runtime_ms": runtime_base + index as u64 * 100,
            "classification_counts": {
                "exposed": 0,
                "weakly_exposed": if status == "complete" { 1 } else { 0 },
                "reachable_unrevealed": 0,
                "no_static_path": 0,
                "infection_unknown": 0,
                "propagation_unknown": 0,
                "static_unknown": if status == "parse-failed" { 1 } else { 0 },
            },
            "alignment_counts": {
                "direct": 0,
                "alias": 0,
                "changed_sink_token": 0,
                "orthogonal": if status == "complete" { 1 } else { 0 },
                "unknown": if status == "partial" { 1 } else { 0 },
                "absent": if matches!(status, "parse-failed" | "crashed" | "timed-out") { 1 } else { 0 },
            },
        });
        if ran && let Some(entry) = row.as_object_mut() {
            entry.insert(
                "digests".to_string(),
                json!({
                    "raw": DIGEST_ONE,
                    "output": DIGEST_TWO,
                    "evidence": DIGEST_ONE,
                }),
            );
        }
        if status == "complete"
            && let Some(entry) = row.as_object_mut()
        {
            entry.insert(
                "repeat".to_string(),
                json!({
                    "comparable_with": "pass-1",
                    "gap_ids_stable": true,
                    "unstable_gap_ids": [],
                }),
            );
        }
        if !ran && let Some(entry) = row.as_object_mut() {
            entry.insert(
                    "classification_counts".to_string(),
                    json!({"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                );
            entry.insert(
                    "alignment_counts".to_string(),
                    json!({"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
                );
            entry.remove("runtime_ms");
        }
        rows.push(row);
    }
    // Derived aggregates over the five run rows (complete/partial/
    // parse-failed/timed-out/crashed): one crash, one parse failure, one
    // timeout; runtimes base..base+400 (min base, median base+200, max
    // base+400, total base*5+1000); classification weakly_exposed=1 +
    // static_unknown=1; alignment orthogonal=1, unknown=1, absent=3.
    // Only the complete row carries repeat evidence, so the stability
    // aggregates are omitted (under-evidenced — the producer shape).
    json!({
        "schema_version": "0.3",
        "kind": "python_eval_sweep_report",
        "spec": SPEC,
        "tier": TIER,
        "manifest_digest": manifest_sha,
        "ripr": {
            "source_sha": SOURCE_SHA,
            "tree_digest": DIGEST_ONE,
            "binary_digest": binary_digest,
            "version": "ripr 0.11.0",
            "features": ["python"],
            "build_profile": "debug",
        },
        "summary": {
            "repos_total": 8,
            "repos_run": 5,
            "repos_skipped": 0,
            "repos_clone_failed": 1,
            "crash_count": 1,
            "crash_rate": 0.2,
            "parse_failure_count": 1,
            "parse_failure_rate": 0.2,
            "timed_out_count": 1,
            "runtime_ms_min": runtime_base,
            "runtime_ms_median": runtime_base + 200,
            "runtime_ms_max": runtime_base + 400,
            "runtime_ms_total": runtime_base * 5 + 1000,
            "classification_counts": {
                "exposed": 0,
                "weakly_exposed": 1,
                "reachable_unrevealed": 0,
                "no_static_path": 0,
                "infection_unknown": 0,
                "propagation_unknown": 0,
                "static_unknown": 1,
            },
            "alignment_counts": {
                "direct": 0,
                "alias": 0,
                "changed_sink_token": 0,
                "orthogonal": 1,
                "unknown": 1,
                "absent": 3,
            },
            "gate_status": "review",
            "gate_reason": "1 crash over 5 run rows",
        },
        "repos": rows,
    })
}

/// Terminal dispositions for the seven non-complete rows of the standard
/// sandbox candidate.
fn dispositions_value() -> Value {
    let entries = [
        ("bravo", "dispositioned-current"),
        ("charlie", "reproduced-current"),
        ("delta", "historical-not-reproduced"),
        ("echo", "reproduced-current"),
        ("foxtrot", "unsupported-input"),
        ("golf", "infrastructure-tempfail"),
        ("hotel", "upstream-pin-unavailable"),
    ];
    let mut dispositions = Vec::new();
    for (id, disposition) in entries {
        dispositions.push(json!({
            "id": id,
            "disposition": disposition,
            "evidence_ref": format!("digests.evidence for {id}"),
            "owner": "language-adapter",
            "recovery_route": "rerun the managed refresh after the fix lands",
        }));
    }
    json!({
        "schema_version": "0.1",
        "kind": DISPOSITIONS_KIND,
        "spec": SPEC,
        "dispositions": dispositions,
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let text =
        serde_json::to_string_pretty(value).map_err(|error| format!("serialize: {error}"))?;
    std::fs::write(path, format!("{text}\n")).map_err(|error| format!("write: {error}"))
}

fn args_for(sandbox: &TestSandbox, extra: &[&str]) -> Vec<String> {
    let mut args = vec![
        "--candidate".to_string(),
        sandbox.path("candidate.json").to_string_lossy().to_string(),
        "--manifest".to_string(),
        sandbox.manifest_path_string(),
        "--state-dir".to_string(),
        sandbox.state_dir(),
    ];
    args.extend(extra.iter().map(|text| text.to_string()));
    args
}

/// Builds the sandbox, writes the manifest + a valid candidate, and
/// returns the sandbox with the parsed report args (dry run by default).
fn prepared_args(label: &str, extra: &[&str]) -> Result<(TestSandbox, Vec<String>), String> {
    let sandbox = TestSandbox::new(label)?;
    let manifest_sha = sandbox.write_manifest()?;
    let candidate = candidate_value(&sandbox, &manifest_sha, &binary_digest_v1(), 100);
    write_json(&sandbox.path("candidate.json"), &candidate)?;
    let args = args_for(&sandbox, extra);
    Ok((sandbox, args))
}

/// Parses a written file as strict JSON.
fn read_strict(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("read {}: {error}", path.to_string_lossy()))?;
    parse_json_without_duplicate_keys(&text)
        .map_err(|error| format!("parse {}: {error}", path.to_string_lossy()))
}

fn expect_fail(result: Result<(), String>, needle: &str) -> Result<(), String> {
    expect_fail_all(result, &[needle])
}

/// Asserts a fail-closed result: Err, mentioning EVERY needle and the
/// rerun command.
fn expect_fail_all(result: Result<(), String>, needles: &[&str]) -> Result<(), String> {
    let error = match result {
        Ok(()) => {
            return Err(format!(
                "expected failure containing `{needles:?}`, got success"
            ));
        }
        Err(error) => error,
    };
    for needle in needles {
        if !error.contains(needle) {
            return Err(format!("failure `{error}` must mention `{needle}`"));
        }
    }
    // Refusals the shared validator raises carry its own rerun command
    // (`eval-sweep check`); the report route's own refusals carry this
    // command. Either is acceptable: both name a deterministic rerun.
    if !error.contains("rerun: cargo xtask eval-sweep") {
        return Err(format!("failure `{error}` must carry a rerun command"));
    }
    Ok(())
}

fn with_dispositions(args: &[String], dispositions: &str) -> Vec<String> {
    args.iter()
        .map(|arg| arg.replace("DISPOSITIONS", dispositions))
        .collect()
}

fn dry_run_dispositions(sandbox: &TestSandbox, args: &[String]) -> Result<Vec<String>, String> {
    let dispositions = sandbox.dispositions_path()?;
    Ok(with_dispositions(args, &dispositions))
}

/// Reads the accepted receipt the sandbox pointer names.
fn accepted_receipt(sandbox: &TestSandbox) -> Result<(Value, PathBuf), String> {
    let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
    let receipt_file = pointer
        .get("receipt_file")
        .and_then(Value::as_str)
        .ok_or_else(|| "pointer must name its receipt".to_string())?
        .to_string();
    let receipt_path = sandbox.path(&format!("accepted/{receipt_file}"));
    let receipt = read_strict(&receipt_path)?;
    Ok((receipt, receipt_path))
}

/// Currentness args with matching identity inputs (the source sha and
/// binary the candidate bound).
fn currentness_args(sandbox: &TestSandbox, binary_path: &Path, extra: &[&str]) -> Vec<String> {
    let mut args = vec![
        "--check-currentness".to_string(),
        "--manifest".to_string(),
        sandbox.manifest_path_string(),
        "--state-dir".to_string(),
        sandbox.state_dir(),
        "--ripr-source-sha".to_string(),
        SOURCE_SHA.to_string(),
        "--ripr-bin".to_string(),
        binary_path.to_string_lossy().to_string(),
    ];
    args.extend(extra.iter().map(|text| text.to_string()));
    args
}

fn write_binary_v1(sandbox: &TestSandbox) -> Result<PathBuf, String> {
    let path = sandbox.path("ripr-v1.bin");
    std::fs::write(&path, BINARY_BYTES_V1).map_err(|error| format!("write binary: {error}"))?;
    Ok(path)
}

fn write_binary_v2(sandbox: &TestSandbox) -> Result<PathBuf, String> {
    let path = sandbox.path("ripr-v2.bin");
    std::fs::write(&path, BINARY_BYTES_V2).map_err(|error| format!("write binary: {error}"))?;
    Ok(path)
}

/// Prepares an accepted sandbox: manifest + candidate + dispositions,
/// accepted with pointer. Returns the sandbox.
fn accepted_sandbox(label: &str) -> Result<TestSandbox, String> {
    let (sandbox, args) = prepared_args(label, &["--dispositions", "DISPOSITIONS", "--accept"])?;
    let dispositions = sandbox.dispositions_path()?;
    run_report(&with_dispositions(&args, &dispositions))?;
    Ok(sandbox)
}

// -- happy paths ---------------------------------------------------------

#[test]
fn bare_command_requires_a_mode() -> Result<(), String> {
    let error = match run_report(&[]) {
        Ok(()) => return Err("bare invocation must fail with usage".to_string()),
        Err(error) => error,
    };
    assert!(
        error.contains("requires --candidate") && error.contains("--check-currentness"),
        "bare invocation must explain its modes: {error}"
    );
    Ok(())
}

#[test]
fn valid_candidate_dry_run_renders_agreeing_json_and_markdown() -> Result<(), String> {
    let (sandbox, args) = prepared_args("dry-run", &["--dispositions", "DISPOSITIONS"])?;
    let args = dry_run_dispositions(&sandbox, &args)?;
    run_report(&args)?;

    let report_text = std::fs::read_to_string(crate::reports_dir().join(REPORT_JSON))
        .map_err(|error| format!("read report: {error}"))?;
    let receipt = parse_json_without_duplicate_keys(&report_text)
        .map_err(|error| format!("parse report: {error}"))?;
    let markdown = std::fs::read_to_string(crate::reports_dir().join(REPORT_MD))
        .map_err(|error| format!("read markdown: {error}"))?;

    // JSON and Markdown derive from the same validated rows: same
    // subjects, same counts.
    let subjects = receipt
        .get("subjects")
        .and_then(Value::as_array)
        .ok_or_else(|| "receipt must carry subjects".to_string())?;
    assert_eq!(subjects.len(), 8, "all eight subjects appear exactly once");
    let mut ids_in_json = Vec::new();
    for subject in subjects {
        ids_in_json.push(
            subject
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "subject id".to_string())?
                .to_string(),
        );
    }
    for id in [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
    ] {
        assert!(ids_in_json.contains(&id.to_string()), "{id} must appear");
        assert!(markdown.contains(id), "markdown must name {id}");
    }
    let counts = receipt
        .get("counts")
        .and_then(Value::as_object)
        .ok_or_else(|| "receipt counts".to_string())?;
    for (name, count) in counts {
        let numerator = count.get("numerator").and_then(Value::as_u64);
        let denominator = count.get("denominator").and_then(Value::as_u64);
        assert!(
            numerator.is_some() && denominator.is_some(),
            "count `{name}` must carry numerator and denominator"
        );
        let numerator = numerator.unwrap_or(0);
        assert!(
            markdown.contains(&numerator.to_string()),
            "markdown must agree with count {name}={numerator}"
        );
    }
    let numerator = |name: &str| -> Result<u64, String> {
        counts
            .get(name)
            .and_then(|count| count.get("numerator"))
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("count {name}"))
    };
    assert_eq!(numerator("selected")?, 8);
    assert_eq!(numerator("run")?, 5);
    // Dispositions projected on non-complete subjects; none on complete.
    let disposed = subjects
        .iter()
        .filter(|subject| subject.get("disposition").is_some())
        .count();
    assert_eq!(
        disposed, 7,
        "every non-complete subject carries a disposition"
    );
    let alpha = subjects
        .iter()
        .find(|subject| subject.get("id").and_then(Value::as_str) == Some("alpha"))
        .ok_or_else(|| "alpha row".to_string())?;
    assert!(
        alpha.get("disposition").is_none(),
        "complete rows carry no disposition"
    );
    // Non-claims are embedded in the artifact itself.
    let non_claims = receipt
        .get("non_claims")
        .and_then(Value::as_array)
        .ok_or_else(|| "non_claims".to_string())?;
    assert!(!non_claims.is_empty());
    Ok(())
}

#[test]
fn accept_appends_immutably_and_moves_pointer() -> Result<(), String> {
    let sandbox = TestSandbox::new("accept-immut")?;
    let manifest_sha = sandbox.write_manifest()?;
    let dispositions = sandbox.dispositions_path()?;

    let candidate_a = candidate_value(&sandbox, &manifest_sha, &binary_digest_v1(), 100);
    write_json(&sandbox.path("candidate.json"), &candidate_a)?;
    run_report(&with_dispositions(
        &args_for(&sandbox, &["--dispositions", "DISPOSITIONS", "--accept"]),
        &dispositions,
    ))?;

    let pointer_path = sandbox.path("accepted/current.json");
    let pointer_one = read_strict(&pointer_path)?;
    let receipt_file_one = pointer_one
        .get("receipt_file")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt_file".to_string())?
        .to_string();
    let receipt_bytes_one = std::fs::read(sandbox.path(&format!("accepted/{receipt_file_one}")))
        .map_err(|error| format!("read receipt one: {error}"))?;

    // A second, distinct-but-valid candidate (different runtimes),
    // accepted afterwards: adds a receipt without mutating the first; the
    // pointer moves to the newest.
    let candidate_b = candidate_value(&sandbox, &manifest_sha, &binary_digest_v1(), 500);
    write_json(&sandbox.path("candidate.json"), &candidate_b)?;
    run_report(&with_dispositions(
        &args_for(&sandbox, &["--dispositions", "DISPOSITIONS", "--accept"]),
        &dispositions,
    ))?;

    let pointer_two = read_strict(&pointer_path)?;
    let receipt_file_two = pointer_two
        .get("receipt_file")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt_file".to_string())?
        .to_string();
    assert_ne!(
        receipt_file_one, receipt_file_two,
        "the second accept must add a new content-addressed receipt"
    );
    let receipt_bytes_two = std::fs::read(sandbox.path(&format!("accepted/{receipt_file_two}")))
        .map_err(|error| format!("read receipt two: {error}"))?;
    assert_eq!(
        receipt_bytes_one,
        std::fs::read(sandbox.path(&format!("accepted/{receipt_file_one}")))
            .map_err(|error| format!("re-read receipt one: {error}"))?,
        "the first accepted receipt must be byte-identical after the second accept"
    );

    // The pointer identifies exactly the newest accepted receipt.
    let receipt_sha = sha256_hex(&receipt_bytes_two);
    assert_eq!(
        pointer_two.get("receipt_sha256").and_then(Value::as_str),
        Some(receipt_sha.as_str()),
    );
    assert_eq!(
        pointer_two.get("receipt_file").and_then(Value::as_str),
        Some(receipt_file_two.as_str()),
    );

    // Re-accepting the SAME candidate is an idempotent no-op: no third
    // receipt, artifacts untouched, pointer unchanged.
    run_report(&with_dispositions(
        &args_for(&sandbox, &["--dispositions", "DISPOSITIONS", "--accept"]),
        &dispositions,
    ))?;
    let receipts = std::fs::read_dir(sandbox.path("accepted/receipts"))
        .map_err(|error| format!("read receipts dir: {error}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.ends_with(".json") && !name.ends_with(".candidate.json")
        })
        .count();
    assert_eq!(receipts, 2, "idempotent re-accept must not add a receipt");
    Ok(())
}

#[test]
fn pointer_contains_no_totals_only_identity() -> Result<(), String> {
    let (sandbox, args) = prepared_args(
        "pointer-shape",
        &[
            "--dispositions",
            "DISPOSITIONS",
            "--accept",
            "--as-of",
            "2026-09-10T00:00:00Z",
        ],
    )?;
    let dispositions = sandbox.dispositions_path()?;
    run_report(&with_dispositions(&args, &dispositions))?;

    let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
    let mut keys: Vec<&str> = pointer
        .as_object()
        .ok_or_else(|| "pointer object".to_string())?
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "as_of",
            "command_contract_version",
            "kind",
            "manifest_sha256",
            "receipt_file",
            "receipt_sha256",
            "ripr",
            "schema_version",
            "spec",
            "subjects"
        ],
        "the pointer's field set is exactly identity fields"
    );
    // No total/rate-shaped field anywhere in the pointer tree.
    let lowered = serde_json::to_string(&pointer)
        .map_err(|error| format!("serialize pointer: {error}"))?
        .to_ascii_lowercase();
    for banned in [
        "\"total",
        "count\"",
        "\"rate",
        "numerator",
        "denominator",
        "crash",
        "runtime_ms",
    ] {
        assert!(
            !lowered.contains(banned),
            "pointer must not carry totals/rates; found `{banned}`"
        );
    }
    Ok(())
}

// -- candidate validation failures ---------------------------------------

#[test]
fn hand_edited_candidate_totals_fail() -> Result<(), String> {
    let (sandbox, args) = prepared_args("hand-edited", &["--dispositions", "DISPOSITIONS"])?;
    let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
    if let Some(summary) = candidate.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("repos_total".to_string(), json!(9));
    }
    write_json(&sandbox.path("candidate.json"), &candidate)?;
    let dispositions = sandbox.dispositions_path()?;
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "hand-edited aggregate",
    )
}

#[test]
fn missing_and_duplicate_subjects_fail() -> Result<(), String> {
    let (sandbox, args) = prepared_args("missing-subject", &["--dispositions", "DISPOSITIONS"])?;
    let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
    if let Some(rows) = candidate.get_mut("repos").and_then(Value::as_array_mut) {
        rows.pop();
    }
    write_json(&sandbox.path("candidate.json"), &candidate)?;
    let dispositions = sandbox.dispositions_path()?;
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "changed denominator",
    )?;

    let (sandbox, args) = prepared_args("dup-subject", &["--dispositions", "DISPOSITIONS"])?;
    let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
    if let Some(rows) = candidate.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(first) = rows.first().cloned()
    {
        rows.push(first);
    }
    write_json(&sandbox.path("candidate.json"), &candidate)?;
    let dispositions = sandbox.dispositions_path()?;
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "duplicate subject row",
    )
}

#[test]
fn non_complete_row_without_disposition_fails() -> Result<(), String> {
    // No --dispositions at all: the seven non-complete rows are
    // unexplained, so the report refuses.
    let (_sandbox, args) = prepared_args("no-dispositions", &[])?;
    expect_fail(run_report(&args), "carry no terminal disposition")
}

#[test]
fn disposition_gaps_fail_closed() -> Result<(), String> {
    // Missing one disposition (hotel's): its non-complete row is
    // unexplained.
    let (sandbox, args) = prepared_args("disp-gap", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(entries) = value.get_mut("dispositions").and_then(Value::as_array_mut) {
        entries.retain(|entry| entry.get("id").and_then(Value::as_str) != Some("hotel"));
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "hotel",
    )?;

    // A disposition without its owner fails (every owned disposition type
    // is actionable by definition).
    let (sandbox, args) = prepared_args("disp-owner", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(first) = value
        .get_mut("dispositions")
        .and_then(Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(|entry| entry.as_object_mut())
    {
        first.remove("owner");
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "requires a non-empty owner",
    )?;

    // The same for the recovery route.
    let (sandbox, args) = prepared_args("disp-route", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(first) = value
        .get_mut("dispositions")
        .and_then(Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(|entry| entry.as_object_mut())
    {
        first.remove("recovery_route");
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "requires a non-empty recovery_route",
    )?;

    // An oversized owner defeats the bounded-artifact contract exactly
    // like an oversized note: every bounded-artifact field is capped.
    let (sandbox, args) = prepared_args("disp-huge-owner", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(first) = value
        .get_mut("dispositions")
        .and_then(Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(|entry| entry.as_object_mut())
    {
        first.insert("owner".to_string(), json!("x".repeat(NOTE_MAX_CHARS + 1)));
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "exceeds the 512-character bound",
    )?;

    // A disposition for a COMPLETE row contradicts the run.
    let (sandbox, args) = prepared_args("disp-complete", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(entries) = value.get_mut("dispositions").and_then(Value::as_array_mut) {
        entries.push(json!({
            "id": "alpha",
            "disposition": "dispositioned-current",
            "evidence_ref": "none",
            "owner": "language-adapter",
            "recovery_route": "none needed",
        }));
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "a terminal disposition contradicts a complete run",
    )?;

    // Duplicate ids fail.
    let (sandbox, args) = prepared_args("disp-dup", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(entries) = value.get_mut("dispositions").and_then(Value::as_array_mut)
        && let Some(first) = entries.first().cloned()
    {
        entries.push(first);
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "duplicate disposition",
    )?;

    // Unknown ids fail.
    let (sandbox, args) = prepared_args("disp-unknown", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(entries) = value.get_mut("dispositions").and_then(Value::as_array_mut) {
        entries.push(json!({
            "id": "outsider",
            "disposition": "dispositioned-current",
            "evidence_ref": "none",
            "owner": "language-adapter",
            "recovery_route": "none",
        }));
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "outside the candidate denominator",
    )
}

#[test]
fn disposition_vocabulary_and_hygiene_fail_closed() -> Result<(), String> {
    // Unknown disposition type.
    let (sandbox, args) = prepared_args("disp-vocab", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(first) = value
        .get_mut("dispositions")
        .and_then(Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(|entry| entry.as_object_mut())
    {
        first.insert("disposition".to_string(), json!("mostly_fine"));
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "unknown disposition",
    )?;

    // An absolute host path in a disposition note fails the artifact
    // hygiene scan (the drive letter is assembled at runtime so this
    // source file never contains a local absolute path).
    let (sandbox, args) = prepared_args("disp-abs", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    let absolute = format!("{}:\\Users\\agent\\notes.txt", 'C');
    if let Some(first) = value
        .get_mut("dispositions")
        .and_then(Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(|entry| entry.as_object_mut())
    {
        first.insert("notes".to_string(), json!(absolute));
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "drive-letter absolute path",
    )?;

    // A secret-shaped token fails the hygiene scan.
    let (sandbox, args) = prepared_args("disp-secret", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(first) = value
        .get_mut("dispositions")
        .and_then(Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(|entry| entry.as_object_mut())
    {
        first.insert(
            "notes".to_string(),
            json!("retry with api_key=hunter2 or it fails"),
        );
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "secret-shaped token",
    )?;

    // An oversized note is an unbounded log and fails.
    let (sandbox, args) = prepared_args("disp-huge", &["--dispositions", "DISPOSITIONS"])?;
    let mut value = dispositions_value();
    if let Some(first) = value
        .get_mut("dispositions")
        .and_then(Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(|entry| entry.as_object_mut())
    {
        first.insert("notes".to_string(), json!("x".repeat(NOTE_MAX_CHARS + 1)));
    }
    let dispositions = {
        let path = sandbox.path("dispositions.json");
        write_json(&path, &value)?;
        path.to_string_lossy().to_string()
    };
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "character bound",
    )
}

#[test]
fn historical_0_2_candidate_refuses_acceptance_with_typed_note() -> Result<(), String> {
    let sandbox = TestSandbox::new("historical")?;
    sandbox.write_manifest()?;
    // A 0.2-shaped receipt (historical retained shape); the route refuses
    // on the schema before any validation is spent on it.
    let rows: Vec<Value> = sandbox
            .manifest_value()
            .get("repos")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|repo| {
                json!({
                    "id": repo.get("id").cloned().unwrap_or_default(),
                    "sha": repo.get("sha").cloned().unwrap_or_default(),
                    "shape": "pytest_library",
                    "outcome": "ok",
                    "runtime_ms": 100,
                    "gap_ids": [],
                    "gap_ids_stable": true,
                    "unstable_gap_ids": [],
                    "stderr_excerpt": "",
                    "classification_counts": {"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0},
                    "alignment_counts": {"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0},
                })
            })
            .collect();
    let historical = json!({
        "schema_version": "0.2",
        "kind": "python_eval_sweep_report",
        "spec": SPEC,
        "tier": TIER,
        "summary": {
            "repos_total": 8,
            "repos_run": 8,
            "repos_skipped": 0,
            "repos_clone_failed": 0,
            "crash_count": 0,
            "crash_rate": 0.0,
            "parse_failure_count": 0,
            "parse_failure_rate": 0.0,
            "timed_out_count": 0,
            "runtime_ms_min": 100,
            "runtime_ms_median": 100,
            "runtime_ms_max": 100,
            "runtime_ms_total": 800,
            "gap_id_stable_count": 8,
            "gap_id_unstable_count": 0,
            "gap_id_stability_rate": 1.0,
            "classification_counts": {"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0},
            "alignment_counts": {"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0},
            "gate_status": "pass",
            "gate_reason": "8 repo(s) analyzed",
        },
        "repos": rows,
    });
    write_json(&sandbox.path("candidate.json"), &historical)?;
    expect_fail(
        run_report(&args_for(&sandbox, &["--accept"])),
        "historical receipts remain valid retained artifacts",
    )?;
    // Nothing was accepted.
    assert!(
        !sandbox.path("accepted/current.json").exists(),
        "a refused candidate must not move the pointer"
    );
    Ok(())
}

// -- currentness ----------------------------------------------------------

#[test]
fn no_pointer_reports_not_run_and_exits_zero() -> Result<(), String> {
    let sandbox = TestSandbox::new("not-run")?;
    sandbox.write_manifest()?;
    let binary = write_binary_v1(&sandbox)?;
    run_report(&currentness_args(&sandbox, &binary, &[]))?;
    Ok(())
}

#[test]
fn currentness_is_current_with_matching_identities() -> Result<(), String> {
    let sandbox = accepted_sandbox("current-ok")?;
    let binary = write_binary_v1(&sandbox)?;
    run_report(&currentness_args(&sandbox, &binary, &[]))?;
    Ok(())
}

#[test]
fn binary_digest_change_flips_stale() -> Result<(), String> {
    let sandbox = accepted_sandbox("stale-binary")?;
    // Different binary bytes: the mechanical law flips the pointer stale.
    let binary = write_binary_v2(&sandbox)?;
    expect_fail_all(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        &["STALE", "analyzer binary moved"],
    )
}

#[test]
fn manifest_digest_change_flips_stale() -> Result<(), String> {
    let sandbox = accepted_sandbox("stale-manifest")?;
    let binary = write_binary_v1(&sandbox)?;
    // Rewrite the manifest (same semantics, different bytes): the bound
    // manifest digest no longer matches the file.
    let manifest_path = sandbox.path("manifest.json");
    let mut value = read_strict(&manifest_path)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "description".to_string(),
            json!("report-route sandbox manifest (edited)"),
        );
    }
    write_json(&manifest_path, &value)?;
    expect_fail(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        "accepted manifest changed",
    )
}

#[test]
fn moved_malformed_manifest_still_reaches_stale_with_both_reasons() -> Result<(), String> {
    // A changed manifest whose new bytes ALSO fail accepted-state
    // validation (an unknown top-level key) must reach the promised
    // `stale` verdict, never abort with a schema error: the digest
    // comparison runs on the raw bytes first, so the verdict carries BOTH
    // the digest-movement reason and the validation failure (named with
    // the offending key), and the gate still exits nonzero.
    let sandbox = accepted_sandbox("stale-malformed-manifest")?;
    let binary = write_binary_v1(&sandbox)?;
    let manifest_path = sandbox.path("manifest.json");
    let mut value = read_strict(&manifest_path)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("surprise_total".to_string(), json!(9));
    }
    write_json(&manifest_path, &value)?;
    expect_fail_all(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        &[
            "STALE",
            "accepted manifest changed",
            "accepted manifest failed validation",
            "surprise_total",
        ],
    )
}

#[test]
fn accepted_row_or_tree_change_flips_stale() -> Result<(), String> {
    let sandbox = accepted_sandbox("stale-row")?;
    let binary = write_binary_v1(&sandbox)?;
    // Edit one retained candidate row's tree identity post-accept: the
    // retained candidate bytes AND the bound row digest both move.
    let (accepted_receipt, _receipt_path) = accepted_receipt(&sandbox)?;
    let candidate_sha = accepted_receipt
        .get("candidate")
        .and_then(|candidate| candidate.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| "candidate binding".to_string())?
        .to_string();
    let candidate_path = sandbox.path(&format!("accepted/receipts/{candidate_sha}.candidate.json"));
    let mut candidate = read_strict(&candidate_path)?;
    if let Some(rows) = candidate.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(first) = rows.first_mut().and_then(|row| row.as_object_mut())
    {
        first.insert("tree_digest".to_string(), json!(DIGEST_TWO));
    }
    write_json(&candidate_path, &candidate)?;
    expect_fail_all(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        &[
            "STALE",
            "retained candidate changed",
            "accepted row bytes changed for subject `alpha`",
        ],
    )
}

#[test]
fn input_change_flips_stale() -> Result<(), String> {
    let sandbox = accepted_sandbox("stale-input")?;
    let binary = write_binary_v1(&sandbox)?;
    // The subject's synthetic diff bytes move: the bound input digest no
    // longer matches the recomputed current input.
    std::fs::write(sandbox.path("diffs/alpha.diff"), "diff-for-alpha (moved)\n")
        .map_err(|error| format!("write diff: {error}"))?;
    expect_fail_all(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        &["STALE", "input moved for subject `alpha`"],
    )
}

#[test]
fn pointer_config_edit_flips_stale() -> Result<(), String> {
    let sandbox = accepted_sandbox("stale-config")?;
    let binary = write_binary_v1(&sandbox)?;
    // Editing a bound identity copy INSIDE the pointer (here: config
    // profile) is a pointer-vs-receipt disagreement: stale.
    let pointer_path = sandbox.path("accepted/current.json");
    let mut pointer = read_strict(&pointer_path)?;
    if let Some(alpha) = pointer
        .get_mut("subjects")
        .and_then(Value::as_object_mut)
        .and_then(|subjects| subjects.get_mut("alpha"))
        .and_then(|entry| entry.as_object_mut())
    {
        alpha.insert("config_profile".to_string(), json!("edited"));
    }
    write_json(&pointer_path, &pointer)?;
    expect_fail_all(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        &["STALE", "`config_profile` no longer matches"],
    )
}

#[test]
fn pointer_features_edit_flips_stale() -> Result<(), String> {
    let sandbox = accepted_sandbox("stale-features")?;
    let binary = write_binary_v1(&sandbox)?;
    // The bound feature copy is part of the identity block; editing it is
    // a pointer-vs-receipt disagreement: stale.
    let pointer_path = sandbox.path("accepted/current.json");
    let mut pointer = read_strict(&pointer_path)?;
    if let Some(ripr) = pointer
        .get_mut("ripr")
        .and_then(|block| block.as_object_mut())
    {
        ripr.insert("features".to_string(), json!(["python", "extra"]));
    }
    write_json(&pointer_path, &pointer)?;
    expect_fail_all(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        &["STALE", "`features` no longer matches"],
    )
}

#[test]
fn source_sha_change_flips_stale() -> Result<(), String> {
    let sandbox = accepted_sandbox("stale-source")?;
    let binary = write_binary_v1(&sandbox)?;
    expect_fail(
        run_report(&currentness_args(
            &sandbox,
            &binary,
            &[
                "--ripr-source-sha",
                "1111222233334444555566667777888899990000",
            ],
        )),
        "analyzer source moved",
    )
}

#[test]
fn editing_as_of_never_repairs_staleness() -> Result<(), String> {
    // Stale via the binary movement...
    let sandbox = accepted_sandbox("as-of-edit")?;
    let binary = write_binary_v2(&sandbox)?;
    expect_fail(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        "STALE",
    )?;
    // ...and still stale after the as-of disclosure string is edited: the
    // comparison never reads as-of.
    let pointer_path = sandbox.path("accepted/current.json");
    let mut pointer = read_strict(&pointer_path)?;
    if let Some(object) = pointer.as_object_mut() {
        object.insert(
            "as_of".to_string(),
            json!("2099-01-01T00:00:00Z (freshened)"),
        );
    }
    write_json(&pointer_path, &pointer)?;
    expect_fail(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        "STALE",
    )?;

    // The mirror case: a CURRENT pointer stays current when only the
    // as-of string is edited (as-of is a disclosure, not an identity).
    let sandbox = accepted_sandbox("as-of-edit-current")?;
    let binary = write_binary_v1(&sandbox)?;
    run_report(&currentness_args(&sandbox, &binary, &[]))?;
    let pointer_path = sandbox.path("accepted/current.json");
    let mut pointer = read_strict(&pointer_path)?;
    if let Some(object) = pointer.as_object_mut() {
        object.insert("as_of".to_string(), json!("2099-01-01T00:00:00Z (edited)"));
    }
    write_json(&pointer_path, &pointer)?;
    run_report(&currentness_args(&sandbox, &binary, &[]))?;
    Ok(())
}

#[test]
fn malformed_as_of_pointer_fails_currentness() -> Result<(), String> {
    // The as-of value never enters the identity comparison, but a
    // malformed disclosure (empty, non-string, oversized) must fail the
    // check naming the field instead of riding along inside an otherwise
    // current pointer.
    for (label, value, needle) in [
        ("as-of-empty", json!(""), "as-of must be non-empty"),
        ("as-of-number", json!(5), "as-of must be a JSON string"),
        (
            "as-of-oversized",
            json!("x".repeat(NOTE_MAX_CHARS + 1)),
            "512-character bound",
        ),
    ] {
        let sandbox = accepted_sandbox(label)?;
        let binary = write_binary_v1(&sandbox)?;
        let pointer_path = sandbox.path("accepted/current.json");
        let mut pointer = read_strict(&pointer_path)?;
        if let Some(object) = pointer.as_object_mut() {
            object.insert("as_of".to_string(), value);
        }
        write_json(&pointer_path, &pointer)?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &[needle, "as_of"],
        )?;
    }

    // A normal as_of value passes the hygiene check and currentness.
    let sandbox = accepted_sandbox("as-of-shape-good")?;
    let binary = write_binary_v1(&sandbox)?;
    let pointer_path = sandbox.path("accepted/current.json");
    let mut pointer = read_strict(&pointer_path)?;
    if let Some(object) = pointer.as_object_mut() {
        object.insert("as_of".to_string(), json!("2026-09-10T00:00:00Z"));
    }
    write_json(&pointer_path, &pointer)?;
    run_report(&currentness_args(&sandbox, &binary, &[]))?;
    Ok(())
}

#[test]
fn unverifiable_identities_disclose_without_claiming_current() -> Result<(), String> {
    let sandbox = accepted_sandbox("unverifiable")?;
    // No --ripr-bin: the binary identity cannot be recomputed, so the
    // verdict must not claim `current`. The command exits 0 with the
    // disclosure; the pure comparison is asserted directly.
    let args = vec![
        "--check-currentness".to_string(),
        "--manifest".to_string(),
        sandbox.manifest_path_string(),
        "--state-dir".to_string(),
        sandbox.state_dir(),
        "--ripr-source-sha".to_string(),
        SOURCE_SHA.to_string(),
    ];
    run_report(&args)?;

    let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
    let pointer_object = pointer
        .as_object()
        .ok_or_else(|| "pointer object".to_string())?;
    let (accepted_receipt, receipt_path) = accepted_receipt(&sandbox)?;
    let receipt_sha = sha256_hex(
        &std::fs::read(&receipt_path).map_err(|error| format!("read receipt: {error}"))?,
    );
    let candidate_binding = accepted_receipt
        .get("candidate")
        .and_then(|candidate| candidate.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| "candidate binding".to_string())?
        .to_string();
    let candidate_path = sandbox.path(&format!(
        "accepted/receipts/{candidate_binding}.candidate.json"
    ));
    let candidate = read_strict(&candidate_path)?;
    let candidate_sha = sha256_hex(
        &std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?,
    );
    let current = CurrentIdentity {
        ripr_source_sha: Some(SOURCE_SHA.to_string()),
        ripr_binary_digest: None,
        subject_inputs: BTreeMap::new(),
    };
    let (manifest_value, manifest_sha) = load_strict_json(&sandbox.manifest_path_string())?;
    let accepted = validate_accepted_manifest(&manifest_value, manifest_sha.clone())?;
    let comparison = compare_currentness(&CurrentnessInputs {
        pointer: pointer_object,
        accepted_receipt: &accepted_receipt,
        receipt_sha256: &receipt_sha,
        candidate: &candidate,
        candidate_sha256: &candidate_sha,
        accepted: Some(&accepted),
        current_manifest_sha256: &manifest_sha,
        manifest_validation_error: None,
        current: &current,
    })?;
    assert_eq!(
        comparison.verdict,
        CurrentnessVerdict::Unverifiable,
        "a missing binary recompute input must leave currentness unverifiable: {:?}",
        comparison.unverifiable
    );
    assert!(
        comparison
            .unverifiable
            .iter()
            .any(|reason| reason.contains("binary")),
        "the binary gap must be disclosed: {:?}",
        comparison.unverifiable
    );
    Ok(())
}

#[test]
fn config_input_substitution_flips_stale() -> Result<(), String> {
    // A candidate that names a DIFFERENT portable input file for a
    // subject — hashing the substitute's bytes — must not pass
    // currentness: the pointer's config input must BE the
    // manifest-declared input path for that subject, and the refusal
    // names both paths.
    let (sandbox, args) =
        prepared_args("input-sub", &["--dispositions", "DISPOSITIONS", "--accept"])?;
    let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
    if let Some(alpha) = candidate
        .get_mut("repos")
        .and_then(Value::as_array_mut)
        .and_then(|rows| rows.first_mut())
        .and_then(|row| row.as_object_mut())
    {
        if let Some(config) = alpha.get_mut("config").and_then(Value::as_object_mut) {
            config.insert("input".to_string(), json!("diffs/evil.diff"));
        }
        alpha.insert(
            "input_digest".to_string(),
            json!(sha256_hex(b"diff-for-alpha (substituted)\n")),
        );
    }
    write_json(&sandbox.path("candidate.json"), &candidate)?;
    std::fs::write(
        sandbox.path("diffs/evil.diff"),
        "diff-for-alpha (substituted)\n",
    )
    .map_err(|error| format!("write substituted diff: {error}"))?;
    let dispositions = sandbox.dispositions_path()?;
    run_report(&with_dispositions(&args, &dispositions))?;

    let binary = write_binary_v1(&sandbox)?;
    expect_fail_all(
        run_report(&currentness_args(&sandbox, &binary, &[])),
        &[
            "STALE",
            "input path substituted for subject `alpha`",
            "diffs/evil.diff",
            "diffs/alpha.diff",
        ],
    )
}

#[test]
fn missing_input_digest_is_never_current() -> Result<(), String> {
    // A candidate row that records no input_digest leaves the pointer
    // with no input identity for that subject; with every other identity
    // matching, the gate must still refuse `current` — a missing subject
    // identity is disclosed unverifiable and names the subject.
    let (sandbox, args) = prepared_args(
        "missing-input",
        &["--dispositions", "DISPOSITIONS", "--accept"],
    )?;
    let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
    if let Some(alpha) = candidate
        .get_mut("repos")
        .and_then(Value::as_array_mut)
        .and_then(|rows| rows.first_mut())
        .and_then(|row| row.as_object_mut())
    {
        alpha.remove("input_digest");
    }
    write_json(&sandbox.path("candidate.json"), &candidate)?;
    let dispositions = sandbox.dispositions_path()?;
    run_report(&with_dispositions(&args, &dispositions))?;

    // Direct comparison: source/binary identities match and every
    // declared input recomputes, yet alpha's missing input digest must
    // keep the verdict unverifiable.
    let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
    let pointer_object = pointer
        .as_object()
        .ok_or_else(|| "pointer object".to_string())?;
    let (accepted_receipt, receipt_path) = accepted_receipt(&sandbox)?;
    let receipt_sha = sha256_hex(
        &std::fs::read(&receipt_path).map_err(|error| format!("read receipt: {error}"))?,
    );
    let candidate_binding = accepted_receipt
        .get("candidate")
        .and_then(|candidate| candidate.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| "candidate binding".to_string())?
        .to_string();
    let candidate_path = sandbox.path(&format!(
        "accepted/receipts/{candidate_binding}.candidate.json"
    ));
    let candidate = read_strict(&candidate_path)?;
    let candidate_sha = sha256_hex(
        &std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?,
    );
    let (manifest_value, manifest_sha) = load_strict_json(&sandbox.manifest_path_string())?;
    let accepted = validate_accepted_manifest(&manifest_value, manifest_sha.clone())?;
    let current = CurrentIdentity {
        ripr_source_sha: Some(SOURCE_SHA.to_string()),
        ripr_binary_digest: Some(binary_digest_v1()),
        subject_inputs: recompute_subject_inputs(&accepted, &sandbox.manifest_path_string()),
    };
    let comparison = compare_currentness(&CurrentnessInputs {
        pointer: pointer_object,
        accepted_receipt: &accepted_receipt,
        receipt_sha256: &receipt_sha,
        candidate: &candidate,
        candidate_sha256: &candidate_sha,
        accepted: Some(&accepted),
        current_manifest_sha256: &manifest_sha,
        manifest_validation_error: None,
        current: &current,
    })?;
    assert_eq!(
        comparison.verdict,
        CurrentnessVerdict::Unverifiable,
        "a subject with no bound input identity must never read current: stale={:?} unverifiable={:?}",
        comparison.stale,
        comparison.unverifiable
    );
    assert!(
        comparison
            .unverifiable
            .iter()
            .any(|reason| reason.contains("alpha") && reason.contains("input")),
        "the disclosure must name the subject and the input identity: {:?}",
        comparison.unverifiable
    );

    // End to end: the command exits 0 with the disclosure (unverifiable
    // is disclosed in full, never a gate pass dressed as current).
    let binary = write_binary_v1(&sandbox)?;
    run_report(&currentness_args(&sandbox, &binary, &[]))?;
    Ok(())
}

#[test]
fn split_portable_rejects_traversal_component_shapes() -> Result<(), String> {
    // The split is the containment boundary: empty components (consecutive
    // separators), `.`, `..`, and separator-only paths are rejected, so no
    // pointer-shaped portable path can walk outside the state directory.
    let good = split_portable("pointer", "receipt_file", "state", "receipts/abc123.json")?;
    assert_eq!(
        good,
        PathBuf::from("state").join("receipts").join("abc123.json"),
        "the normal receipt_file shape still joins"
    );
    for bad in [
        "receipts//../../secret",
        "a//b",
        "receipts/./abc.json",
        "./receipts/abc.json",
        ".",
        "..",
        "receipts/",
        "receipts//",
        "//",
        "",
    ] {
        let error = match split_portable("pointer", "receipt_file", "state", bad) {
            Ok(path) => {
                return Err(format!(
                    "split_portable must reject `{bad}`, joined `{}`",
                    path.display()
                ));
            }
            Err(error) => error,
        };
        assert!(
            error.contains("receipt_file"),
            "refusal `{error}` must name the field"
        );
    }
    Ok(())
}

#[test]
fn pointer_receipt_file_with_empty_or_dot_components_is_refused() -> Result<(), String> {
    // A crafted pointer receipt_file cannot use consecutive separators or
    // `.` components to slip a traversal path past the portable-path
    // checks: every such shape is refused naming `receipt_file`, while the
    // normal `receipts/<sha>.json` shape still reads.
    for (index, bad) in [
        "receipts//leak.json",
        "receipts/./leak.json",
        "a//..//b/../../outside",
    ]
    .into_iter()
    .enumerate()
    {
        let sandbox = accepted_sandbox(&format!("portable-{index}"))?;
        let binary = write_binary_v1(&sandbox)?;
        let pointer_path = sandbox.path("accepted/current.json");
        let mut pointer = read_strict(&pointer_path)?;
        if let Some(object) = pointer.as_object_mut() {
            object.insert("receipt_file".to_string(), json!(bad));
        }
        write_json(&pointer_path, &pointer)?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &["receipt_file"],
        )?;
    }
    // The normal shape still passes end to end.
    let sandbox = accepted_sandbox("portable-good")?;
    let binary = write_binary_v1(&sandbox)?;
    run_report(&currentness_args(&sandbox, &binary, &[]))?;
    Ok(())
}

#[test]
fn missing_toolchain_identity_is_never_current() -> Result<(), String> {
    // A pointer that binds no `features` or `build_profile` copy (the
    // candidate recorded none) cannot verify that identity: the verdict
    // must read `unverifiable` — never `current` — with the field named
    // (the step-6 subject rule applied to the toolchain block).
    for (label, field) in [
        ("tc-no-features", "features"),
        ("tc-no-profile", "build_profile"),
    ] {
        let sandbox = accepted_sandbox(label)?;
        let pointer_path = sandbox.path("accepted/current.json");
        let mut pointer = read_strict(&pointer_path)?;
        if let Some(ripr) = pointer
            .get_mut("ripr")
            .and_then(|block| block.as_object_mut())
        {
            ripr.remove(field);
        }
        write_json(&pointer_path, &pointer)?;
        let pointer_object = pointer
            .as_object()
            .ok_or_else(|| "pointer object".to_string())?;
        let (accepted_receipt, receipt_path) = accepted_receipt(&sandbox)?;
        let receipt_sha = sha256_hex(
            &std::fs::read(&receipt_path).map_err(|error| format!("read receipt: {error}"))?,
        );
        let candidate_binding = accepted_receipt
            .get("candidate")
            .and_then(|candidate| candidate.get("sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| "candidate binding".to_string())?
            .to_string();
        let candidate_path = sandbox.path(&format!(
            "accepted/receipts/{candidate_binding}.candidate.json"
        ));
        let candidate = read_strict(&candidate_path)?;
        let candidate_sha = sha256_hex(
            &std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?,
        );
        let (manifest_value, manifest_sha) = load_strict_json(&sandbox.manifest_path_string())?;
        let accepted = validate_accepted_manifest(&manifest_value, manifest_sha.clone())?;
        let current = CurrentIdentity {
            ripr_source_sha: Some(SOURCE_SHA.to_string()),
            ripr_binary_digest: Some(binary_digest_v1()),
            subject_inputs: recompute_subject_inputs(&accepted, &sandbox.manifest_path_string()),
        };
        let comparison = compare_currentness(&CurrentnessInputs {
            pointer: pointer_object,
            accepted_receipt: &accepted_receipt,
            receipt_sha256: &receipt_sha,
            candidate: &candidate,
            candidate_sha256: &candidate_sha,
            accepted: Some(&accepted),
            current_manifest_sha256: &manifest_sha,
            manifest_validation_error: None,
            current: &current,
        })?;
        assert_eq!(
            comparison.verdict,
            CurrentnessVerdict::Unverifiable,
            "a pointer binding no `{field}` must never read current: stale={:?} unverifiable={:?}",
            comparison.stale,
            comparison.unverifiable
        );
        assert!(
            comparison
                .unverifiable
                .iter()
                .any(|reason| reason.contains("toolchain") && reason.contains(field)),
            "the disclosure must name the missing toolchain identity `{field}`: {:?}",
            comparison.unverifiable
        );

        // End to end: the command exits 0 with the disclosure.
        let binary = write_binary_v1(&sandbox)?;
        run_report(&currentness_args(&sandbox, &binary, &[]))?;
    }
    Ok(())
}

#[test]
fn disposition_owner_pipes_and_newlines_cannot_split_the_markdown_table() -> Result<(), String> {
    // A free-text disposition owner carrying a pipe and a newline must
    // not split or extend the accepted Markdown table: the cell renders
    // escaped and flattened while the receipt JSON keeps the raw value.
    // The pure derivation path (sidecar -> accepted receipt -> Markdown)
    // avoids the shared dry-run report files, which sibling tests
    // rewrite concurrently.
    let sandbox = TestSandbox::new("md-escape")?;
    let manifest_sha = sandbox.write_manifest()?;
    let candidate = candidate_value(&sandbox, &manifest_sha, &binary_digest_v1(), 100);
    let candidate_path = sandbox.path("candidate.json");
    write_json(&candidate_path, &candidate)?;
    let candidate_sha = sha256_hex(
        &std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?,
    );
    let rows = read_candidate_rows(&candidate)?;
    let mut value = dispositions_value();
    if let Some(first) = value
        .get_mut("dispositions")
        .and_then(Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(|entry| entry.as_object_mut())
    {
        first.insert("owner".to_string(), json!("team\nops|lead"));
    }
    let dispositions_path = sandbox.path("dispositions.json");
    write_json(&dispositions_path, &value)?;
    let dispositions = load_dispositions(&dispositions_path.to_string_lossy(), &rows)?;
    require_disposition_coverage(&rows, &dispositions)?;
    let (manifest_value, current_manifest_sha) = load_strict_json(&sandbox.manifest_path_string())?;
    let accepted = validate_accepted_manifest(&manifest_value, current_manifest_sha.clone())?;
    let receipt = build_accepted_receipt(
        &accepted,
        &current_manifest_sha,
        &candidate,
        &candidate_sha,
        &rows,
        &dispositions,
        &[],
    );
    let markdown = render_accepted_markdown(&receipt)?;
    assert!(
        markdown.contains("team ops\\|lead"),
        "the owner cell must render escaped and flattened: {markdown}"
    );
    assert!(
        !markdown.contains("team\nops") && !markdown.contains("team|lead"),
        "the raw owner value must not leak into the table: {markdown}"
    );
    // Table integrity: every subjects-section line keeps exactly the five
    // structural cell separators (an unescaped pipe or a raw newline would
    // break that count).
    let section = markdown
        .split("## Subjects")
        .nth(1)
        .and_then(|rest| rest.split("## Non-claims").next())
        .ok_or_else(|| "markdown subjects section".to_string())?;
    for line in section.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let structural = line.replace("\\|", "");
        assert_eq!(
            structural.matches('|').count(),
            5,
            "every subjects-table line must keep exactly five cell separators: {line}"
        );
    }

    // The JSON keeps the raw value.
    let raw_owner = receipt
        .get("subjects")
        .and_then(Value::as_array)
        .and_then(|subjects| {
            subjects
                .iter()
                .find(|subject| subject.get("id").and_then(Value::as_str) == Some("bravo"))
        })
        .and_then(|subject| subject.get("disposition"))
        .and_then(|disposition| disposition.get("owner"))
        .and_then(Value::as_str)
        .ok_or_else(|| "bravo disposition owner".to_string())?;
    assert_eq!(
        raw_owner, "team\nops|lead",
        "the receipt JSON keeps the raw disposition value"
    );
    Ok(())
}

// -- derived receipt content ----------------------------------------------

#[test]
fn stability_mismatch_reasons_are_recorded_in_the_receipt() -> Result<(), String> {
    let (sandbox, args) =
        prepared_args("mismatch", &["--dispositions", "DISPOSITIONS", "--accept"])?;
    // Flip the one compared row's repeat comparison to unstable: the
    // mismatch list must name the subject and its unstable gap ids.
    let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
    if let Some(rows) = candidate.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(first) = rows.first_mut().and_then(|row| row.as_object_mut())
        && let Some(Value::Object(repeat)) = first.get_mut("repeat")
    {
        repeat.insert("gap_ids_stable".to_string(), json!(false));
        repeat.insert(
            "unstable_gap_ids".to_string(),
            json!(["gap:python:alpha-1"]),
        );
    }
    write_json(&sandbox.path("candidate.json"), &candidate)?;
    let dispositions = sandbox.dispositions_path()?;
    run_report(&with_dispositions(&args, &dispositions))?;

    let (receipt, _path) = accepted_receipt(&sandbox)?;
    let mismatches = receipt
        .get("stability")
        .and_then(|stability| stability.get("mismatch_subjects"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        mismatches.len(),
        1,
        "the one unstable comparison is recorded"
    );
    assert_eq!(
        mismatches
            .first()
            .and_then(|entry| entry.get("id"))
            .and_then(Value::as_str),
        Some("alpha"),
    );
    assert!(
        mismatches
            .first()
            .and_then(|entry| entry.get("unstable_gap_ids"))
            .and_then(Value::as_array)
            .map(|ids| ids.contains(&json!("gap:python:alpha-1")))
            .unwrap_or(false),
        "the mismatch reason carries the unstable gap ids"
    );
    Ok(())
}

#[test]
fn runtime_envelope_is_derived_and_bounded() -> Result<(), String> {
    let (sandbox, args) =
        prepared_args("runtime", &["--dispositions", "DISPOSITIONS", "--accept"])?;
    let dispositions = sandbox.dispositions_path()?;
    run_report(&with_dispositions(&args, &dispositions))?;

    let (receipt, _path) = accepted_receipt(&sandbox)?;
    let envelope = receipt
        .get("runtime_envelope")
        .ok_or_else(|| "runtime_envelope".to_string())?;
    assert_eq!(
        envelope.get("status").and_then(Value::as_str),
        Some("reliable")
    );
    assert_eq!(envelope.get("min_ms").and_then(Value::as_u64), Some(100));
    assert_eq!(envelope.get("median_ms").and_then(Value::as_u64), Some(300));
    assert_eq!(envelope.get("max_ms").and_then(Value::as_u64), Some(500));
    assert_eq!(envelope.get("total_ms").and_then(Value::as_u64), Some(1500));
    // The count carries the analyzed-row denominator (5 run rows).
    let count = envelope
        .get("count")
        .and_then(Value::as_object)
        .ok_or_else(|| "runtime_envelope.count".to_string())?;
    assert_eq!(count.get("numerator").and_then(Value::as_u64), Some(5));
    assert_eq!(count.get("denominator").and_then(Value::as_u64), Some(5));
    Ok(())
}

#[test]
fn counts_and_health_are_derived_from_rows() -> Result<(), String> {
    let (sandbox, args) = prepared_args("counts", &["--dispositions", "DISPOSITIONS", "--accept"])?;
    let dispositions = sandbox.dispositions_path()?;
    run_report(&with_dispositions(&args, &dispositions))?;

    let (receipt, _path) = accepted_receipt(&sandbox)?;
    let counts = receipt
        .get("counts")
        .and_then(Value::as_object)
        .ok_or_else(|| "counts".to_string())?;
    // Row facts: 5 materialized (the ran rows), 5 detected/available,
    // 1 stale, 1 tempfail, 0 license-blocked.
    let numerator = |name: &str| -> Result<u64, String> {
        counts
            .get(name)
            .and_then(|count| count.get("numerator"))
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("count {name}"))
    };
    assert_eq!(numerator("selected")?, 8);
    assert_eq!(numerator("run")?, 5);
    assert_eq!(numerator("materialized")?, 5);
    assert_eq!(numerator("available")?, 5);
    assert_eq!(numerator("stale")?, 1);
    assert_eq!(numerator("tempfail")?, 1);
    assert_eq!(numerator("license_blocked")?, 0);
    // Every count in the outcomes/health/distribution blocks carries
    // numerator + denominator (the selected denominator) — no bare
    // denominator-free number anywhere.
    let count_shape = |name: &str, count: &Value| -> Result<(), String> {
        let shape_ok = count.get("numerator").and_then(Value::as_u64).is_some()
            && count.get("denominator").and_then(Value::as_u64).is_some();
        if shape_ok {
            Ok(())
        } else {
            Err(format!(
                "count `{name}` must carry numerator and denominator: {count}"
            ))
        }
    };
    let numerator_in =
        |block: &serde_json::Map<String, Value>, name: &str| -> Result<u64, String> {
            block
                .get(name)
                .and_then(|count| count.get("numerator"))
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("count {name}"))
        };
    let outcomes = receipt
        .get("outcomes")
        .and_then(Value::as_object)
        .ok_or_else(|| "outcomes".to_string())?;
    for (name, count) in outcomes {
        count_shape(name, count)?;
    }
    assert_eq!(numerator_in(outcomes, "complete")?, 1);
    assert_eq!(numerator_in(outcomes, "crashed")?, 1);
    assert_eq!(numerator_in(outcomes, "parse_failed")?, 1);
    assert_eq!(numerator_in(outcomes, "timed_out")?, 1);
    assert_eq!(numerator_in(outcomes, "unsupported")?, 1);
    let health = receipt
        .get("health")
        .and_then(Value::as_object)
        .ok_or_else(|| "health".to_string())?;
    let detection = health
        .get("project_detection")
        .and_then(Value::as_object)
        .ok_or_else(|| "project_detection".to_string())?;
    let corpus = health
        .get("corpus_selection")
        .and_then(Value::as_object)
        .ok_or_else(|| "corpus_selection".to_string())?;
    for (name, count) in detection.iter().chain(corpus) {
        count_shape(name, count)?;
    }
    assert_eq!(numerator_in(detection, "detected")?, 5);
    assert_eq!(numerator_in(detection, "unrecorded")?, 0);
    assert_eq!(numerator_in(corpus, "selected")?, 5);
    assert_eq!(numerator_in(corpus, "absent")?, 3);
    // Distributions agree with the rows: weakly_exposed=1, static_unknown=1.
    let distributions = receipt
        .get("distributions")
        .and_then(Value::as_object)
        .ok_or_else(|| "distributions".to_string())?;
    let classification = distributions
        .get("classification")
        .and_then(Value::as_object)
        .ok_or_else(|| "classification".to_string())?;
    let alignment = distributions
        .get("alignment")
        .and_then(Value::as_object)
        .ok_or_else(|| "alignment".to_string())?;
    for (name, count) in classification.iter().chain(alignment) {
        count_shape(name, count)?;
    }
    assert_eq!(numerator_in(classification, "weakly_exposed")?, 1,);
    assert_eq!(numerator_in(classification, "static_unknown")?, 1);
    // Each distribution bucket carries the selected denominator (non-run
    // rows contribute no buckets, so sums below 8 are the honest shape).
    assert_eq!(
        classification
            .get("weakly_exposed")
            .and_then(|count| count.get("denominator"))
            .and_then(Value::as_u64),
        Some(8),
    );
    assert_eq!(numerator_in(alignment, "orthogonal")?, 1);
    assert_eq!(numerator_in(alignment, "absent")?, 3);
    // The limitation distribution carries a named disclosure, never an
    // invented taxonomy.
    assert!(
        distributions
            .get("limitation")
            .and_then(|limitation| limitation.get("disclosure"))
            .and_then(Value::as_str)
            .map(|text| text.contains("no limitation distribution"))
            .unwrap_or(false),
    );
    Ok(())
}

#[test]
fn dry_run_writes_no_accepted_state() -> Result<(), String> {
    let (sandbox, args) = prepared_args("dry-no-state", &["--dispositions", "DISPOSITIONS"])?;
    let dispositions = sandbox.dispositions_path()?;
    run_report(&with_dispositions(&args, &dispositions))?;
    assert!(
        !sandbox.path("accepted").exists(),
        "a dry run must not create accepted state"
    );
    Ok(())
}

#[test]
fn pointer_field_hygiene_fails_on_secrets() -> Result<(), String> {
    // A pointer as-of carrying a secret-shaped token fails hygiene before
    // any write.
    let (sandbox, args) = prepared_args(
        "pointer-hygiene",
        &[
            "--dispositions",
            "DISPOSITIONS",
            "--accept",
            "--as-of",
            "api_key=hunter2",
        ],
    )?;
    let dispositions = sandbox.dispositions_path()?;
    expect_fail(
        run_report(&with_dispositions(&args, &dispositions)),
        "secret-shaped token",
    )?;
    assert!(
        !sandbox.path("accepted/current.json").exists(),
        "a hygiene failure must not move the pointer"
    );
    Ok(())
}

// -- acceptance write-path integrity --------------------------------------

#[test]
fn candidate_modified_after_parse_refuses_acceptance() -> Result<(), String> {
    // The accept-time window: a candidate file edited between the initial
    // parse and the acceptance writes must be refused, so the accepted
    // artifacts can never retain new bytes under the OLD candidate
    // digest while the pointer moves.
    let (sandbox, _args) = prepared_args("candidate-edit", &[])?;
    let candidate_path = sandbox.path("candidate.json");
    let (_value, parsed_sha) = load_strict_json(&candidate_path.to_string_lossy())?;
    // The post-parse edit:
    let mut edited = read_strict(&candidate_path)?;
    if let Some(rows) = edited.get_mut("repos").and_then(Value::as_array_mut) {
        rows.pop();
    }
    write_json(&candidate_path, &edited)?;
    let error = match revalidate_candidate_bytes(&candidate_path, &parsed_sha) {
        Ok(_) => return Err("an edited candidate must be refused at acceptance".to_string()),
        Err(error) => error,
    };
    for needle in ["changed after validation", parsed_sha.as_str()] {
        assert!(
            error.contains(needle),
            "refusal `{error}` must mention `{needle}`"
        );
    }
    // The bytes actually on disk still revalidate: the honest rerun path
    // re-parses the current bytes and records their digest.
    let current_bytes =
        std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?;
    let current_sha = sha256_hex(&current_bytes);
    let verified = revalidate_candidate_bytes(&candidate_path, &current_sha)?;
    assert_eq!(
        verified, current_bytes,
        "verified bytes are exactly the file bytes"
    );
    Ok(())
}

#[test]
fn pointer_replacement_preserves_a_readable_current_pointer() -> Result<(), String> {
    // Replacing an established pointer must leave a readable current.json
    // in place after the write: the staged file is written and flushed,
    // then renamed over the existing pointer without removing it first
    // (remove+rename is only the documented fallback for hosts that
    // refuse rename-over-existing, after the staged bytes are durable).
    let sandbox = TestSandbox::new("pointer-replace")?;
    let state_dir = sandbox.path("accepted");
    std::fs::create_dir_all(&state_dir).map_err(|error| format!("create state dir: {error}"))?;
    let pointer_path = state_dir.join(POINTER_FILE);
    write_json(
        &pointer_path,
        &json!({"schema_version": POINTER_SCHEMA, "kind": POINTER_KIND, "spec": SPEC}),
    )?;
    write_pointer_atomically(&state_dir, "{\"replaced\": true}")?;
    let replaced = read_strict(&pointer_path)?;
    assert_eq!(
        replaced.get("replaced").and_then(Value::as_bool),
        Some(true),
        "the new pointer is in place after the replacement"
    );
    // A second replacement exercises rename-over-existing again: the
    // pointer stays readable and parseable throughout.
    write_pointer_atomically(&state_dir, "{\"replaced\": false}")?;
    let replaced = read_strict(&pointer_path)?;
    assert_eq!(
        replaced.get("replaced").and_then(Value::as_bool),
        Some(false)
    );
    Ok(())
}

#[test]
fn truncated_accepted_artifacts_are_repaired_on_reacceptance() -> Result<(), String> {
    // A digest-addressed artifact whose on-disk bytes do not hash to its
    // own address is not a valid prior artifact (truncated by an
    // interrupted write, or edited): re-acceptance repairs it with
    // exactly the digest-named bytes instead of refusing forever as
    // "conflicting content".
    let (sandbox, args) = prepared_args(
        "accept-repair",
        &["--dispositions", "DISPOSITIONS", "--accept"],
    )?;
    let dispositions = sandbox.dispositions_path()?;
    let args = with_dispositions(&args, &dispositions);
    run_report(&args)?;

    let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
    let receipt_file = pointer
        .get("receipt_file")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt_file".to_string())?
        .to_string();
    let receipt_path = sandbox.path(&format!("accepted/{receipt_file}"));
    let (accepted_receipt, _path) = accepted_receipt(&sandbox)?;
    let candidate_sha = accepted_receipt
        .get("candidate")
        .and_then(|candidate| candidate.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| "candidate.sha256".to_string())?
        .to_string();
    let candidate_path = sandbox.path(&format!("accepted/receipts/{candidate_sha}.candidate.json"));
    let receipt_bytes =
        std::fs::read(&receipt_path).map_err(|error| format!("read receipt: {error}"))?;
    let candidate_bytes =
        std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?;
    // The published bytes hash to their digest addresses by construction.
    assert_eq!(
        sha256_hex(&receipt_bytes),
        pointer
            .get("receipt_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "receipt_sha256".to_string())?,
        "the accepted receipt's bytes hash to its digest address"
    );
    assert_eq!(
        sha256_hex(&candidate_bytes),
        candidate_sha,
        "the retained candidate's bytes hash to its digest address"
    );

    // Simulate interrupted writes: truncated bytes at both digest
    // addresses.
    std::fs::write(&receipt_path, &receipt_bytes[..receipt_bytes.len() / 2])
        .map_err(|error| format!("truncate receipt: {error}"))?;
    std::fs::write(
        &candidate_path,
        &candidate_bytes[..candidate_bytes.len() / 2],
    )
    .map_err(|error| format!("truncate candidate: {error}"))?;

    // Re-acceptance repairs both instead of refusing.
    run_report(&args)?;
    let repaired_receipt =
        std::fs::read(&receipt_path).map_err(|error| format!("reread receipt: {error}"))?;
    let repaired_candidate =
        std::fs::read(&candidate_path).map_err(|error| format!("reread candidate: {error}"))?;
    assert_eq!(
        repaired_receipt, receipt_bytes,
        "the receipt must hold exactly the staged bytes after repair"
    );
    assert_eq!(
        repaired_candidate, candidate_bytes,
        "the retained candidate must hold exactly the verified bytes after repair"
    );
    Ok(())
}

#[test]
fn edited_markdown_refuses_reacceptance_and_matching_is_idempotent() -> Result<(), String> {
    // An existing accepted Markdown is verified on re-acceptance: edited
    // bytes under the accepted receipt's digest are a typed refusal (the
    // mirror of the existing-different-JSON rule); matching bytes are an
    // idempotent no-op.
    let (sandbox, args) =
        prepared_args("md-edit", &["--dispositions", "DISPOSITIONS", "--accept"])?;
    let dispositions = sandbox.dispositions_path()?;
    let args = with_dispositions(&args, &dispositions);
    run_report(&args)?;
    let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
    let receipt_sha = pointer
        .get("receipt_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt_sha256".to_string())?
        .to_string();
    let markdown_path = sandbox.path(&format!("accepted/receipts/{receipt_sha}.md"));
    let original =
        std::fs::read_to_string(&markdown_path).map_err(|error| format!("read md: {error}"))?;

    // Corrupt the retained Markdown, then re-accept the same candidate:
    // the edited artifact is refused, never silently kept or rewritten.
    std::fs::write(&markdown_path, "# edited after acceptance\n")
        .map_err(|error| format!("write markdown: {error}"))?;
    expect_fail(run_report(&args), "different bytes")?;

    // Restored bytes: re-acceptance is an idempotent no-op.
    std::fs::write(&markdown_path, &original)
        .map_err(|error| format!("restore markdown: {error}"))?;
    run_report(&args)?;
    Ok(())
}
