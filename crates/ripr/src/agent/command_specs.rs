//! Producer-owned typed descriptions for the canonical agent routes.

use super::loop_commands::{
    agent_brief_command, agent_packet_command, agent_receipt_command, agent_verify_command,
    shell_arg,
};
use crate::domain::{
    CancellationPolicy, CommandAuthorityBoundary, CommandCostClass, CommandExecutionMode,
    CommandPlatform, CommandRole, CommandSpec, EnvironmentPolicy, ExpectedResultParser,
    NetworkPolicy, StdinPolicy,
};

/// Compatibility command displays are usable only when they carry content.
///
/// Typed [`CommandSpec`] validation remains the execution authority. This
/// predicate owns the narrower legacy-display readiness contract shared by
/// producers and consumers that still carry compatibility strings.
pub(crate) fn command_display_is_nonblank(display: &str) -> bool {
    !display.trim().is_empty()
}

/// A legacy command-display list is complete only when every entry is usable.
/// Rejecting mixed blank/nonblank lists avoids letting a valid first entry
/// hide a content-free route later in the same producer-owned collection.
pub(crate) fn command_displays_are_complete(displays: &[String]) -> bool {
    !displays.is_empty()
        && displays
            .iter()
            .all(|display| command_display_is_nonblank(display))
}

pub(crate) fn agent_verify_command_spec(
    root: &str,
    before_path: &str,
    after_path: &str,
    out_path: Option<&str>,
) -> CommandSpec {
    let display = agent_verify_command(root, before_path, after_path, out_path);
    let expected_writes = out_path.into_iter().map(ToOwned::to_owned).collect();
    command_spec(
        "ripr:agent:verify",
        CommandRole::Verify,
        if out_path.is_some() {
            CommandExecutionMode::ShellRequired
        } else {
            CommandExecutionMode::Direct
        },
        vec![
            "agent".to_string(),
            "verify".to_string(),
            "--root".to_string(),
            root.to_string(),
            "--before".to_string(),
            before_path.to_string(),
            "--after".to_string(),
            after_path.to_string(),
            "--json".to_string(),
        ],
        expected_writes,
        display,
    )
}

pub(crate) fn agent_receipt_command_spec(
    root: &str,
    verify_json: &str,
    seam_id: &str,
    out_path: Option<&str>,
) -> CommandSpec {
    let display = agent_receipt_command(root, verify_json, seam_id, out_path);
    let mut args = vec![
        "agent".to_string(),
        "receipt".to_string(),
        "--root".to_string(),
        root.to_string(),
        "--verify-json".to_string(),
        verify_json.to_string(),
        "--seam-id".to_string(),
        seam_id.to_string(),
        "--json".to_string(),
    ];
    let expected_writes = out_path
        .map(|path| {
            args.extend(["--out".to_string(), path.to_string()]);
            vec![path.to_string()]
        })
        .unwrap_or_default();
    command_spec(
        "ripr:agent:receipt",
        CommandRole::Receipt,
        CommandExecutionMode::Direct,
        args,
        expected_writes,
        display,
    )
}

/// The retained-artifact routes a regeneration command rebuilds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentArtifactRoute {
    /// `ripr agent packet` — the seam packet document.
    Packet,
    /// `ripr agent brief` — the seam brief document.
    Brief,
}

impl AgentArtifactRoute {
    fn command_word(self) -> &'static str {
        match self {
            AgentArtifactRoute::Packet => "packet",
            AgentArtifactRoute::Brief => "brief",
        }
    }

    fn command_id(self) -> &'static str {
        match self {
            AgentArtifactRoute::Packet => "ripr:agent:packet",
            AgentArtifactRoute::Brief => "ripr:agent:brief",
        }
    }
}

/// FIX #1617: the regeneration route rebuilds a retained artifact document
/// into a file — the `> <out>` redirect is shell semantics, so the route is
/// `ShellRequired` and names the file as its expected write.
pub(crate) fn agent_regeneration_command_spec(
    route: AgentArtifactRoute,
    root: &str,
    seam_id: &str,
    out_path: &str,
) -> CommandSpec {
    let display = match route {
        AgentArtifactRoute::Packet => agent_packet_command(root, seam_id, out_path),
        AgentArtifactRoute::Brief => agent_brief_command(root, seam_id, out_path),
    };
    command_spec(
        route.command_id(),
        CommandRole::Regeneration,
        CommandExecutionMode::ShellRequired,
        vec![
            "agent".to_string(),
            route.command_word().to_string(),
            "--root".to_string(),
            root.to_string(),
            "--seam-id".to_string(),
            seam_id.to_string(),
            "--json".to_string(),
        ],
        vec![out_path.to_string()],
        display,
    )
}

/// FIX #1617: the inspection route reads the same document on stdout — no
/// redirect, no writes, directly executable.
/// Consumed by the #1617 follow-up slices (hover/protocol surfaces) once
/// they carry the inspection route; the stdout display and recovery are
/// already pinned here so the contract cannot drift before wiring.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "wired by the #1617 slice that carries inspection routes"
    )
)]
pub(crate) fn agent_inspection_command_spec(
    route: AgentArtifactRoute,
    root: &str,
    seam_id: &str,
) -> CommandSpec {
    // The display is formatted here rather than in loop_commands: the
    // inspection producer is the only consumer until its surface slice
    // wires it, and loop_commands is for builders with live callers.
    let display = format!(
        "ripr agent {} --root {} --seam-id {} --json",
        route.command_word(),
        shell_arg(root),
        shell_arg(seam_id)
    );
    command_spec(
        route.command_id(),
        CommandRole::Inspection,
        CommandExecutionMode::Direct,
        vec![
            "agent".to_string(),
            route.command_word().to_string(),
            "--root".to_string(),
            root.to_string(),
            "--seam-id".to_string(),
            seam_id.to_string(),
            "--json".to_string(),
        ],
        Vec::new(),
        display,
    )
}

/// The artifact-input flags `pr-review front-panel` accepts. FIX (round-2
/// review): mirrors `parse_pr_review_front_panel_options` — the panel
/// requires at least one of these, so a display without any input flag is
/// not a runnable front-panel command and must not gain typed authority.
const FRONT_PANEL_INPUT_FLAGS: [&str; 12] = [
    "--pr-guidance",
    "--first-action",
    "--assistant-proof",
    "--assistant-health",
    "--ledger",
    "--baseline-delta",
    "--zero-status",
    "--gate-decision",
    "--recommendation-calibration",
    "--mutation-calibration",
    "--coverage-frontier",
    "--receipt",
];

/// The report-regeneration routes whose displays the ledger carries:
/// strict per-route flag templates owned here, so recovery is producer
/// knowledge rather than token guessing (FIX #1617 slice 2).
const REPORT_REGENERATION_ROUTES: [(&str, &[&str]); 2] = [
    (
        "front-panel",
        &[
            "--root",
            "--pr-guidance",
            "--first-action",
            "--assistant-proof",
            "--assistant-health",
            "--ledger",
            "--baseline-delta",
            "--zero-status",
            "--gate-decision",
            "--recommendation-calibration",
            "--mutation-calibration",
            "--coverage-frontier",
            "--receipt",
            "--out",
            "--out-md",
        ],
    ),
    (
        "index",
        &[
            "--root",
            "--reports-dir",
            "--review-dir",
            "--receipts-dir",
            "--workflow-dir",
            "--agent-dir",
            "--pilot-dir",
            "--ci-dir",
            "--out",
            "--out-md",
        ],
    ),
];

/// The loop-template facts for one report-regeneration route. FIX (round-1
/// review): the lookup is by route word — never by table position — so
/// reordering `REPORT_REGENERATION_ROUTES` cannot silently swap a route's
/// template. FIX (round-2 review): a route may require all of one flag set
/// (`required`) and at least one of another (`requires_any`).
struct ReportRouteTemplate {
    command_id: &'static str,
    allowed: &'static [&'static str],
    required: &'static [&'static str],
    requires_any: &'static [&'static str],
}

fn report_route_template(command_word: &str) -> Option<ReportRouteTemplate> {
    let allowed = REPORT_REGENERATION_ROUTES
        .iter()
        .find(|(route, _)| *route == command_word)
        .map(|(_, allowed)| *allowed)?;
    let no_flags: &[&str] = &[];
    let (command_id, required, requires_any) = match command_word {
        "front-panel" => (
            "ripr:pr-review:front-panel",
            no_flags,
            &FRONT_PANEL_INPUT_FLAGS[..],
        ),
        "index" => (
            "ripr:reports:index",
            &["--root", "--reports-dir"][..],
            no_flags,
        ),
        _ => return None,
    };
    Some(ReportRouteTemplate {
        command_id,
        allowed,
        required,
        requires_any,
    })
}

/// FIX #1617 slice 2: recover a typed regeneration spec for the canonical
/// report-regeneration routes (`ripr pr-review front-panel`, `ripr reports
/// index`, `ripr reports gap-ledger`, and the `ripr check …
/// --format repo-exposure-json` redirect). Loop-template routes accept
/// their closed flag set in any order without repeats and require the
/// route's mandatory flags; the gap-ledger and check routes match their
/// exact token shape. Values are taken positionally exactly as the CLI
/// parses them. The report routes write their `--out`/`--out-md` outputs
/// themselves (`Direct`); the check route writes through a shell redirect
/// (`ShellRequired`, redirect target as the expected write).
pub(crate) fn report_regeneration_command_spec_from_display(command: &str) -> Option<CommandSpec> {
    let words = shell_words(command)?;
    if words.first().map(String::as_str) != Some("ripr") {
        return None;
    }
    match words.get(1).map(String::as_str) {
        // Route word at index 1: the repo-exposure check route.
        Some("check") => recover_check_repo_exposure_spec(&words, command),
        Some("reports") => match words.get(2).map(String::as_str) {
            Some("gap-ledger") => recover_gap_ledger_spec(&words, command),
            Some("index") => recover_loop_template_spec(&words, command, "index"),
            _ => None,
        },
        Some("pr-review") if words.get(2).map(String::as_str) == Some("front-panel") => {
            recover_loop_template_spec(&words, command, "front-panel")
        }
        _ => None,
    }
}

/// Loop-template recovery: flags must belong to the route's closed
/// template, may appear in any order, must not repeat, and the route's
/// mandatory flags (all of `required`, at least one of `requires_any`)
/// must be present. Values are taken positionally exactly as the CLI
/// parses them; a flag-shaped token where a value belongs means the value
/// is missing, so recovery fails closed. FIX (round-1 review): the
/// recovered spec passes the same validation a producer spec must, so a
/// traversing or absolute output path stays legacy-string-only.
fn recover_loop_template_spec(
    words: &[String],
    command: &str,
    command_word: &str,
) -> Option<CommandSpec> {
    let template = report_route_template(command_word)?;
    let mut args = vec![words[1].to_string(), words[2].to_string()];
    let mut expected_writes = Vec::new();
    let mut seen = Vec::new();
    let mut index = 3;
    while index < words.len() {
        let token = words.get(index)?;
        if !template.allowed.contains(&token.as_str()) {
            return None;
        }
        if seen.contains(&token.as_str()) {
            return None;
        }
        seen.push(token.as_str());
        args.push(token.clone());
        let value = words.get(index + 1)?;
        if value.starts_with("--") && template.allowed.contains(&value.as_str()) {
            return None;
        }
        args.push(value.clone());
        if token == "--out" || token == "--out-md" {
            expected_writes.push(value.clone());
        }
        index += 2;
    }
    for required in template.required {
        if !seen.contains(required) {
            return None;
        }
    }
    if !template.requires_any.is_empty()
        && !template
            .requires_any
            .iter()
            .any(|required| seen.contains(required))
    {
        return None;
    }
    let spec = command_spec(
        template.command_id,
        CommandRole::Regeneration,
        CommandExecutionMode::Direct,
        args,
        expected_writes,
        command.to_string(),
    );
    if spec.validate().is_err() {
        return None;
    }
    Some(spec)
}

/// The check `--mode` vocabulary, mirroring `cli/parse/mode.rs`.
const CHECK_MODE_VALUES: [&str; 5] = ["instant", "draft", "fast", "deep", "ready"];

/// Exact-shape recovery for `ripr check --root R --mode M --format
/// repo-exposure-json > OUT` (10 tokens). The redirect is shell semantics,
/// so the recovered spec is `ShellRequired` and names the redirect target
/// as its expected write; argv stops before the redirect.
fn recover_check_repo_exposure_spec(words: &[String], command: &str) -> Option<CommandSpec> {
    if words.len() != 10
        || words[2] != "--root"
        || words[4] != "--mode"
        || words[6] != "--format"
        || words[7] != "repo-exposure-json"
        || words[8] != ">"
    {
        return None;
    }
    // FIX (round-2 review): the mode value must be one the CLI actually
    // accepts (`cli/parse/mode.rs`) — an unsupported mode must not gain
    // typed authority. This also fails closed on a flag-shaped token where
    // the mode value belongs.
    if !CHECK_MODE_VALUES.contains(&words[5].as_str()) {
        return None;
    }
    let spec = command_spec(
        "ripr:check:repo-exposure",
        CommandRole::Regeneration,
        CommandExecutionMode::ShellRequired,
        words[1..8].to_vec(),
        vec![words[9].clone()],
        command.to_string(),
    );
    // A traversing or absolute redirect target fails validation, so the
    // route stays legacy-string-only (same fail-closed rule as the agent
    // artifact routes).
    if spec.validate().is_err() {
        return None;
    }
    Some(spec)
}

/// The exact `ripr reports gap-ledger` shapes the first-pr recovery
/// surfaces emit. The route writes its own `--out` JSON document plus the
/// Markdown output, so the recovered spec is `Direct` and names both actual
/// writes. FIX (round-3 review): when the display carries only `--out`, the
/// CLI defaults `--out-md` independently to
/// DEFAULT_GAP_DECISION_LEDGER_MD_OUT — not a with_extension derivation of
/// `--out` — so the default constant is what the expected writes name.
fn recover_gap_ledger_spec(words: &[String], command: &str) -> Option<CommandSpec> {
    // (argv, --out value, explicit --out-md value when present)
    let (args, out, out_md) = match words.len() {
        9 if words[3] == "--repo-exposure" && words[5] == "--out" && words[7] == "--out-md" => (
            words[1..9].to_vec(),
            words[6].as_str(),
            Some(words[8].as_str()),
        ),
        11 if words[3] == "--check-output"
            && words[5] == "--root"
            && words[7] == "--out"
            && words[9] == "--out-md" =>
        {
            (
                words[1..11].to_vec(),
                words[8].as_str(),
                Some(words[10].as_str()),
            )
        }
        7 if words[3] == "--repo-exposure" && words[5] == "--out" => {
            (words[1..7].to_vec(), words[6].as_str(), None)
        }
        9 if words[3] == "--check-output" && words[5] == "--root" && words[7] == "--out" => {
            (words[1..9].to_vec(), words[8].as_str(), None)
        }
        // Other shapes (including a display with no output flag at all)
        // fail closed: the exact positional contract cannot state the
        // route's writes.
        _ => return None,
    };
    let markdown_out = out_md.map(ToOwned::to_owned).unwrap_or_else(|| {
        crate::output::gap_decision_ledger::DEFAULT_GAP_DECISION_LEDGER_MD_OUT.to_string()
    });
    let spec = command_spec(
        "ripr:reports:gap-ledger",
        CommandRole::Regeneration,
        CommandExecutionMode::Direct,
        args,
        vec![out.to_string(), markdown_out],
        command.to_string(),
    );
    if spec.validate().is_err() {
        return None;
    }
    Some(spec)
}

/// Recover a typed spec only for canonical agent routes. Arbitrary
/// user-supplied test commands remain legacy advisory text until a producer
/// supplies their executable and argument boundary.
pub(crate) fn agent_command_spec_from_display(command: &str) -> Option<CommandSpec> {
    let words = shell_words(command)?;
    if words.iter().any(|word| {
        word.chars()
            .any(|character| matches!(character, ';' | '&' | '|' | '<' | '`' | '$'))
    }) {
        return None;
    }
    if words.first().map(String::as_str) != Some("ripr")
        || words.get(1).map(String::as_str) != Some("agent")
    {
        return None;
    }
    match words.get(2).map(String::as_str) {
        Some("verify") => {
            let redirect = words.iter().position(|word| word == ">");
            let (args, execution_mode, expected_writes) = match redirect {
                Some(redirect) => {
                    if redirect + 2 != words.len() {
                        return None;
                    }
                    let out_path = words.get(redirect + 1)?.as_str();
                    (
                        words.get(1..redirect)?.to_vec(),
                        CommandExecutionMode::ShellRequired,
                        vec![out_path.to_string()],
                    )
                }
                None => (
                    words.get(1..)?.to_vec(),
                    CommandExecutionMode::Direct,
                    Vec::new(),
                ),
            };
            if args.is_empty() {
                return None;
            }
            Some(command_spec(
                "ripr:agent:verify",
                CommandRole::Verify,
                execution_mode,
                args,
                expected_writes,
                command.to_string(),
            ))
        }
        Some("packet") | Some("brief") => {
            let route = if words[2] == "packet" {
                AgentArtifactRoute::Packet
            } else {
                AgentArtifactRoute::Brief
            };
            // FIX (round-1 review): the display is only granted typed
            // authority when it is byte-canonically shaped — exactly the
            // producer argv (stdout form, or the redirect form) with no
            // missing, extra, or reordered flags.
            // Values are matched positionally — exactly how the agent CLI
            // parses them (cli/parse::expect_value accepts any token,
            // dash-prefixed included) — so dash-prefixed roots, seam ids,
            // and output paths keep typed recovery (round-3 review).
            let flags_in_position = words.len() >= 8
                && words[3] == "--root"
                && words[5] == "--seam-id"
                && words[7] == "--json";
            let stdout_shape = words.len() == 8 && flags_in_position;
            let redirect_shape = words.len() == 10 && flags_in_position && words[8] == ">";
            if !stdout_shape && !redirect_shape {
                return None;
            }
            let redirect = words.iter().position(|word| word == ">");
            let (role, execution_mode, args, expected_writes) = match redirect {
                Some(redirect) => {
                    if redirect + 2 != words.len() {
                        return None;
                    }
                    let out_path = words.get(redirect + 1)?.as_str();
                    (
                        CommandRole::Regeneration,
                        CommandExecutionMode::ShellRequired,
                        words.get(1..redirect)?.to_vec(),
                        vec![out_path.to_string()],
                    )
                }
                None => (
                    CommandRole::Inspection,
                    CommandExecutionMode::Direct,
                    words.get(1..)?.to_vec(),
                    Vec::new(),
                ),
            };
            if args.is_empty() || args.iter().any(|arg| arg == "--out") {
                return None;
            }
            // FIX (round-4 review): a recovered spec passes the same
            // validation a producer spec must - a traversing or absolute
            // expected write stays legacy-string-only instead of gaining
            // typed authority.
            let spec = command_spec(
                route.command_id(),
                role,
                execution_mode,
                args,
                expected_writes,
                command.to_string(),
            );
            if spec.validate().is_err() {
                return None;
            }
            Some(spec)
        }
        Some("receipt") => {
            let args = words.get(1..)?.to_vec();
            if args.is_empty() || args.iter().any(|arg| arg == ">") {
                return None;
            }
            let mut expected_writes = Vec::new();
            let mut out_seen = false;
            for (index, arg) in args.iter().enumerate() {
                if arg != "--out" {
                    continue;
                }
                if out_seen {
                    return None;
                }
                let out_path = args.get(index + 1)?;
                if out_path.is_empty() || out_path == ">" {
                    return None;
                }
                expected_writes.push(out_path.clone());
                out_seen = true;
            }
            Some(command_spec(
                "ripr:agent:receipt",
                CommandRole::Receipt,
                CommandExecutionMode::Direct,
                args,
                expected_writes,
                command.to_string(),
            ))
        }
        _ => None,
    }
}

fn command_spec(
    command_id: &str,
    role: CommandRole,
    execution_mode: CommandExecutionMode,
    args: Vec<String>,
    expected_writes: Vec<String>,
    display: String,
) -> CommandSpec {
    CommandSpec {
        schema_version: CommandSpec::SCHEMA_VERSION.to_string(),
        command_id: command_id.to_string(),
        role,
        execution_mode,
        program: "ripr".to_string(),
        args,
        cwd: ".".to_string(),
        env_set: Vec::new(),
        env_passthrough: Vec::new(),
        environment_policy: EnvironmentPolicy::Clean,
        stdin: StdinPolicy::Null,
        timeout_ms: 120_000,
        cancellation: CancellationPolicy::Allowed,
        network_policy: NetworkPolicy::Forbidden,
        expected_result_parser: ExpectedResultParser::DeclaredJson,
        expected_exit_codes: vec![0],
        expected_writes,
        cost_class: CommandCostClass::Unknown,
        platforms: vec![
            CommandPlatform::Linux,
            CommandPlatform::Macos,
            CommandPlatform::Windows,
        ],
        display,
        authority_boundary: match role {
            CommandRole::Verify => CommandAuthorityBoundary::VerificationRouteOnly,
            CommandRole::Receipt => CommandAuthorityBoundary::ReceiptRouteOnly,
            CommandRole::Regeneration => CommandAuthorityBoundary::RegenerationRouteOnly,
            CommandRole::Inspection => CommandAuthorityBoundary::InspectionRouteOnly,
            CommandRole::TargetedRerun => CommandAuthorityBoundary::TargetedRerunRouteOnly,
        },
    }
}

fn shell_words(command: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in command.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match (quote, character) {
            // #3231: inside single quotes a backslash is literal — the
            // renderer (`loop_commands::shell_arg`) documents bash
            // single-quote semantics, and the recovery parser must agree or
            // a single-quoted Windows-like token cannot round-trip. Outside
            // single quotes the backslash keeps its escape meaning (unquoted
            // and double-quoted behavior).
            (Some('\''), '\\') => current.push(character),
            (_, '\\') => escaped = true,
            (Some(active), value) if value == active => quote = None,
            (Some(_), ';' | '&' | '|' | '<' | '>' | '`' | '$') => {
                return None;
            }
            (Some(_), value) => current.push(value),
            (None, '\'' | '"') => quote = Some(character),
            (None, value) if value.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            (None, '>') => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
                words.push(">".to_string());
            }
            (None, value) => current.push(value),
        }
    }
    if escaped || quote.is_some() {
        return None;
    }
    if !current.is_empty() {
        words.push(current);
    }
    Some(words)
}

/// Shared role/boundary coherence assertion for the route specs.
#[cfg(test)]
fn ensure_role(
    spec: &crate::domain::CommandSpec,
    role: crate::domain::CommandRole,
    boundary: crate::domain::CommandAuthorityBoundary,
) -> Result<(), String> {
    if spec.role != role || spec.authority_boundary != boundary {
        return Err(format!(
            "spec `{}` carried role/boundary {:?}/{:?}, expected {:?}/{:?}",
            spec.command_id, spec.role, spec.authority_boundary, role, boundary
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::loop_commands::{
        WORKFLOW_AFTER_SNAPSHOT_ARTIFACT, WORKFLOW_AGENT_RECEIPT_ARTIFACT,
        WORKFLOW_AGENT_VERIFY_ARTIFACT, WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
    };

    #[test]
    fn legacy_command_display_readiness_rejects_empty_whitespace_and_mixed_lists()
    -> Result<(), String> {
        let valid = vec!["cargo test -p ripr".to_string(), "pytest tests".to_string()];
        if !command_displays_are_complete(&valid) {
            return Err("nonblank compatibility displays were rejected".to_string());
        }
        for invalid in [
            Vec::<String>::new(),
            vec!["".to_string()],
            vec![" \t ".to_string()],
            vec!["cargo test -p ripr".to_string(), "  ".to_string()],
        ] {
            if command_displays_are_complete(&invalid) {
                return Err(format!(
                    "content-free command list was accepted: {invalid:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn typed_agent_routes_preserve_argv_and_disclose_shell_redirection() -> Result<(), String> {
        let verify = agent_verify_command_spec(
            ".",
            WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
            WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
            Some(WORKFLOW_AGENT_VERIFY_ARTIFACT),
        );
        if verify.execution_mode != CommandExecutionMode::ShellRequired {
            return Err("verify redirect was not marked shell_required".to_string());
        }
        if verify.args.get(0..2) != Some(["agent".to_string(), "verify".to_string()].as_slice()) {
            return Err(format!("unexpected verify argv: {:?}", verify.args));
        }
        verify.validate().map_err(|err| err.to_string())?;

        let receipt = agent_receipt_command_spec(
            ".",
            WORKFLOW_AGENT_VERIFY_ARTIFACT,
            "seam-a",
            Some(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
        );
        if receipt.execution_mode != CommandExecutionMode::Direct {
            return Err("receipt --out route was not marked direct".to_string());
        }
        if !receipt
            .args
            .windows(2)
            .any(|pair| pair == ["--out", WORKFLOW_AGENT_RECEIPT_ARTIFACT])
        {
            return Err(format!("receipt argv omitted --out: {:?}", receipt.args));
        }
        receipt.validate().map_err(|err| err.to_string())
    }

    /// #3231: inside single quotes a backslash is literal, so a
    /// single-quoted Windows-like token round-trips byte-for-byte through
    /// the bounded recovery parser — the renderer (`shell_arg`) and the
    /// parser must agree on bash single-quote semantics. The token is built
    /// at runtime so no literal drive-letter path sits in the source tree.
    #[test]
    fn single_quoted_backslash_tokens_round_trip_through_recovery() -> Result<(), String> {
        let windows_root = format!("C:{}ws{}repo", '\\', '\\');
        let display = format!(
            "ripr agent verify --root '{windows_root}' --before before.json --after after.json --json"
        );
        let spec =
            agent_command_spec_from_display(&display).ok_or("quoted route was not recoverable")?;
        if !spec
            .args
            .windows(2)
            .any(|pair| pair == ["--root", windows_root.as_str()])
        {
            return Err(format!(
                "single-quoted Windows-like token lost its backslashes: {:?}",
                spec.args
            ));
        }
        // Renderer agreement: the same token rendered by `shell_arg` (inside
        // `agent_verify_command`) recovers to the identical argv entry.
        let rendered = crate::agent::loop_commands::agent_verify_command(
            &windows_root,
            "before.json",
            "after.json",
            None,
        );
        if !rendered.contains(&format!("'{windows_root}'")) {
            return Err(format!("renderer left the token unquoted: {rendered}"));
        }
        let round_tripped = agent_command_spec_from_display(&rendered)
            .ok_or("rendered route was not recoverable")?;
        if !round_tripped
            .args
            .windows(2)
            .any(|pair| pair == ["--root", windows_root.as_str()])
        {
            return Err(format!(
                "rendered token did not round-trip byte-for-byte: {:?}",
                round_tripped.args
            ));
        }
        Ok(())
    }

    /// #3231: outside single quotes the backslash keeps its escape meaning —
    /// unquoted and double-quoted tokens consume it as an escape, matching
    /// the pre-fix behavior the canonical display tokens rely on.
    #[test]
    fn backslash_outside_single_quotes_keeps_escape_semantics() -> Result<(), String> {
        if shell_words(r"a\b c") != Some(vec!["ab".to_string(), "c".to_string()]) {
            return Err("unquoted backslash must escape the next character".to_string());
        }
        if shell_words(r#""a\b" c"#) != Some(vec!["ab".to_string(), "c".to_string()]) {
            return Err("double-quoted backslash must escape the next character".to_string());
        }
        if shell_words(r"'a\b' c") != Some(vec!["a\\b".to_string(), "c".to_string()]) {
            return Err("single-quoted backslash must stay literal".to_string());
        }
        Ok(())
    }

    /// #3231 drift test: producer-owned typed routes carry the semantic
    /// Windows-like token directly in `args[]`, built from semantic inputs —
    /// they never parse their own display text through `shell_words`. A drift
    /// here would re-establish display text as argv authority (#1906).
    #[test]
    fn producer_owned_routes_preserve_windows_argv_without_display_parsing() -> Result<(), String> {
        let windows_root = format!("C:{}ws{}repo", '\\', '\\');
        let verify = agent_verify_command_spec(&windows_root, "before.json", "after.json", None);
        if !verify
            .args
            .windows(2)
            .any(|pair| pair == ["--root", windows_root.as_str()])
        {
            return Err(format!(
                "verify spec lost the semantic Windows token in args: {:?}",
                verify.args
            ));
        }
        let receipt = agent_receipt_command_spec(&windows_root, "verify.json", "seam-a", None);
        if !receipt
            .args
            .windows(2)
            .any(|pair| pair == ["--root", windows_root.as_str()])
        {
            return Err(format!(
                "receipt spec lost the semantic Windows token in args: {:?}",
                receipt.args
            ));
        }
        Ok(())
    }

    #[test]
    fn typed_route_recovery_rejects_arbitrary_test_commands() -> Result<(), String> {
        if agent_command_spec_from_display("cargo test pricing").is_some() {
            return Err("arbitrary test command was promoted without a producer".to_string());
        }
        if agent_command_spec_from_display(
            "ripr agent verify --root . --before before.json --after after.json --json && whoami",
        )
        .is_some()
        {
            return Err("compound shell command crossed the typed route boundary".to_string());
        }
        if agent_command_spec_from_display(
            "ripr agent verify --root . --before before.json --after after.json --json || whoami",
        )
        .is_some()
        {
            return Err(
                "alternate compound shell command crossed the typed route boundary".to_string(),
            );
        }
        if agent_command_spec_from_display(
            "ripr agent verify --root . --before before.json --after after.json --json \">\" verify.json",
        )
        .is_some()
        {
            return Err("quoted redirection operator crossed the typed route boundary".to_string());
        }
        let verify = agent_command_spec_from_display(
            "ripr agent verify --root . --before before.json --after after.json --json > verify.json",
        )
        .ok_or_else(|| "canonical verify route was not recoverable".to_string())?;
        if verify.execution_mode != CommandExecutionMode::ShellRequired {
            return Err("recovered verify redirect was not shell_required".to_string());
        }
        if verify.args.get(0..2) != Some(["agent".to_string(), "verify".to_string()].as_slice()) {
            return Err(format!(
                "recovered verify argv omitted route words: {:?}",
                verify.args
            ));
        }
        verify.validate().map_err(|err| err.to_string())?;

        let receipt = agent_command_spec_from_display(
            "ripr agent receipt --root . --verify-json verify.json --seam-id seam-a --json --out receipt.json",
        )
        .ok_or_else(|| "canonical receipt route was not recoverable".to_string())?;
        if receipt.args.get(0..2) != Some(["agent".to_string(), "receipt".to_string()].as_slice()) {
            return Err(format!(
                "recovered receipt argv omitted route words: {:?}",
                receipt.args
            ));
        }
        if receipt.expected_writes != ["receipt.json".to_string()] {
            return Err(format!(
                "recovered receipt output path was not preserved: {:?}",
                receipt.expected_writes
            ));
        }
        if agent_command_spec_from_display(
            "ripr agent receipt --root . --verify-json verify.json --seam-id seam-a --json --out",
        )
        .is_some()
        {
            return Err("receipt route accepted a missing --out path".to_string());
        }
        receipt.validate().map_err(|err| err.to_string())
    }

    /// FIX #1617: regeneration and inspection specs exist for the retained
    /// artifact routes, keep their roles and authority boundaries distinct
    /// from verify/receipt, and round-trip semantic argv through shell
    /// quoting (spaces, quotes, unicode) without argument loss.
    #[test]
    fn regeneration_and_inspection_specs_round_trip() -> Result<(), String> {
        let root = "./my repo";
        let seam_id = "seam:ünïcode \"quoted\"";
        let out_path = "target/ripr/artifacts/out file.json";

        let regeneration = super::agent_regeneration_command_spec(
            super::AgentArtifactRoute::Packet,
            root,
            seam_id,
            out_path,
        );
        ensure_role(
            &regeneration,
            crate::domain::CommandRole::Regeneration,
            crate::domain::CommandAuthorityBoundary::RegenerationRouteOnly,
        )?;
        if regeneration.execution_mode != crate::domain::CommandExecutionMode::ShellRequired {
            return Err("a redirect-writing regeneration route must be ShellRequired".to_string());
        }
        if regeneration.expected_writes != [out_path.to_string()] {
            return Err(format!(
                "regeneration expected writes must name the redirect target: {:?}",
                regeneration.expected_writes
            ));
        }
        // Semantic argv equality: the program args carry the values whole
        // (spaces, quotes, unicode included) — joining for the shell is the
        // display's job, never the spec's.
        let expected_args = vec![
            "agent".to_string(),
            "packet".to_string(),
            "--root".to_string(),
            root.to_string(),
            "--seam-id".to_string(),
            seam_id.to_string(),
            "--json".to_string(),
        ];
        if regeneration.args != expected_args {
            return Err(format!(
                "regeneration args must equal the semantic argv: {:?}",
                regeneration.args
            ));
        }
        // The display is produced by the same builder the legacy surfaces
        // render, so byte-parity with existing output is structural.
        if regeneration.display != super::agent_packet_command(root, seam_id, out_path) {
            return Err("regeneration display must match the legacy packet display".to_string());
        }
        regeneration.validate().map_err(|err| err.to_string())?;

        let inspection =
            super::agent_inspection_command_spec(super::AgentArtifactRoute::Brief, root, seam_id);
        ensure_role(
            &inspection,
            crate::domain::CommandRole::Inspection,
            crate::domain::CommandAuthorityBoundary::InspectionRouteOnly,
        )?;
        if inspection.execution_mode != crate::domain::CommandExecutionMode::Direct {
            return Err("a stdout inspection route must be Direct".to_string());
        }
        if !inspection.expected_writes.is_empty() {
            return Err("an inspection route must not declare writes".to_string());
        }
        inspection.validate().map_err(|err| err.to_string())?;

        // Roles are distinct slots: the four canonical ids never collide and
        // each carries its own role.
        let verify = super::agent_verify_command_spec(root, "before.json", "after.json", None);
        let receipt =
            super::agent_receipt_command_spec(root, "verify.json", seam_id, Some("r.json"));
        let ids = [
            (verify.command_id.as_str(), verify.role),
            (receipt.command_id.as_str(), receipt.role),
            (regeneration.command_id.as_str(), regeneration.role),
            (inspection.command_id.as_str(), inspection.role),
        ];
        for (index, (id, role)) in ids.iter().enumerate() {
            if ids[index + 1..]
                .iter()
                .any(|(other_id, other_role)| other_id == id || other_role == role)
            {
                return Err(format!("route id/role collision at `{id}` ({role:?})"));
            }
        }

        // Display recovery recognizes both artifact routes in both modes
        // and rejects their non-canonical shapes.
        let recovered = super::agent_command_spec_from_display(&regeneration.display)
            .ok_or("the regeneration display was not recoverable")?;
        if recovered.role != crate::domain::CommandRole::Regeneration
            || recovered.execution_mode != crate::domain::CommandExecutionMode::ShellRequired
            || recovered.expected_writes != [out_path.to_string()]
        {
            return Err("regeneration recovery lost the typed facts".to_string());
        }
        let recovered = super::agent_command_spec_from_display(&inspection.display)
            .ok_or("the inspection display was not recoverable")?;
        if recovered.role != crate::domain::CommandRole::Inspection
            || recovered.execution_mode != crate::domain::CommandExecutionMode::Direct
        {
            return Err("inspection recovery lost the typed facts".to_string());
        }
        if super::agent_command_spec_from_display(
            "ripr agent packet --root . --seam-id s --json --out out.json",
        )
        .is_some()
        {
            return Err(
                "artifact routes must not accept --out (the write is the redirect)".to_string(),
            );
        }
        if super::agent_command_spec_from_display(
            "ripr agent packet --root . --seam-id s --json > out.json extra",
        )
        .is_some()
        {
            return Err(
                "artifact routes must not accept a redirect with trailing tokens".to_string(),
            );
        }
        // FIX (round-4 review): a traversing expected write fails validation,
        // so recovery keeps the route legacy-string-only.
        if super::agent_command_spec_from_display(
            "ripr agent packet --root . --seam-id s --json > ../outside.json",
        )
        .is_some()
        {
            return Err("a traversing expected write must stay legacy-string-only".to_string());
        }
        Ok(())
    }

    /// FIX #1617 slice 2: the three first_pr recovery surfaces recover
    /// typed specs only at their exact token shapes; any deviation — wrong
    /// flag order, unknown or missing flags, extra tokens, a compound `&&`
    /// command — stays legacy-string-only.
    #[test]
    fn first_pr_report_routes_recover_exact_shapes_and_reject_deviations() -> Result<(), String> {
        // Route 1: the repo-exposure check redirect (route word at index 1),
        // built by the same renderer first_pr emits for byte parity.
        let check_display = crate::agent::loop_commands::check_repo_exposure_command(
            ".",
            "instant",
            "target/ripr/reports/repo-exposure.json",
        );
        let check = super::report_regeneration_command_spec_from_display(&check_display)
            .ok_or("the repo-exposure check route was not recoverable")?;
        ensure_role(
            &check,
            CommandRole::Regeneration,
            CommandAuthorityBoundary::RegenerationRouteOnly,
        )?;
        if check.command_id != "ripr:check:repo-exposure" {
            return Err(format!("unexpected check command id: {}", check.command_id));
        }
        if check.execution_mode != CommandExecutionMode::ShellRequired {
            return Err("the redirect check route must be ShellRequired".to_string());
        }
        if check.expected_writes != ["target/ripr/reports/repo-exposure.json"] {
            return Err(format!(
                "check expected writes must name the redirect target: {:?}",
                check.expected_writes
            ));
        }
        if check.args
            != [
                "check".to_string(),
                "--root".to_string(),
                ".".to_string(),
                "--mode".to_string(),
                "instant".to_string(),
                "--format".to_string(),
                "repo-exposure-json".to_string(),
            ]
        {
            return Err(format!(
                "check argv must stop before the redirect: {:?}",
                check.args
            ));
        }
        check.validate().map_err(|err| err.to_string())?;

        // Route 2: the repo-exposure gap-ledger bridge.
        let repo_exposure_display = "ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md";
        let repo_exposure =
            super::report_regeneration_command_spec_from_display(repo_exposure_display)
                .ok_or("the repo-exposure gap-ledger route was not recoverable")?;
        ensure_role(
            &repo_exposure,
            CommandRole::Regeneration,
            CommandAuthorityBoundary::RegenerationRouteOnly,
        )?;
        if repo_exposure.command_id != "ripr:reports:gap-ledger" {
            return Err(format!(
                "unexpected gap-ledger command id: {}",
                repo_exposure.command_id
            ));
        }
        if repo_exposure.execution_mode != CommandExecutionMode::Direct {
            return Err(
                "the gap-ledger routes write their own outputs, so must be Direct".to_string(),
            );
        }
        if repo_exposure.expected_writes
            != [
                "target/ripr/reports/gap-decision-ledger.json".to_string(),
                "target/ripr/reports/gap-decision-ledger.md".to_string(),
            ]
        {
            return Err(format!(
                "gap-ledger expected writes must name --out then --out-md: {:?}",
                repo_exposure.expected_writes
            ));
        }
        repo_exposure.validate().map_err(|err| err.to_string())?;

        // Route 3: the check-output gap-ledger bridge.
        let check_output = super::report_regeneration_command_spec_from_display(
            "ripr reports gap-ledger --check-output target/ripr/reports/check.json --root . --out ledger.json --out-md ledger.md",
        )
        .ok_or("the check-output gap-ledger route was not recoverable")?;
        if check_output.command_id != "ripr:reports:gap-ledger"
            || check_output.execution_mode != CommandExecutionMode::Direct
            || check_output.expected_writes != ["ledger.json".to_string(), "ledger.md".to_string()]
        {
            return Err("check-output gap-ledger recovery lost the typed facts".to_string());
        }
        check_output.validate().map_err(|err| err.to_string())?;

        // FIX (round-2 review): a display with only --out still writes the
        // Markdown twin derived via with_extension(out, "md") — both actual
        // writes must be represented. Repo-exposure 7-token shape:
        let derived = super::report_regeneration_command_spec_from_display(
            "ripr reports gap-ledger --repo-exposure repo.json --out ledger.json",
        )
        .ok_or("the single-output gap-ledger route was not recoverable")?;
        if derived.expected_writes
            != [
                "ledger.json".to_string(),
                "target/ripr/reports/gap-decision-ledger.md".to_string(),
            ]
        {
            return Err(format!(
                "default Markdown output missing from expected writes: {:?}",
                derived.expected_writes
            ));
        }
        derived.validate().map_err(|err| err.to_string())?;
        // Check-output 9-token shape with the twin derived from a
        // multi-dot name (with_extension replaces the extension).
        let derived = super::report_regeneration_command_spec_from_display(
            "ripr reports gap-ledger --check-output check.json --root . --out day.one.json",
        )
        .ok_or("the single-output check-output bridge was not recoverable")?;
        if derived.expected_writes
            != [
                "day.one.json".to_string(),
                "target/ripr/reports/gap-decision-ledger.md".to_string(),
            ]
        {
            return Err(format!(
                "check-output single-override writes must name the CLI default Markdown: {:?}",
                derived.expected_writes
            ));
        }
        // A display without any output flag fails closed: the exact shape
        // cannot state the route's writes.
        if super::report_regeneration_command_spec_from_display(
            "ripr reports gap-ledger --repo-exposure repo.json",
        )
        .is_some()
        {
            return Err(
                "a gap-ledger route without output flags must stay legacy-string-only".to_string(),
            );
        }

        // Values are matched by position: dash-prefixed values keep typed
        // recovery exactly as the CLI would parse them.
        if super::report_regeneration_command_spec_from_display(
            "ripr reports gap-ledger --repo-exposure -repo.json --out -ledger.json --out-md -ledger.md",
        )
        .is_none()
        {
            return Err("dash-prefixed positional values must keep typed recovery".to_string());
        }

        // Deviations fail closed. Wrong flag order:
        if super::report_regeneration_command_spec_from_display(
            "ripr reports gap-ledger --out ledger.json --repo-exposure repo.json --out-md ledger.md",
        )
        .is_some()
        {
            return Err("a reordered gap-ledger route must stay legacy-string-only".to_string());
        }
        // Unknown flag:
        if super::report_regeneration_command_spec_from_display(
            "ripr reports gap-ledger --repo-exposure repo.json --out ledger.json --out-md ledger.md --format repo-exposure-json",
        )
        .is_some()
        {
            return Err("an unknown flag must stay legacy-string-only".to_string());
        }
        // Wrong --format value:
        if super::report_regeneration_command_spec_from_display(
            "ripr check --root . --mode instant --format agent-seam-packets-json > out.json",
        )
        .is_some()
        {
            return Err(
                "a non repo-exposure-json check route must stay legacy-string-only".to_string(),
            );
        }
        // FIX (round-2 review): the --mode value must come from the CLI
        // vocabulary (cli/parse/mode.rs); anything else fails closed.
        if super::report_regeneration_command_spec_from_display(
            "ripr check --root . --mode bogus --format repo-exposure-json > out.json",
        )
        .is_some()
        {
            return Err("an unsupported mode must stay legacy-string-only".to_string());
        }
        for mode in ["instant", "draft", "fast", "deep", "ready"] {
            let display = format!(
                "ripr check --root . --mode {mode} --format repo-exposure-json > repo.json"
            );
            if super::report_regeneration_command_spec_from_display(&display).is_none() {
                return Err(format!("supported mode `{mode}` must keep typed recovery"));
            }
        }
        // Extra token after the redirect:
        if super::report_regeneration_command_spec_from_display(
            "ripr check --root . --mode instant --format repo-exposure-json > out.json extra",
        )
        .is_some()
        {
            return Err("an extra trailing token must stay legacy-string-only".to_string());
        }
        // A compound && command first_pr renders for the default python
        // bridge is two routes in one shell line — neither half alone.
        if super::report_regeneration_command_spec_from_display(
            "ripr check --root . --base origin/main --json > target/ripr/reports/check.json && ripr reports gap-ledger --check-output target/ripr/reports/check.json --root . --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md",
        )
        .is_some()
        {
            return Err("a compound && command must stay legacy-string-only".to_string());
        }
        // FIX (round-1 review): the loop-template routes pass the same
        // validation — a traversing output path stays legacy-string-only.
        if super::report_regeneration_command_spec_from_display(
            "ripr pr-review front-panel --root . --pr-guidance guidance.md --out ../outside.json",
        )
        .is_some()
        {
            return Err(
                "a loop route with a traversing write must stay legacy-string-only".to_string(),
            );
        }
        // FIX (round-2 review): the panel requires at least one explicit
        // artifact input — the corpus display with only --root/--out is an
        // invalid command per the CLI and stays legacy-string-only.
        if super::report_regeneration_command_spec_from_display(
            "ripr pr-review front-panel --root . --out panel.json --out-md panel.md",
        )
        .is_some()
        {
            return Err(
                "front-panel without an artifact input must stay legacy-string-only".to_string(),
            );
        }
        // Template lookup is by route word: front-panel must not accept an
        // index-only flag, and index requires its own mandatory flags.
        if super::report_regeneration_command_spec_from_display(
            "ripr pr-review front-panel --root . --reports-dir target/ripr/reports --pr-guidance guidance.md --out panel.json --out-md panel.md",
        )
        .is_some()
        {
            return Err("front-panel accepted an index-only flag".to_string());
        }
        if super::report_regeneration_command_spec_from_display(
            "ripr reports index --root . --out index.json --out-md index.md",
        )
        .is_some()
        {
            return Err("index accepted a display missing --reports-dir".to_string());
        }

        // FIX (round-2 review): each supported input family recovers, with
        // the input flag carried in argv and the outputs named as writes.
        for (label, input_flag) in [
            ("pr-guidance", "--pr-guidance"),
            ("ledger", "--ledger"),
            ("receipt", "--receipt"),
        ] {
            let display = format!(
                "ripr pr-review front-panel --root . {input_flag} artifacts/{label}.json --out panel.json --out-md panel.md"
            );
            let panel = super::report_regeneration_command_spec_from_display(&display)
                .ok_or_else(|| format!("the front-panel {label} form must keep recovering"))?;
            if panel.command_id != "ripr:pr-review:front-panel"
                || panel.execution_mode != CommandExecutionMode::Direct
                || panel.expected_writes != ["panel.json".to_string(), "panel.md".to_string()]
            {
                return Err(format!(
                    "front-panel {label} recovery lost the typed facts: {:?}",
                    panel.expected_writes
                ));
            }
            if !panel.args.contains(&input_flag.to_string()) {
                return Err(format!(
                    "front-panel {label} argv must carry the input flag: {:?}",
                    panel.args
                ));
            }
            panel.validate().map_err(|err| err.to_string())?;
        }
        // The remaining input families are inside the allowed template.
        if super::report_regeneration_command_spec_from_display(
            "ripr pr-review front-panel --receipt r.json --gate-decision gate.json --zero-status zero.json --coverage-frontier frontier.json --out panel.json --out-md panel.md",
        )
        .is_none()
        {
            return Err("front-panel multi-input form must keep recovering".to_string());
        }

        // The pre-existing loop-template routes keep working unchanged.
        let front_panel = super::report_regeneration_command_spec_from_display(
            "ripr pr-review front-panel --root . --assistant-proof proof.json --out panel.json --out-md panel.md",
        )
        .ok_or("the front-panel loop route must keep recovering")?;
        if front_panel.command_id != "ripr:pr-review:front-panel" {
            return Err(format!(
                "unexpected front-panel id: {}",
                front_panel.command_id
            ));
        }
        let index = super::report_regeneration_command_spec_from_display(
            "ripr reports index --root . --reports-dir target/ripr/reports --out index.json --out-md index.md",
        )
        .ok_or("the reports index loop route must keep recovering")?;
        if index.command_id != "ripr:reports:index" {
            return Err(format!("unexpected reports index id: {}", index.command_id));
        }
        Ok(())
    }
}
