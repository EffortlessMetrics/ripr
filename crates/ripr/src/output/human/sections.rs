use crate::config::RiprConfig;
use crate::domain::{Finding, LanguageId, LanguageStatus};
use crate::output::preview_actionability::{
    PreviewActionability, PreviewRawEvidenceRef, preview_actionability_for,
};
use crate::output::python_repair_card::{PythonRepairCard, python_repair_card};
use crate::output::typescript_preview_card::{TypeScriptPreviewCard, typescript_preview_card};

use super::evidence_lines::{evidence_path_lines, weakness_lines};

pub(crate) fn render_finding_with_config(finding: &Finding, config: &RiprConfig) -> String {
    let mut out = String::new();
    let severity = config.severity().for_exposure(&finding.class).as_str();
    out.push_str(&format!(
        "{} {}:{}\n",
        severity.to_ascii_uppercase(),
        finding.probe.location.file.display(),
        finding.probe.location.line
    ));

    out.push_str("\nChanged\n");
    if let Some(before) = &finding.probe.before {
        out.push_str(&format!("  before: {before}\n"));
    }
    if let Some(after) = &finding.probe.after {
        out.push_str(&format!("  after:  {after}\n"));
    } else {
        out.push_str(&format!("  expr:   {}\n", finding.probe.expression));
    }

    out.push_str("\nProbe\n");
    out.push_str(&format!(
        "  family: {}\n  delta:  {}\n",
        finding.probe.family.as_str(),
        finding.probe.delta.as_str()
    ));
    if let Some(owner) = &finding.probe.owner {
        out.push_str(&format!("  owner:  {owner}\n"));
    }
    if let Some(gap) = &finding.canonical_gap {
        out.push_str(&format!("  canonical gap: {}\n", gap.id));
    }

    if should_render_language_metadata(finding) {
        out.push_str("\nLanguage\n");
        if let Some(language) = finding.language {
            out.push_str(&format!("  language: {}\n", language.as_str()));
        }
        if let Some(status) = finding.language_status {
            out.push_str(&format!("  status: {}\n", status.as_str()));
        }
        if let Some(owner_kind) = finding.owner_kind {
            out.push_str(&format!("  owner kind: {}\n", owner_kind.as_str()));
        }
    }

    if let Some(actionability) = preview_actionability_for(finding) {
        push_preview_actionability(&mut out, &actionability);
    }

    out.push_str("\nStatic exposure\n");
    out.push_str(&format!(
        "  {} ({}, confidence {:.2})\n",
        finding.class.as_str(),
        severity,
        finding.confidence
    ));

    out.push_str("\nEvidence\n");
    for line in evidence_path_lines(finding) {
        out.push_str(&format!("  - {line}\n"));
    }

    let weakness = weakness_lines(finding);
    if !weakness.is_empty() {
        out.push_str("\nWeakness\n");
        for line in weakness {
            out.push_str(&format!("  - {line}\n"));
        }
    }

    let stop_reasons = finding.effective_stop_reasons();
    if !stop_reasons.is_empty() {
        out.push_str("\nStop reasons:\n");
        for reason in &stop_reasons {
            out.push_str(&format!("  - {}\n", reason.as_str()));
        }
    }

    if let Some(card) = python_repair_card(finding) {
        push_python_repair_card(&mut out, &card);
    } else if let Some(card) = typescript_preview_card(finding) {
        push_typescript_preview_card(&mut out, &card);
    } else if let Some(placement) = repair_placement_from_evidence(finding) {
        out.push_str("\nRepair placement\n");
        out.push_str(&format!("  suggested file: {}\n", placement.test_file));
        out.push_str(&format!("  suggested test: {}\n", placement.test_name));
        if let Some(node_id) = placement.test_node_id {
            out.push_str(&format!("  pytest node: {node_id}\n"));
        }
        out.push_str(&format!(
            "  verify: {} ({})\n",
            placement.verify_command, placement.verify_confidence
        ));
    }

    if let Some(step) = &finding.recommended_next_step {
        out.push_str("\nNext step\n");
        out.push_str(&format!("  {step}\n"));
    }

    out
}

fn push_preview_actionability(out: &mut String, actionability: &PreviewActionability) {
    out.push_str("\nPreview actionability\n");
    out.push_str(&format!(
        "  authority: {}\n",
        actionability.authority_boundary
    ));
    out.push_str(&format!("  gap state: {}\n", actionability.gap_state));
    out.push_str(&format!(
        "  category: {}\n",
        actionability.actionability_category
    ));
    out.push_str(&format!(
        "  repair packet ready: {}\n",
        actionability.repair_packet_ready
    ));
    out.push_str(&format!(
        "  why not actionable: {}\n",
        actionability.why_not_actionable
    ));
    out.push_str(&format!("  repair route: {}\n", actionability.repair_route));
    if !actionability.missing_actionability_fields.is_empty() {
        out.push_str(&format!(
            "  missing fields: {}\n",
            actionability.missing_actionability_fields.join(", ")
        ));
    }
    if !actionability.missing_graph_legs.is_empty() {
        out.push_str(&format!(
            "  missing graph legs: {}\n",
            actionability.missing_graph_legs.join(", ")
        ));
    }
    if let Some(unlock_condition) = &actionability.unlock_condition {
        out.push_str(&format!("  unlock condition: {unlock_condition}\n"));
    }
    out.push_str(&format!(
        "  evidence needed: {}\n",
        actionability.evidence_needed_to_promote
    ));
    for raw_ref in &actionability.raw_evidence_refs {
        out.push_str("  raw evidence: ");
        push_raw_ref(out, raw_ref);
        out.push('\n');
    }
}

fn push_raw_ref(out: &mut String, raw_ref: &PreviewRawEvidenceRef) {
    if let (Some(file), Some(line)) = (raw_ref.file.as_deref(), raw_ref.line) {
        if let Some(leg) = &raw_ref.leg {
            out.push_str(&format!("{leg} "));
        }
        out.push_str(&format!("{file}:{line}"));
        if let Some(kind) = &raw_ref.kind {
            out.push_str(&format!(" ({kind})"));
        }
        if let Some(source_id) = &raw_ref.source_id {
            out.push_str(&format!(" source={source_id}"));
        }
        if let Some(owner) = &raw_ref.owner {
            out.push_str(&format!(" owner={owner}"));
        }
        if let Some(sample) = &raw_ref.sample {
            out.push_str(&format!(" sample={sample}"));
        }
    } else {
        out.push_str(&raw_ref.raw);
    }
}

fn push_python_repair_card(out: &mut String, card: &PythonRepairCard) {
    out.push_str("\nPython repair card (preview/advisory)\n");
    out.push_str(&format!("  card version: {}\n", card.card_version));
    out.push_str(&format!("  canonical gap: {}\n", card.canonical_gap_id));
    out.push_str(&format!(
        "  authority: {} ({}/{})\n",
        card.authority_boundary, card.language, card.language_status
    ));
    out.push_str(&format!("  repair action: {}\n", card.repair_action));
    out.push_str(&format!("  changed owner: {}\n", card.changed_owner));
    out.push_str(&format!("  changed behavior: {}\n", card.changed_behavior));
    out.push_str(&format!(
        "  current test evidence: {}\n",
        card.current_test_evidence
    ));
    out.push_str(&format!(
        "  missing discriminator: {}\n",
        card.missing_discriminator
    ));
    out.push_str(&format!(
        "  recommended test shape: {}\n",
        card.recommended_test_shape
    ));
    out.push_str(&format!(
        "  suggested assertion: {}\n",
        card.suggested_assertion
    ));
    out.push_str(&format!("  suggested file: {}\n", card.suggested_test_file));
    out.push_str(&format!("  suggested test: {}\n", card.suggested_test_name));
    if let Some(node_id) = &card.suggested_test_node_id {
        out.push_str(&format!("  pytest node: {node_id}\n"));
    }
    out.push_str(&format!(
        "  verify: {} ({})\n",
        card.verify_command, card.verify_command_confidence
    ));
    if let Some(command) = &card.receipt_command {
        out.push_str(&format!("  receipt command: {command}\n"));
    } else {
        out.push_str(&format!("  receipt: {}\n", card.receipt_status));
    }
    out.push_str(&format!("  receipt guidance: {}\n", card.receipt_guidance));
    out.push_str("  stop conditions:\n");
    for condition in &card.stop_conditions {
        out.push_str(&format!("    - {condition}\n"));
    }
    out.push_str("  limits:\n");
    for limit in &card.limits {
        out.push_str(&format!("    - {limit}\n"));
    }
}

fn push_typescript_preview_card(out: &mut String, card: &TypeScriptPreviewCard) {
    out.push_str("\nTypeScript preview card (advisory)\n");
    out.push_str(&format!("  card version: {}\n", card.card_version));
    out.push_str(&format!(
        "  authority: {} ({}/{})\n",
        card.authority_boundary, card.language, card.language_status
    ));
    out.push_str(&format!("  owner: {}\n", card.owner));
    if let Some(owner_kind) = &card.owner_kind {
        out.push_str(&format!("  owner kind: {owner_kind}\n"));
    }
    out.push_str(&format!("  changed behavior: {}\n", card.changed_behavior));
    if let Some(test) = &card.related_test {
        out.push_str(&format!(
            "  related test: {}:{} {}\n",
            test.file, test.line, test.name
        ));
    }
    out.push_str(&format!(
        "  oracle: {} ({})\n",
        card.oracle_kind, card.oracle_strength
    ));
    if let Some(grip) = &card.bun_cross_language_grip {
        out.push_str("  Bun cross-language grip:\n");
        out.push_str(&format!("    state: {}\n", grip.state));
        out.push_str(&format!(
            "    Rust seam: {} owner={} boundary={}\n",
            grip.rust_file, grip.rust_owner, grip.rust_boundary
        ));
        out.push_str(&format!(
            "    TypeScript evidence: {} verdict={} confidence={}\n",
            grip.ts_test_file, grip.ts_verdict, grip.bridge_confidence
        ));
        if !grip.missing_discriminators.is_empty() {
            out.push_str(&format!(
                "    missing discriminators: {}\n",
                grip.missing_discriminators.join(", ")
            ));
        }
        if !grip.missing_graph_legs.is_empty() {
            out.push_str(&format!(
                "    missing graph legs: {}\n",
                grip.missing_graph_legs.join(", ")
            ));
        }
        if let Some(unlock_condition) = &grip.unlock_condition {
            out.push_str(&format!("    unlock condition: {unlock_condition}\n"));
        }
        out.push_str(&format!(
            "    limitation category: {}\n",
            grip.limitation_category
        ));
        out.push_str(&format!("    repair route: {}\n", grip.repair_route));
        out.push_str(&format!("    action: {}\n", grip.action));
        out.push_str(&format!(
            "    suggested test file: {}\n",
            grip.suggested_test_file
        ));
        if let Some(placement) = &grip.placement {
            out.push_str(&format!(
                "    placement: rank {} {}\n",
                placement.rank, placement.suggested_test_file
            ));
            out.push_str(&format!("    placement reason: {}\n", placement.reason));
        }
        out.push_str(&format!("    authority: {}\n", grip.authority_boundary));
        out.push_str(&format!(
            "    repair packet ready: {}\n",
            grip.repair_packet_ready
        ));
    }
    if let Some(discriminator) = &card.missing_discriminator {
        out.push_str(&format!("  missing discriminator: {discriminator}\n"));
    }
    out.push_str(&format!(
        "  suggested assertion shape: {}\n",
        card.suggested_assertion_shape
    ));
    if !card.static_limits.is_empty() {
        out.push_str(&format!(
            "  static limits: {}\n",
            card.static_limits.join(", ")
        ));
    }
    if let Some(command) = &card.verify_command {
        out.push_str(&format!("  verify: {command}\n"));
    }
    out.push_str(&format!(
        "  repair packet ready: {}\n",
        card.repair_packet_ready
    ));
    out.push_str(&format!(
        "  why not actionable: {}\n",
        card.why_not_actionable
    ));
    out.push_str(&format!("  repair route: {}\n", card.repair_route));
    out.push_str("  limits:\n");
    for limit in &card.limits {
        out.push_str(&format!("    - {limit}\n"));
    }
}

struct RepairPlacement<'a> {
    test_file: &'a str,
    test_name: &'a str,
    test_node_id: Option<&'a str>,
    verify_command: &'a str,
    verify_confidence: &'a str,
}

fn repair_placement_from_evidence(finding: &Finding) -> Option<RepairPlacement<'_>> {
    Some(RepairPlacement {
        test_file: evidence_value(finding, "suggested_test_file: ")?,
        test_name: evidence_value(finding, "suggested_test_name: ")?,
        test_node_id: evidence_value(finding, "suggested_test_node_id: "),
        verify_command: evidence_value(finding, "suggested_verify_command: ")?,
        verify_confidence: evidence_value(finding, "suggested_verify_command_confidence: ")?,
    })
}

fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix(prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn should_render_language_metadata(finding: &Finding) -> bool {
    finding
        .language
        .is_some_and(|language| language != LanguageId::Rust)
        || finding
            .language_status
            .is_some_and(|status| status != LanguageStatus::Stable)
        || finding.owner_kind.is_some()
}
