mod agent;
mod command;
mod commands;
mod commands_agent_support;
mod commands_context;
mod commands_numeric;
mod commands_options;
mod commands_timestamps;
mod execute;
mod help;
mod parse;
mod rerun;
mod suggest;

use crate::agent::loop_commands::{
    WORKFLOW_AGENT_BRIEF_ARTIFACT, WORKFLOW_AGENT_PACKET_ARTIFACT,
    WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT, WORKFLOW_COMMANDS_MARKDOWN_ARTIFACT,
    WORKFLOW_MANIFEST_ARTIFACT,
};
use crate::app::repair_attempt::BeforeArtifactSource;
use std::fs::File;
use std::path::Path;

pub fn run(mut args: Vec<String>) -> Result<(), String> {
    let version_requested = parse::top_level_version_requested(&args);
    // #2610: extract --verbose before command dispatch so it works with any
    // subcommand. Version is a side-effect-free identity query, so it must not
    // emit the verbose diagnostic even when callers append or prepend it.
    if !version_requested && let Some(pos) = args.iter().position(|a| a == "--verbose" || a == "-v")
    {
        args.remove(pos);
        crate::set_verbose(true);
        eprintln!("ripr: verbose mode enabled");
    }
    // Selection is side-effect-free parsing; the lock is acquired before the
    // first side-effecting step (workflow execution and attempt publication),
    // so a lock loser fails closed without producing any workflow artifacts.
    let before_attempt = before_repair_attempt(&args)?;
    let _before_lock = before_attempt
        .as_ref()
        .map(|options| lock_before_repair_attempt(&options.root))
        .transpose()?;
    execute::execute(parse::parse_args(args)?)?;
    if let Some(options) = before_attempt {
        persist_before_repair_attempt(&options)?;
    }
    Ok(())
}

/// Serialize before-phase execution and attempt publication per repository.
/// The workflow artifacts under `target/ripr/workflow` are repository-global,
/// so a concurrent before phase could otherwise publish this invocation's
/// copies under its own attempt identity. Acquisition is non-blocking: a
/// concurrent before phase gets a bounded error instead of waiting on the
/// lock. The lock is an OS file-handle lock, so it is released on drop or
/// process exit and cannot go stale.
fn lock_before_repair_attempt(root: &Path) -> Result<File, String> {
    let directory = root.join(crate::app::repair_attempt::REPAIR_ATTEMPT_DIRECTORY);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("create {} failed: {error}", directory.display()))?;
    let lock_path = directory.join(".before.lock");
    let lock = File::create(&lock_path)
        .map_err(|error| format!("create {} failed: {error}", lock_path.display()))?;
    if let Err(error) = lock.try_lock() {
        if matches!(error, std::fs::TryLockError::WouldBlock) {
            return Err(
                "another before-phase repair attempt is in progress for this repository; retry after it finishes"
                    .to_string(),
            );
        }
        return Err(format!("lock {} failed: {error}", lock_path.display()));
    }
    Ok(lock)
}

fn before_repair_attempt(args: &[String]) -> Result<Option<agent::AgentRepairOptions>, String> {
    if args.get(1).map(String::as_str) != Some("agent")
        || args.get(2).map(String::as_str) != Some("repair")
    {
        return Ok(None);
    }
    match agent::parse_agent_args(&args[2..])? {
        agent::AgentCommand::Repair(options)
            if options.phase == agent::AgentRepairPhase::Before =>
        {
            Ok(Some(options))
        }
        _ => Ok(None),
    }
}

fn persist_before_repair_attempt(options: &agent::AgentRepairOptions) -> Result<(), String> {
    let root = &options.root;
    let seam_id = options
        .seam_id
        .as_deref()
        .ok_or_else(|| "before-phase repair attempt is missing its seam identity".to_string())?;
    let workflow_manifest = root.join(WORKFLOW_MANIFEST_ARTIFACT);
    let commands_markdown = root.join(WORKFLOW_COMMANDS_MARKDOWN_ARTIFACT);
    let agent_brief = root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT);
    let before_snapshot = root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT);
    let agent_packet = root.join(WORKFLOW_AGENT_PACKET_ARTIFACT);
    let packet_bytes = std::fs::read(&agent_packet)
        .map_err(|error| format!("read {} failed: {error}", agent_packet.display()))?;
    let packet_text = String::from_utf8(packet_bytes.clone())
        .map_err(|error| format!("agent packet is not UTF-8: {error}"))?;
    let policy = crate::app::repair_attempt::edit_cage_policy_from_packet(&packet_text, seam_id)?;
    let edit_cage_baseline = root.join("target/ripr/workflow/attempt-baseline.json");
    crate::app::repair_attempt::write_edit_cage_baseline(root, &edit_cage_baseline, &policy)?;

    // The Python repair-trust binding (#3568, RIPR-SPEC-0176) is verified
    // BEFORE the durable attempt is published: every drift, ambiguity, unsafe
    // surface, or missing authorization fails here, before any edit
    // transaction can begin. The verified record is then staged into the
    // attempt as a digest-pinned artifact. The binding's head pin is passed
    // into the publication transaction, so a HEAD move between preparation
    // and publication refuses before any attempt record exists.
    let binding = match &options.python_repair_trust {
        Some(selection) => Some(crate::app::python_repair_binding::prepare_binding(
            root,
            seam_id,
            &policy,
            &before_snapshot,
            &packet_bytes,
            selection,
            &options.edit_authorization,
        )?),
        None => None,
    };

    let mut sources = vec![
        BeforeArtifactSource {
            role: "workflow_manifest",
            path: &workflow_manifest,
        },
        BeforeArtifactSource {
            role: "commands_markdown",
            path: &commands_markdown,
        },
        BeforeArtifactSource {
            role: "agent_brief",
            path: &agent_brief,
        },
        BeforeArtifactSource {
            role: "before_snapshot",
            path: &before_snapshot,
        },
        BeforeArtifactSource {
            role: "agent_packet",
            path: &agent_packet,
        },
        BeforeArtifactSource {
            role: "edit_cage_baseline",
            path: &edit_cage_baseline,
        },
    ];
    if let Some(binding) = &binding {
        sources.push(crate::app::repair_attempt::BeforeArtifactSource {
            role: crate::app::python_repair_binding::BINDING_ARTIFACT_ROLE,
            path: &binding.record_path,
        });
    }
    let result = crate::app::repair_attempt::begin_repair_attempt_with(
        crate::app::repair_attempt::BeginRepairAttemptOptions {
            root,
            root_argument: root,
            seam_id,
            sources: &sources,
            expected_repository_head: binding
                .as_ref()
                .map(|binding| binding.verified.head.as_str()),
            // The after phase of a trust-bound attempt always re-verifies the
            // explicit authorization, so the published follow-up must name
            // the required flags with an explicit placeholder identity — the
            // driver never persists a granted authorization.
            next_command_suffix: binding
                .as_ref()
                .map(|_| " --edit-authorized --edit-authority <operator-or-agent-identity>"),
        },
    )?;
    if let Some(binding) = &binding {
        eprintln!(
            "ripr: python repair-trust binding staged for selection attempt `{}` (selection digest {})",
            binding.verified.attempt_id, binding.verified.selection_digest
        );
    }
    eprintln!(
        "ripr: repair attempt {} is awaiting the focused test edit",
        result.manifest.repair_attempt_id.as_str()
    );
    eprintln!("ripr: attempt manifest: {}", result.manifest_path.display());
    eprintln!(
        "ripr: attempt next command: {}",
        result.manifest.next_command
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn test_root(label: &str) -> Result<std::path::PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("test clock failed: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-before-lock-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("create {} failed: {error}", root.display()))?;
        Ok(root)
    }

    #[test]
    fn before_lock_fails_closed_while_held_and_releases_on_drop() -> Result<(), String> {
        let root = test_root("contention")?;
        let result = (|| -> Result<(), String> {
            let guard = lock_before_repair_attempt(&root)?;
            let second = lock_before_repair_attempt(&root);
            drop(guard);
            match second {
                Err(error) if error.contains("in progress") => {}
                other => {
                    return Err(format!(
                        "second before-phase lock acquisition was not rejected: {other:?}"
                    ));
                }
            }
            let reacquired = lock_before_repair_attempt(&root)?;
            drop(reacquired);
            Ok(())
        })();
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        result
    }

    #[test]
    fn before_lock_reports_an_unusable_lock_path() -> Result<(), String> {
        let root = test_root("unusable-path")?;
        let lock_path = root
            .join(crate::app::repair_attempt::REPAIR_ATTEMPT_DIRECTORY)
            .join(".before.lock");
        std::fs::create_dir_all(&lock_path).map_err(|error| {
            format!(
                "create lock collision {} failed: {error}",
                lock_path.display()
            )
        })?;
        let result = lock_before_repair_attempt(&root);
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        match result {
            Err(error) if error.contains("create") && error.contains(".before.lock") => Ok(()),
            other => Err(format!("unusable lock path was not reported: {other:?}")),
        }
    }

    #[test]
    fn before_repair_attempt_selects_only_the_before_phase() -> Result<(), String> {
        let before = before_repair_attempt(&args(&[
            "ripr",
            "agent",
            "repair",
            "--root",
            ".",
            "--seam-id",
            "seam:sample",
            "--phase",
            "before",
        ]))?;
        let Some(before) = before else {
            return Err("before phase was not selected for attempt persistence".to_string());
        };
        if before.seam_id.as_deref() != Some("seam:sample")
            || before.attempt_id.is_some()
            || before.phase != agent::AgentRepairPhase::Before
        {
            return Err(format!("unexpected before-phase options: {before:?}"));
        }

        let seam_after = before_repair_attempt(&args(&[
            "ripr",
            "agent",
            "repair",
            "--seam-id",
            "seam:sample",
            "--phase",
            "after",
        ]))?;
        if seam_after.is_some() {
            return Err(
                "seam-selected after phase attempted to create a new repair attempt".to_string(),
            );
        }
        let attempt_after = before_repair_attempt(&args(&[
            "ripr",
            "agent",
            "repair",
            "--attempt",
            "repair-attempt-0123456789abcdef01234567",
            "--phase",
            "after",
        ]))?;
        if attempt_after.is_some() {
            return Err(
                "attempt-selected after phase attempted to create a new repair attempt".to_string(),
            );
        }
        let unrelated = before_repair_attempt(&args(&["ripr", "check", "--help"]))?;
        if unrelated.is_some() {
            return Err("unrelated command attempted to create a repair attempt".to_string());
        }
        Ok(())
    }

    #[test]
    fn run_rejects_unknown_command() {
        assert_eq!(
            run(args(&["ripr", "unknown"])),
            Err("unknown command \"unknown\". Run `ripr --help`.".to_string())
        );
    }

    #[test]
    fn run_dispatches_check_parse_errors() {
        assert_eq!(
            run(args(&["ripr", "check", "--format", "xml"])),
            Err(
                "unknown format \"xml\"; see `ripr check --help` for the accepted formats"
                    .to_string()
            )
        );
    }

    #[test]
    fn run_dispatches_doctor_root_parse_errors() {
        assert_eq!(
            run(args(&["ripr", "doctor", "--root"])),
            Err("missing value for --root".to_string())
        );
    }

    #[test]
    fn run_dispatches_init_parse_errors() {
        assert_eq!(
            run(args(&["ripr", "init", "--root"])),
            Err("missing value for --root".to_string())
        );
    }

    #[test]
    fn run_dispatches_remaining_top_level_commands() {
        assert_eq!(run(args(&["ripr"])), Ok(()));
        assert_eq!(run(args(&["ripr", "--version"])), Ok(()));
        assert_eq!(
            run(args(&["ripr", "explain"])),
            Err("missing finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "context"])),
            Err("missing --at or --finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "diff", "--format", "xml"])),
            Err("unknown diff format \"xml\"; expected `human`, `text`, `md`, `markdown`, or `json`".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "lsp", "--bad"])),
            Err("unknown lsp argument \"--bad\". Run `ripr lsp --help`.".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "agent", "brief", "--diff", "change.diff"])),
            Err(
                "agent brief requires --json (the supported output for this subcommand)"
                    .to_string()
            )
        );
        assert_eq!(
            run(args(&["ripr", "first-pr", "--gap-ledger"])),
            Err("missing value for --gap-ledger".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "start-here", "--gap-ledger"])),
            Err("missing value for --gap-ledger".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "first-action", "--assistant-proof"])),
            Err("missing value for --assistant-proof".to_string())
        );
    }
}
