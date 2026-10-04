//! Production-path proof for the #5213 resource-cost trace.
//!
//! The unit tests in `analysis::resource_cost` prove the record shape and the
//! platform mapping. These prove the two claims only a real child process can
//! establish: the receipt appears on stderr when the existing opt-in switch is
//! set, and stdout plus default stderr are byte-identical when it is not.

use std::path::{Path, PathBuf};
use std::process::Command;

const TRACE_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TRACE";
const RECEIPT_PREFIX: &str = "ripr_resource_cost_receipt ";

fn workspace_root() -> Result<PathBuf, String> {
    // CARGO_MANIFEST_DIR is <repo>/crates/ripr.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("cannot resolve repository root from {}", manifest.display()))
}

struct Run {
    stdout: Vec<u8>,
    stderr: String,
    status_ok: bool,
}

/// One spawn site. `trace` sets or clears the opt-in switch; every other
/// variable is inherited so the child sees the same repository the test does.
fn run_ripr(args: &[&str], trace: Option<&str>) -> Result<Run, String> {
    let root = workspace_root()?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_ripr"));
    command.args(args).current_dir(&root).env_remove(TRACE_ENV);
    if let Some(value) = trace {
        command.env(TRACE_ENV, value);
    }
    let output = command
        .output()
        .map_err(|error| format!("run exact ripr binary {args:?}: {error}"))?;
    Ok(Run {
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        status_ok: output.status.success(),
    })
}

fn receipts(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter_map(|line| line.strip_prefix(RECEIPT_PREFIX))
        .collect()
}

fn parse(body: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(body).map_err(|error| format!("receipt is not JSON: {error}\n{body}"))
}

/// The exact command `cargo xtask repo-exposure-latency-report` runs, so the
/// receipt is exercised on the production route rather than a test-only
/// spelling. The bounded gap fixture keeps the run small.
const CHECK_ARGS: &[&str] = &[
    "check",
    "--root",
    "fixtures/boundary_gap/input",
    "--format",
    "repo-exposure-json",
];

#[test]
fn tracing_off_leaves_stdout_and_stderr_byte_identical() -> Result<(), String> {
    let untraced = run_ripr(CHECK_ARGS, None)?;
    // The switch is presence-based, so an explicitly empty value is still on.
    // That is the pre-existing contract and this test must not blur it.
    let traced = run_ripr(CHECK_ARGS, Some("1"))?;

    if !untraced.status_ok || !traced.status_ok {
        return Err(format!(
            "both runs must produce output\nuntraced: {}\ntraced: {}",
            untraced.status_ok, traced.status_ok
        ));
    }
    // An empty receipt set is the claim here; the positive witness that the
    // feature exists at all is the traced run in the sibling test.
    if !receipts(&untraced.stderr).is_empty() {
        return Err(format!(
            "tracing off emitted a resource receipt on stderr:\n{}",
            untraced.stderr
        ));
    }
    if untraced.stdout != traced.stdout {
        return Err(format!(
            "stdout must be byte-identical with and without the trace switch\nuntraced {} bytes\ntraced {} bytes",
            untraced.stdout.len(),
            traced.stdout.len()
        ));
    }
    // Beyond the receipt, the only lines a traced run may add are the trace
    // family's own pre-existing phase lines. Everything else on stderr must be
    // identical, so the receipt does not smuggle in an unrelated diagnostic.
    let phase_prefix = "ripr_repo_exposure_latency ";
    let traced_other: Vec<&str> = traced
        .stderr
        .lines()
        .filter(|line| !line.starts_with(RECEIPT_PREFIX) && !line.starts_with(phase_prefix))
        .collect();
    let untraced_other: Vec<&str> = untraced.stderr.lines().collect();
    if traced_other != untraced_other {
        return Err(format!(
            "stderr gained a line outside the trace family\nuntraced:\n{}\ntraced:\n{}",
            untraced.stderr, traced.stderr
        ));
    }
    Ok(())
}

#[test]
fn tracing_on_emits_one_attributed_receipt_on_stderr() -> Result<(), String> {
    let traced = run_ripr(CHECK_ARGS, Some("1"))?;
    let receipts = receipts(&traced.stderr);
    if receipts.len() != 1 {
        return Err(format!(
            "expected exactly one resource receipt, got {}:\n{}",
            receipts.len(),
            traced.stderr
        ));
    }
    let receipt = parse(receipts[0])?;

    for field in [
        "schema_version",
        "observer",
        "observer_pid",
        "host_os",
        "host_arch",
        "cpu",
        "peak_resident_bytes",
    ] {
        if receipt.get(field).is_none() {
            return Err(format!("receipt omits `{field}`: {receipt}"));
        }
    }
    // Attribution: a reviewer must be able to tell whose cost these are, and
    // that it was measured inside the analyzed process rather than by the
    // harness that launched it.
    if receipt["observer"] != serde_json::json!("ripr_process_self") {
        return Err(format!("receipt must name its observer scope: {receipt}"));
    }
    let pid = receipt["observer_pid"]
        .as_u64()
        .ok_or_else(|| format!("observer_pid must be a number: {receipt}"))?;
    if pid == 0 || pid == std::process::id() as u64 {
        return Err(format!(
            "observer_pid must be the analyzed child, not 0 or the harness: {pid}"
        ));
    }
    if receipt["host_os"] != serde_json::json!(std::env::consts::OS) {
        return Err(format!(
            "receipt must name the host it observed on: {receipt}"
        ));
    }

    // Every number is observed or explicitly unavailable - never a bare 0 and
    // never a silently absent field.
    for field in ["cpu", "peak_resident_bytes"] {
        let entry = &receipt[field];
        let state = entry
            .get("state")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("{field} must carry an explicit state: {receipt}"))?;
        match state {
            "observed" => {
                if field == "peak_resident_bytes" && entry.get("value").is_none() {
                    return Err(format!("observed {field} must carry a value: {receipt}"));
                }
            }
            "unavailable" => {
                if entry
                    .get("reason")
                    .and_then(serde_json::Value::as_str)
                    .is_none()
                {
                    return Err(format!(
                        "unavailable {field} must name its reason: {receipt}"
                    ));
                }
            }
            other => return Err(format!("{field} has unknown state `{other}`: {receipt}")),
        }
    }

    // Linux and Windows are both wired hosts, so both must observe. This is
    // the branch that would otherwise have papered over a missing Windows
    // implementation behind an "unavailable" state.
    let expected_unit = match std::env::consts::OS {
        "linux" => "linux_user_hz_clock_ticks",
        "windows" => "windows_hundred_nanoseconds",
        // An unwired target must name the reason rather than print zeros.
        other => {
            for field in ["cpu", "peak_resident_bytes"] {
                if receipt[field]["state"] != serde_json::json!("unavailable")
                    || receipt[field]["reason"] != serde_json::json!("platform_not_supported")
                {
                    return Err(format!(
                        "{other} must report {field} as platform_not_supported: {receipt}"
                    ));
                }
            }
            // The rendered analysis document is unchanged by the receipt: it
            // lives only on stderr.
            assert_stdout_is_the_document(&traced)?;
            return Ok(());
        }
    };
    let cpu = &receipt["cpu"];
    if cpu["state"] != serde_json::json!("observed") {
        return Err(format!(
            "{} must observe its own CPU time: {receipt}",
            std::env::consts::OS
        ));
    }
    // An observed CPU must carry the raw source, its unit, and the rate, so
    // the millisecond fields can be recomputed rather than trusted.
    for field in [
        "source_unit",
        "source_unit_per_second",
        "user_source",
        "system_source",
        "user_ms",
        "system_ms",
    ] {
        if cpu.get(field).is_none() {
            return Err(format!("observed CPU omits `{field}`: {receipt}"));
        }
    }
    if cpu["source_unit"] != serde_json::json!(expected_unit) {
        return Err(format!(
            "{} must name `{expected_unit}` as its CPU source unit: {receipt}",
            std::env::consts::OS
        ));
    }
    // Recompute the millisecond fields from the raw source and the named
    // rate. A producer that emitted a plausible-looking number the raw value
    // does not support fails here.
    let rate = cpu["source_unit_per_second"]
        .as_u64()
        .ok_or_else(|| format!("source_unit_per_second must be a number: {receipt}"))?;
    for (source_field, ms_field) in [("user_source", "user_ms"), ("system_source", "system_ms")] {
        let source = cpu[source_field]
            .as_u64()
            .ok_or_else(|| format!("{source_field} must be a number: {receipt}"))?;
        let reported = cpu[ms_field]
            .as_u64()
            .ok_or_else(|| format!("{ms_field} must be a number: {receipt}"))?;
        let expected_ms = source.saturating_mul(1000) / rate.max(1);
        if reported != expected_ms {
            return Err(format!(
                "{ms_field} must equal {source_field} * 1000 / {rate}, got {reported}, expected {expected_ms}: {receipt}"
            ));
        }
    }
    if receipt["peak_resident_bytes"]["state"] != serde_json::json!("observed") {
        return Err(format!(
            "{} must observe its own peak resident set: {receipt}",
            std::env::consts::OS
        ));
    }
    // A peak of zero would pass the state checks above while reporting nothing.
    let peak = receipt["peak_resident_bytes"]["value"]
        .as_u64()
        .ok_or_else(|| format!("peak_resident_bytes must carry a byte count: {receipt}"))?;
    if peak == 0 {
        return Err(format!(
            "peak resident set must be a real byte count: {receipt}"
        ));
    }

    assert_stdout_is_the_document(&traced)?;
    Ok(())
}

fn assert_stdout_is_the_document(traced: &Run) -> Result<(), String> {
    if !traced.stdout.starts_with(b"{") {
        return Err(format!(
            "stdout must remain the analysis document, got: {}",
            String::from_utf8_lossy(&traced.stdout[..traced.stdout.len().min(200)])
        ));
    }
    Ok(())
}
