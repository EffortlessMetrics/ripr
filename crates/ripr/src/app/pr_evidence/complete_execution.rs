//! Inactive complete-execution resource experiment (#1627).
//! This runs the ordinary producer; it does not bypass its line/partial guards
//! and does not establish exhaustive canonical-history coverage.
use super::*;
use std::collections::BTreeMap;

pub(super) const WORKER_FLAG: &str = "--experimental-complete-execution-worker";
pub(super) const GENERATION_FIELD: &str = "experimental_complete_execution";
pub(super) const RECEIPT: &str = "target/ripr/pr/complete-execution.receipt.json";
const ADDRESS_SPACE_MAX: u64 = 2 * 1024 * 1024 * 1024;
const FILE_MAX: u64 = 256 * 1024 * 1024;
#[cfg(target_os = "linux")]
const LIMITS_MAX: u64 = 16 * 1024;

#[cfg(any(target_os = "linux", test))]
fn limits(text: &str, name: &str) -> Result<(u64, u64), String> {
    let mut rows = text.lines().filter_map(|line| line.strip_prefix(name));
    let row = rows
        .next()
        .ok_or_else(|| format!("experimental worker missing {name}"))?;
    if rows.next().is_some() {
        return Err(format!("experimental worker duplicate {name}"));
    }
    let values: Vec<_> = row.split_whitespace().collect();
    if values.len() != 3 || values[2] != "bytes" {
        return Err(format!("experimental worker malformed {name}"));
    }
    let number = |value: &str| {
        value
            .parse::<u64>()
            .map_err(|_| format!("experimental worker nonfinite {name}"))
    };
    Ok((number(values[0])?, number(values[1])?))
}

#[cfg(any(target_os = "linux", test))]
fn require_limits(text: &str, address_space: u64, file: u64) -> Result<(), String> {
    for (name, expected, maximum) in [
        ("Max address space", address_space, ADDRESS_SPACE_MAX),
        ("Max file size", file, FILE_MAX),
    ] {
        if expected == 0 || expected > maximum || limits(text, name)? != (expected, expected) {
            return Err(format!(
                "experimental worker {name} is not the requested finite soft/hard limit"
            ));
        }
    }
    if limits(text, "Max core file size")? != (0, 0) {
        return Err("experimental worker core-file limit is not zero".to_string());
    }
    Ok(())
}

fn verify_limits(address_space: u64, file: u64) -> Result<(), String> {
    if address_space == 0 || address_space > ADDRESS_SPACE_MAX || file == 0 || file > FILE_MAX {
        return Err("experimental worker invalid finite resource profile".to_string());
    }
    #[cfg(target_os = "linux")]
    {
        let mut text = String::new();
        fs::File::open("/proc/self/limits")
            .map_err(|error| {
                format!("experimental worker resource verification unavailable: {error}")
            })?
            .take(LIMITS_MAX + 1)
            .read_to_string(&mut text)
            .map_err(|error| {
                format!("experimental worker resource verification failed: {error}")
            })?;
        if text.len() as u64 > LIMITS_MAX {
            return Err(
                "experimental worker resource verification exceeds its byte bound".to_string(),
            );
        }
        require_limits(&text, address_space, file)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (address_space, file);
        Err("experimental complete-execution requires qualified Linux resource limits".to_string())
    }
}

pub(super) fn run_worker(args: &[String]) -> Result<(), String> {
    let address_space = args
        .first()
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| "experimental worker missing address-space bound".to_string())?;
    let file = args
        .get(1)
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| "experimental worker missing file bound".to_string())?;
    // Verify before repository/config/Git reads, analysis or full JSON conversion.
    verify_limits(address_space, file)?;
    let nonce = args
        .get(2)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or_else(|| "experimental worker missing or malformed generation nonce".to_string())?;
    let options = parse_options(&args[3..])?;
    if options.check || !options.base_explicit {
        return Err(
            "experimental worker requires an explicit producer base and cannot use --check"
                .to_string(),
        );
    }
    let repo = std::env::current_dir()
        .map_err(|error| format!("experimental worker current directory: {error}"))?;
    ensure_root_inside_invocation_repo(&repo, &options.root)?;
    let generation = json!({
        "schema_version": "ripr.complete_execution_experiment.v1",
        "nonce": nonce,
        "address_space_bytes": address_space,
        "file_bytes": file,
        "coverage": "not_established",
        "production_admission": false,
    });
    let result = (|| {
        write_pr_evidence_with_generation(&repo, &options, run_ripr_check, Some(&generation))?;
        let check: Value = serde_json::from_slice(
            &fs::read(repo.join(PR_CHECK_JSON))
                .map_err(|error| format!("experimental worker check read: {error}"))?,
        )
        .map_err(|error| format!("experimental worker check JSON: {error}"))?;
        if check
            .pointer("/analysis_outcome/analysis_complete")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(
                "experimental worker ordinary producer did not complete analysis".to_string(),
            );
        }
        let subject: Value = serde_json::from_slice(
            &fs::read(repo.join(PR_CHECK_SUBJECT_JSON))
                .map_err(|error| format!("experimental worker subject read: {error}"))?,
        )
        .map_err(|error| format!("experimental worker subject JSON: {error}"))?;
        if subject.get(GENERATION_FIELD) != Some(&generation) {
            return Err("experimental worker generation publication mismatch".to_string());
        }
        let mut artifacts = BTreeMap::new();
        for path in [
            PR_EVIDENCE_JSON,
            PR_EVIDENCE_MD,
            PR_CHECK_JSON,
            PR_CHECK_SUBJECT_JSON,
            PR_REVIEW_INPUT_JSON,
            PR_CANONICAL_DIFF,
        ] {
            let (digest, _) = digest_file(&repo.join(path))?;
            artifacts.insert(path, digest);
        }
        let receipt = json!({
            "schema_version": "ripr.complete_execution_experiment_receipt.v1",
            "nonce": nonce,
            "address_space_bytes": address_space,
            "file_bytes": file,
            "base_sha": subject["base_sha"],
            "head_sha": subject["head_sha"],
            "head_tree": subject["head_tree"],
            "check_sha256": subject["check_sha256"],
            "check_byte_count": subject["check_byte_count"],
            "canonical_diff_sha256": subject["canonical_diff_sha256"],
            "configuration_fingerprint": subject["configuration_fingerprint"],
            "index_entries": subject["canonical_finding_index_entry_count"],
            "index_bytes": subject["canonical_finding_index_byte_count"],
            "coverage": "not_established",
            "production_admission": false,
            "artifacts": artifacts,
        });
        let bytes = serde_json::to_vec(&receipt)
            .map_err(|error| format!("experimental worker receipt: {error}"))?;
        if bytes.len() > 16 * 1024 {
            return Err("experimental worker receipt exceeds its byte bound".to_string());
        }
        crate::atomic_file::write(&repo.join(RECEIPT), &bytes, RECEIPT)
    })();
    if result.is_err() {
        // A failure after publication cannot leave reusable authority.
        remove_stale_check_artifact(&repo)?;
        match fs::remove_file(repo.join(RECEIPT)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("experimental worker receipt revocation: {error}")),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_verification_refuses_missing_nonfinite_duplicate_and_mismatched_limits() {
        let valid = "Max address space         1073741824 1073741824 bytes\nMax file size             16777216 16777216 bytes\nMax core file size        0 0 bytes\n";
        assert!(require_limits(valid, 1073741824, 16777216).is_ok());
        for invalid in [
            valid.replace("1073741824 1073741824", "unlimited unlimited"),
            valid.replace("1073741824 1073741824", "1073741824 2147483648"),
            valid.replace("Max address space", "Max unknown"),
            format!("{valid}Max address space 1073741824 1073741824 bytes\n"),
            valid.replace("0 0 bytes", "0 1 bytes"),
        ] {
            assert!(require_limits(&invalid, 1073741824, 16777216).is_err());
        }
        assert!(require_limits(valid, 0, 16777216).is_err());
        assert!(require_limits(valid, ADDRESS_SPACE_MAX + 1, 16777216).is_err());
    }

    #[test]
    fn experimental_generation_is_rejected_even_when_null_or_structurally_complete() {
        for generation in [
            Value::Null,
            json!({"schema_version":"ripr.complete_execution_experiment.v1",
                "coverage":"complete", "production_admission":true}),
        ] {
            let mut packet = json!({"status":"ok"});
            packet[GENERATION_FIELD] = generation;
            assert!(reject_pr_evidence_error_packet(&packet).is_some());
        }
        assert!(reject_pr_evidence_error_packet(&json!({"status":"ok"})).is_none());
    }
}
