//! Owned, hard-disabled complete-execution experiment. No guard override.
use super::*;
use crate::run::{ByteCaptureBudget, capture_bytes_in_dir_with_budget};
use serde::Deserialize;
use std::time::Instant;

pub(super) const EXPERIMENT_FLAG: &str = "--experimental-complete-execution";
const WORKER_FLAG: &str = "--experimental-complete-execution-worker";
pub(super) const RECEIPT: &str = "target/ripr/pr/complete-execution.receipt.json";
const RECEIPT_MAX: u64 = 16 * 1024;
const ADDRESS_SPACE_MAX: u64 = 2 * 1024 * 1024 * 1024;
const FILE_MAX: u64 = 256 * 1024 * 1024;
const STREAM_MAX: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: String,
    nonce: String,
    address_space_bytes: u64,
    file_bytes: u64,
    base_sha: String,
    head_sha: String,
    head_tree: String,
    check_sha256: String,
    check_byte_count: u64,
    canonical_diff_sha256: String,
    configuration_fingerprint: String,
    index_entries: u64,
    index_bytes: u64,
    coverage: String,
    production_admission: bool,
    artifacts: ArtifactDigests,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactDigests {
    #[serde(rename = "target/ripr/pr/repo-exposure.json")]
    packet: String,
    #[serde(rename = "target/ripr/pr/repo-exposure.md")]
    markdown: String,
    #[serde(rename = "target/ripr/pr/check.json")]
    check: String,
    #[serde(rename = "target/ripr/pr/check.subject.json")]
    subject: String,
    #[serde(rename = "target/ripr/pr/review-input.json")]
    review: String,
    #[serde(rename = "target/ripr/pr/check.diff")]
    diff: String,
}
impl ArtifactDigests {
    fn entries(&self) -> [(&'static str, &str); 6] {
        [
            (PR_EVIDENCE_JSON, &self.packet),
            (PR_EVIDENCE_MD, &self.markdown),
            (PR_CHECK_JSON, &self.check),
            (PR_CHECK_SUBJECT_JSON, &self.subject),
            (PR_REVIEW_INPUT_JSON, &self.review),
            (PR_CANONICAL_DIFF, &self.diff),
        ]
    }
}

fn require_regular(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "experimental retained artifact {} unavailable: {error}",
            path.display()
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "experimental retained artifact {} is not a regular owned file",
            path.display()
        ));
    }
    Ok(())
}

// The parent never parses the full check/index. Fixed-count streaming digest
// checks close post-worker mutation without copying a large artifact body.
fn verify_artifacts(repo: &Path, receipt: &Receipt) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(pr_evidence_timeout_secs()?);
    let mut total = 0u64;
    let maximum = receipt
        .file_bytes
        .checked_mul(6)
        .ok_or_else(|| "experimental artifact total bound overflow".to_string())?;
    for (relative, expected) in receipt.artifacts.entries() {
        require_regular(&repo.join(relative))?;
        let file = fs::File::open(repo.join(relative))
            .map_err(|error| format!("experimental artifact {relative} unavailable: {error}"))?;
        let metadata = file.metadata().map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.len() > receipt.file_bytes {
            return Err(format!(
                "experimental artifact {relative} exceeds its finite file bound"
            ));
        }
        let mut reader = BufReader::new(file);
        let mut digest = Sha256::new();
        let mut count = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if Instant::now() >= deadline {
                return Err("experimental artifact verification exceeded its deadline".to_string());
            }
            let bytes = reader
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if bytes == 0 {
                break;
            }
            count = count
                .checked_add(bytes as u64)
                .ok_or_else(|| "experimental artifact count overflow".to_string())?;
            total = total
                .checked_add(bytes as u64)
                .ok_or_else(|| "experimental artifact total count overflow".to_string())?;
            if count > receipt.file_bytes || total > maximum {
                return Err("experimental artifacts grew beyond their finite bound".to_string());
            }
            digest.update(&buffer[..bytes]);
        }
        if format!("sha256:{:x}", digest.finalize()) != expected
            || (relative == PR_CHECK_JSON
                && (count != receipt.check_byte_count || expected != receipt.check_sha256))
            || (relative == PR_CANONICAL_DIFF && expected != receipt.canonical_diff_sha256)
        {
            return Err(format!(
                "experimental artifact {relative} does not match its completion receipt"
            ));
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn inherited_limit(text: &str, name: &str, maximum: u64) -> Result<u64, String> {
    let mut rows = text.lines().filter_map(|line| line.strip_prefix(name));
    let row = rows
        .next()
        .ok_or_else(|| format!("experimental launcher missing {name}"))?;
    if rows.next().is_some() {
        return Err(format!("experimental launcher duplicate {name}"));
    }
    let fields: Vec<_> = row.split_whitespace().collect();
    if fields.len() != 3 || fields[2] != "bytes" {
        return Err(format!("experimental launcher malformed {name}"));
    }
    let mut limit = maximum;
    for field in &fields[..2] {
        if *field != "unlimited" {
            limit = limit.min(field.parse::<u64>().map_err(|error| {
                format!("experimental launcher malformed {name} ceiling: {error}")
            })?);
        }
    }
    if limit == 0 {
        return Err(format!("experimental launcher inherited zero {name}"));
    }
    Ok(limit)
}

fn profile() -> Result<(u64, u64), String> {
    #[cfg(target_os = "linux")]
    {
        let mut text = String::new();
        fs::File::open("/proc/self/limits")
            .map_err(|error| format!("experimental launcher limits unavailable: {error}"))?
            .take(RECEIPT_MAX + 1)
            .read_to_string(&mut text)
            .map_err(|error| format!("experimental launcher limits read: {error}"))?;
        if text.len() as u64 > RECEIPT_MAX {
            return Err("experimental launcher limits exceed their bound".to_string());
        }
        Ok((
            inherited_limit(&text, "Max address space", ADDRESS_SPACE_MAX)?,
            inherited_limit(&text, "Max file size", FILE_MAX)?,
        ))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("experimental complete-execution requires qualified Linux resource limits".to_string())
    }
}

fn revoke(repo: &Path) -> Result<(), String> {
    // Revoke before resource discovery, binary build, limiter setup or spawn.
    remove_stale_check_artifact(repo)?;
    match fs::remove_file(repo.join(RECEIPT)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("experimental launcher receipt revocation: {error}")),
    }
}

pub(super) fn run_experiment(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .filter(|arg| arg.as_str() == EXPERIMENT_FLAG)
        .count()
        != 1
    {
        return Err("experimental complete-execution flag must occur exactly once".to_string());
    }
    let ordinary: Vec<_> = args
        .iter()
        .filter(|arg| arg.as_str() != EXPERIMENT_FLAG)
        .cloned()
        .collect();
    let options = parse_options(&ordinary)?;
    if options.check {
        return Err("experimental complete-execution is a producer experiment; --check remains production-only".to_string());
    }
    let repo = repo_root()?;
    revoke(&repo)?;
    let bounds = profile()?;
    if env::var_os("RIPR_BIN").is_some() {
        return Err("experimental complete-execution requires the pinned built worker; RIPR_BIN is supported only by the ordinary compatibility path".to_string());
    }
    let args = [
        "build".to_string(),
        "--manifest-path".to_string(),
        repo.join("Cargo.toml").display().to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--quiet".to_string(),
    ];
    run_output_owned_with_timeout(
        "cargo",
        &args,
        tool_build_timeout()?,
        "build of the pinned complete-execution worker",
    )?;
    let binary = built_ripr_binary_path(&repo)?.display().to_string();
    let receipt = run_candidate(
        &repo,
        &binary,
        &options,
        Path::new("/usr/bin/prlimit"),
        bounds,
    )?;
    // A native success is experimental only. Never turn it into gate success.
    Err(format!(
        "experimental worker completed under {} address-space bytes with {} index entries ({} index bytes); exhaustive coverage {} and production admission disabled",
        receipt.address_space_bytes, receipt.index_entries, receipt.index_bytes, receipt.coverage
    ))
}

fn run_candidate(
    repo: &Path,
    binary: &str,
    options: &PrEvidenceOptions,
    limiter: &Path,
    bounds: (u64, u64),
) -> Result<Receipt, String> {
    revoke(repo)?;
    let (address_space, file) = bounds;
    if address_space == 0 || address_space > ADDRESS_SPACE_MAX || file == 0 || file > FILE_MAX {
        return Err("experimental launcher invalid finite resource profile".to_string());
    }
    let nonce = format!(
        "{:x}{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    );
    let base = resolve_revision(repo, &options.base, "commit")?;
    let head = resolve_revision(repo, &options.head, "commit")?;
    let head_tree = resolve_revision(repo, &head, "tree")?;
    let args = vec![
        format!("--as={address_space}:{address_space}"),
        format!("--fsize={file}:{file}"),
        "--core=0:0".to_string(),
        "--".to_string(),
        binary.to_string(),
        "pr-evidence".to_string(),
        WORKER_FLAG.to_string(),
        address_space.to_string(),
        file.to_string(),
        nonce.clone(),
        "--root".to_string(),
        options.root.clone(),
        "--base".to_string(),
        base.clone(),
        "--head".to_string(),
        head.clone(),
    ];
    let result = (|| {
        let output = capture_bytes_in_dir_with_budget(
            limiter,
            &args,
            (repo, None),
            &[],
            ByteCaptureBudget {
                timeout: Duration::from_secs(pr_evidence_timeout_secs()?),
                stdout_bytes: STREAM_MAX,
                stderr_bytes: STREAM_MAX,
            },
            "experimental complete-execution worker",
        )?;
        // Byte capture refuses any truncation/drain incompleteness itself.
        if output.timed_out || !output.status.is_some_and(|status| status.success()) {
            return Err(format!(
                "experimental complete-execution worker refused (timeout={}, {})",
                output.timed_out,
                describe_native_status(output.status)
            ));
        }
        read_receipt(
            repo,
            &nonce,
            (address_space, file),
            &base,
            &head,
            &head_tree,
        )
    })();
    if result.is_err() {
        revoke(repo)?;
    }
    result
}

fn read_receipt(
    repo: &Path,
    nonce: &str,
    bounds: (u64, u64),
    base: &str,
    head: &str,
    head_tree: &str,
) -> Result<Receipt, String> {
    let path = repo.join(RECEIPT);
    require_regular(&path)?;
    let mut bytes = Vec::new();
    let file = fs::File::open(&path)
        .map_err(|error| format!("experimental completion receipt missing: {error}"))?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("experimental completion receipt is not a regular file".to_string());
    }
    file.take(RECEIPT_MAX + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("experimental completion receipt read: {error}"))?;
    if bytes.len() as u64 > RECEIPT_MAX {
        return Err("experimental completion receipt exceeds its bound".to_string());
    }
    // Typed exact-document parsing rejects duplicate fields, unknown fields
    // and extra documents/trailing non-whitespace; no last-line extraction.
    let receipt: Receipt = serde_json::from_slice(&bytes)
        .map_err(|error| format!("experimental completion receipt malformed: {error}"))?;
    let sha = |value: &str| {
        value.len() == 71
            && value.starts_with("sha256:")
            && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
    };
    let commit =
        |value: &str| value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if receipt.schema_version != "ripr.complete_execution_experiment_receipt.v1"
        || receipt.nonce != nonce
        || (receipt.address_space_bytes, receipt.file_bytes) != bounds
        || receipt.base_sha != base
        || receipt.head_sha != head
        || !commit(&receipt.head_tree)
        || receipt.head_tree != head_tree
        || !sha(&receipt.check_sha256)
        || !sha(&receipt.canonical_diff_sha256)
        || receipt.check_byte_count == 0
        || receipt.configuration_fingerprint.is_empty()
        || receipt.index_entries > ripr::review_input::REVIEW_INDEX_MAX_ENTRIES as u64
        || receipt.index_bytes > ripr::review_input::REVIEW_INDEX_MAX_BYTES as u64
        || receipt.coverage != "not_established"
        || receipt.production_admission
        || receipt
            .artifacts
            .entries()
            .iter()
            .any(|(_, digest)| !sha(digest))
    {
        return Err(
            "experimental completion receipt stale, incomplete or inconsistent".to_string(),
        );
    }
    verify_artifacts(repo, &receipt)?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal<T>(result: Result<T, String>, context: &str) -> Result<String, String> {
        let Err(error) = result else {
            return Err(format!("{context} was unexpectedly accepted"));
        };
        Ok(error)
    }

    #[test]
    fn inherited_lower_limits_are_never_raised() -> Result<(), String> {
        assert_eq!(
            inherited_limit(
                "Max address space unlimited unlimited bytes",
                "Max address space",
                2048
            )?,
            2048
        );
        assert_eq!(
            inherited_limit(
                "Max address space 1024 1536 bytes",
                "Max address space",
                2048
            )?,
            1024
        );
        for text in [
            "Max address space 0 1024 bytes",
            "Max address space bad 1024 bytes",
            "Max address space 1 2 bytes\nMax address space 1 2 bytes",
        ] {
            let error = refusal(
                inherited_limit(text, "Max address space", 2048),
                "invalid inherited ceiling",
            )?;
            assert!(error.starts_with("experimental launcher"), "{error}");
        }
        Ok(())
    }

    #[test]
    fn completion_receipt_refuses_stale_missing_duplicate_extra_and_mutated_artifacts()
    -> Result<(), String> {
        let repo = super::super::tests::temp_repo("ripr-complete-receipt-counterexamples")?;
        let result = (|| {
            let bytes = b"bounded artifact";
            let digest = format!("sha256:{:x}", Sha256::digest(bytes));
            let mut artifacts = Map::new();
            for relative in [
                PR_EVIDENCE_JSON,
                PR_EVIDENCE_MD,
                PR_CHECK_JSON,
                PR_CHECK_SUBJECT_JSON,
                PR_REVIEW_INPUT_JSON,
                PR_CANONICAL_DIFF,
            ] {
                super::super::tests::write_repo_file(&repo, relative, "bounded artifact")?;
                artifacts.insert(relative.to_string(), json!(digest));
            }
            let base = "a".repeat(40);
            let head = "b".repeat(40);
            let valid = json!({
                "schema_version":"ripr.complete_execution_experiment_receipt.v1",
                "nonce":"ab", "address_space_bytes":2048, "file_bytes":512,
                "base_sha":base, "head_sha":head, "head_tree":"c".repeat(40),
                "check_sha256":digest, "check_byte_count":bytes.len(),
                "canonical_diff_sha256":digest, "configuration_fingerprint":"fixture",
                "index_entries":0, "index_bytes":100,
                "coverage":"not_established", "production_admission":false,
                "artifacts":artifacts,
            });
            let write = |body: &str| super::super::tests::write_repo_file(&repo, RECEIPT, body);
            let text = serde_json::to_string(&valid).map_err(|error| error.to_string())?;
            write(&text)?;
            read_receipt(&repo, "ab", (2048, 512), &base, &head, &"c".repeat(40))?;
            for (field, wrong, expected) in [
                ("nonce", json!("stale"), "inconsistent"),
                ("coverage", json!("complete"), "inconsistent"),
                ("production_admission", json!(true), "inconsistent"),
                ("base_sha", json!(head), "inconsistent"),
                ("address_space_bytes", json!(1024), "inconsistent"),
                ("check_byte_count", json!(999), "does not match"),
                ("head_tree", json!("d".repeat(40)), "inconsistent"),
                ("unexpected", json!(true), "malformed"),
            ] {
                let mut malformed = valid.clone();
                malformed[field] = wrong;
                write(&serde_json::to_string(&malformed).map_err(|error| error.to_string())?)?;
                let error = refusal(
                    read_receipt(&repo, "ab", (2048, 512), &base, &head, &"c".repeat(40)),
                    field,
                )?;
                assert!(error.contains(expected), "{field}: {error}");
            }
            for malformed in [
                format!("{text} {text}"),
                text.replacen("\"nonce\":\"ab\"", "\"nonce\":\"ab\",\"nonce\":\"ab\"", 1),
                text.replacen(
                    "\"artifacts\":{",
                    &format!("\"artifacts\":{{\"{PR_CHECK_JSON}\":\"{digest}\","),
                    1,
                ),
            ] {
                assert_ne!(malformed, text);
                write(&malformed)?;
                let error = refusal(
                    read_receipt(&repo, "ab", (2048, 512), &base, &head, &"c".repeat(40)),
                    "duplicate or trailing receipt",
                )?;
                assert!(error.contains("malformed"), "{error}");
            }
            write(&text)?;

            #[cfg(target_os = "linux")]
            {
                // A FIFO is a real blocking-open counterexample, not a
                // serialization/setup failure. Both admission reads must
                // refuse before opening it.
                for relative in [PR_CHECK_SUBJECT_JSON, RECEIPT] {
                    fs::remove_file(repo.join(relative)).map_err(|error| error.to_string())?;
                    let args = vec![repo.join(relative).display().to_string()];
                    let output = capture_bytes_in_dir_with_budget(
                        Path::new("/usr/bin/mkfifo"),
                        &args,
                        (&repo, None),
                        &[],
                        ByteCaptureBudget {
                            timeout: Duration::from_secs(30),
                            stdout_bytes: STREAM_MAX,
                            stderr_bytes: STREAM_MAX,
                        },
                        "owned FIFO counterexample fixture",
                    )?;
                    assert!(
                        !output.timed_out && output.status.is_some_and(|status| status.success())
                    );
                    let error = refusal(
                        read_receipt(&repo, "ab", (2048, 512), &base, &head, &"c".repeat(40)),
                        "FIFO artifact",
                    )?;
                    assert!(error.contains("not a regular owned file"), "{error}");
                    fs::remove_file(repo.join(relative)).map_err(|error| error.to_string())?;
                    if relative == RECEIPT {
                        write(&text)?;
                    } else {
                        super::super::tests::write_repo_file(&repo, relative, "bounded artifact")?;
                    }
                }
            }
            super::super::tests::write_repo_file(&repo, PR_CHECK_JSON, "mutated")?;
            let error = refusal(
                read_receipt(&repo, "ab", (2048, 512), &base, &head, &"c".repeat(40)),
                "mutated artifact",
            )?;
            assert!(error.contains("does not match"), "{error}");
            fs::remove_file(repo.join(RECEIPT)).map_err(|error| error.to_string())?;
            let error = refusal(
                read_receipt(&repo, "ab", (2048, 512), &base, &head, &"c".repeat(40)),
                "missing receipt",
            )?;
            assert!(error.contains("unavailable"), "{error}");
            Ok(())
        })();
        fs::remove_dir_all(&repo).map_err(|error| error.to_string())?;
        result
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_experiment_uses_real_producer_and_refuses_consumers_then_recovers()
    -> Result<(), String> {
        let _cwd_guard = crate::acquire_test_cwd_read_guard();
        let binary = crate::reports::fixtures::ripr_fixture_binary()?;
        let repo = super::super::tests::temp_repo("ripr-complete-execution-native")?;
        let result = (|| {
            super::super::tests::run_git(
                &repo,
                &["-c", "init.templateDir=", "init", "--quiet", "-b", "trunk"],
            )?;
            for (key, value) in [
                ("user.name", "RIPR Experiment Fixture"),
                ("user.email", "experiment@example.invalid"),
                ("commit.gpgSign", "false"),
            ] {
                super::super::tests::run_git(&repo, &["config", key, value])?;
            }
            super::super::tests::write_repo_file(&repo, ".gitignore", "target/\n")?;
            super::super::tests::write_repo_file(
                &repo,
                "Cargo.toml",
                "[package]\nname = \"complete-experiment-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            )?;
            super::super::tests::write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 0 }\n",
            )?;
            super::super::tests::run_git(&repo, &["add", "-A"])?;
            super::super::tests::run_git(&repo, &["commit", "--quiet", "-m", "base"])?;
            super::super::tests::write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 1 }\n",
            )?;
            super::super::tests::run_git(&repo, &["commit", "--quiet", "-a", "-m", "predicate"])?;
            let options = PrEvidenceOptions {
                base: resolve_revision(&repo, "HEAD~1", "commit")?,
                head: resolve_revision(&repo, "HEAD", "commit")?,
                ..PrEvidenceOptions::default()
            };
            let ordinary = |check: bool| -> Result<bool, String> {
                let mut args = vec![
                    "pr-evidence".into(),
                    "--base".into(),
                    options.base.clone(),
                    "--head".into(),
                    options.head.clone(),
                ];
                if check {
                    args.push("--check".into());
                }
                let out = capture_bytes_in_dir_with_budget(
                    Path::new(&binary),
                    &args,
                    (&repo, None),
                    &[],
                    ByteCaptureBudget {
                        timeout: Duration::from_mins(2),
                        stdout_bytes: STREAM_MAX,
                        stderr_bytes: STREAM_MAX,
                    },
                    "ordinary producer equivalence control",
                )?;
                Ok(!out.timed_out && out.status.is_some_and(|status| status.success()))
            };
            let review = || {
                ripr::cli::run(vec![
                    "ripr".into(),
                    "review-comments".into(),
                    "--root".into(),
                    repo.display().to_string(),
                    "--base".into(),
                    options.base.clone(),
                    "--head".into(),
                    options.head.clone(),
                    "--check-output".into(),
                    repo.join(PR_CHECK_JSON).display().to_string(),
                    "--out".into(),
                    repo.join("target/review.json").display().to_string(),
                ])
            };
            assert!(ordinary(false)?);
            assert!(ordinary(true)?);
            check_pr_evidence(&repo, &options)?;
            // Subject-only marking defeats the packet-level fast path and
            // distinguishes both saved subject validators from the old code.
            let original_subject = fs::read_to_string(repo.join(PR_CHECK_SUBJECT_JSON))
                .map_err(|error| error.to_string())?;
            for generation in [
                Value::Null,
                json!({"coverage":"complete","production_admission":true}),
            ] {
                let mut subject: Value =
                    serde_json::from_str(&original_subject).map_err(|error| error.to_string())?;
                subject["experimental_complete_execution"] = generation;
                super::super::tests::write_repo_file(
                    &repo,
                    PR_CHECK_SUBJECT_JSON,
                    &serde_json::to_string(&subject).map_err(|error| error.to_string())?,
                )?;
                assert!(!ordinary(true)?);
                let error = refusal(
                    check_pr_evidence(&repo, &options),
                    "experimental saved check",
                )?;
                assert!(error.contains("experimental complete-execution"), "{error}");
                let error = refusal(
                    review().map_err(|error| error.message().to_string()),
                    "experimental strict review",
                )?;
                assert!(error.contains("experimental complete-execution"), "{error}");
            }
            super::super::tests::write_repo_file(&repo, PR_CHECK_SUBJECT_JSON, &original_subject)?;
            assert!(ordinary(true)?);
            check_pr_evidence(&repo, &options)?;
            review().map_err(|error| error.message().to_string())?;
            let baseline: Value = serde_json::from_slice(
                &fs::read(repo.join(PR_CHECK_JSON)).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(
                baseline
                    .pointer("/analysis_outcome/analysis_complete")
                    .and_then(Value::as_bool),
                Some(true)
            );
            assert!(
                baseline["findings"]
                    .as_array()
                    .is_some_and(|findings| !findings.is_empty())
            );
            let reviewed: Value = serde_json::from_slice(
                &fs::read(repo.join("target/review.json")).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            assert_eq!(
                reviewed["analysis_scope"]["basis"],
                "producer_check_projection"
            );
            assert!(
                reviewed["analysis_scope"]["classified_seams_considered"]
                    .as_u64()
                    .is_some_and(|count| count > 0)
            );
            let receipt = run_candidate(
                &repo,
                &binary,
                &options,
                Path::new("/usr/bin/prlimit"),
                profile()?,
            )?;
            assert_eq!(receipt.coverage, "not_established");
            let experimental: Value = serde_json::from_slice(
                &fs::read(repo.join(PR_CHECK_JSON)).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            for field in ["mode", "summary", "findings", "analysis_outcome"] {
                assert_eq!(
                    experimental[field], baseline[field],
                    "under-cap {field} semantics"
                );
            }
            assert!(!ordinary(true)?);
            let error = refusal(
                check_pr_evidence(&repo, &options),
                "experimental saved check",
            )?;
            assert!(error.contains("experimental complete-execution"), "{error}");
            let error = refusal(
                review().map_err(|error| error.message().to_string()),
                "experimental strict review",
            )?;
            assert!(error.contains("experimental complete-execution"), "{error}");
            let error = refusal(
                run_candidate(
                    &repo,
                    &binary,
                    &options,
                    Path::new("/missing-ripr-experiment-limiter"),
                    profile()?,
                ),
                "missing limiter",
            )?;
            assert!(
                error.starts_with("failed to run experimental complete-execution worker:"),
                "{error}"
            );
            assert!(!repo.join(PR_CHECK_SUBJECT_JSON).exists());
            assert!(!repo.join(RECEIPT).exists());
            let error = refusal(
                run_candidate(
                    &repo,
                    &binary,
                    &options,
                    Path::new("/usr/bin/prlimit"),
                    (0, FILE_MAX),
                ),
                "invalid resource profile",
            )?;
            assert!(error.contains("invalid finite resource profile"), "{error}");
            assert!(!repo.join(PR_CHECK_SUBJECT_JSON).exists());
            assert!(ordinary(false)?);
            assert!(ordinary(true)?);
            check_pr_evidence(&repo, &options)?;
            review().map_err(|error| error.message().to_string())?;
            Ok(())
        })();
        fs::remove_dir_all(&repo).map_err(|error| error.to_string())?;
        result
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "native memory-denial child, invoked by the bounded parent control"]
    fn bounded_memory_child() -> Result<(), String> {
        let text = fs::read_to_string("/proc/self/limits").map_err(|error| error.to_string())?;
        let limit = inherited_limit(&text, "Max address space", u64::MAX)?;
        if limit > 512 * 1024 * 1024 {
            return Err("memory child did not receive a finite tested ceiling".to_string());
        }
        println!("RIPR_NATIVE_LIMIT_VERIFIED");
        let mut bytes = Vec::<u8>::new();
        if bytes.try_reserve_exact(1024 * 1024 * 1024).is_err() {
            return Err("RIPR_NATIVE_ALLOCATION_REFUSED".to_string());
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_limit_denial_is_observed_after_successful_startup() -> Result<(), String> {
        let executable = env::current_exe().map_err(|error| error.to_string())?;
        let ceiling = profile()?.0.min(512 * 1024 * 1024);
        let args = vec![
            format!("--as={ceiling}:{ceiling}"),
            "--core=0:0".into(),
            "--".into(),
            executable.display().to_string(),
            "--exact".into(),
            "reports::pr_evidence::complete_execution::tests::bounded_memory_child".into(),
            "--ignored".into(),
            "--nocapture".into(),
            "--test-threads=1".into(),
        ];
        let output = capture_bytes_in_dir_with_budget(
            Path::new("/usr/bin/prlimit"),
            &args,
            (&repo_root()?, None),
            &[],
            ByteCaptureBudget {
                timeout: Duration::from_secs(30),
                stdout_bytes: STREAM_MAX,
                stderr_bytes: STREAM_MAX,
            },
            "native allocation-denial control",
        )?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!output.timed_out);
        assert!(output.status.is_some_and(|status| !status.success()));
        assert!(stdout.contains("RIPR_NATIVE_LIMIT_VERIFIED"), "{stdout}");
        assert!(
            stdout.contains("RIPR_NATIVE_ALLOCATION_REFUSED")
                || String::from_utf8_lossy(&output.stderr)
                    .contains("RIPR_NATIVE_ALLOCATION_REFUSED")
        );
        Ok(())
    }
}
