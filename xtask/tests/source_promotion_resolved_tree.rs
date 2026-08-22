use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempRoot(PathBuf);

impl TempRoot {
    fn create() -> Result<Self, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock predates Unix epoch: {error}"))?
            .as_nanos();
        for attempt in 0..128_u32 {
            let root = std::env::temp_dir().join(format!(
                "ripr-j5-network-policy-{}-{timestamp}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Ok(Self(root)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "failed to create J5 network-policy fixture root: {error}"
                    ));
                }
            }
        }
        Err("failed to allocate J5 network-policy fixture root".to_string())
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

fn write(root: &Path, relative: &str, contents: &str) -> Result<(), String> {
    let path = root.join(relative);
    let parent = path
        .parent()
        .ok_or_else(|| format!("fixture path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create fixture parent {}: {error}", parent.display()))?;
    fs::write(&path, contents)
        .map_err(|error| format!("write fixture {}: {error}", path.display()))
}

fn repeated_literal(literal: &str, count: usize) -> String {
    std::iter::repeat_n(literal, count)
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_network_policy(root: &Path) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("check-network-policy")
        .current_dir(root)
        .output()
        .map_err(|error| format!("run production network-policy checker: {error}"))
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn j5_source_ledger_rejects_and_semantic_reconciliation_passes() -> Result<(), String> {
    let fixture = TempRoot::create()?;
    let root = fixture.path();

    write(
        root,
        ".github/workflows/source-promotion-contract.yml",
        "curl",
    )?;
    write(
        root,
        "crates/ripr/src/analysis/probes/subprocess.rs",
        &repeated_literal("curl", 6),
    )?;
    write(
        root,
        "crates/ripr/src/output/typescript_packet_projection.rs",
        &repeated_literal("curl", 4),
    )?;

    write(
        root,
        ".github/workflows/server-archive-qualification.yml",
        "curl",
    )?;
    write(
        root,
        "crates/ripr/src/output/perl_gap_record_projection.rs",
        &repeated_literal("curl", 5),
    )?;
    write(root, "xtask/src/tests.rs", &repeated_literal("curl", 2))?;

    let source_only_ledger = r#"# Allowlisted network surfaces.
#
# Format:
# path|pattern|max_count|owner|reason
.github/workflows/source-promotion-contract.yml|curl|1|source|source-only live row
crates/ripr/src/analysis/probes/subprocess.rs|curl|6|source|source-only live row
crates/ripr/src/output/typescript_packet_projection.rs|curl|4|shared|shared live row
crates/ripr/src/output/typescript_packet_projection.rs|http|3|source|stale zero-count row
"#;
    write(root, "policy/network_allowlist.txt", source_only_ledger)?;

    let rejected = run_network_policy(root)?;
    assert!(
        !rejected.status.success(),
        "J5-shaped source-only ledger unexpectedly passed"
    );
    let rejection = combined_output(&rejected);
    for expected in [
        ".github/workflows/server-archive-qualification.yml",
        "crates/ripr/src/output/perl_gap_record_projection.rs",
        "xtask/src/tests.rs",
        "crates/ripr/src/output/typescript_packet_projection.rs",
        "http",
    ] {
        assert!(
            rejection.contains(expected),
            "J5-shaped rejection did not name {expected}: {rejection}"
        );
    }

    let reconciled_ledger = r#"# Allowlisted network surfaces.
#
# Format:
# path|pattern|max_count|owner|reason
.github/workflows/source-promotion-contract.yml|curl|1|source|source-only live row
.github/workflows/server-archive-qualification.yml|curl|1|swarm|W7-owned live row
crates/ripr/src/analysis/probes/subprocess.rs|curl|6|source|source-only live row
crates/ripr/src/output/perl_gap_record_projection.rs|curl|5|swarm|W7-owned live row
crates/ripr/src/output/typescript_packet_projection.rs|curl|4|shared|shared live row
xtask/src/tests.rs|curl|2|swarm|W7-owned live row
"#;
    write(root, "policy/network_allowlist.txt", reconciled_ledger)?;

    let accepted = run_network_policy(root)?;
    assert!(
        accepted.status.success(),
        "semantically reconciled ledger failed: {}",
        combined_output(&accepted)
    );
    Ok(())
}
