//! Two-binary proof harness for Perl (Campaign 31 item 3; architecture
//! corrected post perl-lsp-swarm #3294).
//!
//! Runs the real producer→consumer loop:
//!
//! ```text
//! perl-ripr-facts ripr-facts --schema ripr-perl-facts-v1 --root <fixture> ...
//!   | ripr check --perl-facts <out> --json
//! ```
//!
//! This is the harness that turns the Perl consumer from a scaffold into a
//! *working preview* once a real Perl facts exporter is available. The producer
//! (`perl-ripr-facts`, post perl-lsp-swarm #3294) is a thin batch CLI over
//! parser/workspace/semantic-facts crates — NOT the LSP server. ripr-swarm does
//! not build or vendor it. So this test is **gated on a Perl facts exporter
//! being on PATH**: when no exporter (`perl-ripr-facts`, `perllsp`, `perl-lsp`)
//! is found, the test skips cleanly with a diagnostic (following the `which()`
//! + graceful-degrade precedent in `cli/commands.rs`). When present, it runs
//!   all three outcome cases against the `fixtures/perl_cpan_alpha/input`
//!   CPAN-style project and asserts the honest outcome for each.
//!
//! The committed regression packets under
//! `fixtures/perl_cpan_alpha/expected/regression-packets/` are a SEPARATE,
//! producer-independent baseline (covered by lib tests). This harness is the
//! real-output proof; it is the milestone that depends on the
//! `perl-ripr-facts` producer reaching maturity.

#![cfg(feature = "lang-perl")]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// The CPAN-style fixture root (lib/Pricing.pm + t/pricing.t live here).
fn cpan_alpha_input() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/perl_cpan_alpha/input")
}

fn copy_dir_all(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|err| format!("create {}: {err}", to.display()))?;
    let entries =
        std::fs::read_dir(from).map_err(|err| format!("read {}: {err}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read entry in {}: {err}", from.display()))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|err| format!("file type {}: {err}", source.display()))?;
        if file_type.is_dir() {
            copy_dir_all(&source, &target)?;
        } else if file_type.is_file() {
            std::fs::copy(&source, &target).map_err(|err| {
                format!("copy {} to {}: {err}", source.display(), target.display())
            })?;
        }
    }
    Ok(())
}

fn write_pricing_test(root: &Path, content: &str) -> Result<(), String> {
    std::fs::write(root.join("t").join("pricing.t"), content)
        .map_err(|err| format!("write t/pricing.t: {err}"))
}

/// A worktree-absolute path to the ripr binary under test (NOT a long `../`
/// that could escape to a stale main-checkout binary).
fn ripr_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ripr"))
}

/// Detect whether a Perl facts exporter binary is available. Resolution order:
/// 1. `PERL_RIPR_FACTS` / `RIPR_PERL_FACTS_EXPORTER` env var (explicit override)
/// 2. `perl-ripr-facts` on PATH (canonical producer post perl-lsp-swarm #3294)
/// 3. `perllsp` / `perl-lsp` on PATH (compatibility wrappers)
///
/// The assertion is packet-semantic — the harness does not care which binary
/// produced the packet, only that a valid `ripr-perl-facts-v1` JSON was emitted.
fn producer_on_path() -> Option<String> {
    // 1. Env override (highest priority).
    if let Ok(path) = std::env::var("PERL_RIPR_FACTS") {
        if !path.is_empty() {
            return Some(path);
        }
    }
    if let Ok(path) = std::env::var("RIPR_PERL_FACTS_EXPORTER") {
        if !path.is_empty() {
            return Some(path);
        }
    }
    // 2. Canonical exporter.
    if which("perl-ripr-facts") {
        return Some("perl-ripr-facts".to_string());
    }
    // 3. Compatibility wrappers.
    if which("perllsp") {
        Some("perllsp".to_string())
    } else if which("perl-lsp") {
        Some("perl-lsp".to_string())
    } else {
        None
    }
}

/// Mirror of `cli/commands.rs::which()` (which is private to that module).
fn which(bin: &str) -> bool {
    let lookup = if cfg!(unix) { "which" } else { "where" };
    Command::new(lookup)
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Run `<producer> ripr-facts ...` with the SPEC-0064-canonical arg surface
/// (`perl-ripr-facts ripr-facts --schema ... --root ... --base ... --head
/// ... --fact-classes ... --diff ... --out ...`). This is the surface
/// `PerlLspFactExportRequest::render_command` builds, NOT the non-spec surface
/// used by the live `invoke_perl_lsp_producer` (see the item-4 flag in
/// `app/check.rs`). Returns Ok on success; Err names the failure.
fn run_producer(producer: &str, root: &Path, diff: &str, out: &str) -> Result<PathBuf, String> {
    let out_path = root.join(out);
    let argv = [
        "ripr-facts",
        "--schema",
        "ripr-perl-facts-v1",
        "--root",
        ".",
        "--base",
        "origin/main",
        "--head",
        "HEAD",
        "--fact-classes",
        "owners,changes,tests,oracles",
        "--diff",
        diff,
        "--out",
        out,
    ];
    let output = Command::new(producer)
        .current_dir(root)
        .args(argv)
        .output()
        .map_err(|err| format!("spawn {producer}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{producer} ripr-facts failed (exit {:?})\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if !out_path.is_file() {
        return Err(format!(
            "{producer} ripr-facts completed but packet not found at {}",
            out_path.display()
        ));
    }
    Ok(out_path)
}

/// Run the worktree-absolute ripr binary with `check --perl-facts <out> --json`.
/// Returns the captured stdout on success; Err names the failure.
fn run_ripr(facts: &Path, root: &Path) -> Result<String, String> {
    let output = Command::new(ripr_bin())
        .arg("check")
        .arg("--root")
        .arg(root)
        .arg("--perl-facts")
        .arg(facts)
        .arg("--json")
        .output()
        .map_err(|err| format!("spawn ripr: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "ripr check failed (exit {:?})\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_report(stdout: &str) -> Result<Value, String> {
    serde_json::from_str(stdout).map_err(|err| format!("parse ripr check JSON: {err}"))
}

fn is_pricing_finding(finding: &Value) -> bool {
    finding
        .get("probe")
        .and_then(|probe| probe.get("file"))
        .and_then(Value::as_str)
        == Some("lib/Pricing.pm")
        && (has_string_recursive(finding, "calculate_discount")
            || has_string_recursive(finding, "$amount")
            || has_string_recursive(finding, "change:lib/Pricing.pm"))
}

fn has_pricing_classification(report: &Value, classification: &str) -> bool {
    report
        .get("findings")
        .and_then(Value::as_array)
        .is_some_and(|findings| {
            findings.iter().any(|finding| {
                is_pricing_finding(finding)
                    && finding.get("classification").and_then(Value::as_str) == Some(classification)
            })
        })
}

fn has_any_pricing_classification(report: &Value, classifications: &[&str]) -> bool {
    classifications
        .iter()
        .any(|classification| has_pricing_classification(report, classification))
}

fn has_key_recursive(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(map) => {
            map.contains_key(key) || map.values().any(|child| has_key_recursive(child, key))
        }
        Value::Array(items) => items.iter().any(|child| has_key_recursive(child, key)),
        _ => false,
    }
}

fn has_string_recursive(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Object(map) => map
            .values()
            .any(|child| has_string_recursive(child, needle)),
        Value::Array(items) => items
            .iter()
            .any(|child| has_string_recursive(child, needle)),
        _ => false,
    }
}

fn has_no_repair_packet(report: &Value) -> bool {
    !has_key_recursive(report, "canonical_gap")
        && !has_key_recursive(report, "canonical_gap_id")
        && !has_key_recursive(report, "perl_repair_packet")
}

#[test]
fn two_binary_proof_three_outcomes_against_real_perllsp() -> Result<(), String> {
    // GATE: this is the producer-dependent proof. If no perllsp is on PATH,
    // skip cleanly (not fail) and name the perl-lsp-swarm Phase B dependency.
    // Returning Ok(()) (rather than `#[ignore]`) follows the `which()` +
    // graceful-degrade precedent in cli/commands.rs and keeps the test in the
    // default run set, so it runs automatically once a Perl facts exporter is on PATH.
    let Some(producer) = producer_on_path() else {
        eprintln!(
            "SKIP perl_two_binary_harness: no Perl facts exporter on PATH \
             (`perl-ripr-facts`, `perllsp`, `perl-lsp` all absent). The real \
             two-binary proof depends on the perl-ripr-facts producer reaching \
             maturity (post perl-lsp-swarm #3294). The committed regression \
             packets under fixtures/perl_cpan_alpha/expected/regression-packets/ \
             remain the producer-independent consumer baseline (covered by lib tests)."
        );
        return Ok(());
    };

    let tmp = std::env::temp_dir().join(format!("ripr-perl-two-binary-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|err| format!("create temp dir: {err}"))?;
    let actionable_root = tmp.join("cpan-alpha-actionable");
    copy_dir_all(&cpan_alpha_input(), &actionable_root)?;
    write_pricing_test(
        &actionable_root,
        "use strict;\nuse warnings;\nuse Test::More;\nuse Pricing;\n\nok(Pricing::calculate_discount(100), 'discount applies');\n\ndone_testing();\n",
    )?;

    // Outcome 1 — actionable (weak ok oracle, boundary change diff.patch).
    let facts1 = run_producer(
        &producer,
        &actionable_root,
        "diff.patch",
        "actionable-facts.json",
    )?;
    let json1 = run_ripr(&facts1, &actionable_root)?;
    let report1 = parse_report(&json1)?;
    // The boundary change reaches the weak oracle but does not reveal the
    // changed behavior: a reachable_unrevealed (actionable) finding, NOT
    // exposed. We assert the report did not credit it as exposed.
    assert!(
        has_any_pricing_classification(&report1, &["reachable_unrevealed", "weakly_exposed"]),
        "actionable case must produce a Pricing reachable_unrevealed or weakly_exposed finding: {json1}"
    );
    assert!(
        !has_pricing_classification(&report1, "exposed"),
        "actionable case must not classify the weak-oracle Pricing boundary change as exposed: {json1}"
    );

    // Outcome 2 — already-observed (exact is oracle aligned to changed sink).
    // Same boundary change, but the consumer's H2 sink-alignment must mark it
    // already-observed when the exact is() oracle's observed_sink aligns.
    let observed_root = tmp.join("cpan-alpha-observed");
    copy_dir_all(&cpan_alpha_input(), &observed_root)?;
    write_pricing_test(
        &observed_root,
        "use strict;\nuse warnings;\nuse Test::More;\nuse Pricing;\n\nis(Pricing::calculate_discount(100), 90, 'discount is 10% at threshold');\n\ndone_testing();\n",
    )?;
    let facts2 = run_producer(
        &producer,
        &observed_root,
        "boundary_change.diff",
        "observed-facts.json",
    )?;
    let json2 = run_ripr(&facts2, &observed_root)?;
    let report2 = parse_report(&json2)?;
    assert!(
        has_pricing_classification(&report2, "exposed"),
        "already-observed case must classify the Pricing boundary change as exposed: {json2}"
    );
    assert!(
        has_no_repair_packet(&report2),
        "already-observed case must not emit a repair packet or canonical repair gap: {json2}"
    );

    // Outcome 3 — limited (dynamic dispatch).
    let root = tmp.join("cpan-alpha-limited");
    copy_dir_all(&cpan_alpha_input(), &root)?;
    let facts3 = run_producer(
        &producer,
        &root,
        "dynamic_dispatch.diff",
        "limited-facts.json",
    )?;
    let json3 = run_ripr(&facts3, &root)?;
    let report3 = parse_report(&json3)?;
    // The limited case must surface a named limitation, not a repair packet.
    assert!(
        has_string_recursive(&report3, "dynamic_dispatch")
            || has_string_recursive(&report3, "dynamic dispatch"),
        "the dynamic-dispatch case must surface a named dynamic_dispatch limitation: {json3}"
    );
    assert!(
        has_no_repair_packet(&report3),
        "dynamic-dispatch case must not emit a repair packet or canonical repair gap: {json3}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}
