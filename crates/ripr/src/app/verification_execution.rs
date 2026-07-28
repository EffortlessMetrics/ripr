//! Bounded execution of one producer-owned verification route.
//!
//! This module is an explicit process boundary. It accepts a canonical
//! producer-shaped agent packet and executes the single direct `ripr agent
//! verify` route it declares. It never invokes a shell, never executes display
//! text, and never issues a repair receipt.
//!
//! # How route authority is established
//!
//! Authority is layered, because packet self-consistency alone is not
//! provenance — every field of a packet is caller-supplied, so a caller who
//! rewrites all copies of the route coherently would otherwise be accepted.
//!
//! 1. **Consistency** ([`validate_packet_route`]) — the typed
//!    `command_specs.verify` array must be exactly reproducible from the
//!    packet's own `verification_commands`, and the headline `verify_command`
//!    must resolve to the same route. This buys one property: the command a
//!    reviewer reads is the command that runs. On its own it proves nothing
//!    about origin.
//! 2. **Provenance** ([`bind_producer_route`]) — the route's `--before` and
//!    `--after` inputs must each pass the landed repo-exposure provenance
//!    contract: canonical shape, `ripr` producer identity, a repository root
//!    equal to the selected root, a full-SHA HEAD, and a recomputed content
//!    commitment. RIPR then **recomputes** the canonical verify route over
//!    those validated artifacts and requires the packet's route to equal it.
//!
//! The packet therefore chooses *which validated producer artifacts to
//! compare*; it never authors the command. A coherently rewritten packet
//! pointing at anything that is not a provenance-valid producer artifact is
//! refused.
//!
//! Descendant containment is a declared limitation: the workspace forbids
//! `unsafe_code` and the dependency policy admits neither `libc` nor
//! `windows-sys`, so no process-group or job-object substrate is available.
//! The authority boundary compensates structurally — the only executable route
//! is one leaf `ripr agent verify` invocation, and termination is asserted
//! against that owned child.

use crate::agent::artifact::current_git_head;
use crate::agent::command_specs::{agent_command_spec_from_display, agent_verify_command_spec};
use crate::domain::{
    CancellationPolicy, CommandAuthorityBoundary, CommandExecutionMode, CommandRole, CommandSpec,
    EnvironmentPolicy, ExpectedResultParser, NetworkPolicy, StdinPolicy, VerificationCurrentnessV1,
    VerificationExecutionResultV1, VerificationProcessDispositionV1,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PACKET_SCHEMA_VERSION: &str = "0.3";
const MAX_OUTPUT_BYTES: u64 = 1_048_576;
const MAX_CANCEL_AFTER_MS: u64 = 3_600_000;
const RESPONSE_SCHEMA_VERSION: &str = "1";

const CLAIM_BOUNDARY: &str = "Process observation only; this does not prove mutation adequacy, correctness, coverage, requirement satisfaction, or merge approval.";
const STATIC_EVIDENCE_BOUNDARY: &str =
    "Static before/after movement remains separate from verification process disposition.";
const DESCENDANT_CONTAINMENT: &str = "owned_child_only: no process-group or job-object substrate is available under the workspace unsafe_code and dependency policy; the authority boundary admits only one leaf verify route.";

/// Variable names allowed to cross into the child, values taken from the parent.
///
/// `EnvironmentPolicy::Clean` means "no ambient application or credential
/// variables", not "an empty environment block". The verify route invokes `git`,
/// so clearing `PATH` outright makes a passing observation unreachable on every
/// real repository — the child fails with `program not found` and every run
/// reports a false negative. Only this platform floor crosses the boundary, and
/// it is disclosed in the preflight so the posture is auditable rather than
/// implied.
const ENVIRONMENT_FLOOR: &[&str] = &[
    "PATH",
    // Windows platform essentials. Without SystemRoot/windir, process and
    // socket initialization fails outright on Windows.
    "SystemRoot",
    "SystemDrive",
    "windir",
    "COMSPEC",
    "PATHEXT",
    "TEMP",
    "TMP",
    // Unix platform essentials. HOME is required for git to resolve its own
    // configuration; it is ambient but not a credential.
    "HOME",
    "TMPDIR",
    "LANG",
    "LC_ALL",
];

/// Build the child environment: an empty block plus the disclosed floor.
///
/// Anything not named in [`ENVIRONMENT_FLOOR`] — tokens, cloud credentials, CI
/// secrets — is dropped, so ambient secrets cannot reach the child or its
/// captured output.
fn child_environment() -> Vec<(String, String)> {
    ENVIRONMENT_FLOOR
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| ((*name).to_string(), value))
        })
        .collect()
}

/// Public dispositions for this command surface. Every terminal outcome is
/// reported on stdout as one of these tokens.
const DISPOSITION_PASS: &str = "verification_executed_pass";
const DISPOSITION_FAIL: &str = "verification_executed_fail";
const DISPOSITION_NOT_FOUND: &str = "verification_command_not_found";
const DISPOSITION_REJECTED: &str = "verification_rejected_policy";
const DISPOSITION_WRONG_ROOT: &str = "verification_wrong_root";
const DISPOSITION_TIMED_OUT: &str = "verification_timed_out";
const DISPOSITION_CANCELLED: &str = "verification_cancelled";
const DISPOSITION_OUTPUT_LIMITED: &str = "verification_output_limited";
const DISPOSITION_REPOSITORY_CHANGED: &str = "verification_repository_changed";
const DISPOSITION_WRITE_FAILED: &str = "verification_result_write_failed";

/// A pre-execution refusal carrying the public disposition it maps to.
#[derive(Clone, Debug)]
struct Refusal {
    disposition: &'static str,
    reason: String,
}

impl Refusal {
    fn new(disposition: &'static str, reason: impl Into<String>) -> Self {
        Self {
            disposition,
            reason: reason.into(),
        }
    }
}

/// Lets tests and callers that only need the human reason use `?` directly.
impl From<Refusal> for String {
    fn from(refusal: Refusal) -> Self {
        format!("{}: {}", refusal.disposition, refusal.reason)
    }
}

fn rejected(reason: impl Into<String>) -> Refusal {
    Refusal::new(DISPOSITION_REJECTED, reason)
}

fn wrong_root(reason: impl Into<String>) -> Refusal {
    Refusal::new(DISPOSITION_WRONG_ROOT, reason)
}

#[derive(Clone, Debug, Serialize)]
struct InputIdentity {
    flag: &'static str,
    path: String,
    sha256: String,
    /// Provenance fields copied from the validated repo-exposure artifact.
    artifact_input_identity: String,
    artifact_snapshot_identity: String,
    artifact_repository_head: String,
    artifact_currentness: &'static str,
}

#[derive(Clone, Debug)]
struct ValidatedPacket {
    command_spec: CommandSpec,
    source_artifact: String,
    inputs: Vec<InputIdentity>,
}

/// A packet route that passed the consistency and route-class checks but has
/// not yet been bound to provenance-validated producer artifacts.
#[derive(Clone, Debug)]
struct CandidateRoute {
    command_spec: CommandSpec,
    source_artifact: String,
    declared_before: String,
    declared_after: String,
    before_path: PathBuf,
    after_path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
struct Preflight<'a> {
    program: &'a str,
    args: &'a [String],
    cwd: String,
    timeout_ms: u64,
    network_policy: String,
    environment_policy: String,
    stdin_policy: String,
    expected_writes: &'a [String],
    expected_exit_codes: &'a [i32],
    cost_class: String,
    side_effect_class: &'static str,
    /// Names — never values — of the variables allowed into the child.
    environment_floor: Vec<&'static str>,
    descendant_containment: &'static str,
}

#[derive(Debug)]
struct OutputCapture {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
struct ProcessObservation {
    disposition: VerificationProcessDispositionV1,
    exit_status: Option<i32>,
    observed_exit_status: Option<i32>,
    exit_signal: Option<i32>,
    duration_ms: u64,
    stdout: OutputCapture,
    stderr: OutputCapture,
    cancellation_requested: bool,
}

#[derive(Serialize)]
struct ExecutionResponse<'a> {
    schema_version: &'static str,
    disposition: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    executed: bool,
    result_committed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_artifact: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inputs: Option<&'a [InputIdentity]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preflight: Option<Preflight<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command_spec: Option<&'a CommandSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<&'a VerificationExecutionResultV1>,
    /// Present when the terminal disposition forced `exit_status` to be dropped
    /// from the committed result to satisfy the transported-result invariant.
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_exit_status: Option<i32>,
    descendant_containment: &'static str,
    static_evidence_boundary: &'static str,
    claim_boundary: &'static str,
}

/// The rendered public outcome of one execution attempt.
pub(crate) struct ExecutionOutcome {
    pub(crate) rendered: String,
    pub(crate) disposition: &'static str,
    /// True when RIPR could not produce and commit a bounded observation.
    pub(crate) failed: bool,
}

/// Execute one validated producer-owned verification packet.
///
/// Every terminal state — including pre-execution refusals — is rendered as a
/// typed JSON response so the public disposition is observable on stdout.
pub(crate) fn execute_verify_packet(
    root: &Path,
    packet_path: &Path,
    result_path: &Path,
    authorized: bool,
    cancel_after_ms: Option<u64>,
) -> ExecutionOutcome {
    match run(root, packet_path, result_path, authorized, cancel_after_ms) {
        Ok(outcome) => outcome,
        Err(refusal) => refusal_outcome(&refusal),
    }
}

fn refusal_outcome(refusal: &Refusal) -> ExecutionOutcome {
    let response = ExecutionResponse {
        schema_version: RESPONSE_SCHEMA_VERSION,
        disposition: refusal.disposition,
        reason: Some(refusal.reason.as_str()),
        executed: false,
        result_committed: false,
        source_artifact: None,
        inputs: None,
        preflight: None,
        command_spec: None,
        result: None,
        observed_exit_status: None,
        descendant_containment: DESCENDANT_CONTAINMENT,
        static_evidence_boundary: STATIC_EVIDENCE_BOUNDARY,
        claim_boundary: CLAIM_BOUNDARY,
    };
    ExecutionOutcome {
        rendered: render(&response),
        disposition: refusal.disposition,
        failed: true,
    }
}

/// Render a response, falling back to a minimal hand-built object so this
/// surface never loses its public disposition to a serializer error.
fn render(response: &ExecutionResponse<'_>) -> String {
    match serde_json::to_string_pretty(response) {
        Ok(text) => text + "\n",
        Err(_) => format!(
            "{{\n  \"schema_version\": \"{}\",\n  \"disposition\": \"{}\"\n}}\n",
            RESPONSE_SCHEMA_VERSION, response.disposition
        ),
    }
}

fn run(
    root: &Path,
    packet_path: &Path,
    result_path: &Path,
    authorized: bool,
    cancel_after_ms: Option<u64>,
) -> Result<ExecutionOutcome, Refusal> {
    if !authorized {
        return Err(rejected("execution requires explicit --authorize"));
    }
    let root = canonical_directory(root, "verification execution root")?;
    let packet_path = confined_existing_file(&root, packet_path, "--packet")?;
    let result_path = confined_output_path(&root, result_path, "--result-json")?;
    let validated = validate_packet(&root, &packet_path)?;
    if let Some(delay) = cancel_after_ms
        && (delay == 0 || delay > MAX_CANCEL_AFTER_MS || delay >= validated.command_spec.timeout_ms)
    {
        return Err(rejected(format!(
            "--cancel-after-ms must be between 1 and {MAX_CANCEL_AFTER_MS} and less than the command timeout"
        )));
    }

    let spec = &validated.command_spec;
    let root_identity = display_path(&root);
    let head_before = current_git_head(&root)
        .map_err(|error| rejected(format!("read HEAD before execution failed: {error}")))?;
    let dirty_before = git_worktree_dirty(&root)?;

    // Disclosure is derived from the validated spec, never asserted as fixed
    // text, so it cannot drift away from what is actually executed.
    let preflight = Preflight {
        program: spec.program.as_str(),
        args: spec.args.as_slice(),
        cwd: root_identity.clone(),
        timeout_ms: spec.timeout_ms,
        network_policy: format!("{:?}", spec.network_policy),
        environment_policy: format!("{:?}", spec.environment_policy),
        stdin_policy: format!("{:?}", spec.stdin),
        expected_writes: spec.expected_writes.as_slice(),
        expected_exit_codes: spec.expected_exit_codes.as_slice(),
        cost_class: format!("{:?}", spec.cost_class),
        side_effect_class: "read_only_observation",
        environment_floor: ENVIRONMENT_FLOOR
            .iter()
            .filter(|name| std::env::var(name).is_ok())
            .copied()
            .collect(),
        descendant_containment: DESCENDANT_CONTAINMENT,
    };
    if let Ok(disclosure) = serde_json::to_string(&preflight) {
        eprintln!("verification preflight: {disclosure}");
    }

    let executable = std::env::current_exe()
        .map_err(|error| rejected(format!("resolve ripr executable failed: {error}")))?;
    let observation = run_process(&executable, spec, &root, cancel_after_ms)?;
    let head_after = current_git_head(&root)
        .map_err(|error| rejected(format!("read HEAD after execution failed: {error}")))?;
    let dirty_after = git_worktree_dirty(&root)?;
    let currentness = if head_before != head_after {
        VerificationCurrentnessV1::HistoricalNoncurrent
    } else if dirty_before || dirty_after {
        VerificationCurrentnessV1::DirtyWorktree
    } else {
        VerificationCurrentnessV1::Current
    };
    let result = VerificationExecutionResultV1 {
        schema_version: VerificationExecutionResultV1::SCHEMA_VERSION.to_string(),
        root_identity,
        head_before: head_before.clone(),
        head_after: head_after.clone(),
        command_spec_sha256: crate::domain::command_spec_sha256(spec)
            .map_err(|error| rejected(format!("command spec digest failed: {error}")))?,
        process_disposition: observation.disposition,
        exit_status: observation.exit_status,
        stdout_sha256: digest(&observation.stdout.bytes),
        stderr_sha256: digest(&observation.stderr.bytes),
        currentness,
        duration_ms: observation.duration_ms,
        exit_signal: observation.exit_signal,
        stdout_bytes: observation.stdout.bytes.len() as u64,
        stderr_bytes: observation.stderr.bytes.len() as u64,
        stdout_truncated: observation.stdout.truncated,
        stderr_truncated: observation.stderr.truncated,
        cancellation_requested: observation.cancellation_requested,
    };
    result
        .validate_against(spec, &result.root_identity, &head_before, &head_after)
        .map_err(|error| rejected(format!("result failed transported validation: {error}")))?;

    let disposition = response_disposition(&result, spec);
    // `exit_status` is dropped from the committed result whenever a limit or
    // termination state wins, because the transported contract forbids pairing
    // one with the other. The raw code stays visible here instead of vanishing.
    let observed_exit_status = if result.exit_status.is_none() {
        observation.observed_exit_status
    } else {
        None
    };
    let mut response = ExecutionResponse {
        schema_version: RESPONSE_SCHEMA_VERSION,
        disposition,
        reason: None,
        executed: true,
        result_committed: false,
        source_artifact: Some(validated.source_artifact.as_str()),
        inputs: Some(validated.inputs.as_slice()),
        preflight: Some(preflight),
        command_spec: Some(spec),
        result: Some(&result),
        observed_exit_status,
        descendant_containment: DESCENDANT_CONTAINMENT,
        static_evidence_boundary: STATIC_EVIDENCE_BOUNDARY,
        claim_boundary: CLAIM_BOUNDARY,
    };

    // Commit first, then report what was committed. Set `result_committed`
    // before serializing so the artifact on disk and the stdout render agree
    // (#2396 review: serializing with `false` then flipping to `true` for
    // stdout contradicts the committed file).
    response.result_committed = true;
    let committed = atomic_write(&result_path, render(&response).as_bytes());
    match committed {
        Ok(()) => Ok(ExecutionOutcome {
            rendered: render(&response),
            disposition,
            failed: false,
        }),
        Err(reason) => {
            response.result_committed = false;
            response.disposition = DISPOSITION_WRITE_FAILED;
            response.reason = Some(reason.as_str());
            Ok(ExecutionOutcome {
                rendered: render(&response),
                disposition: DISPOSITION_WRITE_FAILED,
                failed: true,
            })
        }
    }
}

/// Validate one canonical producer-shaped envelope and re-derive its typed
/// verify spec from the packet's own display commands.
///
/// The typed `command_specs.verify` array must equal exactly what
/// `agent_command_spec_from_display` yields for the packet's
/// `verification_commands`, so a caller cannot display one route and execute
/// another. This is a consistency check, not a provenance check — see the
/// module documentation for what it does not establish.
fn validate_packet(root: &Path, packet_path: &Path) -> Result<ValidatedPacket, Refusal> {
    let candidate = validate_packet_route(root, packet_path)?;
    bind_producer_route(root, candidate)
}

/// Consistency and route-class layer.
///
/// Checks the envelope shape, that the typed specs are reproducible from the
/// packet's own display commands, that exactly one route is executable, and that
/// the inputs resolve under the root. Passing this does **not** make a route
/// producer-owned — see [`bind_producer_route`].
fn validate_packet_route(root: &Path, packet_path: &Path) -> Result<CandidateRoute, Refusal> {
    let text = fs::read_to_string(packet_path)
        .map_err(|error| rejected(format!("read packet failed: {error}")))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| rejected(format!("parse packet failed: {error}")))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(PACKET_SCHEMA_VERSION) {
        return Err(rejected(format!(
            "packet schema_version must be {PACKET_SCHEMA_VERSION}"
        )));
    }
    let packets = value
        .get("packets")
        .and_then(Value::as_array)
        .ok_or_else(|| rejected("packet envelope must contain a packets array"))?;
    if packets.len() != 1 {
        return Err(rejected(format!(
            "exactly one packet is required for execution; found {}",
            packets.len()
        )));
    }
    let packet = packets
        .first()
        .and_then(Value::as_object)
        .ok_or_else(|| rejected("packet entry must be an object"))?;

    let displays = packet
        .get("verification_commands")
        .and_then(Value::as_array)
        .ok_or_else(|| rejected("packet verification_commands is required"))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| rejected("verification_commands entries must be strings"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Reproduce the producer projection. This is the same derivation the
    // producer performs, so any divergence is caller authorship.
    let reproduced = displays
        .iter()
        .filter_map(|display| agent_command_spec_from_display(display))
        .collect::<Vec<_>>();

    let typed_value = packet
        .get("command_specs")
        .and_then(|specs| specs.get("verify"))
        .ok_or_else(|| rejected("producer command_specs.verify is required"))?;
    let typed_array = typed_value
        .as_array()
        .ok_or_else(|| rejected("command_specs.verify must be an array of typed specs"))?;
    let typed: Vec<CommandSpec> = typed_array
        .iter()
        .map(|entry| {
            serde_json::from_value(entry.clone())
                .map_err(|error| rejected(format!("parse typed verify spec failed: {error}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if typed != reproduced {
        return Err(rejected(
            "typed command_specs.verify is not reproducible from the packet verification_commands; the displayed route and the typed route must agree",
        ));
    }

    // Exactly one executable route. Zero means nothing in this packet is
    // executable; more than one is ambiguous and is refused rather than guessed.
    let mut executable = typed
        .iter()
        .filter(|spec| is_executable_verify_route(spec).is_ok())
        .cloned()
        .collect::<Vec<_>>();
    if executable.len() != 1 {
        let detail = typed
            .iter()
            .map(|spec| match is_executable_verify_route(spec) {
                Ok(()) => "executable".to_string(),
                Err(reason) => reason,
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(rejected(format!(
            "exactly one executable verify route is required; found {} of {} typed specs [{}]",
            executable.len(),
            typed.len(),
            detail
        )));
    }
    let command_spec = executable.remove(0);

    // The packet's headline display route must be the same route we execute, so
    // the command a reviewer reads is the command that ran.
    let headline = packet
        .get("verify_command")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("packet verify_command is required"))?;
    let headline_spec = agent_command_spec_from_display(headline)
        .ok_or_else(|| rejected("packet verify_command is not a canonical route"))?;
    if headline_spec != command_spec {
        return Err(rejected(
            "packet verify_command does not match the single executable typed verify route",
        ));
    }

    command_spec
        .validate()
        .map_err(|error| rejected(format!("invalid command spec: {error}")))?;
    is_executable_verify_route(&command_spec).map_err(rejected)?;

    if route_arg_path(&command_spec.args, "--root")?.as_path() != Path::new(".") {
        return Err(wrong_root(
            "command must select the invocation root with --root .",
        ));
    }
    let before = route_arg_path(&command_spec.args, "--before")?;
    let after = route_arg_path(&command_spec.args, "--after")?;
    let before_path = confined_existing_file(root, &before, "verify --before")?;
    let after_path = confined_existing_file(root, &after, "verify --after")?;

    Ok(CandidateRoute {
        command_spec,
        source_artifact: display_path(packet_path),
        declared_before: path_arg_text(&before),
        declared_after: path_arg_text(&after),
        before_path,
        after_path,
    })
}

/// Bind a candidate route to provenance-validated producer artifacts.
///
/// This is what makes the executed route producer-owned rather than merely
/// self-consistent. Both `--before` and `--after` must pass the landed
/// repo-exposure provenance contract — canonical shape, `ripr` producer
/// identity, a repository root equal to the selected root, a full-SHA HEAD, and
/// a recomputed content commitment. RIPR then **recomputes** the canonical
/// verify route over those validated artifacts and requires the packet's route
/// to equal it. The packet therefore selects which validated artifacts to
/// compare; it does not get to author the command.
fn bind_producer_route(root: &Path, candidate: CandidateRoute) -> Result<ValidatedPacket, Refusal> {
    let before = validated_artifact(root, &candidate.before_path, "before")?;
    let after = validated_artifact(root, &candidate.after_path, "after")?;
    if before.artifact.base_revision != after.artifact.base_revision {
        return Err(rejected(format!(
            "verify inputs are incomparable: base revisions differ ({:?} vs {:?})",
            before.artifact.base_revision, after.artifact.base_revision
        )));
    }

    // The authority step: the route is constructed here, not accepted.
    let recomputed = agent_verify_command_spec(
        ".",
        &candidate.declared_before,
        &candidate.declared_after,
        None,
    );
    if candidate.command_spec != recomputed {
        return Err(rejected(
            "packet route does not equal the canonical verify route recomputed from the validated producer artifacts",
        ));
    }

    Ok(ValidatedPacket {
        command_spec: recomputed,
        source_artifact: candidate.source_artifact,
        inputs: vec![
            input_identity("--before", &candidate.declared_before, before),
            input_identity("--after", &candidate.declared_after, after),
        ],
    })
}

struct ArtifactBinding {
    artifact: crate::agent::artifact::ValidatedArtifact,
    sha256: String,
}

fn validated_artifact(
    root: &Path,
    path: &Path,
    label: &'static str,
) -> Result<ArtifactBinding, Refusal> {
    let bytes = fs::read(path)
        .map_err(|error| wrong_root(format!("read {label} input failed: {error}")))?;
    let raw = String::from_utf8(bytes.clone())
        .map_err(|error| rejected(format!("{label} input is not valid UTF-8: {error}")))?;
    let artifact = crate::agent::artifact::validate_repo_exposure_artifact(root, &raw, label)
        .map_err(|error| {
            rejected(format!(
                "{label} input failed provenance validation: {error}"
            ))
        })?;
    Ok(ArtifactBinding {
        artifact,
        sha256: digest(&bytes),
    })
}

fn input_identity(flag: &'static str, declared: &str, binding: ArtifactBinding) -> InputIdentity {
    InputIdentity {
        flag,
        path: declared.to_string(),
        sha256: binding.sha256,
        artifact_input_identity: binding.artifact.input_identity,
        artifact_snapshot_identity: binding.artifact.snapshot_identity,
        artifact_repository_head: binding.artifact.repository_head,
        artifact_currentness: match binding.artifact.currentness {
            crate::agent::artifact::ArtifactCurrentness::Current => "current",
            crate::agent::artifact::ArtifactCurrentness::DirtyWorktree => "dirty_worktree",
            crate::agent::artifact::ArtifactCurrentness::Historical => "historical",
        },
    }
}

/// Render a declared route path exactly as the producer wrote it, so the
/// recomputed route can be compared byte-for-byte.
fn path_arg_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The executable subset of verify routes: one direct, clean-environment,
/// no-network, no-write `ripr agent verify ... --json` invocation.
fn is_executable_verify_route(spec: &CommandSpec) -> Result<(), String> {
    if spec.role != CommandRole::Verify {
        return Err("role is not verify".into());
    }
    if spec.authority_boundary != CommandAuthorityBoundary::VerificationRouteOnly {
        return Err("authority boundary is not verification-route-only".into());
    }
    if spec.execution_mode != CommandExecutionMode::Direct {
        return Err("execution mode is not direct".into());
    }
    if spec.program != "ripr" {
        return Err("program is not ripr".into());
    }
    if spec.cwd != "." {
        return Err("cwd is not the invocation root".into());
    }
    if spec.environment_policy != EnvironmentPolicy::Clean
        || !spec.env_set.is_empty()
        || !spec.env_passthrough.is_empty()
    {
        return Err("environment policy is not clean".into());
    }
    if spec.stdin != StdinPolicy::Null {
        return Err("stdin policy is not null".into());
    }
    if spec.network_policy != NetworkPolicy::Forbidden {
        return Err("network policy is not forbidden".into());
    }
    if spec.cancellation != CancellationPolicy::Allowed {
        return Err("cancellation is not allowed".into());
    }
    if spec.expected_result_parser != ExpectedResultParser::DeclaredJson {
        return Err("expected result parser is not declared JSON".into());
    }
    if !spec.expected_writes.is_empty() {
        return Err("route declares writes".into());
    }
    if !spec.expected_exit_codes.contains(&0) {
        return Err("route does not accept exit code 0".into());
    }
    if spec.timeout_ms == 0 {
        return Err("route has no timeout".into());
    }
    if spec.args.first().map(String::as_str) != Some("agent")
        || spec.args.get(1).map(String::as_str) != Some("verify")
        || spec.args.last().map(String::as_str) != Some("--json")
    {
        return Err("args are not the canonical agent verify route".into());
    }
    Ok(())
}

fn route_arg_path(args: &[String], flag: &str) -> Result<PathBuf, Refusal> {
    let index = args
        .iter()
        .position(|arg| arg == flag)
        .ok_or_else(|| rejected(format!("command is missing {flag}")))?;
    if args.iter().filter(|arg| arg.as_str() == flag).count() != 1 {
        return Err(rejected(format!("command repeats {flag}")));
    }
    let value = args
        .get(index + 1)
        .ok_or_else(|| rejected(format!("command is missing a value for {flag}")))?;
    if value.is_empty() || value.contains('\0') {
        return Err(rejected(format!("invalid value for {flag}")));
    }
    Ok(PathBuf::from(value))
}

fn run_process(
    executable: &Path,
    spec: &CommandSpec,
    root: &Path,
    cancel_after_ms: Option<u64>,
) -> Result<ProcessObservation, Refusal> {
    let started = Instant::now();
    let mut child = match Command::new(executable)
        .args(&spec.args)
        .current_dir(root)
        .env_clear()
        .envs(child_environment())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return Ok(ProcessObservation {
                disposition: VerificationProcessDispositionV1::FailedToStart,
                exit_status: None,
                observed_exit_status: None,
                exit_signal: None,
                duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                stdout: OutputCapture {
                    bytes: Vec::new(),
                    truncated: false,
                },
                stderr: OutputCapture {
                    bytes: format!("spawn failed: {error} (kind {:?})", error.kind()).into_bytes(),
                    truncated: false,
                },
                cancellation_requested: false,
            });
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| rejected("stdout pipe unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| rejected("stderr pipe unavailable"))?;
    let (stdout_rx, stdout_thread) = spawn_capture(stdout);
    let (stderr_rx, stderr_thread) = spawn_capture(stderr);
    let deadline = started + Duration::from_millis(spec.timeout_ms);
    let cancel_deadline = cancel_after_ms.map(|value| started + Duration::from_millis(value));

    // Exactly one loop exit wins. Order is deliberate: an exit already observed
    // is terminal; otherwise cancellation precedes the timeout, and an output
    // overflow is checked last so a process that finished under the cap is not
    // reclassified.
    let mut exited: Option<(Option<i32>, Option<i32>)> = None;
    let terminal;
    let mut cancellation_requested = false;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| rejected(format!("wait failed: {error}")))?
        {
            exited = Some((status.code(), exit_signal(&status)));
            terminal = VerificationProcessDispositionV1::Completed;
            break;
        }
        if let Some(cancel_deadline) = cancel_deadline
            && Instant::now() >= cancel_deadline
        {
            kill_child(&mut child)?;
            terminal = VerificationProcessDispositionV1::Cancelled;
            cancellation_requested = true;
            break;
        }
        if Instant::now() >= deadline {
            kill_child(&mut child)?;
            terminal = VerificationProcessDispositionV1::TimedOut;
            break;
        }
        if stdout_rx.try_recv().is_ok() || stderr_rx.try_recv().is_ok() {
            kill_child(&mut child)?;
            terminal = VerificationProcessDispositionV1::OutputLimitExceeded;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = child.wait();
    let stdout = stdout_thread
        .join()
        .map_err(|_panic| rejected("stdout reader panicked"))?
        .map_err(rejected)?;
    let stderr = stderr_thread
        .join()
        .map_err(|_panic| rejected("stderr reader panicked"))?
        .map_err(rejected)?;

    // Truncation discovered only after the reader drained supersedes a clean
    // exit: the observation is incomplete, so it must not read as a pass.
    let truncated = stdout.truncated || stderr.truncated;
    let disposition = if terminal == VerificationProcessDispositionV1::Completed && truncated {
        VerificationProcessDispositionV1::OutputLimitExceeded
    } else {
        terminal
    };
    let observed_exit_status = exited.and_then(|(code, _)| code);
    let exit_signal = exited.and_then(|(_, signal)| signal);
    // The transported contract forbids an exit status on any non-completed
    // disposition, so it is dropped here and surfaced separately.
    let exit_status = if disposition == VerificationProcessDispositionV1::Completed {
        observed_exit_status
    } else {
        None
    };
    let exit_signal = if disposition == VerificationProcessDispositionV1::Completed {
        exit_signal
    } else {
        None
    };
    Ok(ProcessObservation {
        disposition,
        exit_status,
        observed_exit_status,
        exit_signal,
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        stdout,
        stderr,
        cancellation_requested,
    })
}

fn spawn_capture<R: Read + Send + 'static>(
    mut reader: R,
) -> (
    Receiver<bool>,
    thread::JoinHandle<Result<OutputCapture, String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 8192];
        let mut truncated = false;
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| format!("read process output failed: {error}"))?;
            if read == 0 {
                break;
            }
            let remaining = MAX_OUTPUT_BYTES.saturating_sub(bytes.len() as u64) as usize;
            if read > remaining {
                bytes.extend_from_slice(buffer.get(..remaining).unwrap_or_default());
                truncated = true;
                let _ = tx.send(true);
                break;
            }
            bytes.extend_from_slice(buffer.get(..read).unwrap_or_default());
        }
        Ok(OutputCapture { bytes, truncated })
    });
    (rx, handle)
}

fn kill_child(child: &mut Child) -> Result<(), Refusal> {
    child
        .kill()
        .map_err(|error| rejected(format!("terminate child failed: {error}")))
}

fn response_disposition(
    result: &VerificationExecutionResultV1,
    spec: &CommandSpec,
) -> &'static str {
    if result.head_before != result.head_after {
        return DISPOSITION_REPOSITORY_CHANGED;
    }
    match result.process_disposition {
        VerificationProcessDispositionV1::FailedToStart => DISPOSITION_NOT_FOUND,
        VerificationProcessDispositionV1::TimedOut => DISPOSITION_TIMED_OUT,
        VerificationProcessDispositionV1::Cancelled => DISPOSITION_CANCELLED,
        VerificationProcessDispositionV1::OutputLimitExceeded => DISPOSITION_OUTPUT_LIMITED,
        VerificationProcessDispositionV1::Completed => match result.exit_status {
            Some(code) if spec.expected_exit_codes.contains(&code) => DISPOSITION_PASS,
            _ => DISPOSITION_FAIL,
        },
    }
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut rendered = String::from("sha256:");
    for byte in hasher.finalize() {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

/// Read tracked-file dirtiness through the shared git helper, which is the
/// single production spawn point for git in this crate.
fn git_worktree_dirty(root: &Path) -> Result<bool, Refusal> {
    let status = crate::git::run_git(root, &["status", "--porcelain", "--untracked-files=no"])
        .map_err(|error| {
            Refusal::new(
                DISPOSITION_REPOSITORY_CHANGED,
                format!("read worktree status failed: {error}"),
            )
        })?;
    Ok(!status.trim().is_empty())
}

/// Render a path for comparison and for the recorded identity.
///
/// `canonicalize` yields a `\\?\C:\...` verbatim prefix on Windows. Leaving it
/// in place makes every `starts_with` comparison against a non-canonical root
/// fail and leaks an unstable identity into the result, so drive-letter
/// verbatim prefixes are stripped. UNC verbatim paths are left untouched
/// because stripping theirs changes their meaning.
fn normalize(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

fn display_path(path: &Path) -> String {
    normalize(path).to_string_lossy().replace('\\', "/")
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, Refusal> {
    let canonical = path
        .canonicalize()
        .map_err(|error| wrong_root(format!("canonicalize {label} failed: {error}")))?;
    if !canonical.is_dir() {
        return Err(wrong_root(format!("{label} is not a directory")));
    }
    Ok(normalize(&canonical))
}

fn confined_existing_file(root: &Path, path: &Path, label: &str) -> Result<PathBuf, Refusal> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|error| wrong_root(format!("canonicalize {label} failed: {error}")))?;
    let canonical = normalize(&canonical);
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(wrong_root(format!("{label} must be a file under root")));
    }
    Ok(canonical)
}

fn confined_output_path(root: &Path, path: &Path, label: &str) -> Result<PathBuf, Refusal> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(wrong_root(format!(
            "{label} must be a root-relative path without .."
        )));
    }
    let candidate = root.join(path);
    let parent = candidate
        .parent()
        .ok_or_else(|| wrong_root(format!("{label} has no parent")))?;
    let parent = parent
        .canonicalize()
        .map_err(|error| wrong_root(format!("canonicalize {label} parent failed: {error}")))?;
    let parent = normalize(&parent);
    if !parent.starts_with(root) {
        return Err(wrong_root(format!("{label} must stay under root")));
    }
    Ok(parent.join(
        candidate
            .file_name()
            .ok_or_else(|| wrong_root(format!("{label} has no file name")))?,
    ))
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "result path has no parent".to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("clock failed: {error}"))?
        .as_nanos();
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        stamp
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("create temporary result failed: {error}"))?;
    if let Err(error) = file.write_all(contents).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("write temporary result failed: {error}"));
    }
    if path.exists() {
        let _ = fs::remove_file(&temporary);
        return Err("refusing to overwrite an existing result".into());
    }
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("commit result failed: {error}")
    })
}

#[cfg(unix)]
fn exit_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::command_specs::agent_verify_command_spec;

    /// Build a canonical producer-shaped envelope for `displays`.
    ///
    /// This mirrors `output::agent_seam_packets::gap_record_command_specs_json`
    /// — `command_specs.verify` is an array derived from
    /// `verification_commands` — so the fixture cannot drift into a shape the
    /// producer never emits.
    fn producer_envelope(displays: &[String], headline: &str) -> Value {
        let verify = displays
            .iter()
            .filter_map(|display| agent_command_spec_from_display(display))
            .collect::<Vec<_>>();
        serde_json::json!({
            "schema_version": PACKET_SCHEMA_VERSION,
            "packets_total": 1,
            "packets": [{
                "verify_command": headline,
                "verification_commands": displays,
                "command_specs": {"verify": verify, "receipt": []},
            }],
        })
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Result<Self, String> {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "ripr-vexec-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            let path = path.canonicalize().map_err(|error| error.to_string())?;
            Ok(Self {
                path: normalize(&path),
            })
        }

        fn write(&self, name: &str, contents: &[u8]) -> Result<(), String> {
            fs::write(self.path.join(name), contents).map_err(|error| error.to_string())
        }

        fn write_packet(&self, name: &str, envelope: &Value) -> Result<PathBuf, String> {
            let bytes = serde_json::to_vec(envelope).map_err(|error| error.to_string())?;
            self.write(name, &bytes)?;
            Ok(self.path.join(name))
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn seeded_root(label: &str) -> Result<TempRoot, String> {
        let root = TempRoot::new(label)?;
        root.write("before.json", b"{}")?;
        root.write("after.json", b"{}")?;
        Ok(root)
    }

    #[test]
    fn digest_is_prefixed_and_stable() {
        assert_eq!(digest(b"secret-canary"), digest(b"secret-canary"));
        assert!(digest(b"secret-canary").starts_with("sha256:"));
    }

    #[test]
    fn accepts_the_canonical_producer_array_shape() -> Result<(), String> {
        let root = seeded_root("accept")?;
        let display = agent_verify_command_spec(".", "before.json", "after.json", None).display;
        let envelope = producer_envelope(std::slice::from_ref(&display), &display);
        let packet = root.write_packet("packet.json", &envelope)?;
        let candidate = validate_packet_route(&root.path, &packet)?;
        assert_eq!(candidate.command_spec.program, "ripr");
        assert_eq!(candidate.declared_before, "before.json");
        assert_eq!(candidate.declared_after, "after.json");
        // Provenance binding is a separate layer, demonstrated end-to-end in
        // `cli_smoke`, where real `ripr check` artifacts exist.
        Ok(())
    }

    #[test]
    fn rejects_a_single_object_verify_spec_the_producer_never_emits() -> Result<(), String> {
        let root = seeded_root("object")?;
        let spec = agent_verify_command_spec(".", "before.json", "after.json", None);
        let envelope = serde_json::json!({
            "schema_version": PACKET_SCHEMA_VERSION,
            "packets": [{
                "verify_command": spec.display,
                "verification_commands": [spec.display],
                "command_specs": {"verify": spec},
            }],
        });
        let packet = root.write_packet("packet.json", &envelope)?;
        let error = validate_packet_route(&root.path, &packet)
            .err()
            .ok_or("expected a refusal")?;
        assert_eq!(error.disposition, DISPOSITION_REJECTED);
        assert!(error.reason.contains("must be an array"));
        Ok(())
    }

    #[test]
    fn rejects_caller_authored_typed_spec_not_reproducible_from_displays() -> Result<(), String> {
        let root = seeded_root("forged")?;
        let display = agent_verify_command_spec(".", "before.json", "after.json", None).display;
        let mut envelope = producer_envelope(std::slice::from_ref(&display), &display);
        // Swap the verify route for a receipt route while leaving the display
        // text untouched: the classic borrowed-authority forgery.
        envelope["packets"][0]["command_specs"]["verify"][0]["args"][1] =
            Value::String("receipt".to_string());
        let packet = root.write_packet("packet.json", &envelope)?;
        let error = validate_packet_route(&root.path, &packet)
            .err()
            .ok_or("expected a refusal")?;
        assert!(error.reason.contains("not reproducible"));
        Ok(())
    }

    #[test]
    fn rejects_every_caller_mutated_typed_field() -> Result<(), String> {
        let root = seeded_root("fields")?;
        let display = agent_verify_command_spec(".", "before.json", "after.json", None).display;
        let mutations: Vec<(&str, Value)> = vec![
            ("program", Value::String("cmd".to_string())),
            ("cwd", Value::String("..".to_string())),
            ("timeout_ms", Value::from(1_u64)),
            ("environment_policy", Value::String("inherit".to_string())),
            ("network_policy", Value::String("allowed".to_string())),
            ("role", Value::String("receipt".to_string())),
            (
                "authority_boundary",
                Value::String("receipt_route_only".to_string()),
            ),
            (
                "execution_mode",
                Value::String("shell_required".to_string()),
            ),
            ("display", Value::String("ripr agent verify".to_string())),
            (
                "command_id",
                Value::String("ripr:agent:receipt".to_string()),
            ),
        ];
        for (field, replacement) in mutations {
            let mut envelope = producer_envelope(std::slice::from_ref(&display), &display);
            envelope["packets"][0]["command_specs"]["verify"][0][field] = replacement;
            let packet = root.write_packet(&format!("packet-{field}.json"), &envelope)?;
            let error = validate_packet_route(&root.path, &packet)
                .err()
                .ok_or_else(|| format!("expected a refusal for a mutated {field}"))?;
            assert_eq!(
                error.disposition, DISPOSITION_REJECTED,
                "mutated {field} produced {}",
                error.reason
            );
        }
        Ok(())
    }

    /// The load-bearing negative result for this module's trust story.
    ///
    /// A caller who mutates *every* duplicated route representation coherently —
    /// `verification_commands`, the typed `command_specs.verify`, and the
    /// headline `verify_command` — passes the consistency layer, because all of
    /// those fields are caller-supplied and therefore agree with each other.
    ///
    /// The provenance layer is what refuses it: the rewritten route points at
    /// files that are not provenance-valid repo-exposure producer artifacts, so
    /// no route can be recomputed and execution is refused. This is the test
    /// that separates "self-consistent" from "producer-owned".
    #[test]
    fn coherent_whole_packet_mutation_passes_consistency_but_fails_provenance() -> Result<(), String>
    {
        let root = seeded_root("coherent")?;
        root.write("alt-before.json", b"{}")?;
        root.write("alt-after.json", b"{}")?;
        // A route the producer never emitted, rewritten consistently everywhere.
        let forged = crate::agent::loop_commands::agent_verify_command(
            ".",
            "alt-before.json",
            "alt-after.json",
            None,
        );
        let envelope = producer_envelope(std::slice::from_ref(&forged), &forged);
        let packet = root.write_packet("coherent.json", &envelope)?;
        // Consistency alone cannot tell this apart from a producer packet.
        let candidate = validate_packet_route(&root.path, &packet)?;
        assert!(
            candidate
                .command_spec
                .args
                .iter()
                .any(|arg| arg == "alt-before.json"),
            "the consistency layer sees a well-formed, self-agreeing route"
        );

        // Provenance is what refuses it: these inputs are not producer artifacts.
        let error = validate_packet(&root.path, &packet)
            .err()
            .ok_or("a coherent forgery must be refused by the provenance layer")?;
        assert_eq!(error.disposition, DISPOSITION_REJECTED);
        assert!(
            error.reason.contains("provenance validation"),
            "refusal must name the provenance failure, got: {}",
            error.reason
        );

        // And coherence still cannot buy a different command, a shell, or an
        // escape from the root even before provenance is consulted.
        for escape in [
            crate::agent::loop_commands::agent_verify_command(
                ".",
                "../outside.json",
                "alt-after.json",
                None,
            ),
            format!("{forged} > written.json"),
            "ripr agent receipt --root . --verify-json alt-before.json --seam-id x --json"
                .to_string(),
        ] {
            let envelope = producer_envelope(std::slice::from_ref(&escape), &escape);
            let packet = root.write_packet("escape-attempt.json", &envelope)?;
            assert!(
                validate_packet_route(&root.path, &packet).is_err(),
                "coherent rewriting must not admit {escape}"
            );
        }
        Ok(())
    }

    #[test]
    fn rejects_ambiguous_and_empty_executable_route_sets() -> Result<(), String> {
        let root = seeded_root("ambiguous")?;
        let display = agent_verify_command_spec(".", "before.json", "after.json", None).display;

        // Two identical executable routes are ambiguous, not "obviously the same".
        let envelope = producer_envelope(&[display.clone(), display.clone()], &display);
        let packet = root.write_packet("two.json", &envelope)?;
        let error = validate_packet_route(&root.path, &packet)
            .err()
            .ok_or("expected a refusal for two routes")?;
        assert!(error.reason.contains("exactly one executable verify route"));

        // A redirect route is shell-required, so nothing is executable.
        let redirect = format!("{display} > out.json");
        let envelope = producer_envelope(std::slice::from_ref(&redirect), &redirect);
        let packet = root.write_packet("redirect.json", &envelope)?;
        let error = validate_packet_route(&root.path, &packet)
            .err()
            .ok_or("expected a refusal for a redirect route")?;
        assert!(error.reason.contains("exactly one executable verify route"));
        Ok(())
    }

    #[test]
    fn rejects_inputs_outside_the_selected_root() -> Result<(), String> {
        let root = seeded_root("escape")?;
        let display = crate::agent::loop_commands::agent_verify_command(
            ".",
            "../escaped.json",
            "after.json",
            None,
        );
        let envelope = producer_envelope(std::slice::from_ref(&display), &display);
        let packet = root.write_packet("packet.json", &envelope)?;
        let error = validate_packet_route(&root.path, &packet)
            .err()
            .ok_or("expected a refusal")?;
        assert_eq!(error.disposition, DISPOSITION_WRONG_ROOT);
        Ok(())
    }

    #[test]
    fn unauthorized_execution_is_refused_with_a_public_disposition() -> Result<(), String> {
        let root = seeded_root("unauthorized")?;
        let display = agent_verify_command_spec(".", "before.json", "after.json", None).display;
        let envelope = producer_envelope(std::slice::from_ref(&display), &display);
        let packet = root.write_packet("packet.json", &envelope)?;
        let outcome =
            execute_verify_packet(&root.path, &packet, Path::new("result.json"), false, None);
        assert_eq!(outcome.disposition, DISPOSITION_REJECTED);
        assert!(outcome.failed);
        let parsed: Value =
            serde_json::from_str(&outcome.rendered).map_err(|error| error.to_string())?;
        assert_eq!(
            parsed.get("disposition").and_then(Value::as_str),
            Some(DISPOSITION_REJECTED)
        );
        assert_eq!(parsed.get("executed").and_then(Value::as_bool), Some(false));
        assert!(!root.path.join("result.json").exists());
        Ok(())
    }

    #[test]
    fn output_reader_caps_without_returning_raw_content() -> Result<(), String> {
        let payload = vec![b'x'; (MAX_OUTPUT_BYTES + 10) as usize];
        let (rx, handle) = spawn_capture(std::io::Cursor::new(payload));
        let output = handle
            .join()
            .map_err(|_panic| "reader panicked".to_string())??;
        assert!(rx.recv().map_err(|error| error.to_string())?);
        assert!(output.truncated);
        assert_eq!(output.bytes.len() as u64, MAX_OUTPUT_BYTES);
        assert!(!String::from_utf8_lossy(&output.bytes).contains("secret-canary"));
        Ok(())
    }

    /// Build a result carrying `disposition` so the real
    /// `response_disposition` is exercised rather than a restated copy of it.
    fn result_with(
        disposition: VerificationProcessDispositionV1,
        exit_status: Option<i32>,
        head_after: &str,
    ) -> VerificationExecutionResultV1 {
        VerificationExecutionResultV1 {
            schema_version: VerificationExecutionResultV1::SCHEMA_VERSION.to_string(),
            root_identity: "/root".to_string(),
            head_before: "a".repeat(40),
            head_after: head_after.to_string(),
            command_spec_sha256: format!("sha256:{}", "0".repeat(64)),
            process_disposition: disposition,
            exit_status,
            stdout_sha256: format!("sha256:{}", "0".repeat(64)),
            stderr_sha256: format!("sha256:{}", "0".repeat(64)),
            currentness: VerificationCurrentnessV1::Current,
            duration_ms: 1,
            exit_signal: None,
            stdout_bytes: 0,
            stderr_bytes: 0,
            stdout_truncated: false,
            stderr_truncated: false,
            cancellation_requested: false,
        }
    }

    #[test]
    fn every_process_disposition_maps_to_its_public_token() {
        let spec = agent_verify_command_spec(".", "before.json", "after.json", None);
        let head = "a".repeat(40);
        let cases: Vec<(VerificationProcessDispositionV1, Option<i32>, &str)> = vec![
            (
                VerificationProcessDispositionV1::Completed,
                Some(0),
                DISPOSITION_PASS,
            ),
            (
                VerificationProcessDispositionV1::Completed,
                Some(1),
                DISPOSITION_FAIL,
            ),
            (
                VerificationProcessDispositionV1::Completed,
                None,
                DISPOSITION_FAIL,
            ),
            (
                VerificationProcessDispositionV1::FailedToStart,
                None,
                DISPOSITION_NOT_FOUND,
            ),
            (
                VerificationProcessDispositionV1::TimedOut,
                None,
                DISPOSITION_TIMED_OUT,
            ),
            (
                VerificationProcessDispositionV1::Cancelled,
                None,
                DISPOSITION_CANCELLED,
            ),
            (
                VerificationProcessDispositionV1::OutputLimitExceeded,
                None,
                DISPOSITION_OUTPUT_LIMITED,
            ),
        ];
        for (disposition, exit_status, expected) in cases {
            let result = result_with(disposition, exit_status, &head);
            assert_eq!(
                response_disposition(&result, &spec),
                expected,
                "{disposition:?} with exit {exit_status:?}"
            );
        }
    }

    #[test]
    fn head_movement_outranks_a_clean_exit() {
        let spec = agent_verify_command_spec(".", "before.json", "after.json", None);
        let moved = result_with(
            VerificationProcessDispositionV1::Completed,
            Some(0),
            &"b".repeat(40),
        );
        assert_eq!(
            response_disposition(&moved, &spec),
            DISPOSITION_REPOSITORY_CHANGED
        );
    }

    /// A truncated-but-clean exit must not be committed as a pass, and must not
    /// carry an exit status the transported contract forbids.
    #[test]
    fn truncated_clean_exit_is_limited_and_drops_its_exit_status() -> Result<(), String> {
        let spec = agent_verify_command_spec(".", "before.json", "after.json", None);
        let result = result_with(
            VerificationProcessDispositionV1::OutputLimitExceeded,
            None,
            &"a".repeat(40),
        );
        assert_eq!(
            response_disposition(&result, &spec),
            DISPOSITION_OUTPUT_LIMITED
        );
        // The domain validator is the authority that forbids the pairing.
        let paired = result_with(
            VerificationProcessDispositionV1::OutputLimitExceeded,
            Some(0),
            &"a".repeat(40),
        );
        assert!(
            paired
                .validate_against(
                    &spec,
                    &paired.root_identity,
                    &paired.head_before,
                    &paired.head_after
                )
                .is_err(),
            "an exit status paired with output_limit_exceeded must fail closed"
        );
        Ok(())
    }

    /// An ambient credential must not cross the process boundary. `PATH` must,
    /// or the verify route cannot find `git` and every run reports a false
    /// failure.
    #[test]
    fn environment_floor_drops_secrets_and_keeps_the_platform_minimum() {
        let canaries = [
            "RIPR_TEST_SECRET_CANARY",
            "GITHUB_TOKEN",
            "AWS_SECRET_ACCESS_KEY",
        ];
        for name in canaries {
            assert!(
                !ENVIRONMENT_FLOOR.contains(&name),
                "{name} must never be in the environment floor"
            );
        }
        assert!(
            ENVIRONMENT_FLOOR.contains(&"PATH"),
            "PATH is required for the verify route to resolve git"
        );
        let names = child_environment()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        for name in canaries {
            assert!(!names.contains(&name.to_string()), "{name} leaked to child");
        }
        // Every name that survives is one the floor declared.
        for name in &names {
            assert!(
                ENVIRONMENT_FLOOR.contains(&name.as_str()),
                "undeclared variable {name} reached the child"
            );
        }
    }

    #[test]
    fn normalize_strips_only_drive_letter_verbatim_prefixes() {
        // `check-local-context` forbids drive-letter path literals anywhere in
        // tracked files, so the Windows shapes are assembled from parts.
        let drive = "C:";
        let plain = format!(r"{drive}\work\repo");
        assert_eq!(
            normalize(Path::new(&format!(r"\\?\{plain}"))),
            PathBuf::from(&plain)
        );
        // Stripping a UNC verbatim prefix would change the path's meaning.
        assert_eq!(
            normalize(Path::new(r"\\?\UNC\server\share")),
            PathBuf::from(r"\\?\UNC\server\share")
        );
        assert_eq!(
            normalize(Path::new("/work/repo")),
            PathBuf::from("/work/repo")
        );
    }

    #[test]
    fn result_destination_failure_is_reported_and_not_overwritten() -> Result<(), String> {
        let root = TempRoot::new("write")?;
        root.write("result.json", b"existing")?;
        let error = atomic_write(&root.path.join("result.json"), b"replacement")
            .err()
            .ok_or("expected a write refusal")?;
        assert!(error.contains("refusing to overwrite"));
        assert_eq!(
            fs::read_to_string(root.path.join("result.json")).map_err(|error| error.to_string())?,
            "existing"
        );
        // No temporary file is left behind.
        let leftovers = fs::read_dir(&root.path)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        Ok(())
    }
}
