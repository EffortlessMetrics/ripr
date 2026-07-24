//! Editor fixture-corpus validators for `check-fixture-contracts`: the
//! editor_gap_cockpit, editor_first_run_usability, editor_first_pr_bridge,
//! editor_adoption_assurance, and editor_actionable_gap_queue corpora, with
//! their const case/file tables and shared gap-case helpers.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items referenced outside this module are `pub(crate)` and
//! re-exported from `main.rs` so existing call sites (`dispatch.rs`,
//! `dogfood.rs`, and `tests.rs`) compile unchanged.

use super::*;

const EDITOR_GAP_COCKPIT_FIXTURE_ROOT: &str = "fixtures/editor_gap_cockpit";
const EDITOR_GAP_COCKPIT_CASES: &[&str] = &[
    "rust_actionable",
    "typescript_preview_static_limit",
    "python_preview_static_limit",
    "disabled_language",
    "wrong_root",
    "stale_artifact",
    "no_actionable_gap",
];
const EDITOR_GAP_COCKPIT_EXPECTED_FILES: &[&str] = &[
    "lsp-diagnostics.json",
    "lsp-hover.md",
    "lsp-code-actions.json",
    "vscode-status.json",
    "gap-projection.json",
];

pub(crate) fn validate_editor_gap_cockpit_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new(EDITOR_GAP_COCKPIT_FIXTURE_ROOT);
    if !root.exists() {
        violations.push(format!(
            "editor gap cockpit fixture corpus is missing {}",
            normalize_path(root)
        ));
        return Ok(());
    }
    let spec = root.join("SPEC.md");
    if !spec.exists() {
        violations.push(format!(
            "editor gap cockpit fixture corpus is missing {}",
            normalize_path(&spec)
        ));
    } else {
        let spec_text = read_text_lossy(&spec)?;
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0047"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0047`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    for case in EDITOR_GAP_COCKPIT_CASES {
        validate_editor_gap_cockpit_fixture_case(root, case, violations)?;
    }
    Ok(())
}

pub(crate) fn validate_editor_gap_cockpit_fixture_case(
    root: &Path,
    case: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let expected = root.join(case).join("expected");
    for file in EDITOR_GAP_COCKPIT_EXPECTED_FILES {
        let path = expected.join(file);
        if !path.exists() {
            violations.push(format!(
                "editor gap cockpit case {case} is missing {}",
                normalize_path(&path)
            ));
        }
    }
    let projection_path = expected.join("gap-projection.json");
    if projection_path.exists() {
        let projection = read_json_value(&projection_path)?;
        if json_string_field(&projection, "schema_version").as_deref() != Some("0.1") {
            violations.push(format!(
                "{} schema_version must be 0.1",
                normalize_path(&projection_path)
            ));
        }
        if json_string_field(&projection, "case").as_deref() != Some(case) {
            violations.push(format!(
                "{} case must be {case}",
                normalize_path(&projection_path)
            ));
        }
    }
    let diagnostics_path = expected.join("lsp-diagnostics.json");
    let actions_path = expected.join("lsp-code-actions.json");
    if diagnostics_path.exists() && actions_path.exists() {
        let diagnostics = read_json_value(&diagnostics_path)?;
        let actions = read_json_value(&actions_path)?;
        validate_editor_gap_case_semantics(case, &diagnostics, &actions, violations);
    }
    let hover_path = expected.join("lsp-hover.md");
    if hover_path.exists() {
        let hover = read_text_lossy(&hover_path)?;
        validate_editor_gap_hover(case, &hover, violations);
    }
    Ok(())
}

fn validate_editor_gap_case_semantics(
    case: &str,
    diagnostics: &Value,
    actions: &Value,
    violations: &mut Vec<String>,
) {
    let diagnostics_array = diagnostics
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let action_titles = actions
        .get("actions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| json_str_field(item, "title").map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match case {
        "rust_actionable" => {
            require_gap_case_diagnostic(case, &diagnostics_array, "rust", "stable", violations);
            for title in [
                "Inspect gap: copy repair packet",
                "Write targeted test: open best related test",
                "Verify after test: copy verify command",
                "Review result: copy receipt command",
                "Refresh Analysis - Saved Workspace Check",
            ] {
                if !action_titles.iter().any(|value| value == title) {
                    violations.push(format!(
                        "editor gap cockpit case {case} is missing action `{title}`"
                    ));
                }
            }
        }
        "typescript_preview_static_limit" => {
            require_gap_case_diagnostic(
                case,
                &diagnostics_array,
                "typescript",
                "preview",
                violations,
            );
            require_action_title(
                case,
                &action_titles,
                "Inspect gap: copy static-limit note",
                violations,
            );
        }
        "python_preview_static_limit" => {
            require_gap_case_diagnostic(case, &diagnostics_array, "python", "preview", violations);
            require_action_title(
                case,
                &action_titles,
                "Inspect gap: copy static-limit note",
                violations,
            );
        }
        "disabled_language" | "wrong_root" | "stale_artifact" | "no_actionable_gap" => {
            if !diagnostics_array.is_empty() {
                violations.push(format!(
                    "editor gap cockpit case {case} must not project diagnostics"
                ));
            }
            if !(action_titles.len() == 1
                && action_titles[0] == "Refresh Analysis - Saved Workspace Check")
            {
                violations.push(format!(
                    "editor gap cockpit case {case} must fail closed to refresh-only actions"
                ));
            }
        }
        _ => {}
    }
}

fn require_gap_case_diagnostic(
    case: &str,
    diagnostics: &[Value],
    language: &str,
    language_status: &str,
    violations: &mut Vec<String>,
) {
    if diagnostics.len() != 1 {
        violations.push(format!(
            "editor gap cockpit case {case} must project exactly one diagnostic"
        ));
    }
    let Some(diagnostic) = diagnostics.first() else {
        return;
    };
    let data = diagnostic.get("data").unwrap_or(&Value::Null);
    if json_str_field(data, "language") != Some(language) {
        violations.push(format!(
            "editor gap cockpit case {case} diagnostic language must be {language}"
        ));
    }
    if json_str_field(data, "language_status") != Some(language_status) {
        violations.push(format!(
            "editor gap cockpit case {case} diagnostic language_status must be {language_status}"
        ));
    }
    if json_str_field(data, "canonical_gap_id").is_none() {
        violations.push(format!(
            "editor gap cockpit case {case} diagnostic must carry canonical_gap_id"
        ));
    }
}

fn require_action_title(
    case: &str,
    action_titles: &[String],
    title: &str,
    violations: &mut Vec<String>,
) {
    if !action_titles.iter().any(|value| value == title) {
        violations.push(format!(
            "editor gap cockpit case {case} is missing action `{title}`"
        ));
    }
}

fn validate_editor_gap_hover(case: &str, hover: &str, violations: &mut Vec<String>) {
    for heading in ["## Evidence boundary", "## Gap state", "## Limits"] {
        if !hover.contains(heading) {
            violations.push(format!(
                "editor gap cockpit case {case} hover is missing `{heading}`"
            ));
        }
    }
    if case.contains("preview") {
        let static_limit = hover.find("Static limit:");
        let action = hover.find("Suggested action:");
        if static_limit.is_none() || action.is_none() || static_limit > action {
            violations.push(format!(
                "editor gap cockpit case {case} hover must show static limits before action language"
            ));
        }
    }
}

const EDITOR_FIRST_RUN_USABILITY_FIXTURE_ROOT: &str = "fixtures/editor_first_run_usability";
pub(crate) const EDITOR_FIRST_RUN_USABILITY_CASES: &[&str] = &[
    "setup_ok",
    "server_missing",
    "config_missing",
    "language_disabled",
    "adapter_unavailable",
    "artifact_missing",
    "artifact_stale",
    "receipt_found",
    "receipt_gap_mismatch",
    "receipt_improved",
    "receipt_unchanged",
];
const EDITOR_FIRST_RUN_USABILITY_EXPECTED_FILES: &[&str] = &[
    "vscode-status.json",
    "setup-diagnosis.md",
    "lsp-code-actions.json",
    "receipt-status.json",
];

pub(crate) fn validate_editor_first_run_usability_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new(EDITOR_FIRST_RUN_USABILITY_FIXTURE_ROOT);
    if !root.exists() {
        violations.push(format!(
            "editor first-run usability fixture corpus is missing {}",
            normalize_path(root)
        ));
        return Ok(());
    }
    let spec = root.join("SPEC.md");
    if !spec.exists() {
        violations.push(format!(
            "editor first-run usability fixture corpus is missing {}",
            normalize_path(&spec)
        ));
    } else {
        let spec_text = read_text_lossy(&spec)?;
        for spec_id in ["RIPR-SPEC-0049", "RIPR-SPEC-0050"] {
            if !spec_text.lines().any(|line| line.contains(spec_id)) {
                violations.push(format!("{} is missing `{spec_id}`", normalize_path(&spec)));
            }
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    for case in EDITOR_FIRST_RUN_USABILITY_CASES {
        validate_editor_first_run_usability_case(root, case, violations)?;
    }
    Ok(())
}

fn validate_editor_first_run_usability_case(
    root: &Path,
    case: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let expected = root.join(case).join("expected");
    for file in EDITOR_FIRST_RUN_USABILITY_EXPECTED_FILES {
        let path = expected.join(file);
        if !path.exists() {
            violations.push(format!(
                "editor first-run usability case {case} is missing {}",
                normalize_path(&path)
            ));
        }
    }
    let status_path = expected.join("vscode-status.json");
    if status_path.exists() {
        let status = read_json_value(&status_path)?;
        validate_editor_first_run_status(case, &status, violations);
    }
    let actions_path = expected.join("lsp-code-actions.json");
    if actions_path.exists() {
        let actions = read_json_value(&actions_path)?;
        validate_editor_first_run_actions(case, &actions, violations);
    }
    let receipt_path = expected.join("receipt-status.json");
    if receipt_path.exists() {
        let receipt = read_json_value(&receipt_path)?;
        validate_editor_first_run_receipt(case, &receipt, violations);
    }
    let diagnosis_path = expected.join("setup-diagnosis.md");
    if diagnosis_path.exists() {
        let diagnosis = read_text_lossy(&diagnosis_path)?;
        for required in [
            "RIPR setup diagnosis",
            "Next safe action",
            "Limits",
            "no source edits",
        ] {
            if !diagnosis.contains(required) {
                violations.push(format!(
                    "editor first-run usability case {case} setup diagnosis is missing `{required}`"
                ));
            }
        }
    }
    Ok(())
}

fn validate_editor_first_run_status(case: &str, status: &Value, violations: &mut Vec<String>) {
    if json_string_field(status, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor first-run usability case {case} vscode-status schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_first_run_usability/{case}");
    if json_string_field(status, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor first-run usability case {case} vscode-status fixture must be {expected_fixture}"
        ));
    }
    if matches!(
        case,
        "setup_ok" | "receipt_found" | "receipt_improved" | "receipt_unchanged"
    ) && json_string_field(status, "next_safe_action").is_none()
    {
        violations.push(format!(
            "editor first-run usability case {case} must name a next_safe_action"
        ));
    }
    if matches!(
        case,
        "server_missing"
            | "language_disabled"
            | "adapter_unavailable"
            | "artifact_stale"
            | "receipt_gap_mismatch"
    ) && json_string_field(status, "projection").as_deref() != Some("fail_closed")
    {
        violations.push(format!(
            "editor first-run usability case {case} must fail closed"
        ));
    }
}

fn validate_editor_first_run_actions(case: &str, actions: &Value, violations: &mut Vec<String>) {
    if json_string_field(actions, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor first-run usability case {case} lsp-code-actions schema_version must be 0.1"
        ));
    }
    let items = actions
        .get("actions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let first_repair_actions = items
        .iter()
        .filter(|item| {
            json_string_field(item, "title").as_deref() == Some("Copy first repair packet")
                || json_string_field(item, "command").as_deref() == Some("ripr.copyContext")
        })
        .collect::<Vec<_>>();
    let has_first_repair_packet = !first_repair_actions.is_empty();
    if matches!(
        case,
        "setup_ok" | "receipt_found" | "receipt_improved" | "receipt_unchanged"
    ) && !has_first_repair_packet
    {
        violations.push(format!(
            "editor first-run usability case {case} must include Copy first repair packet"
        ));
    }
    if matches!(
        case,
        "server_missing"
            | "config_missing"
            | "language_disabled"
            | "adapter_unavailable"
            | "artifact_missing"
            | "artifact_stale"
            | "receipt_gap_mismatch"
    ) && has_first_repair_packet
    {
        violations.push(format!(
            "editor first-run usability case {case} must not expose first repair packet"
        ));
    }
    for action in first_repair_actions {
        validate_editor_first_run_repair_action(case, action, violations);
    }
}

fn validate_editor_first_run_repair_action(
    case: &str,
    action: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(action, "title").as_deref() != Some("Copy first repair packet") {
        violations.push(format!(
            "editor first-run usability case {case} first repair action must use the Copy first repair packet title"
        ));
    }
    if json_string_field(action, "command").as_deref() != Some("ripr.copyContext") {
        violations.push(format!(
            "editor first-run usability case {case} first repair action must use ripr.copyContext"
        ));
    }
    let packet = action
        .get("arguments")
        .and_then(Value::as_array)
        .and_then(|args| args.iter().find_map(|arg| json_string_field(arg, "packet")));
    let Some(packet) = packet else {
        violations.push(format!(
            "editor first-run usability case {case} first repair action must carry a packet argument"
        ));
        return;
    };
    for required in [
        "RIPR first repair packet",
        "Gap identity:",
        "Suggested action:",
        "Verify command:",
        "Receipt command:",
        "Limits and non-claims:",
    ] {
        if !packet.contains(required) {
            violations.push(format!(
                "editor first-run usability case {case} first repair packet is missing `{required}`"
            ));
        }
    }
}

fn validate_editor_first_run_receipt(case: &str, receipt: &Value, violations: &mut Vec<String>) {
    if json_string_field(receipt, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor first-run usability case {case} receipt-status schema_version must be 0.1"
        ));
    }
    let state = json_string_field(receipt, "receipt_state");
    let expected_state = match case {
        "receipt_found" => Some("receipt_found"),
        "receipt_gap_mismatch" => Some("receipt_gap_mismatch"),
        "receipt_improved" => Some("receipt_movement_improved"),
        "receipt_unchanged" => Some("receipt_movement_unchanged"),
        "artifact_stale" => Some("receipt_stale"),
        _ => None,
    };
    if let Some(expected_state) =
        expected_state.filter(|expected| state.as_deref() != Some(*expected))
    {
        violations.push(format!(
            "editor first-run usability {case} must record {expected_state}"
        ));
    }
    if json_bool_field(receipt, "runtime_adequacy_claim") != Some(false) {
        violations.push(format!(
            "editor first-run usability case {case} receipt-status must deny runtime_adequacy_claim"
        ));
    }
    if json_bool_field(receipt, "gate_eligibility_claim") != Some(false) {
        violations.push(format!(
            "editor first-run usability case {case} receipt-status must deny gate_eligibility_claim"
        ));
    }
}

const EDITOR_FIRST_PR_BRIDGE_FIXTURE_ROOT: &str = "fixtures/editor_first_pr_bridge";
const EDITOR_FIRST_PR_BRIDGE_CASES: &[&str] = &[
    "setup_ok",
    "packet_missing",
    "packet_found_repairable",
    "packet_no_action",
    "packet_stale",
    "packet_wrong_root",
    "packet_malformed",
    "receipt_improved_packet_ready",
    "receipt_unchanged_packet_ready",
];
const EDITOR_FIRST_PR_BRIDGE_EXPECTED_FILES: &[&str] = &[
    "vscode-status.json",
    "setup-diagnosis.md",
    "lsp-diagnostics.json",
    "lsp-code-actions.json",
    "first-pr-status.json",
];

pub(crate) fn validate_editor_first_pr_bridge_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new(EDITOR_FIRST_PR_BRIDGE_FIXTURE_ROOT);
    if !root.exists() {
        violations.push(format!(
            "editor first-pr bridge fixture corpus is missing {}",
            normalize_path(root)
        ));
        return Ok(());
    }
    let spec = root.join("SPEC.md");
    if !spec.exists() {
        violations.push(format!(
            "editor first-pr bridge fixture corpus is missing {}",
            normalize_path(&spec)
        ));
    } else {
        let spec_text = read_text_lossy(&spec)?;
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0052"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0052`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    for case in EDITOR_FIRST_PR_BRIDGE_CASES {
        validate_editor_first_pr_bridge_case(root, case, violations)?;
    }
    Ok(())
}

fn validate_editor_first_pr_bridge_case(
    root: &Path,
    case: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let expected = root.join(case).join("expected");
    for file in EDITOR_FIRST_PR_BRIDGE_EXPECTED_FILES {
        let path = expected.join(file);
        if !path.exists() {
            violations.push(format!(
                "editor first-pr bridge case {case} is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let status_path = expected.join("vscode-status.json");
    if status_path.exists() {
        let status = read_json_value(&status_path)?;
        validate_editor_first_pr_bridge_status(case, &status, violations);
    }
    let actions_path = expected.join("lsp-code-actions.json");
    if actions_path.exists() {
        let actions = read_json_value(&actions_path)?;
        validate_editor_first_pr_bridge_actions(case, &actions, violations);
    }
    let packet_path = expected.join("first-pr-status.json");
    if packet_path.exists() {
        let packet = read_json_value(&packet_path)?;
        validate_editor_first_pr_bridge_packet(case, &packet, violations);
    }
    let diagnostics_path = expected.join("lsp-diagnostics.json");
    if diagnostics_path.exists() {
        let diagnostics = read_json_value(&diagnostics_path)?;
        validate_editor_first_pr_bridge_diagnostics(case, &diagnostics, violations);
    }
    let diagnosis_path = expected.join("setup-diagnosis.md");
    if diagnosis_path.exists() {
        let diagnosis = read_text_lossy(&diagnosis_path)?;
        for required in [
            "RIPR setup diagnosis",
            "First PR packet",
            "Next safe action",
            "Limits",
            "no source edits",
        ] {
            if !diagnosis.contains(required) {
                violations.push(format!(
                    "editor first-pr bridge case {case} setup diagnosis is missing `{required}`"
                ));
            }
        }
    }
    Ok(())
}

fn validate_editor_first_pr_bridge_status(
    case: &str,
    status: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(status, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor first-pr bridge case {case} vscode-status schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_first_pr_bridge/{case}");
    if json_string_field(status, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor first-pr bridge case {case} vscode-status fixture must be {expected_fixture}"
        ));
    }
    if matches!(
        case,
        "packet_missing" | "packet_stale" | "packet_wrong_root" | "packet_malformed"
    ) && json_string_field(status, "projection").as_deref() != Some("fail_closed")
    {
        violations.push(format!(
            "editor first-pr bridge case {case} must fail closed"
        ));
    }
    if json_string_field(status, "next_safe_action").is_none() {
        violations.push(format!(
            "editor first-pr bridge case {case} must name a next_safe_action"
        ));
    }
}

fn validate_editor_first_pr_bridge_actions(
    case: &str,
    actions: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(actions, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor first-pr bridge case {case} lsp-code-actions schema_version must be 0.1"
        ));
    }
    let items = actions
        .get("actions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let commands = items
        .iter()
        .filter_map(|item| json_string_field(item, "command"))
        .collect::<BTreeSet<_>>();
    let has_open_or_copy = commands.contains("ripr.openFirstPrPacket")
        || commands.contains("ripr.copyFirstPrSummary")
        || commands.contains("ripr.copyFirstPrRepairPacket")
        || commands.contains("ripr.copyFirstPrVerifyCommand")
        || commands.contains("ripr.copyFirstPrReceiptCommand");
    let has_repair_scoped_actions = commands.contains("ripr.copyFirstPrRepairPacket")
        || commands.contains("ripr.copyFirstPrVerifyCommand")
        || commands.contains("ripr.copyFirstPrReceiptCommand");
    if matches!(
        case,
        "packet_missing" | "packet_stale" | "packet_wrong_root" | "packet_malformed"
    ) && has_open_or_copy
    {
        violations.push(format!(
            "editor first-pr bridge case {case} must suppress first-pr open/copy actions"
        ));
    }
    if matches!(
        case,
        "packet_found_repairable"
            | "receipt_improved_packet_ready"
            | "receipt_unchanged_packet_ready"
    ) && !has_repair_scoped_actions
    {
        violations.push(format!(
            "editor first-pr bridge case {case} must expose bounded repair, verify, and receipt actions"
        ));
    }
    if matches!(case, "packet_no_action" | "setup_ok") && has_repair_scoped_actions {
        violations.push(format!(
            "editor first-pr bridge case {case} must not expose diagnostic-scoped repair actions"
        ));
    }
    if !commands.contains("ripr.copyFirstPrRegenerationGuidance") {
        violations.push(format!(
            "editor first-pr bridge case {case} must include regeneration guidance"
        ));
    }
}

fn validate_editor_first_pr_bridge_packet(
    case: &str,
    packet: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(packet, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor first-pr bridge case {case} first-pr-status schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_first_pr_bridge/{case}");
    if json_string_field(packet, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor first-pr bridge case {case} first-pr-status fixture must be {expected_fixture}"
        ));
    }
    let expected_state = match case {
        "setup_ok" => "found",
        "packet_missing" => "missing",
        "packet_found_repairable" => "top_repairable_gap",
        "packet_no_action" => "no_action",
        "packet_stale" => "stale",
        "packet_wrong_root" => "wrong_root",
        "packet_malformed" => "malformed",
        "receipt_improved_packet_ready" | "receipt_unchanged_packet_ready" => "top_repairable_gap",
        _ => "unknown",
    };
    if json_string_field(packet, "packet_state").as_deref() != Some(expected_state) {
        violations.push(format!(
            "editor first-pr bridge case {case} first-pr-status packet_state must be {expected_state}"
        ));
    }
    let expected_receipt_movement = match case {
        "receipt_improved_packet_ready" => Some("improved"),
        "receipt_unchanged_packet_ready" => Some("unchanged"),
        _ => None,
    };
    if let Some(expected_movement) = expected_receipt_movement
        && json_string_field(packet, "receipt_movement").as_deref() != Some(expected_movement)
    {
        violations.push(format!(
            "editor first-pr bridge case {case} receipt_movement must be {expected_movement}"
        ));
    }
    if editor_first_pr_bridge_case_requires_first_screen_contract(case) {
        validate_editor_first_pr_bridge_first_screen_contract(case, packet, violations);
    }
    for field in [
        "runtime_adequacy_claim",
        "mutation_proof_claim",
        "policy_gate_claim",
        "pr_ready_claim",
    ] {
        if json_bool_field(packet, field) != Some(false) {
            violations.push(format!(
                "editor first-pr bridge case {case} first-pr-status must deny {field}"
            ));
        }
    }
}

pub(crate) fn editor_first_pr_bridge_case_requires_first_screen_contract(case: &str) -> bool {
    matches!(
        case,
        "packet_found_repairable"
            | "receipt_improved_packet_ready"
            | "receipt_unchanged_packet_ready"
    )
}

pub(crate) fn validate_editor_first_pr_bridge_first_screen_contract(
    case: &str,
    packet: &Value,
    violations: &mut Vec<String>,
) {
    for (field, expected) in [
        ("changed_behavior", FIRST_PR_BOUNDARY_CHANGED_BEHAVIOR),
        (
            "current_evidence_strength",
            FIRST_PR_BOUNDARY_CURRENT_EVIDENCE_STRENGTH,
        ),
        (
            "missing_discriminator",
            FIRST_PR_BOUNDARY_MISSING_DISCRIMINATOR,
        ),
        (
            "focused_proof_intent",
            FIRST_PR_BOUNDARY_FOCUSED_PROOF_INTENT,
        ),
        (
            "static_evidence_boundary",
            FIRST_PR_STATIC_EVIDENCE_BOUNDARY,
        ),
    ] {
        match json_string_field(packet, field) {
            Some(value) if value == expected => {}
            Some(value) => violations.push(format!(
                "editor first-pr bridge case {case} first-pr-status {field} must be {expected:?}, got {value:?}"
            )),
            None => violations.push(format!(
                "editor first-pr bridge case {case} first-pr-status must name {field}"
            )),
        }
    }
}

fn validate_editor_first_pr_bridge_diagnostics(
    case: &str,
    diagnostics: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(diagnostics, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor first-pr bridge case {case} lsp-diagnostics schema_version must be 0.1"
        ));
    }
    let items = diagnostics
        .get("diagnostics")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if matches!(
        case,
        "packet_found_repairable"
            | "receipt_improved_packet_ready"
            | "receipt_unchanged_packet_ready"
    ) && items.is_empty()
    {
        violations.push(format!(
            "editor first-pr bridge case {case} must include the matching diagnostic identity"
        ));
    }
}

const EDITOR_ADOPTION_ASSURANCE_FIXTURE_ROOT: &str = "fixtures/editor_adoption_assurance";
pub(crate) const EDITOR_ADOPTION_ASSURANCE_CASES: &[&str] = &[
    "setup_ok",
    "server_missing",
    "server_version_mismatch",
    "no_workspace",
    "multi_root",
    "wrong_root_artifact",
    "stale_receipt",
    "first_pr_packet_ready",
    "first_pr_packet_mismatch",
    "preview_adapter_unavailable",
];
const EDITOR_ADOPTION_ASSURANCE_EXPECTED_FILES: &[&str] = &[
    "vscode-status.json",
    "setup-diagnosis.md",
    "lsp-diagnostics.json",
    "lsp-code-actions.json",
    "first-pr-status.json",
    "receipt-status.json",
];

pub(crate) fn validate_editor_adoption_assurance_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new(EDITOR_ADOPTION_ASSURANCE_FIXTURE_ROOT);
    if !root.exists() {
        violations.push(format!(
            "editor adoption assurance fixture corpus is missing {}",
            normalize_path(root)
        ));
        return Ok(());
    }
    let spec = root.join("SPEC.md");
    if !spec.exists() {
        violations.push(format!(
            "editor adoption assurance fixture corpus is missing {}",
            normalize_path(&spec)
        ));
    } else {
        let spec_text = read_text_lossy(&spec)?;
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0054"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0054`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    for case in EDITOR_ADOPTION_ASSURANCE_CASES {
        validate_editor_adoption_assurance_case(root, case, violations)?;
    }
    Ok(())
}

fn validate_editor_adoption_assurance_case(
    root: &Path,
    case: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let expected = root.join(case).join("expected");
    for file in EDITOR_ADOPTION_ASSURANCE_EXPECTED_FILES {
        let path = expected.join(file);
        if !path.exists() {
            violations.push(format!(
                "editor adoption assurance case {case} is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let status_path = expected.join("vscode-status.json");
    if status_path.exists() {
        let status = read_json_value(&status_path)?;
        validate_editor_adoption_assurance_status(case, &status, violations);
    }
    let actions_path = expected.join("lsp-code-actions.json");
    if actions_path.exists() {
        let actions = read_json_value(&actions_path)?;
        validate_editor_adoption_assurance_actions(case, &actions, violations);
    }
    let first_pr_path = expected.join("first-pr-status.json");
    if first_pr_path.exists() {
        let first_pr = read_json_value(&first_pr_path)?;
        validate_editor_adoption_assurance_first_pr(case, &first_pr, violations);
    }
    let receipt_path = expected.join("receipt-status.json");
    if receipt_path.exists() {
        let receipt = read_json_value(&receipt_path)?;
        validate_editor_adoption_assurance_receipt(case, &receipt, violations);
    }
    let diagnostics_path = expected.join("lsp-diagnostics.json");
    if diagnostics_path.exists() {
        let diagnostics = read_json_value(&diagnostics_path)?;
        validate_editor_adoption_assurance_diagnostics(case, &diagnostics, violations);
    }
    let diagnosis_path = expected.join("setup-diagnosis.md");
    if diagnosis_path.exists() {
        let diagnosis = read_text_lossy(&diagnosis_path)?;
        for required in [
            "RIPR setup diagnosis",
            "Compatibility",
            "Workspace",
            "First PR packet",
            "Receipt",
            "Next safe action",
            "Limits",
            "no source edits",
            "not a gate decision",
            "not runtime proof",
        ] {
            if !diagnosis.contains(required) {
                violations.push(format!(
                    "editor adoption assurance case {case} setup diagnosis is missing `{required}`"
                ));
            }
        }
    }
    Ok(())
}

fn validate_editor_adoption_assurance_status(
    case: &str,
    status: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(status, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor adoption assurance case {case} vscode-status schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_adoption_assurance/{case}");
    if json_string_field(status, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor adoption assurance case {case} vscode-status fixture must be {expected_fixture}"
        ));
    }
    let expected_status = match case {
        "setup_ok" => "setup_ok",
        "server_missing" => "server_missing",
        "server_version_mismatch" => "server_version_mismatch",
        "no_workspace" => "no_workspace",
        "multi_root" => "multi_root_ambiguous",
        "wrong_root_artifact" => "wrong_root_artifact",
        "stale_receipt" => "receipt_stale",
        "first_pr_packet_ready" => "first_pr_packet_ready",
        "first_pr_packet_mismatch" => "first_pr_packet_mismatch",
        "preview_adapter_unavailable" => "preview_adapter_unavailable",
        _ => "unknown",
    };
    if json_string_field(status, "status").as_deref() != Some(expected_status) {
        violations.push(format!(
            "editor adoption assurance case {case} status must be {expected_status}"
        ));
    }
    if editor_adoption_assurance_case_fails_closed(case)
        && json_string_field(status, "projection").as_deref() != Some("fail_closed")
    {
        violations.push(format!(
            "editor adoption assurance case {case} must fail closed"
        ));
    }
    if json_string_field(status, "next_safe_action").is_none() {
        violations.push(format!(
            "editor adoption assurance case {case} must name a next_safe_action"
        ));
    }
}

fn validate_editor_adoption_assurance_actions(
    case: &str,
    actions: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(actions, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor adoption assurance case {case} lsp-code-actions schema_version must be 0.1"
        ));
    }
    let items = actions
        .get("actions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let commands = items
        .iter()
        .filter_map(|item| json_string_field(item, "command"))
        .collect::<BTreeSet<_>>();
    let has_repair_or_proof_action = commands.contains("ripr.openRelatedTest")
        || commands.contains("ripr.copyContext")
        || commands.contains("ripr.copyFirstPrRepairPacket")
        || commands.contains("ripr.copyFirstPrVerifyCommand")
        || commands.contains("ripr.copyFirstPrReceiptCommand")
        || commands.contains("ripr.openFirstPrPacket");
    if editor_adoption_assurance_case_fails_closed(case) && has_repair_or_proof_action {
        violations.push(format!(
            "editor adoption assurance case {case} must suppress repair/proof actions"
        ));
    }
    if case == "first_pr_packet_ready" {
        for command in [
            "ripr.openFirstPrPacket",
            "ripr.copyFirstPrRepairPacket",
            "ripr.copyFirstPrVerifyCommand",
            "ripr.copyFirstPrReceiptCommand",
            "ripr.refresh",
        ] {
            if !commands.contains(command) {
                violations.push(format!(
                    "editor adoption assurance case {case} must include {command}"
                ));
            }
        }
    }
    if !commands.contains("ripr.refresh") {
        violations.push(format!(
            "editor adoption assurance case {case} must include refresh guidance"
        ));
    }
}

fn validate_editor_adoption_assurance_first_pr(
    case: &str,
    first_pr: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(first_pr, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor adoption assurance case {case} first-pr-status schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_adoption_assurance/{case}");
    if json_string_field(first_pr, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor adoption assurance case {case} first-pr-status fixture must be {expected_fixture}"
        ));
    }
    let expected_state = match case {
        "setup_ok" => "missing",
        "server_missing"
        | "server_version_mismatch"
        | "no_workspace"
        | "multi_root"
        | "stale_receipt"
        | "preview_adapter_unavailable" => "not_projected",
        "wrong_root_artifact" => "wrong_root",
        "first_pr_packet_ready" => "top_repairable_gap",
        "first_pr_packet_mismatch" => "gap_mismatch",
        _ => "unknown",
    };
    if json_string_field(first_pr, "packet_state").as_deref() != Some(expected_state) {
        violations.push(format!(
            "editor adoption assurance case {case} first-pr-status packet_state must be {expected_state}"
        ));
    }
    for field in [
        "runtime_adequacy_claim",
        "mutation_proof_claim",
        "policy_gate_claim",
        "pr_ready_claim",
    ] {
        if json_bool_field(first_pr, field) != Some(false) {
            violations.push(format!(
                "editor adoption assurance case {case} first-pr-status must deny {field}"
            ));
        }
    }
}

fn validate_editor_adoption_assurance_receipt(
    case: &str,
    receipt: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(receipt, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor adoption assurance case {case} receipt-status schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_adoption_assurance/{case}");
    if json_string_field(receipt, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor adoption assurance case {case} receipt-status fixture must be {expected_fixture}"
        ));
    }
    let expected_state = match case {
        "setup_ok" | "first_pr_packet_ready" => "receipt_missing",
        "server_missing"
        | "server_version_mismatch"
        | "no_workspace"
        | "multi_root"
        | "preview_adapter_unavailable" => RECEIPT_NOT_APPLICABLE,
        "wrong_root_artifact" => "receipt_wrong_root",
        "stale_receipt" => "receipt_stale",
        "first_pr_packet_mismatch" => "receipt_gap_mismatch",
        _ => "unknown",
    };
    if json_string_field(receipt, "receipt_state").as_deref() != Some(expected_state) {
        violations.push(format!(
            "editor adoption assurance case {case} receipt-status receipt_state must be {expected_state}"
        ));
    }
    for field in [
        "runtime_adequacy_claim",
        "mutation_proof_claim",
        "gate_eligibility_claim",
        "merge_readiness_claim",
    ] {
        if json_bool_field(receipt, field) != Some(false) {
            violations.push(format!(
                "editor adoption assurance case {case} receipt-status must deny {field}"
            ));
        }
    }
}

fn validate_editor_adoption_assurance_diagnostics(
    case: &str,
    diagnostics: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(diagnostics, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor adoption assurance case {case} lsp-diagnostics schema_version must be 0.1"
        ));
    }
    let items = diagnostics
        .get("diagnostics")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if case == "first_pr_packet_ready" && items.is_empty() {
        violations.push(
            "editor adoption assurance first_pr_packet_ready must include a diagnostic identity"
                .to_string(),
        );
    }
    if editor_adoption_assurance_case_fails_closed(case) && !items.is_empty() {
        violations.push(format!(
            "editor adoption assurance case {case} must not project diagnostics from unsafe state"
        ));
    }
}

pub(crate) fn editor_adoption_assurance_case_fails_closed(case: &str) -> bool {
    matches!(
        case,
        "server_missing"
            | "server_version_mismatch"
            | "no_workspace"
            | "multi_root"
            | "wrong_root_artifact"
            | "stale_receipt"
            | "first_pr_packet_mismatch"
            | "preview_adapter_unavailable"
    )
}

const EDITOR_ACTIONABLE_GAP_QUEUE_FIXTURE_ROOT: &str = "fixtures/editor_actionable_gap_queue";
const EDITOR_ACTIONABLE_GAP_QUEUE_CASES: &[&str] = &[
    "setup_ok",
    "top_gap_ready",
    "multiple_gaps_bounded",
    "no_actionable_gap",
    "report_only_static_limit",
    "stale_actionable_packet",
    "wrong_root_packet",
    "malformed_packet",
    "receipt_improved",
    "receipt_unchanged",
];
const EDITOR_ACTIONABLE_GAP_QUEUE_EXPECTED_FILES: &[&str] = &[
    "vscode-status.json",
    "lsp-code-actions.json",
    "current-repair-packet.md",
    "repo-gap-map.md",
    "receipt-status.json",
];

pub(crate) fn validate_editor_actionable_gap_queue_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new(EDITOR_ACTIONABLE_GAP_QUEUE_FIXTURE_ROOT);
    if !root.exists() {
        violations.push(format!(
            "editor actionable gap queue fixture corpus is missing {}",
            normalize_path(root)
        ));
        return Ok(());
    }
    let spec = root.join("SPEC.md");
    if !spec.exists() {
        violations.push(format!(
            "editor actionable gap queue fixture corpus is missing {}",
            normalize_path(&spec)
        ));
    } else {
        let spec_text = read_text_lossy(&spec)?;
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0055"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0055`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    for case in EDITOR_ACTIONABLE_GAP_QUEUE_CASES {
        validate_editor_actionable_gap_queue_case(root, case, violations)?;
    }
    Ok(())
}

fn validate_editor_actionable_gap_queue_case(
    root: &Path,
    case: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let expected = root.join(case).join("expected");
    for file in EDITOR_ACTIONABLE_GAP_QUEUE_EXPECTED_FILES {
        let path = expected.join(file);
        if !path.exists() {
            violations.push(format!(
                "editor actionable gap queue case {case} is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let status_path = expected.join("vscode-status.json");
    if status_path.exists() {
        let status = read_json_value(&status_path)?;
        validate_editor_actionable_gap_queue_status(case, &status, violations);
    }
    let actions_path = expected.join("lsp-code-actions.json");
    if actions_path.exists() {
        let actions = read_json_value(&actions_path)?;
        validate_editor_actionable_gap_queue_actions(case, &actions, violations);
    }
    let receipt_path = expected.join("receipt-status.json");
    if receipt_path.exists() {
        let receipt = read_json_value(&receipt_path)?;
        validate_editor_actionable_gap_queue_receipt(case, &receipt, violations);
    }
    let repair_packet_path = expected.join("current-repair-packet.md");
    if repair_packet_path.exists() {
        let packet = read_text_lossy(&repair_packet_path)?;
        validate_editor_actionable_gap_queue_repair_packet(case, &packet, violations);
    }
    let repo_map_path = expected.join("repo-gap-map.md");
    if repo_map_path.exists() {
        let map = read_text_lossy(&repo_map_path)?;
        validate_editor_actionable_gap_queue_repo_map(case, &map, violations);
    }
    Ok(())
}

fn validate_editor_actionable_gap_queue_status(
    case: &str,
    status: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(status, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor actionable gap queue case {case} vscode-status schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_actionable_gap_queue/{case}");
    if json_string_field(status, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor actionable gap queue case {case} vscode-status fixture must be {expected_fixture}"
        ));
    }
    let expected_status = match case {
        "setup_ok" => "queue_available",
        "top_gap_ready" | "multiple_gaps_bounded" | "receipt_improved" | "receipt_unchanged" => {
            "top_actionable_gap"
        }
        "no_actionable_gap" => "no_action",
        "report_only_static_limit" => "static_limit_only",
        "stale_actionable_packet" => "stale",
        "wrong_root_packet" => "wrong_root",
        "malformed_packet" => "malformed",
        _ => "unknown",
    };
    if json_string_field(status, "queue_status").as_deref() != Some(expected_status) {
        violations.push(format!(
            "editor actionable gap queue case {case} queue_status must be {expected_status}"
        ));
    }
    if editor_actionable_gap_queue_case_fails_closed(case)
        && json_string_field(status, "projection").as_deref() != Some("fail_closed")
    {
        violations.push(format!(
            "editor actionable gap queue case {case} must fail closed"
        ));
    }
    if json_string_field(status, "next_safe_action").is_none() {
        violations.push(format!(
            "editor actionable gap queue case {case} must name a next_safe_action"
        ));
    }
    for field in [
        "runtime_adequacy_claim",
        "mutation_proof_claim",
        "policy_gate_claim",
        "merge_readiness_claim",
    ] {
        if json_bool_field(status, field) != Some(false) {
            violations.push(format!(
                "editor actionable gap queue case {case} vscode-status must deny {field}"
            ));
        }
    }
    if editor_actionable_gap_queue_case_has_repair(case)
        && json_string_field(
            status.get("top_gap").unwrap_or(&Value::Null),
            "canonical_gap_id",
        )
        .is_none()
    {
        violations.push(format!(
            "editor actionable gap queue case {case} must name top_gap.canonical_gap_id"
        ));
    }
    if case == "multiple_gaps_bounded"
        && status
            .get("actionable_gaps")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            < 2
    {
        violations.push(
            "editor actionable gap queue case multiple_gaps_bounded must include multiple actionable gaps"
                .to_string(),
        );
    }
}

fn validate_editor_actionable_gap_queue_actions(
    case: &str,
    actions: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(actions, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor actionable gap queue case {case} lsp-code-actions schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_actionable_gap_queue/{case}");
    if json_string_field(actions, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor actionable gap queue case {case} lsp-code-actions fixture must be {expected_fixture}"
        ));
    }
    let items = actions
        .get("actions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let commands = items
        .iter()
        .filter_map(|item| json_string_field(item, "command"))
        .collect::<BTreeSet<_>>();
    let allowed_commands = editor_actionable_gap_queue_allowed_commands(case);
    for command in &commands {
        if !allowed_commands.contains(command.as_str()) {
            violations.push(format!(
                "editor actionable gap queue case {case} includes unexpected command {command}"
            ));
        }
    }
    if !commands.contains("ripr.refresh") {
        violations.push(format!(
            "editor actionable gap queue case {case} must include refresh guidance"
        ));
    }
    let repair_or_navigation = commands.contains("ripr.copyCurrentRepairPacket")
        || commands.contains("ripr.openRelatedTest");
    if editor_actionable_gap_queue_case_has_repair(case) {
        for command in [
            "ripr.copyCurrentRepairPacket",
            "ripr.copyRepoGapMap",
            "ripr.openRelatedTest",
            "ripr.refresh",
        ] {
            if !commands.contains(command) {
                violations.push(format!(
                    "editor actionable gap queue case {case} must include {command}"
                ));
            }
        }
    } else {
        if repair_or_navigation {
            violations.push(format!(
                "editor actionable gap queue case {case} must suppress repair/navigation actions"
            ));
        }
        if editor_actionable_gap_queue_case_allows_repo_map(case)
            && !commands.contains("ripr.copyRepoGapMap")
        {
            violations.push(format!(
                "editor actionable gap queue case {case} must include ripr.copyRepoGapMap"
            ));
        }
        if editor_actionable_gap_queue_case_fails_closed(case)
            && commands.contains("ripr.copyRepoGapMap")
        {
            violations.push(format!(
                "editor actionable gap queue case {case} must suppress repo gap map actions"
            ));
        }
    }
}

fn validate_editor_actionable_gap_queue_receipt(
    case: &str,
    receipt: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(receipt, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "editor actionable gap queue case {case} receipt-status schema_version must be 0.1"
        ));
    }
    let expected_fixture = format!("editor_actionable_gap_queue/{case}");
    if json_string_field(receipt, "fixture").as_deref() != Some(expected_fixture.as_str()) {
        violations.push(format!(
            "editor actionable gap queue case {case} receipt-status fixture must be {expected_fixture}"
        ));
    }
    let expected_state = match case {
        "receipt_improved" => RECEIPT_MOVEMENT_IMPROVED,
        "receipt_unchanged" => RECEIPT_MOVEMENT_UNCHANGED,
        "stale_actionable_packet" | "wrong_root_packet" | "malformed_packet" => {
            RECEIPT_NOT_APPLICABLE
        }
        _ => RECEIPT_MISSING,
    };
    if json_string_field(receipt, "receipt_state").as_deref() != Some(expected_state) {
        violations.push(format!(
            "editor actionable gap queue case {case} receipt_state must be {expected_state}"
        ));
    }
    let expected_movement = match case {
        "receipt_improved" => "improved",
        "receipt_unchanged" => "unchanged",
        _ => "not_available",
    };
    if json_string_field(receipt, "movement").as_deref() != Some(expected_movement) {
        violations.push(format!(
            "editor actionable gap queue case {case} movement must be {expected_movement}"
        ));
    }
    for field in [
        "runtime_adequacy_claim",
        "mutation_proof_claim",
        "policy_gate_claim",
    ] {
        if json_bool_field(receipt, field) != Some(false) {
            violations.push(format!(
                "editor actionable gap queue case {case} receipt-status must deny {field}"
            ));
        }
    }
}

fn validate_editor_actionable_gap_queue_repair_packet(
    case: &str,
    packet: &str,
    violations: &mut Vec<String>,
) {
    if editor_actionable_gap_queue_case_has_repair(case) {
        for heading in [
            "Task",
            "Context",
            "Repair",
            "Verification",
            "Receipt",
            "Stop conditions",
            "Do not do",
        ] {
            if !packet.contains(heading) {
                violations.push(format!(
                    "editor actionable gap queue case {case} current repair packet is missing `{heading}`"
                ));
            }
        }
        if !packet.contains("canonical gap") {
            violations.push(format!(
                "editor actionable gap queue case {case} current repair packet must name the canonical gap"
            ));
        }
    } else if !packet.contains("Current repair packet suppressed") {
        violations.push(format!(
            "editor actionable gap queue case {case} must suppress the current repair packet"
        ));
    }
    for forbidden in [
        "Ready to merge",
        "Gate passed",
        "runtime adequate",
        "Mutation proof: yes",
    ] {
        if packet.contains(forbidden) {
            violations.push(format!(
                "editor actionable gap queue case {case} current repair packet must not claim `{forbidden}`"
            ));
        }
    }
}

fn validate_editor_actionable_gap_queue_repo_map(
    case: &str,
    map: &str,
    violations: &mut Vec<String>,
) {
    if editor_actionable_gap_queue_case_allows_repo_map(case) {
        for required in [
            "RIPR repo gap map",
            "Scope",
            "Safe next commands",
            "Non-claims",
            "not a gate decision",
        ] {
            if !map.contains(required) {
                violations.push(format!(
                    "editor actionable gap queue case {case} repo gap map is missing `{required}`"
                ));
            }
        }
    } else if !map.contains("Repo gap map suppressed") {
        violations.push(format!(
            "editor actionable gap queue case {case} must suppress the repo gap map"
        ));
    }
    for forbidden in [
        "Ready to merge",
        "Gate passed",
        "runtime adequate",
        "Mutation proof: yes",
    ] {
        if map.contains(forbidden) {
            violations.push(format!(
                "editor actionable gap queue case {case} repo gap map must not claim `{forbidden}`"
            ));
        }
    }
}

fn editor_actionable_gap_queue_case_has_repair(case: &str) -> bool {
    matches!(
        case,
        "top_gap_ready" | "multiple_gaps_bounded" | "receipt_improved" | "receipt_unchanged"
    )
}

fn editor_actionable_gap_queue_case_fails_closed(case: &str) -> bool {
    matches!(
        case,
        "stale_actionable_packet" | "wrong_root_packet" | "malformed_packet"
    )
}

fn editor_actionable_gap_queue_case_allows_repo_map(case: &str) -> bool {
    !editor_actionable_gap_queue_case_fails_closed(case)
}

fn editor_actionable_gap_queue_allowed_commands(case: &str) -> BTreeSet<&'static str> {
    let commands: &[&str] = if editor_actionable_gap_queue_case_has_repair(case) {
        &[
            "ripr.copyCurrentRepairPacket",
            "ripr.copyRepoGapMap",
            "ripr.openRelatedTest",
            "ripr.refresh",
        ]
    } else if editor_actionable_gap_queue_case_allows_repo_map(case) {
        &["ripr.copyRepoGapMap", "ripr.refresh"]
    } else {
        &["ripr.refresh"]
    };
    commands.iter().copied().collect()
}
