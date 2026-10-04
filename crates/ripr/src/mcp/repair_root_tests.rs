//! #5608: a rendered restart must retain the selected Unix directory identity.

use super::*;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};

struct RootFixture {
    root: FixtureRoot,
    parent: FixtureRoot,
    id: RepairAttemptId,
    decoy: PathBuf,
    foreign: PathBuf,
}

fn literal_root_fixture(label: &str) -> Result<RootFixture, String> {
    let parent = fixture_root(label)?;
    // These are distinct Unix filesystem names, not formatted report paths.
    let root = parent.join("team\\repo 'quoted'");
    std::fs::create_dir(&root).map_err(|error| error.to_string())?;
    let (root, id) = prepare_at(FixtureRoot(root))?;
    let decoy_parent = parent.join("team");
    std::fs::create_dir(&decoy_parent).map_err(|error| error.to_string())?;
    let decoy = decoy_parent.join("repo 'quoted'");
    std::fs::create_dir(&decoy).map_err(|error| error.to_string())?;
    write(&decoy.join("identity"), "different repository")?;
    git(&decoy, &["init"])?;
    let foreign = parent.join("foreign");
    std::fs::create_dir(&foreign).map_err(|error| error.to_string())?;
    assert_ne!(
        std::fs::metadata(&*root).map_err(|e| e.to_string())?.ino(),
        std::fs::metadata(&decoy).map_err(|e| e.to_string())?.ino()
    );
    Ok(RootFixture {
        root,
        parent,
        id,
        decoy,
        foreign,
    })
}

fn shell_argv(fixture: &RootFixture, command: &str) -> Result<Vec<Vec<u8>>, String> {
    let script = fixture.parent.join("capture.sh");
    // Observe what an actual POSIX shell passes; never execute a new repair.
    write(
        &script,
        &format!("ripr() {{ printf '%s\\000' \"$@\"; }}\n{command}\n"),
    )?;
    let output = std::process::Command::new("sh")
        .arg(&script)
        .current_dir(&fixture.foreign)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("required Unix shell failed to start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "shell capture failed: {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if output.stdout.last() != Some(&0) {
        return Err("shell capture did not emit nonempty NUL-separated arguments".to_string());
    }
    Ok(output.stdout[..output.stdout.len() - 1]
        .split(|byte| *byte == 0)
        .map(<[u8]>::to_vec)
        .collect())
}

fn selected_root_argv(
    fixture: &RootFixture,
    command: &str,
    subject_flag: &str,
    subject: &str,
    phase: &str,
) -> Result<(), String> {
    let argv = shell_argv(fixture, command)?;
    let expected = [
        b"agent".as_slice(),
        b"repair",
        b"--root",
        fixture.root.as_os_str().as_bytes(),
        subject_flag.as_bytes(),
        subject.as_bytes(),
        b"--phase",
        phase.as_bytes(),
    ];
    assert_eq!(argv, expected, "restart argv lost the selected Unix root");
    let selected = std::fs::metadata(&*fixture.root).map_err(|e| e.to_string())?;
    let received = std::fs::metadata(Path::new(std::ffi::OsStr::from_bytes(&argv[3])))
        .map_err(|e| e.to_string())?;
    assert_eq!(
        (received.dev(), received.ino()),
        (selected.dev(), selected.ino())
    );
    assert_eq!(
        std::fs::read(fixture.decoy.join("identity")).map_err(|e| e.to_string())?,
        b"different repository"
    );
    Ok(())
}

fn restart_root_parity(fixture: &RootFixture, class: &str) -> Result<(), String> {
    let document = selected_action_parity(
        &fixture.root,
        &fixture.id,
        class,
        Some("repair_attempt_before"),
    )?;
    let cli = crate::app::agent_status::build_agent_attempt_status(
        &fixture.root,
        &fixture.root,
        None,
        &fixture.id,
    )?;
    let cli_command = cli
        .next_action
        .as_ref()
        .ok_or_else(|| "missing CLI restart".to_string())?;
    let mcp_command = document["next_command"]
        .as_str()
        .ok_or_else(|| "missing MCP restart".to_string())?;
    for command in [cli_command.command.as_str(), mcp_command] {
        selected_root_argv(fixture, command, "--seam-id", "seam:freshness", "before")?;
    }
    // Recreate only the bad root-rendering operator as a separate control.
    // The real decoy exists, but its identity must not pass the primary oracle.
    let old_root = crate::agent::loop_commands::bound_root(
        &crate::agent::loop_commands::display_path(&fixture.root),
    );
    let wrong = format!(
        "ripr agent repair --root {} --seam-id seam:freshness --phase before",
        crate::agent::loop_commands::shell_arg(&old_root)
    );
    let wrong_argv = shell_argv(fixture, &wrong)?;
    assert_eq!(wrong_argv.len(), 8);
    assert_eq!(wrong_argv[3], fixture.decoy.as_os_str().as_bytes());
    let received = std::fs::metadata(Path::new(std::ffi::OsStr::from_bytes(&wrong_argv[3])))
        .map_err(|e| e.to_string())?;
    let selected = std::fs::metadata(&*fixture.root).map_err(|e| e.to_string())?;
    assert_ne!(
        (received.dev(), received.ino()),
        (selected.dev(), selected.ino())
    );
    Ok(())
}

#[test]
fn durable_selected_restart_failed_preserves_literal_unix_root() -> Result<(), String> {
    let fixture = literal_root_fixture("restart-failed")?;
    write(
        &fixture.root.join("tests/target.rs"),
        "#[test]\nfn focused() { assert_eq!(1, 1); }\n",
    )?;
    write(&fixture.root.join("outside.rs"), "pub fn forbidden() {}\n")?;
    let manifest = load_repair_attempt_manifest(&fixture.root, &fixture.id)?;
    let packet = find_manifest_artifact_by_role(&manifest, "agent_packet")
        .ok_or_else(|| "missing prepared packet".to_string())?;
    let after = finish_repair_attempt(
        &fixture.root,
        &fixture.id,
        &fixture.root.join(&packet.path),
        crate::edit_cage::HeadMovement::AdmitDescendantCommits,
    )?;
    assert!(after.current);
    assert_eq!(
        after.verdict.status,
        crate::edit_cage::EditCageVerdictStatus::Violated
    );
    assert_eq!(
        load_repair_attempt_manifest(&fixture.root, &fixture.id)?.state,
        RepairAttemptState::Failed
    );
    restart_root_parity(&fixture, "failed")
}

#[test]
fn durable_selected_restart_open_gap_preserves_literal_unix_root() -> Result<(), String> {
    let fixture = literal_root_fixture("restart-open-gap")?;
    let packet = finish_without_receipt(&fixture.root, &fixture.id)?;
    issue_terminal_receipt_with_grip(
        &fixture.root,
        &fixture.id,
        &packet,
        "weakly_gripped",
        "unchanged",
    )?;
    assert_eq!(
        receipt_document(&fixture.root, &fixture.id)?["status"],
        "unchanged"
    );
    restart_root_parity(&fixture, "limited")
}

#[test]
fn durable_selected_awaiting_literal_unix_root_retains_after_control() -> Result<(), String> {
    let fixture = literal_root_fixture("restart-awaiting-positive")?;
    let document = selected_action_parity(
        &fixture.root,
        &fixture.id,
        "awaiting_edit",
        Some("repair_attempt_after"),
    )?;
    let command = document["next_command"]
        .as_str()
        .ok_or_else(|| "missing awaiting continuation".to_string())?;
    selected_root_argv(&fixture, command, "--attempt", fixture.id.as_str(), "after")?;
    assert_eq!(document["command_routes"].as_array().map(Vec::len), Some(2));
    Ok(())
}
