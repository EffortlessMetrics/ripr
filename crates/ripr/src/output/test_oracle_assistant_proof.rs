use serde::Serialize;
use serde_json::Value;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "test_oracle_assistant_loop";
const PASS_FAIL_AUTHORITY: &str = "gate decision when explicitly configured";
pub(crate) const DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT: &str =
    "target/ripr/reports/test-oracle-assistant-proof.json";
pub(crate) const DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_MD_OUT: &str =
    "target/ripr/reports/test-oracle-assistant-proof.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TestOracleAssistantProofInput {
    pub(crate) root: String,
    pub(crate) pr_guidance_path: Option<String>,
    pub(crate) agent_packet_path: Option<String>,
    pub(crate) before_path: Option<String>,
    pub(crate) after_path: Option<String>,
    pub(crate) receipt_path: Option<String>,
    pub(crate) ledger_path: Option<String>,
    pub(crate) coverage_frontier_path: Option<String>,
    pub(crate) gate_decision_path: Option<String>,
    pub(crate) pr_guidance_json: Option<Result<String, String>>,
    pub(crate) agent_packet_json: Option<Result<String, String>>,
    pub(crate) before_json: Option<Result<String, String>>,
    pub(crate) after_json: Option<Result<String, String>>,
    pub(crate) receipt_json: Option<Result<String, String>>,
    pub(crate) ledger_json: Option<Result<String, String>>,
    pub(crate) coverage_frontier_json: Option<Result<String, String>>,
    pub(crate) gate_decision_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TestOracleAssistantProofReport {
    status: String,
    root: String,
    inputs: ProofInputs,
    seam: ProofSeam,
    recommendation: ProofRecommendation,
    handoff: ProofHandoff,
    evidence_movement: ProofEvidenceMovement,
    ci_projection: ProofCiProjection,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofInputs {
    pr_guidance: Option<String>,
    agent_packet: Option<String>,
    before: Option<String>,
    after: Option<String>,
    receipt: Option<String>,
    ledger: Option<String>,
    coverage_frontier: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofSeam {
    seam_id: Option<String>,
    canonical_gap_id: Option<String>,
    owner: Option<String>,
    seam_kind: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    grip_class: Option<String>,
    missing_discriminator: Option<String>,
    evidence_source: String,
    static_limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofRecommendation {
    source: String,
    placement: String,
    summary_only_reason: Option<String>,
    suggested_test: Option<String>,
    related_test: Option<String>,
    assertion_shape: Option<String>,
    verify_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofHandoff {
    source: String,
    artifact: Option<String>,
    agent_command: Option<String>,
    external_provider: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofEvidenceMovement {
    state: String,
    before_class: Option<String>,
    after_class: Option<String>,
    source: String,
    artifact: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofCiProjection {
    ledger: Option<String>,
    coverage_frontier: Option<String>,
    gate_decision: Option<String>,
    pass_fail_authority: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ParsedSources {
    pr_guidance: Option<Value>,
    agent_packet: Option<Value>,
    before: Option<Value>,
    after: Option<Value>,
    receipt: Option<Value>,
    ledger: Option<Value>,
    coverage_frontier: Option<Value>,
    gate_decision: Option<Value>,
    warnings: Vec<String>,
}

pub(crate) fn build_test_oracle_assistant_proof_report(
    input: TestOracleAssistantProofInput,
) -> TestOracleAssistantProofReport {
    let parsed = parse_sources(&input);
    let seam = selected_seam(&parsed);
    let recommendation = recommendation_from_sources(&input, &parsed, &seam);
    let handoff = handoff_from_sources(&input, &parsed, &seam);
    let evidence_movement = evidence_movement_from_sources(&input, &parsed, &seam);
    let movement_warnings = loop_warnings(&parsed, &evidence_movement);
    let mut warnings = parsed.warnings;
    warnings.extend(movement_warnings);
    let status = if seam.seam_id.is_some()
        && evidence_movement.before_class.is_some()
        && evidence_movement.after_class.is_some()
    {
        "advisory"
    } else {
        "incomplete"
    }
    .to_string();

    TestOracleAssistantProofReport {
        status,
        root: input.root,
        inputs: ProofInputs {
            pr_guidance: input.pr_guidance_path,
            agent_packet: input.agent_packet_path,
            before: input.before_path,
            after: input.after_path,
            receipt: input.receipt_path,
            ledger: input.ledger_path.clone(),
            coverage_frontier: input.coverage_frontier_path.clone(),
        },
        seam,
        recommendation,
        handoff,
        evidence_movement,
        ci_projection: ProofCiProjection {
            ledger: input.ledger_path,
            coverage_frontier: input.coverage_frontier_path,
            gate_decision: input.gate_decision_path,
            pass_fail_authority: PASS_FAIL_AUTHORITY.to_string(),
        },
        warnings,
    }
}

pub(crate) fn render_test_oracle_assistant_proof_json(
    report: &TestOracleAssistantProofReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct Limits {
        advisory: bool,
        source_edits: bool,
        generated_tests: bool,
        external_service: bool,
        runtime_mutation_execution: bool,
        ci_blocking_default: bool,
    }

    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        root: &'a str,
        inputs: &'a ProofInputs,
        seam: &'a ProofSeam,
        recommendation: &'a ProofRecommendation,
        handoff: &'a ProofHandoff,
        evidence_movement: &'a ProofEvidenceMovement,
        ci_projection: &'a ProofCiProjection,
        warnings: &'a [String],
        limits: Limits,
    }

    serde_json::to_string_pretty(&JsonReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: &report.status,
        root: &report.root,
        inputs: &report.inputs,
        seam: &report.seam,
        recommendation: &report.recommendation,
        handoff: &report.handoff,
        evidence_movement: &report.evidence_movement,
        ci_projection: &report.ci_projection,
        warnings: &report.warnings,
        limits: Limits {
            advisory: true,
            source_edits: false,
            generated_tests: false,
            external_service: false,
            runtime_mutation_execution: false,
            ci_blocking_default: false,
        },
    })
    .map_err(|err| format!("render test-oracle assistant proof JSON failed: {err}"))
}

pub(crate) fn render_test_oracle_assistant_proof_markdown(
    report: &TestOracleAssistantProofReport,
) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Test-Oracle Assistant Loop\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    out.push_str("Top focused test:\n");
    out.push_str(&format!("- Seam: {}\n", seam_headline(&report.seam)));
    out.push_str(&format!(
        "- Owner: {}\n",
        option_text(report.seam.owner.as_deref(), "unknown")
    ));
    out.push_str(&format!(
        "- Missing discriminator: {}\n",
        report
            .seam
            .missing_discriminator
            .as_deref()
            .map_or("unknown", |value| value)
    ));
    out.push_str(&format!(
        "- Suggested test: {}\n",
        report
            .recommendation
            .suggested_test
            .as_deref()
            .map_or("unknown", |value| value)
    ));
    out.push_str(&format!(
        "- Related test: {}\n",
        report
            .recommendation
            .related_test
            .as_deref()
            .map_or("unknown", |value| value)
    ));
    out.push_str(&format!(
        "- Assertion shape: {}\n",
        report
            .recommendation
            .assertion_shape
            .as_deref()
            .map_or("unknown", |value| value)
    ));
    out.push_str(&format!(
        "- Verify: {}\n\n",
        report
            .recommendation
            .verify_command
            .as_deref()
            .map_or("unknown", |value| value)
    ));

    out.push_str("Movement:\n");
    out.push_str(&format!(
        "- Before: {}\n",
        report
            .evidence_movement
            .before_class
            .as_deref()
            .map_or("unknown", |value| value)
    ));
    out.push_str(&format!(
        "- After: {}\n",
        report
            .evidence_movement
            .after_class
            .as_deref()
            .map_or("unknown", |value| value)
    ));
    out.push_str(&format!("- State: {}\n", report.evidence_movement.state));
    out.push_str(&format!(
        "- Receipt: {}\n\n",
        report
            .evidence_movement
            .artifact
            .as_deref()
            .map_or("not available", |value| value)
    ));

    out.push_str("Projection:\n");
    out.push_str(&format!(
        "- PR ledger: {}\n",
        report
            .ci_projection
            .ledger
            .as_deref()
            .map_or("not available", |value| value)
    ));
    out.push_str(&format!(
        "- Coverage/grip frontier: {}\n",
        report
            .ci_projection
            .coverage_frontier
            .as_deref()
            .map_or("not available", |value| value)
    ));
    out.push_str(&format!(
        "- Gate: {}\n",
        report
            .ci_projection
            .gate_decision
            .as_deref()
            .map_or("not configured", |value| value)
    ));

    if !report.seam.static_limitations.is_empty() {
        out.push_str("\nStatic limits:\n");
        for limitation in &report.seam.static_limitations {
            out.push_str(&format!("- {limitation}\n"));
        }
    }

    if !report.warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }

    out.push_str("\nLimits:\n");
    out.push_str("- Static RIPR evidence only.\n");
    out.push_str("- Advisory by default.\n");
    out.push_str("- No source edits, generated tests, provider calls, or mutation execution.\n");
    out
}

pub(crate) use crate::output::path::display_path;

fn option_text<'a>(value: Option<&'a str>, fallback: &'a str) -> &'a str {
    match value {
        Some(value) => value,
        None => fallback,
    }
}

fn parse_sources(input: &TestOracleAssistantProofInput) -> ParsedSources {
    let mut parsed = ParsedSources::default();
    parsed.pr_guidance = parse_optional_json(
        "PR guidance",
        input.pr_guidance_path.as_deref(),
        &input.pr_guidance_json,
        &mut parsed.warnings,
    );
    parsed.agent_packet = parse_optional_json(
        "agent packet",
        input.agent_packet_path.as_deref(),
        &input.agent_packet_json,
        &mut parsed.warnings,
    );
    parsed.before = parse_optional_json(
        "before evidence",
        input.before_path.as_deref(),
        &input.before_json,
        &mut parsed.warnings,
    );
    parsed.after = parse_optional_json(
        "after evidence",
        input.after_path.as_deref(),
        &input.after_json,
        &mut parsed.warnings,
    );
    parsed.receipt = parse_optional_json(
        "receipt",
        input.receipt_path.as_deref(),
        &input.receipt_json,
        &mut parsed.warnings,
    );
    parsed.ledger = parse_optional_json(
        "PR evidence ledger",
        input.ledger_path.as_deref(),
        &input.ledger_json,
        &mut parsed.warnings,
    );
    parsed.coverage_frontier = parse_optional_json(
        "coverage/grip frontier",
        input.coverage_frontier_path.as_deref(),
        &input.coverage_frontier_json,
        &mut parsed.warnings,
    );
    parsed.gate_decision = parse_optional_json(
        "gate decision",
        input.gate_decision_path.as_deref(),
        &input.gate_decision_json,
        &mut parsed.warnings,
    );
    parsed
}

fn parse_optional_json(
    label: &str,
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    warnings: &mut Vec<String>,
) -> Option<Value> {
    let path = path?;
    let Some(text) = text else {
        warnings.push(format!(
            "{label} path {path} was supplied but no input text was loaded"
        ));
        return None;
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            warnings.push(format!("optional {label} input {path} is invalid: {error}"));
            return None;
        }
    };
    match serde_json::from_str::<Value>(text) {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(format!("optional {label} input {path} is invalid: {error}"));
            None
        }
    }
}

fn selected_seam(parsed: &ParsedSources) -> ProofSeam {
    let agent_seam = first_agent_seam(parsed.agent_packet.as_ref());
    let agent_record = evidence_record_from_agent_seam(agent_seam);
    let guidance = first_guidance_item(parsed.pr_guidance.as_ref())
        .or_else(|| first_summary_only_item(parsed.pr_guidance.as_ref()));
    let seam_id = string_from_sources(&[
        (agent_record, &["seam_id"]),
        (agent_seam, &["seam_id"]),
        (guidance, &["seam_id"]),
        (parsed.receipt.as_ref(), &["provenance", "seam_id"]),
        (parsed.receipt.as_ref(), &["seam", "seam_id"]),
    ]);
    let before_seam = seam_id
        .as_deref()
        .and_then(|id| find_repo_exposure_seam(parsed.before.as_ref(), id));
    let after_seam = seam_id
        .as_deref()
        .and_then(|id| find_repo_exposure_seam(parsed.after.as_ref(), id));
    let before_record = evidence_record_from_repo_seam(before_seam);
    let after_record = evidence_record_from_repo_seam(after_seam);
    let evidence_source =
        if agent_record.is_some() || before_record.is_some() || after_record.is_some() {
            "evidence_record"
        } else {
            "legacy_fields"
        }
        .to_string();
    ProofSeam {
        seam_id,
        canonical_gap_id: string_from_sources(&[
            (agent_record, &["canonical_gap_id"]),
            (before_record, &["canonical_gap_id"]),
            (after_record, &["canonical_gap_id"]),
        ]),
        owner: string_from_sources(&[
            (agent_record, &["owner"]),
            (before_record, &["owner"]),
            (after_record, &["owner"]),
            (agent_seam, &["owner"]),
            (guidance, &["owner"]),
        ]),
        seam_kind: string_from_sources(&[
            (agent_record, &["seam_kind"]),
            (before_record, &["seam_kind"]),
            (after_record, &["seam_kind"]),
            (agent_seam, &["seam_kind"]),
            (guidance, &["kind"]),
            (guidance, &["seam", "kind"]),
            (before_seam, &["kind"]),
            (after_seam, &["kind"]),
            (parsed.receipt.as_ref(), &["seam", "seam_kind"]),
        ]),
        path: string_from_sources(&[
            (agent_record, &["location", "file"]),
            (before_record, &["location", "file"]),
            (after_record, &["location", "file"]),
            (agent_seam, &["file"]),
            (guidance, &["placement", "path"]),
            (guidance, &["seam", "file"]),
            (before_seam, &["file"]),
            (after_seam, &["file"]),
            (parsed.receipt.as_ref(), &["seam", "file"]),
        ]),
        line: u64_from_sources(&[
            (agent_record, &["location", "line"]),
            (before_record, &["location", "line"]),
            (after_record, &["location", "line"]),
            (agent_seam, &["line"]),
            (guidance, &["placement", "line"]),
            (guidance, &["seam", "line"]),
            (before_seam, &["line"]),
            (after_seam, &["line"]),
            (parsed.receipt.as_ref(), &["seam", "line"]),
        ]),
        grip_class: string_from_sources(&[
            (agent_record, &["grip_class"]),
            (before_record, &["grip_class"]),
            (after_record, &["grip_class"]),
            (agent_seam, &["grip_class"]),
            (agent_seam, &["current_grip"]),
            (guidance, &["grip_class"]),
            (before_seam, &["grip_class"]),
            (after_seam, &["grip_class"]),
        ]),
        missing_discriminator: missing_discriminator(agent_record)
            .or_else(|| missing_discriminator(before_record))
            .or_else(|| missing_discriminator(after_record))
            .or_else(|| missing_discriminator(agent_seam))
            .or_else(|| guidance.and_then(|value| string_path(value, &["missing_discriminator"])))
            .or_else(|| missing_discriminator(before_seam))
            .or_else(|| missing_discriminator(after_seam)),
        evidence_source,
        static_limitations: static_limitations_from_records(&[
            agent_record,
            before_record,
            after_record,
        ]),
    }
}

fn recommendation_from_sources(
    input: &TestOracleAssistantProofInput,
    parsed: &ParsedSources,
    seam: &ProofSeam,
) -> ProofRecommendation {
    let agent_seam = first_agent_seam(parsed.agent_packet.as_ref());
    let agent_record = evidence_record_from_agent_seam(agent_seam);
    let guidance = first_guidance_item(parsed.pr_guidance.as_ref());
    let summary_only = first_summary_only_item(parsed.pr_guidance.as_ref());
    let source = if agent_record.is_some() {
        "evidence_record"
    } else if agent_seam.is_some() {
        "editor_agent_brief"
    } else if guidance.is_some() || summary_only.is_some() {
        "pr_guidance"
    } else {
        "unknown"
    }
    .to_string();
    let placement = {
        let placement = placement_from_guidance(guidance, summary_only);
        if placement == "unknown" && agent_record.is_some() {
            "changed_line".to_string()
        } else {
            placement
        }
    };
    let summary_only_reason = summary_only
        .and_then(|value| string_path(value, &["summary_only_reason"]))
        .or_else(|| summary_only.and_then(|value| string_path(value, &["reason"])));
    let suggested_test = agent_record
        .and_then(suggested_test_from_record)
        .or_else(|| agent_seam.and_then(|value| suggested_test_sentence(value, seam)))
        .or_else(|| guidance.and_then(suggested_test_from_guidance))
        .or_else(|| summary_only.and_then(suggested_test_from_guidance));
    let related_test = agent_record
        .and_then(related_test_from_record)
        .or_else(|| agent_seam.and_then(related_test_from_agent))
        .or_else(|| guidance.and_then(related_test_from_guidance))
        .or_else(|| summary_only.and_then(related_test_from_guidance));
    let assertion_shape = agent_record
        .and_then(assertion_shape_from_record)
        .or_else(|| {
            agent_seam.and_then(|value| string_path(value, &["assertion_shape", "example"]))
        })
        .or_else(|| {
            guidance.and_then(|value| string_path(value, &["suggested_test", "assertion_shape"]))
        })
        .or_else(|| {
            summary_only
                .and_then(|value| string_path(value, &["suggested_test", "assertion_shape"]))
        });
    let verify_command = string_from_sources(&[
        (agent_record, &["recommendation", "verify_command"]),
        (agent_seam, &["verification", "verify_command"]),
        (guidance, &["llm_guidance", "verify_command"]),
        (summary_only, &["llm_guidance", "verify_command"]),
    ]);
    let verify_command = verify_command.or_else(|| {
        if input.before_path.is_some() && input.after_path.is_some() {
            let before = input
                .before_path
                .as_deref()
                .map_or("before.json", |path| path);
            let after = input
                .after_path
                .as_deref()
                .map_or("after.json", |path| path);
            Some(format!(
                "ripr agent verify --root {} --before {} --after {} --json",
                agent_root(parsed.agent_packet.as_ref(), input),
                before,
                after
            ))
        } else {
            None
        }
    });

    ProofRecommendation {
        source,
        placement,
        summary_only_reason,
        suggested_test,
        related_test,
        assertion_shape,
        verify_command,
    }
}

fn handoff_from_sources(
    input: &TestOracleAssistantProofInput,
    parsed: &ParsedSources,
    seam: &ProofSeam,
) -> ProofHandoff {
    let source = if input.agent_packet_path.is_some() {
        "agent_packet"
    } else {
        "not_available"
    }
    .to_string();
    let agent_command = seam.seam_id.as_ref().map(|seam_id| {
        format!(
            "ripr agent start --root {} --seam-id {seam_id} --out target/ripr/workflow",
            agent_root(parsed.agent_packet.as_ref(), input)
        )
    });
    ProofHandoff {
        source,
        artifact: input.agent_packet_path.clone(),
        agent_command,
        external_provider: false,
    }
}

fn evidence_movement_from_sources(
    input: &TestOracleAssistantProofInput,
    parsed: &ParsedSources,
    seam: &ProofSeam,
) -> ProofEvidenceMovement {
    let before_seam = seam
        .seam_id
        .as_deref()
        .and_then(|id| find_repo_exposure_seam(parsed.before.as_ref(), id));
    let after_seam = seam
        .seam_id
        .as_deref()
        .and_then(|id| find_repo_exposure_seam(parsed.after.as_ref(), id));
    let before_record = evidence_record_from_repo_seam(before_seam);
    let after_record = evidence_record_from_repo_seam(after_seam);
    let before_class = string_from_sources(&[
        (parsed.receipt.as_ref(), &["provenance", "before_class"]),
        (parsed.receipt.as_ref(), &["seam", "before"]),
    ])
    .or_else(|| {
        string_from_sources(&[
            (before_record, &["grip_class"]),
            (before_seam, &["grip_class"]),
        ])
    });
    let after_class = string_from_sources(&[
        (parsed.receipt.as_ref(), &["provenance", "after_class"]),
        (parsed.receipt.as_ref(), &["seam", "after"]),
    ])
    .or_else(|| {
        string_from_sources(&[
            (after_record, &["grip_class"]),
            (after_seam, &["grip_class"]),
        ])
    });
    let state = match string_from_sources(&[
        (parsed.receipt.as_ref(), &["provenance", "movement"]),
        (parsed.receipt.as_ref(), &["seam", "change"]),
    ])
    .or_else(|| movement_from_classes(before_class.as_deref(), after_class.as_deref()))
    {
        Some(state) => state,
        None => "unknown".to_string(),
    };
    let has_record_movement = before_record.is_some() || after_record.is_some();
    let (source, artifact) = if input.receipt_path.is_some() {
        ("agent_receipt".to_string(), input.receipt_path.clone())
    } else if has_record_movement {
        ("evidence_record".to_string(), input.after_path.clone())
    } else if input.before_path.is_some() || input.after_path.is_some() {
        ("repo_exposure".to_string(), input.after_path.clone())
    } else {
        ("not_available".to_string(), None)
    };
    ProofEvidenceMovement {
        state,
        before_class,
        after_class,
        source,
        artifact,
    }
}

fn loop_warnings(parsed: &ParsedSources, movement: &ProofEvidenceMovement) -> Vec<String> {
    let mut warnings = Vec::new();
    if movement.state == "unchanged" && before_after_has_new_observed_value(parsed) {
        warnings.push(
            "current replay fixture preserves actual analyzer output: the focused-test snapshot remains weakly_gripped; dogfood receipt work records live movement separately"
                .to_string(),
        );
    }
    warnings
}

fn first_agent_seam(agent_packet: Option<&Value>) -> Option<&Value> {
    let agent_packet = agent_packet?;
    agent_packet
        .get("top_seams")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .or_else(|| {
            agent_packet
                .get("packets")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
        })
}

fn evidence_record_from_agent_seam(agent_seam: Option<&Value>) -> Option<&Value> {
    evidence_record_from_value(agent_seam)
}

fn evidence_record_from_repo_seam(repo_seam: Option<&Value>) -> Option<&Value> {
    evidence_record_from_value(repo_seam)
}

fn evidence_record_from_value(value: Option<&Value>) -> Option<&Value> {
    value
        .and_then(|value| value.get("evidence_record"))
        .filter(|value| value.is_object())
}

fn first_guidance_item(pr_guidance: Option<&Value>) -> Option<&Value> {
    pr_guidance
        .and_then(|value| value.get("comments"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
}

fn first_summary_only_item(pr_guidance: Option<&Value>) -> Option<&Value> {
    pr_guidance
        .and_then(|value| value.get("summary_only"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
}

fn find_repo_exposure_seam<'a>(
    repo_exposure: Option<&'a Value>,
    seam_id: &str,
) -> Option<&'a Value> {
    repo_exposure
        .and_then(|value| value.get("seams"))
        .and_then(Value::as_array)?
        .iter()
        .find(|seam| {
            string_path(seam, &["evidence_record", "seam_id"]).as_deref() == Some(seam_id)
                || string_path(seam, &["seam_id"]).as_deref() == Some(seam_id)
        })
}

fn placement_from_guidance(guidance: Option<&Value>, summary_only: Option<&Value>) -> String {
    if summary_only.is_some() {
        return "summary_only".to_string();
    }
    let Some(guidance) = guidance else {
        return "unknown".to_string();
    };
    match string_path(guidance, &["placement", "mode"]).as_deref() {
        Some("exact_seam_line" | "changed_line" | "changed-line") => "changed_line".to_string(),
        Some("summary_only") => "summary_only".to_string(),
        Some(_) => "unknown".to_string(),
        None => "unknown".to_string(),
    }
}

fn suggested_test_sentence(agent_seam: &Value, seam: &ProofSeam) -> Option<String> {
    let discriminator = seam
        .missing_discriminator
        .as_deref()
        .map(discriminator_subject)?;
    let function = match string_path(agent_seam, &["owner"])
        .and_then(|owner| owner.rsplit("::").next().map(ToOwned::to_owned))
        .filter(|value| !value.trim().is_empty())
    {
        Some(value) => value,
        None => "changed behavior".to_string(),
    };
    let subject = match string_path(agent_seam, &["expression"])
        .or_else(|| string_path(agent_seam, &["changed_expression"]))
        .and_then(|expression| comparison_left_side(&expression))
    {
        Some(value) => value,
        None => "the changed input".to_string(),
    };
    Some(format!(
        "Add a focused test where {subject} == {discriminator} and assert the exact {function} output."
    ))
}

fn suggested_test_from_guidance(guidance: &Value) -> Option<String> {
    string_path(guidance, &["suggested_test", "assertion_shape"])
        .or_else(|| string_path(guidance, &["suggested_test", "intent"]))
        .or_else(|| string_path(guidance, &["suggested_test"]))
}

fn suggested_test_from_record(record: &Value) -> Option<String> {
    let recommended = path_value(record, &["recommendation", "recommended_test"]);
    let name = recommended.and_then(|value| string_path(value, &["name"]));
    let file = recommended.and_then(|value| string_path(value, &["file"]));
    let assertion = assertion_shape_from_record(record);
    match (file, name, assertion) {
        (Some(file), Some(name), Some(assertion)) => {
            Some(format!("Add {file}::{name} with `{assertion}`."))
        }
        (Some(file), Some(name), None) => Some(format!("Add {file}::{name}.")),
        (None, Some(name), Some(assertion)) => Some(format!("Add {name} with `{assertion}`.")),
        (None, Some(name), None) => Some(format!("Add {name}.")),
        (_file, None, Some(assertion)) => Some(format!("Add a focused test with `{assertion}`.")),
        (_file, None, None) => string_path(record, &["recommendation", "reason"]),
    }
}

fn assertion_shape_from_record(record: &Value) -> Option<String> {
    string_path(record, &["recommendation", "assertion_shape", "example"])
        .or_else(|| string_path(record, &["recommendation", "assertion_shape", "kind"]))
}

fn related_test_from_agent(agent_seam: &Value) -> Option<String> {
    let related = agent_seam.get("nearest_strong_test_to_imitate")?;
    let name = string_path(related, &["name"])?;
    let file = string_path(related, &["file"]);
    Some(match file {
        Some(file) => format!("{file}::{name}"),
        None => name,
    })
}

fn related_test_from_record(record: &Value) -> Option<String> {
    let related =
        path_value(record, &["recommendation", "nearest_test_to_imitate"]).or_else(|| {
            record
                .get("related_tests")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
        })?;
    let name = string_path(related, &["name"])?;
    let file = string_path(related, &["file"]);
    Some(match file {
        Some(file) => format!("{file}::{name}"),
        None => name,
    })
}

fn related_test_from_guidance(guidance: &Value) -> Option<String> {
    let name = string_path(guidance, &["suggested_test", "near_test"])
        .or_else(|| string_path(guidance, &["suggested_test", "recommended_name"]))?;
    let file = string_path(guidance, &["suggested_test", "recommended_file"]);
    Some(match file {
        Some(file) => format!("{file}::{name}"),
        None => name,
    })
}

fn movement_from_classes(before: Option<&str>, after: Option<&str>) -> Option<String> {
    let before = before?;
    let after = after?;
    if before == after {
        return Some("unchanged".to_string());
    }
    let before_rank = grip_rank(before);
    let after_rank = grip_rank(after);
    match (before_rank, after_rank) {
        (Some(before), Some(after)) if after > before => Some("improved".to_string()),
        (Some(before), Some(after)) if after < before => Some("regressed".to_string()),
        _ => Some("unknown".to_string()),
    }
}

fn grip_rank(value: &str) -> Option<u8> {
    match value {
        "ungripped" | "reachable_unrevealed" => Some(0),
        "weakly_gripped" | "weakly_exposed" => Some(1),
        "strongly_gripped" | "exposed" => Some(2),
        _ => None,
    }
}

fn before_after_has_new_observed_value(parsed: &ParsedSources) -> bool {
    let Some(seam_id) = string_from_sources(&[
        (parsed.receipt.as_ref(), &["provenance", "seam_id"]),
        (parsed.receipt.as_ref(), &["seam", "seam_id"]),
    ]) else {
        return false;
    };
    let before = find_repo_exposure_seam(parsed.before.as_ref(), &seam_id);
    let after = find_repo_exposure_seam(parsed.after.as_ref(), &seam_id);
    let before_count = observed_value_count(before);
    let after_count = observed_value_count(after);
    after_count > before_count
}

fn observed_value_count(seam: Option<&Value>) -> usize {
    let record = evidence_record_from_repo_seam(seam);
    record
        .or(seam)
        .and_then(|value| value.get("observed_values"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn missing_discriminator(value: Option<&Value>) -> Option<String> {
    let value = value?;
    string_path(value, &["missing_discriminator"]).or_else(|| {
        value
            .get("missing_discriminators")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| string_path(item, &["value"]))
    })
}

fn static_limitations_from_records(records: &[Option<&Value>]) -> Vec<String> {
    let mut limitations = Vec::new();
    for record in records.iter().flatten() {
        let Some(items) = record.get("static_limitations").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let limitation = static_limitation_label(item);
            if !limitation.trim().is_empty() && !limitations.contains(&limitation) {
                limitations.push(limitation);
            }
        }
    }
    limitations
}

fn static_limitation_label(item: &Value) -> String {
    let stage = string_path(item, &["stage"]);
    let state = string_path(item, &["state"]);
    let reason = string_path(item, &["reason"]);
    match (stage, state, reason) {
        (Some(stage), Some(state), Some(reason)) => format!("{stage} {state}: {reason}"),
        (Some(stage), None, Some(reason)) => format!("{stage}: {reason}"),
        (None, Some(state), Some(reason)) => format!("{state}: {reason}"),
        (Some(stage), Some(state), None) => format!("{stage} {state}"),
        (Some(stage), None, None) => stage,
        (None, Some(state), None) => state,
        (None, None, Some(reason)) => reason,
        (None, None, None) => String::new(),
    }
}

fn discriminator_subject(value: &str) -> String {
    match value
        .split(" (")
        .next()
        .filter(|part| !part.trim().is_empty())
    {
        Some(part) => part.to_string(),
        None => value.to_string(),
    }
}

fn comparison_left_side(expression: &str) -> Option<String> {
    for operator in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some((left, _right)) = expression.split_once(operator) {
            let left = left.trim();
            if !left.is_empty() {
                return Some(left.to_string());
            }
        }
    }
    None
}

fn agent_root(agent_packet: Option<&Value>, input: &TestOracleAssistantProofInput) -> String {
    match agent_packet.and_then(|value| string_path(value, &["root"])) {
        Some(root) => root,
        None => input.root.clone(),
    }
}

fn seam_headline(seam: &ProofSeam) -> String {
    match (seam.path.as_deref(), seam.line) {
        (Some(path), Some(line)) => format!("{path}:{line}"),
        (Some(path), None) => path.to_string(),
        (None, Some(line)) => format!("unknown:{line}"),
        (None, None) => "unknown".to_string(),
    }
}

fn string_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<String> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| string_path(value, path)))
}

fn u64_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<u64> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| u64_path(value, path)))
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(value_as_string)
}

fn u64_path(value: &Value, path: &[&str]) -> Option<u64> {
    path_value(value, path).and_then(Value::as_u64)
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn value_as_string(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    value.as_u64().map(|number| number.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_oracle_assistant_proof_matches_canonical_fixture() -> Result<(), String> {
        let repo_root = repo_root()?;
        let fixture =
            repo_root.join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical");
        let pr_guidance = fixture.join("pr-guidance.json");
        let agent_packet =
            repo_root.join("fixtures/boundary_gap/expected/editor-agent-loop/agent-brief.json");
        let before = repo_root
            .join("fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json");
        let after = repo_root
            .join("fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json");
        let receipt =
            repo_root.join("fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json");
        let ledger = fixture.join("pr-evidence-ledger.json");
        let expected_json = fixture.join("test-oracle-assistant-proof.json");
        let expected_md = fixture.join("test-oracle-assistant-proof.md");

        let report = build_test_oracle_assistant_proof_report(TestOracleAssistantProofInput {
            root: ".".to_string(),
            pr_guidance_path: Some(fixture_path(&repo_root, &pr_guidance)),
            agent_packet_path: Some(fixture_path(&repo_root, &agent_packet)),
            before_path: Some(fixture_path(&repo_root, &before)),
            after_path: Some(fixture_path(&repo_root, &after)),
            receipt_path: Some(fixture_path(&repo_root, &receipt)),
            ledger_path: Some(fixture_path(&repo_root, &ledger)),
            coverage_frontier_path: None,
            gate_decision_path: None,
            pr_guidance_json: Some(Ok(read_file(&pr_guidance)?)),
            agent_packet_json: Some(Ok(read_file(&agent_packet)?)),
            before_json: Some(Ok(read_file(&before)?)),
            after_json: Some(Ok(read_file(&after)?)),
            receipt_json: Some(Ok(read_file(&receipt)?)),
            ledger_json: Some(Ok(read_file(&ledger)?)),
            coverage_frontier_json: None,
            gate_decision_json: None,
        });

        assert_eq!(
            render_test_oracle_assistant_proof_json(&report)?,
            read_file(&expected_json)?.trim_end()
        );
        assert_eq!(
            render_test_oracle_assistant_proof_markdown(&report),
            read_file(&expected_md)?
        );
        Ok(())
    }

    #[test]
    fn test_oracle_assistant_proof_reports_incomplete_without_selected_seam() -> Result<(), String>
    {
        let report = build_test_oracle_assistant_proof_report(TestOracleAssistantProofInput {
            root: ".".to_string(),
            pr_guidance_path: None,
            agent_packet_path: None,
            before_path: None,
            after_path: None,
            receipt_path: None,
            ledger_path: None,
            coverage_frontier_path: None,
            gate_decision_path: None,
            pr_guidance_json: None,
            agent_packet_json: None,
            before_json: None,
            after_json: None,
            receipt_json: None,
            ledger_json: None,
            coverage_frontier_json: None,
            gate_decision_json: None,
        });
        let rendered = render_test_oracle_assistant_proof_json(&report)?;
        assert!(rendered.contains("\"status\": \"incomplete\""));
        assert!(rendered.contains("\"external_provider\": false"));
        assert!(rendered.contains("\"ci_blocking_default\": false"));
        Ok(())
    }

    #[test]
    fn test_oracle_assistant_proof_warns_for_invalid_supplied_inputs() -> Result<(), String> {
        let report = build_test_oracle_assistant_proof_report(TestOracleAssistantProofInput {
            root: ".".to_string(),
            pr_guidance_path: Some("comments.json".to_string()),
            agent_packet_path: Some("agent-brief.json".to_string()),
            before_path: None,
            after_path: None,
            receipt_path: None,
            ledger_path: None,
            coverage_frontier_path: None,
            gate_decision_path: None,
            pr_guidance_json: Some(Ok("{".to_string())),
            agent_packet_json: Some(Err("missing agent packet".to_string())),
            before_json: None,
            after_json: None,
            receipt_json: None,
            ledger_json: None,
            coverage_frontier_json: None,
            gate_decision_json: None,
        });
        let rendered = render_test_oracle_assistant_proof_json(&report)?;
        assert!(rendered.contains("optional PR guidance input comments.json is invalid"));
        assert!(rendered.contains("optional agent packet input agent-brief.json is invalid"));
        Ok(())
    }

    #[test]
    fn test_oracle_assistant_proof_keeps_summary_only_guidance_visible() -> Result<(), String> {
        let guidance = r#"{
          "summary_only": [
            {
              "seam_id": "summary-seam",
              "kind": "predicate_boundary",
              "grip_class": "weakly_gripped",
              "missing_discriminator": "threshold equality",
              "summary_only_reason": "no changed line could safely receive an annotation",
              "suggested_test": {
                "assertion_shape": "assert_eq!(discounted_total(threshold), expected)",
                "near_test": "below_threshold_has_no_discount",
                "recommended_file": "tests/pricing.rs"
              }
            }
          ]
        }"#;
        let before = repo_exposure("summary-seam", "weakly_gripped", "src/lib.rs", 7);
        let after = repo_exposure("summary-seam", "strongly_gripped", "src/lib.rs", 7);
        let report = build_test_oracle_assistant_proof_report(TestOracleAssistantProofInput {
            root: ".".to_string(),
            pr_guidance_path: Some("comments.json".to_string()),
            agent_packet_path: None,
            before_path: Some("before.json".to_string()),
            after_path: Some("after.json".to_string()),
            receipt_path: None,
            ledger_path: Some("ledger.json".to_string()),
            coverage_frontier_path: Some("coverage-grip-frontier.json".to_string()),
            gate_decision_path: Some("gate-decision.json".to_string()),
            pr_guidance_json: Some(Ok(guidance.to_string())),
            agent_packet_json: None,
            before_json: Some(Ok(before)),
            after_json: Some(Ok(after)),
            receipt_json: None,
            ledger_json: Some(Ok(r#"{"kind":"pr_evidence_ledger"}"#.to_string())),
            coverage_frontier_json: Some(Ok(r#"{"kind":"coverage_grip_frontier"}"#.to_string())),
            gate_decision_json: Some(Ok(r#"{"kind":"gate_decision"}"#.to_string())),
        });

        let rendered = render_test_oracle_assistant_proof_json(&report)?;
        assert!(rendered.contains("\"status\": \"advisory\""));
        assert!(rendered.contains("\"placement\": \"summary_only\""));
        assert!(rendered.contains("no changed line could safely receive an annotation"));
        assert!(rendered.contains("\"state\": \"improved\""));
        assert!(rendered.contains("\"source\": \"repo_exposure\""));
        assert!(rendered.contains("\"coverage_frontier\": \"coverage-grip-frontier.json\""));
        assert!(rendered.contains("\"gate_decision\": \"gate-decision.json\""));
        assert!(rendered.contains("tests/pricing.rs::below_threshold_has_no_discount"));
        Ok(())
    }

    #[test]
    fn test_oracle_assistant_proof_handles_guidance_only_regression() -> Result<(), String> {
        let guidance = r#"{
          "comments": [
            {
              "seam_id": "regressed-seam",
              "kind": "predicate_boundary",
              "grip_class": "strongly_gripped",
              "missing_discriminator": "variant equality",
              "placement": {"path": "src/lib.rs", "line": 9, "mode": "changed_line"},
              "suggested_test": {
                "intent": "Add one focused discriminator test.",
                "recommended_name": "variant_boundary",
                "recommended_file": "tests/pricing.rs"
              },
              "llm_guidance": {"verify_command": "ripr agent verify --json"}
            }
          ]
        }"#;
        let before = repo_exposure("regressed-seam", "strongly_gripped", "src/lib.rs", 9);
        let after = repo_exposure("regressed-seam", "weakly_gripped", "src/lib.rs", 9);
        let report = build_test_oracle_assistant_proof_report(TestOracleAssistantProofInput {
            root: "fixture-root".to_string(),
            pr_guidance_path: Some("comments.json".to_string()),
            agent_packet_path: None,
            before_path: Some("before.json".to_string()),
            after_path: Some("after.json".to_string()),
            receipt_path: None,
            ledger_path: None,
            coverage_frontier_path: None,
            gate_decision_path: None,
            pr_guidance_json: Some(Ok(guidance.to_string())),
            agent_packet_json: None,
            before_json: Some(Ok(before)),
            after_json: Some(Ok(after)),
            receipt_json: None,
            ledger_json: None,
            coverage_frontier_json: None,
            gate_decision_json: None,
        });

        let rendered = render_test_oracle_assistant_proof_json(&report)?;
        assert!(rendered.contains("\"source\": \"pr_guidance\""));
        assert!(rendered.contains("\"state\": \"regressed\""));
        assert!(rendered.contains("\"suggested_test\": \"Add one focused discriminator test.\""));
        assert!(rendered.contains("\"related_test\": \"tests/pricing.rs::variant_boundary\""));
        assert!(rendered.contains("\"agent_command\": \"ripr agent start --root fixture-root --seam-id regressed-seam --out target/ripr/workflow\""));
        Ok(())
    }

    #[test]
    fn test_oracle_assistant_proof_handles_unknown_movement_and_no_handoff() -> Result<(), String> {
        let guidance = r#"{
          "comments": [
            {
              "seam_id": "unknown-seam",
              "kind": "predicate_boundary",
              "missing_discriminator": "unknown equality",
              "placement": {"path": "src/lib.rs", "line": 12, "mode": "owner_function_line"}
            }
          ]
        }"#;
        let before = repo_exposure("unknown-seam", "opaque", "src/lib.rs", 12);
        let after = repo_exposure("unknown-seam", "static_unknown", "src/lib.rs", 12);
        let report = build_test_oracle_assistant_proof_report(TestOracleAssistantProofInput {
            root: ".".to_string(),
            pr_guidance_path: Some("comments.json".to_string()),
            agent_packet_path: None,
            before_path: Some("before.json".to_string()),
            after_path: Some("after.json".to_string()),
            receipt_path: None,
            ledger_path: None,
            coverage_frontier_path: None,
            gate_decision_path: None,
            pr_guidance_json: Some(Ok(guidance.to_string())),
            agent_packet_json: None,
            before_json: Some(Ok(before)),
            after_json: Some(Ok(after)),
            receipt_json: None,
            ledger_json: None,
            coverage_frontier_json: None,
            gate_decision_json: None,
        });

        let rendered = render_test_oracle_assistant_proof_json(&report)?;
        assert!(rendered.contains("\"placement\": \"unknown\""));
        assert!(rendered.contains("\"state\": \"unknown\""));
        assert!(rendered.contains("\"artifact\": null"));
        assert!(rendered.contains("\"source\": \"not_available\""));
        Ok(())
    }

    #[test]
    fn test_oracle_assistant_proof_prefers_agent_packet_evidence_record() -> Result<(), String> {
        let agent_packet = r#"{
          "root": "fixture-root",
          "packets": [
            {
              "seam_id": "legacy-seam",
              "seam_kind": "legacy_kind",
              "file": "legacy.rs",
              "line": 1,
              "current_grip": "legacy_grip",
              "missing_discriminators": [{"value": "legacy missing"}],
              "evidence_record": {
                "schema_version": "0.1",
                "seam_id": "record-seam",
                "canonical_gap_id": "canonical-record",
                "owner": "pricing::discounted_total",
                "location": {"file": "src/pricing.rs", "line": 42},
                "seam_kind": "predicate_boundary",
                "grip_class": "weakly_gripped",
                "headline_eligible": true,
                "missing_discriminators": [
                  {"value": "record equality", "reason": "record reason"}
                ],
                "recommendation": {
                  "recommended_test": {
                    "file": "tests/pricing.rs",
                    "name": "discounted_total_equality_boundary",
                    "reason": "near related test"
                  },
                  "nearest_test_to_imitate": {
                    "file": "tests/pricing.rs",
                    "name": "above_threshold_discount"
                  },
                  "assertion_shape": {
                    "kind": "exact_return_value",
                    "example": "assert_eq!(discounted_total(100, 100), 90)"
                  },
                  "verify_command": "ripr agent verify --json",
                  "reason": "extend the nearest related test"
                },
                "static_limitations": [
                  {"stage": "propagate", "state": "unknown", "reason": "record limitation"}
                ]
              }
            }
          ]
        }"#;
        let report = build_test_oracle_assistant_proof_report(TestOracleAssistantProofInput {
            root: ".".to_string(),
            pr_guidance_path: None,
            agent_packet_path: Some("agent-packet.json".to_string()),
            before_path: None,
            after_path: None,
            receipt_path: None,
            ledger_path: None,
            coverage_frontier_path: None,
            gate_decision_path: None,
            pr_guidance_json: None,
            agent_packet_json: Some(Ok(agent_packet.to_string())),
            before_json: None,
            after_json: None,
            receipt_json: None,
            ledger_json: None,
            coverage_frontier_json: None,
            gate_decision_json: None,
        });

        let rendered = render_test_oracle_assistant_proof_json(&report)?;
        assert!(rendered.contains("\"seam_id\": \"record-seam\""));
        assert!(rendered.contains("\"canonical_gap_id\": \"canonical-record\""));
        assert!(rendered.contains("\"owner\": \"pricing::discounted_total\""));
        assert!(rendered.contains("\"path\": \"src/pricing.rs\""));
        assert!(rendered.contains("\"line\": 42"));
        assert!(rendered.contains("\"missing_discriminator\": \"record equality\""));
        assert!(rendered.contains("\"evidence_source\": \"evidence_record\""));
        assert!(rendered.contains("propagate unknown: record limitation"));
        assert!(rendered.contains("\"source\": \"evidence_record\""));
        assert!(rendered.contains("tests/pricing.rs::above_threshold_discount"));
        assert!(rendered.contains("assert_eq!(discounted_total(100, 100), 90)"));
        assert!(rendered.contains("\"verify_command\": \"ripr agent verify --json\""));
        assert!(!rendered.contains("legacy missing"));
        Ok(())
    }

    #[test]
    fn test_oracle_assistant_proof_prefers_repo_exposure_evidence_record_movement()
    -> Result<(), String> {
        let guidance = r#"{
          "comments": [
            {
              "seam_id": "record-seam",
              "kind": "legacy_kind",
              "missing_discriminator": "legacy missing",
              "placement": {"path": "legacy.rs", "line": 1, "mode": "changed_line"}
            }
          ]
        }"#;
        let before = repo_exposure_with_record("record-seam", "weakly_gripped", "strongly_gripped");
        let after = repo_exposure_with_record("record-seam", "strongly_gripped", "weakly_gripped");
        let report = build_test_oracle_assistant_proof_report(TestOracleAssistantProofInput {
            root: ".".to_string(),
            pr_guidance_path: Some("comments.json".to_string()),
            agent_packet_path: None,
            before_path: Some("before.json".to_string()),
            after_path: Some("after.json".to_string()),
            receipt_path: None,
            ledger_path: None,
            coverage_frontier_path: None,
            gate_decision_path: None,
            pr_guidance_json: Some(Ok(guidance.to_string())),
            agent_packet_json: None,
            before_json: Some(Ok(before)),
            after_json: Some(Ok(after)),
            receipt_json: None,
            ledger_json: None,
            coverage_frontier_json: None,
            gate_decision_json: None,
        });

        let rendered = render_test_oracle_assistant_proof_json(&report)?;
        assert!(rendered.contains("\"seam_kind\": \"predicate_boundary\""));
        assert!(rendered.contains("\"path\": \"src/record.rs\""));
        assert!(rendered.contains("\"missing_discriminator\": \"record missing\""));
        assert!(rendered.contains("\"before_class\": \"weakly_gripped\""));
        assert!(rendered.contains("\"after_class\": \"strongly_gripped\""));
        assert!(rendered.contains("\"state\": \"improved\""));
        assert!(rendered.contains("\"source\": \"evidence_record\""));
        assert!(!rendered.contains("\"before_class\": \"strongly_gripped\""));
        Ok(())
    }

    fn repo_root() -> Result<PathBuf, String> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "failed to resolve repo root".to_string())
    }

    fn read_file(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))
    }

    fn fixture_path(repo_root: &Path, path: &Path) -> String {
        match path.strip_prefix(repo_root) {
            Ok(path) => display_path(path),
            Err(_err) => display_path(path),
        }
    }

    fn repo_exposure(seam_id: &str, grip_class: &str, file: &str, line: u64) -> String {
        format!(
            r#"{{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {{
      "seam_id": "{seam_id}",
      "kind": "predicate_boundary",
      "file": "{file}",
      "line": {line},
      "grip_class": "{grip_class}",
      "missing_discriminators": [{{"value": "threshold equality"}}],
      "observed_values": ["1"]
    }}
  ]
}}"#
        )
    }

    fn repo_exposure_with_record(
        seam_id: &str,
        record_grip_class: &str,
        legacy_grip_class: &str,
    ) -> String {
        format!(
            r#"{{
  "schema_version": "0.3",
  "scope": "repo",
  "seams": [
    {{
      "seam_id": "legacy-seam",
      "kind": "legacy_kind",
      "file": "legacy.rs",
      "line": 1,
      "grip_class": "{legacy_grip_class}",
      "observed_values": ["legacy"],
      "evidence_record": {{
        "schema_version": "0.1",
        "seam_id": "{seam_id}",
        "owner": "record::owner",
        "location": {{"file": "src/record.rs", "line": 42}},
        "seam_kind": "predicate_boundary",
        "grip_class": "{record_grip_class}",
        "missing_discriminators": [
          {{"value": "record missing", "reason": "record reason"}}
        ],
        "observed_values": [
          {{"value": "100", "line": 42, "text": "record", "context": "function_argument"}}
        ],
        "recommendation": {{
          "assertion_shape": {{"kind": "exact_return_value", "example": "assert_eq!(record(), 1)"}}
        }},
        "static_limitations": []
      }}
    }}
  ]
}}"#
        )
    }
}
