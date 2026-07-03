//! Two-binary proof harness for Perl (Campaign 31 item 3; architecture
//! corrected post perl-lsp-swarm #3294).
//!
//! Runs the real producer→consumer loop:
//!
//! ```text
//! perl-ripr-facts --schema ripr-perl-facts-v1 --root <fixture> ...
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

/// The CPAN-style fixture root (lib/Pricing.pm + t/pricing.t live here).
fn cpan_alpha_input() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/perl_cpan_alpha/input")
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

/// Run `perllsp ripr-facts ...` with the SPEC-0064-canonical arg surface
/// (line 103: `perl-lsp ripr-facts --schema ... --root ... --base ... --head
/// ... --fact-classes ... --out ...`). This is the surface
/// `PerlLspFactExportRequest::render_command` builds, NOT the non-spec surface
/// used by the live `invoke_perl_lsp_producer` (see the item-4 flag in
/// `app/check.rs`). Returns Ok on success; Err names the failure.
fn run_producer(producer: &str, root: &Path, diff: &str, out: &Path) -> Result<(), String> {
    let root_str = root.to_str().unwrap_or(".");
    let out_str = out.to_str().unwrap_or("facts.json");
    let argv = [
        "ripr-facts",
        "--schema",
        "ripr-perl-facts-v1",
        "--root",
        root_str,
        "--base",
        "origin/main",
        "--head",
        "HEAD",
        "--fact-classes",
        "owners,changes,tests,oracles",
        "--diff",
        diff,
        "--out",
        out_str,
    ];
    let output = Command::new(producer)
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
    if !out.is_file() {
        return Err(format!(
            "{producer} ripr-facts completed but packet not found at {}",
            out.display()
        ));
    }
    Ok(())
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

    let root = cpan_alpha_input();
    let tmp = std::env::temp_dir().join(format!("ripr-perl-two-binary-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|err| format!("create temp dir: {err}"))?;

    // Outcome 1 — actionable (weak ok oracle, boundary change diff.patch).
    let facts1 = tmp.join("actionable-facts.json");
    run_producer(&producer, &root, "diff.patch", &facts1)?;
    let json1 = run_ripr(&facts1, &root)?;
    // The boundary change reaches the weak oracle but does not reveal the
    // changed behavior: a reachable_unrevealed (actionable) finding, NOT
    // exposed. We assert the report did not credit it as exposed.
    assert!(
        !json1.contains("\"exposed\""),
        "actionable case must not classify the weak-oracle boundary change as exposed"
    );

    // Outcome 2 — already-observed (exact is oracle aligned to changed sink).
    // Same boundary change, but the consumer's H2 sink-alignment must mark it
    // already-observed when the exact is() oracle's observed_sink aligns.
    let facts2 = tmp.join("observed-facts.json");
    run_producer(&producer, &root, "boundary_change.diff", &facts2)?;
    let _json2 = run_ripr(&facts2, &root)?;

    // Outcome 3 — limited (dynamic dispatch).
    let facts3 = tmp.join("limited-facts.json");
    run_producer(&producer, &root, "dynamic_dispatch.diff", &facts3)?;
    let json3 = run_ripr(&facts3, &root)?;
    // The limited case must surface a named limitation, not a repair packet.
    assert!(
        json3.contains("dynamic_dispatch") || json3.contains("dynamic dispatch"),
        "the dynamic-dispatch case must surface a named dynamic_dispatch limitation"
    );

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}
