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
    fs::write(&path, contents).map_err(|error| format!("write fixture {}: {error}", path.display()))
}

fn repeated_literal(literal: &str, count: usize) -> String {
    std::iter::repeat_n(literal, count)
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_command(root: &Path, program: &Path, args: &[&str], label: &str) -> Result<Output, String> {
    Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("run {label}: {error}"))
}

fn run_git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = run_command(root, Path::new("git"), args, "Git fixture command")?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "Git fixture command failed: {}",
        combined_output(&output)
    ))
}

fn initialize_git_fixture(root: &Path) -> Result<(), String> {
    run_git(root, &["init", "--quiet"])?;
    run_git(root, &["add", "--all"])
}

fn run_network_policy(root: &Path) -> Result<Output, String> {
    run_command(
        root,
        Path::new(env!("CARGO_BIN_EXE_xtask")),
        &["check-network-policy"],
        "production network-policy checker",
    )
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
        ".github/workflows/stale-network-surface.yml",
        "name: stale network surface\n",
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
.github/workflows/stale-network-surface.yml|curl|3|source|stale zero-count row
crates/ripr/src/analysis/probes/subprocess.rs|curl|6|source|source-only live row
crates/ripr/src/output/typescript_packet_projection.rs|curl|4|shared|shared live row
"#;
    write(root, "policy/network_allowlist.txt", source_only_ledger)?;
    initialize_git_fixture(root)?;

    let rejected = run_network_policy(root)?;
    assert!(
        !rejected.status.success(),
        "J5-shaped source-only ledger unexpectedly passed"
    );
    let rejection = combined_output(&rejected);
    for expected in [
        ".github/workflows/server-archive-qualification.yml",
        ".github/workflows/stale-network-surface.yml",
        "crates/ripr/src/output/perl_gap_record_projection.rs",
        "xtask/src/tests.rs",
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

    let duplicate_control = format!(
        "{reconciled_ledger}{}",
        "xtask/src/tests.rs|curl|2|swarm|duplicate semantic key\n"
    );
    write(root, "policy/network_allowlist.txt", &duplicate_control)?;
    let duplicate = run_network_policy(root)?;
    assert!(
        !duplicate.status.success(),
        "duplicate semantic-key row unexpectedly passed"
    );

    let under_count_control = reconciled_ledger.replace(
        "crates/ripr/src/output/perl_gap_record_projection.rs|curl|5|swarm|W7-owned live row",
        "crates/ripr/src/output/perl_gap_record_projection.rs|curl|4|swarm|W7-owned live row",
    );
    write(root, "policy/network_allowlist.txt", &under_count_control)?;
    let under_count = run_network_policy(root)?;
    assert!(
        !under_count.status.success(),
        "actual count above maximum unexpectedly passed"
    );

    let orphan_control = format!(
        "{reconciled_ledger}{}",
        ".github/workflows/stale-network-surface.yml|curl|3|source|raw-union orphan\n"
    );
    write(root, "policy/network_allowlist.txt", &orphan_control)?;
    let orphan = run_network_policy(root)?;
    assert!(
        !orphan.status.success(),
        "raw-union orphan unexpectedly passed"
    );

    let removal_control = reconciled_ledger.replace(
        "crates/ripr/src/output/perl_gap_record_projection.rs|curl|5|swarm|W7-owned live row\n",
        "",
    );
    write(root, "policy/network_allowlist.txt", &removal_control)?;
    let removal = run_network_policy(root)?;
    assert!(
        !removal.status.success(),
        "removing a required semantic row did not falsify the reconciled control"
    );
    assert!(
        combined_output(&removal).contains("crates/ripr/src/output/perl_gap_record_projection.rs"),
        "removal control did not name the missing live row: {}",
        combined_output(&removal)
    );
    Ok(())
}