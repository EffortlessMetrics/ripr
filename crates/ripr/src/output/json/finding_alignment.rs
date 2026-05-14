use crate::domain::{Finding, OracleKind, OracleStrength, RelatedTest};

use super::{array_field, escape, field, number_field};

const PRESENTATION_TEXT_CLASS: &str = "presentation_text";
const GROUP_REASON_DECL_LITERAL: &str = "declaration_and_literal_same_text_constant";
const GROUP_REASON_OWNER: &str = "constant_owner_identity";
const VISIBILITY_UNKNOWN_CATEGORY: &str = "presentation_text_visibility_unknown";
const VISIBILITY_UNKNOWN_REPAIR_ROUTE: &str = "trace_string_constant_to_output_or_snapshot_test";

pub(super) struct FindingAlignmentReport {
    summary: FindingAlignmentSummary,
    items: Vec<FindingAlignmentItem>,
}

struct FindingAlignmentSummary {
    raw_signals: usize,
    canonical_items: usize,
    aligned_raw_findings: usize,
    unaligned_raw_findings: usize,
    duplicate_groups_total: usize,
    actionable_gaps: usize,
    already_observed: usize,
    internal_no_action: usize,
    static_limitations: usize,
    unknown: usize,
    presentation_text_total: usize,
    presentation_text_visibility_unknown: usize,
    presentation_text_duplicate_groups: usize,
    presentation_text_actionable_output_repairs: usize,
}

struct FindingAlignmentItem {
    canonical_gap_id: String,
    canonical_item_kind: String,
    evidence_class: String,
    gap_state: String,
    actionability: String,
    raw_group_size: usize,
    group_reason: String,
    why: String,
    recommended_repair: String,
    related_test: Option<FindingAlignmentRelatedTest>,
    verify_command: String,
    static_limitations: Vec<FindingAlignmentStaticLimitation>,
    confidence: FindingAlignmentConfidence,
    raw_findings: Vec<FindingAlignmentRawFinding>,
    presentation_text: FindingAlignmentPresentationText,
}

struct FindingAlignmentRawFinding {
    file: String,
    line: usize,
    kind: String,
    expression: String,
    probe_kind: String,
    source_id: String,
    evidence_record_ref: String,
}

struct FindingAlignmentStaticLimitation {
    category: String,
    repair_route: String,
    user_actionability: String,
}

struct FindingAlignmentConfidence {
    basis: String,
    notes: Vec<String>,
}

struct FindingAlignmentRelatedTest {
    name: String,
    file: String,
    line: usize,
}

struct FindingAlignmentPresentationText {
    constant_name: String,
    text_literal: Option<String>,
    visibility: String,
    observer: String,
    actionability: String,
    source_kind: String,
    canonical_group_reason: String,
    recommended_observer: String,
}

struct PresentationTextClassification {
    canonical_item_kind: String,
    gap_state: String,
    actionability: String,
    why: String,
    recommended_repair: String,
    related_test: Option<FindingAlignmentRelatedTest>,
    static_limitations: Vec<FindingAlignmentStaticLimitation>,
    confidence: FindingAlignmentConfidence,
    visibility: String,
    observer: String,
    presentation_actionability: String,
    recommended_observer: String,
}

struct PresentationTextSink {
    recommended_observer: &'static str,
    repair_target: &'static str,
    description: &'static str,
}

#[derive(Clone)]
struct PresentationTextDeclaration {
    constant_name: String,
    inline_literal: Option<String>,
}

pub(super) fn report_for_findings(findings: &[Finding]) -> Option<FindingAlignmentReport> {
    let mut used = vec![false; findings.len()];
    let mut items = Vec::new();

    for (index, finding) in findings.iter().enumerate() {
        if used[index] {
            continue;
        }

        let Some(declaration) = parse_presentation_text_declaration(&finding.probe.expression)
        else {
            continue;
        };

        let mut raw_indices = vec![index];
        let literal = declaration.inline_literal.clone().or_else(|| {
            adjacent_literal_index(findings, &used, index).map(|literal_index| {
                raw_indices.push(literal_index);
                parse_string_literal(&findings[literal_index].probe.expression).unwrap_or_default()
            })
        });

        used[index] = true;
        for raw_index in raw_indices.iter().skip(1) {
            used[*raw_index] = true;
        }

        let source_findings = raw_indices
            .iter()
            .map(|raw_index| &findings[*raw_index])
            .collect::<Vec<_>>();
        let classification =
            classify_presentation_text(&declaration.constant_name, &source_findings);
        let raw_findings = raw_indices
            .iter()
            .map(|raw_index| raw_finding_for(&findings[*raw_index]))
            .collect::<Vec<_>>();
        items.push(presentation_text_item(
            &declaration.constant_name,
            literal,
            raw_findings,
            classification,
        ));
    }

    if items.is_empty() {
        return None;
    }

    let summary = summary_for(findings.len(), &items);
    Some(FindingAlignmentReport { summary, items })
}

pub(super) fn report_json(out: &mut String, report: &FindingAlignmentReport, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    field(out, indent + 1, "scope", "supported_classes", true);
    array_field(
        out,
        indent + 1,
        "supported_evidence_classes",
        &[PRESENTATION_TEXT_CLASS.to_string()],
        true,
    );
    out.push_str(&format!("{}\"summary\": ", "  ".repeat(indent + 1)));
    summary_json(out, &report.summary);
    out.push_str(",\n");
    out.push_str(&format!("{}\"items\": [\n", "  ".repeat(indent + 1)));
    for (index, item) in report.items.iter().enumerate() {
        item_json(out, item, indent + 2);
        if index + 1 != report.items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]\n", "  ".repeat(indent + 1)));
    out.push_str(&format!("{sp}}}"));
}

fn summary_for(raw_signals: usize, items: &[FindingAlignmentItem]) -> FindingAlignmentSummary {
    let aligned_raw_findings = items
        .iter()
        .map(|item| item.raw_findings.len())
        .sum::<usize>();
    let duplicate_groups_total = items
        .iter()
        .filter(|item| item.raw_findings.len() > 1)
        .count();
    let actionable_gaps = items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .count();
    let already_observed = items
        .iter()
        .filter(|item| item.gap_state == "already_observed")
        .count();
    let internal_no_action = items
        .iter()
        .filter(|item| item.gap_state == "internal_only")
        .count();
    let static_limitations = items
        .iter()
        .filter(|item| item.gap_state == "static_limitation")
        .count();
    let unknown = items
        .iter()
        .filter(|item| item.gap_state == "unknown")
        .count();
    let presentation_text_visibility_unknown = items
        .iter()
        .filter(|item| item.presentation_text.visibility == "unknown")
        .count();
    let presentation_text_duplicate_groups = items
        .iter()
        .filter(|item| {
            item.presentation_text.canonical_group_reason == GROUP_REASON_DECL_LITERAL
                && item.raw_findings.len() > 1
        })
        .count();
    let presentation_text_actionable_output_repairs = items
        .iter()
        .filter(|item| item.presentation_text.actionability == "add_output_observer")
        .count();

    FindingAlignmentSummary {
        raw_signals,
        canonical_items: items.len(),
        aligned_raw_findings,
        unaligned_raw_findings: raw_signals.saturating_sub(aligned_raw_findings),
        duplicate_groups_total,
        actionable_gaps,
        already_observed,
        internal_no_action,
        static_limitations,
        unknown,
        presentation_text_total: items.len(),
        presentation_text_visibility_unknown,
        presentation_text_duplicate_groups,
        presentation_text_actionable_output_repairs,
    }
}

fn presentation_text_item(
    constant_name: &str,
    text_literal: Option<String>,
    raw_findings: Vec<FindingAlignmentRawFinding>,
    classification: PresentationTextClassification,
) -> FindingAlignmentItem {
    let group_reason = if raw_findings.len() > 1 {
        GROUP_REASON_DECL_LITERAL
    } else {
        GROUP_REASON_OWNER
    };

    FindingAlignmentItem {
        canonical_gap_id: format!("presentation_text::{constant_name}"),
        canonical_item_kind: classification.canonical_item_kind,
        evidence_class: PRESENTATION_TEXT_CLASS.to_string(),
        gap_state: classification.gap_state,
        actionability: classification.actionability,
        raw_group_size: raw_findings.len(),
        group_reason: group_reason.to_string(),
        why: classification.why,
        recommended_repair: classification.recommended_repair,
        related_test: classification.related_test,
        verify_command: "cargo xtask evidence-quality-scorecard".to_string(),
        static_limitations: classification.static_limitations,
        confidence: classification.confidence,
        raw_findings,
        presentation_text: FindingAlignmentPresentationText {
            constant_name: constant_name.to_string(),
            text_literal,
            visibility: classification.visibility,
            observer: classification.observer,
            actionability: classification.presentation_actionability,
            source_kind: "const_decl".to_string(),
            canonical_group_reason: group_reason.to_string(),
            recommended_observer: classification.recommended_observer,
        },
    }
}

fn classify_presentation_text(
    constant_name: &str,
    raw_findings: &[&Finding],
) -> PresentationTextClassification {
    let source_file = raw_findings
        .first()
        .map(|finding| finding.probe.location.file.display().to_string())
        .unwrap_or_default();

    if is_internal_only_text(constant_name, &source_file) {
        return internal_only_classification();
    }

    if let Some(sink) = visible_sink_for(constant_name, &source_file) {
        if let Some((observer, related_test)) = observer_for_findings(raw_findings) {
            return observed_classification(sink, observer, related_test);
        }

        return actionable_output_classification(sink);
    }

    visibility_unknown_classification()
}

fn visibility_unknown_classification() -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "limitation".to_string(),
        gap_state: "static_limitation".to_string(),
        actionability: "inspect_visibility".to_string(),
        why: "Changed presentation text could not be traced to or away from a user-visible output sink.".to_string(),
        recommended_repair:
            "Trace the string constant to a rendered output path or confirm it is internal-only."
                .to_string(),
        related_test: None,
        static_limitations: vec![FindingAlignmentStaticLimitation {
            category: VISIBILITY_UNKNOWN_CATEGORY.to_string(),
            repair_route: VISIBILITY_UNKNOWN_REPAIR_ROUTE.to_string(),
            user_actionability: "unknown_until_visibility_known".to_string(),
        }],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Visibility-unknown presentation text is benchmark-pinned; no user test debt is claimed without an output sink.".to_string(),
            ],
        },
        visibility: "unknown".to_string(),
        observer: "unknown".to_string(),
        presentation_actionability: "static_limitation_visibility_unknown".to_string(),
        recommended_observer: "unknown".to_string(),
    }
}

fn internal_only_classification() -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "no_action".to_string(),
        gap_state: "internal_only".to_string(),
        actionability: "no_action".to_string(),
        why: "Changed label is confined to an internal proof, policy, or config-only path in fixture-backed scope.".to_string(),
        recommended_repair: "No user test action.".to_string(),
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Internal-only presentation labels are benchmark-pinned as no-action evidence, not user-visible output debt.".to_string(),
            ],
        },
        visibility: "internal_only".to_string(),
        observer: "none".to_string(),
        presentation_actionability: "no_action_internal".to_string(),
        recommended_observer: "none".to_string(),
    }
}

fn actionable_output_classification(sink: PresentationTextSink) -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "gap".to_string(),
        gap_state: "actionable".to_string(),
        actionability: "add_output_observer".to_string(),
        why: format!(
            "Changed text flows to {} and no supported output observer is found.",
            sink.description
        ),
        recommended_repair: format!("Add or update a {} for this changed text.", sink.repair_target),
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Visible unobserved presentation text is actionable only for supported sink patterns.".to_string(),
            ],
        },
        visibility: "user_visible".to_string(),
        observer: "none".to_string(),
        presentation_actionability: "add_output_observer".to_string(),
        recommended_observer: sink.recommended_observer.to_string(),
    }
}

fn observed_classification(
    sink: PresentationTextSink,
    observer: &'static str,
    related_test: FindingAlignmentRelatedTest,
) -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "observed".to_string(),
        gap_state: "already_observed".to_string(),
        actionability: "already_observed".to_string(),
        why: format!(
            "Changed text flows to {} and a supported {observer} observer covers it.",
            sink.description
        ),
        recommended_repair: "No new RIPR action.".to_string(),
        related_test: Some(related_test),
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Observed presentation text stays visible as evidence without becoming a user repair.".to_string(),
            ],
        },
        visibility: "user_visible".to_string(),
        observer: observer.to_string(),
        presentation_actionability: "already_observed".to_string(),
        recommended_observer: observer.to_string(),
    }
}

fn visible_sink_for(constant_name: &str, file: &str) -> Option<PresentationTextSink> {
    let file = normalize_token_text(file);

    if name_has_token(constant_name, "HELP")
        && (file.contains("help") || file.contains("cli") || file.contains("command"))
    {
        return Some(PresentationTextSink {
            recommended_observer: "cli_help_output",
            repair_target: "help-output snapshot assertion",
            description: "CLI help output",
        });
    }

    if name_has_token(constant_name, "REPORT") && file.contains("report") {
        return Some(PresentationTextSink {
            recommended_observer: "report_render",
            repair_target: "report-render or golden-output test",
            description: "rendered report output",
        });
    }

    if (name_has_token(constant_name, "TABLE") || name_has_token(constant_name, "DISPLAY"))
        && (file.contains("table") || file.contains("display") || file.contains("render"))
    {
        return Some(PresentationTextSink {
            recommended_observer: "table_render",
            repair_target: "table-render or golden-output test",
            description: "rendered table output",
        });
    }

    None
}

fn is_internal_only_text(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    name_has_token(constant_name, "INTERNAL")
        || name_has_token(constant_name, "PROOF")
        || name_has_token(constant_name, "POLICY")
        || name_has_token(constant_name, "CONFIG")
        || file.contains("proof")
        || file.contains("policy")
        || file.contains("config")
        || file.contains("internal")
}

fn observer_for_findings(
    raw_findings: &[&Finding],
) -> Option<(&'static str, FindingAlignmentRelatedTest)> {
    raw_findings
        .iter()
        .flat_map(|finding| finding.related_tests.iter())
        .filter_map(|test| {
            observer_for_related_test(test).map(|(rank, observer)| (rank, observer, test))
        })
        .min_by_key(|(rank, _, _)| *rank)
        .map(|(_, observer, test)| (observer, related_test_for(test)))
}

fn observer_for_related_test(test: &RelatedTest) -> Option<(u8, &'static str)> {
    let text = normalize_token_text(&format!("{} {}", test.name, test.file.display()));
    let strong_oracle = matches!(
        test.oracle_strength,
        OracleStrength::Strong | OracleStrength::Medium
    );

    if strong_oracle && text.contains("golden") {
        return Some((0, "golden"));
    }

    if test.oracle_kind == OracleKind::Snapshot || (strong_oracle && text.contains("snapshot")) {
        return Some((1, "snapshot"));
    }

    if strong_oracle && (text.contains("help_output") || text.contains("help")) {
        return Some((2, "cli_help_output"));
    }

    if strong_oracle && (text.contains("report") || text.contains("markdown")) {
        return Some((3, "report_render"));
    }

    if strong_oracle && (text.contains("table") || text.contains("display")) {
        return Some((4, "table_render"));
    }

    None
}

fn related_test_for(test: &RelatedTest) -> FindingAlignmentRelatedTest {
    FindingAlignmentRelatedTest {
        name: test.name.clone(),
        file: test.file.display().to_string(),
        line: test.line,
    }
}

fn name_has_token(name: &str, token: &str) -> bool {
    name.to_ascii_uppercase()
        .split('_')
        .any(|part| part == token)
}

fn normalize_token_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn raw_finding_for(finding: &Finding) -> FindingAlignmentRawFinding {
    FindingAlignmentRawFinding {
        file: finding.probe.location.file.display().to_string(),
        line: finding.probe.location.line,
        kind: finding.class.as_str().to_string(),
        expression: finding.probe.expression.clone(),
        probe_kind: finding.probe.family.as_str().to_string(),
        source_id: finding.probe.id.0.clone(),
        evidence_record_ref: finding.id.clone(),
    }
}

fn adjacent_literal_index(
    findings: &[Finding],
    used: &[bool],
    declaration_index: usize,
) -> Option<usize> {
    let declaration = &findings[declaration_index];
    findings
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != declaration_index && !used[*index])
        .filter(|(_, candidate)| candidate.probe.location.file == declaration.probe.location.file)
        .filter(|(_, candidate)| {
            candidate.probe.location.line == declaration.probe.location.line
                || candidate.probe.location.line == declaration.probe.location.line + 1
        })
        .find_map(|(index, candidate)| {
            parse_string_literal(&candidate.probe.expression).map(|_| index)
        })
}

fn parse_presentation_text_declaration(expression: &str) -> Option<PresentationTextDeclaration> {
    let trimmed = expression.trim();
    let const_pos = trimmed.find("const ")?;
    if const_pos > 0
        && trimmed[..const_pos]
            .chars()
            .last()
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return None;
    }

    let after_const = &trimmed[const_pos + "const ".len()..];
    let name_end = after_const
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(after_const.len());
    let constant_name = &after_const[..name_end];
    if constant_name.is_empty() || !is_presentation_text_constant_name(constant_name) {
        return None;
    }

    let after_name = after_const[name_end..].trim_start();
    let after_colon = after_name.strip_prefix(':')?.trim_start();
    let equals_pos = after_colon.find('=')?;
    let ty = after_colon[..equals_pos].trim();
    if !matches!(ty, "&str" | "&'static str") {
        return None;
    }

    let after_equals = after_colon[equals_pos + 1..].trim_start();
    Some(PresentationTextDeclaration {
        constant_name: constant_name.to_string(),
        inline_literal: parse_string_literal(after_equals),
    })
}

fn is_presentation_text_constant_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "TEXT",
        "LABEL",
        "LABELS",
        "HELP",
        "TITLE",
        "MESSAGE",
        "DESCRIPTION",
        "REPORT",
        "DISPLAY",
        "HEADER",
        "FOOTER",
    ]
    .iter()
    .any(|marker| upper.split('_').any(|part| part == *marker))
}

fn parse_string_literal(expression: &str) -> Option<String> {
    let start = expression.find('"')?;
    let mut value = String::new();
    let mut escaped = false;

    for ch in expression[start + 1..].chars() {
        if escaped {
            match ch {
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                other => {
                    value.push('\\');
                    value.push(other);
                }
            }
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }

    None
}

fn summary_json(out: &mut String, summary: &FindingAlignmentSummary) {
    let ratio = if summary.canonical_items == 0 {
        0.0
    } else {
        summary.raw_signals as f64 / summary.canonical_items as f64
    };
    out.push_str(&format!(
        "{{\"raw_signals\":{},\"canonical_items\":{},\"aligned_raw_findings\":{},\"unaligned_raw_findings\":{},\"raw_to_canonical_ratio\":{ratio:.2},\"duplicate_groups_total\":{},\"actionable_gaps\":{},\"already_observed\":{},\"internal_no_action\":{},\"static_limitations\":{},\"unknown\":{},\"presentation_text_total\":{},\"presentation_text_visibility_unknown\":{},\"presentation_text_duplicate_groups\":{},\"presentation_text_actionable_output_repairs\":{}}}",
        summary.raw_signals,
        summary.canonical_items,
        summary.aligned_raw_findings,
        summary.unaligned_raw_findings,
        summary.duplicate_groups_total,
        summary.actionable_gaps,
        summary.already_observed,
        summary.internal_no_action,
        summary.static_limitations,
        summary.unknown,
        summary.presentation_text_total,
        summary.presentation_text_visibility_unknown,
        summary.presentation_text_duplicate_groups,
        summary.presentation_text_actionable_output_repairs
    ));
}

fn item_json(out: &mut String, item: &FindingAlignmentItem, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    field(
        out,
        indent + 1,
        "canonical_gap_id",
        &item.canonical_gap_id,
        true,
    );
    field(
        out,
        indent + 1,
        "canonical_item_kind",
        &item.canonical_item_kind,
        true,
    );
    field(
        out,
        indent + 1,
        "evidence_class",
        &item.evidence_class,
        true,
    );
    field(out, indent + 1, "gap_state", &item.gap_state, true);
    field(out, indent + 1, "actionability", &item.actionability, true);
    number_field(out, indent + 1, "raw_group_size", item.raw_group_size, true);
    field(out, indent + 1, "group_reason", &item.group_reason, true);
    field(out, indent + 1, "why", &item.why, true);
    field(
        out,
        indent + 1,
        "recommended_repair",
        &item.recommended_repair,
        true,
    );
    related_test_json(out, item.related_test.as_ref(), indent + 1);
    out.push_str(",\n");
    field(
        out,
        indent + 1,
        "verify_command",
        &item.verify_command,
        true,
    );
    static_limitations_json(out, &item.static_limitations, indent + 1);
    out.push_str(",\n");
    confidence_json(out, &item.confidence, indent + 1);
    out.push_str(",\n");
    raw_findings_json(out, &item.raw_findings, indent + 1);
    out.push_str(",\n");
    presentation_text_json(out, &item.presentation_text, indent + 1);
    out.push('\n');
    out.push_str(&format!("{sp}}}"));
}

fn related_test_json(
    out: &mut String,
    related_test: Option<&FindingAlignmentRelatedTest>,
    indent: usize,
) {
    out.push_str(&format!("{}\"related_test\": ", "  ".repeat(indent)));
    if let Some(test) = related_test {
        out.push_str("{\n");
        field(out, indent + 1, "name", &test.name, true);
        field(out, indent + 1, "file", &test.file, true);
        number_field(out, indent + 1, "line", test.line, false);
        out.push_str(&format!("{}}}", "  ".repeat(indent)));
    } else {
        out.push_str("null");
    }
}

fn static_limitations_json(
    out: &mut String,
    limitations: &[FindingAlignmentStaticLimitation],
    indent: usize,
) {
    out.push_str(&format!(
        "{}\"static_limitations\": [\n",
        "  ".repeat(indent)
    ));
    for (index, limitation) in limitations.iter().enumerate() {
        let sp = "  ".repeat(indent + 1);
        out.push_str(&format!("{sp}{{\n"));
        field(out, indent + 2, "category", &limitation.category, true);
        field(
            out,
            indent + 2,
            "repair_route",
            &limitation.repair_route,
            true,
        );
        field(
            out,
            indent + 2,
            "user_actionability",
            &limitation.user_actionability,
            false,
        );
        out.push_str(&format!("{sp}}}"));
        if index + 1 != limitations.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", "  ".repeat(indent)));
}

fn confidence_json(out: &mut String, confidence: &FindingAlignmentConfidence, indent: usize) {
    out.push_str(&format!("{}\"confidence\": {{\n", "  ".repeat(indent)));
    field(out, indent + 1, "basis", &confidence.basis, true);
    array_field(out, indent + 1, "notes", &confidence.notes, false);
    out.push_str(&format!("{}}}", "  ".repeat(indent)));
}

fn raw_findings_json(out: &mut String, raw_findings: &[FindingAlignmentRawFinding], indent: usize) {
    out.push_str(&format!("{}\"raw_findings\": [\n", "  ".repeat(indent)));
    for (index, finding) in raw_findings.iter().enumerate() {
        let sp = "  ".repeat(indent + 1);
        out.push_str(&format!("{sp}{{\n"));
        field(out, indent + 2, "file", &finding.file, true);
        number_field(out, indent + 2, "line", finding.line, true);
        field(out, indent + 2, "kind", &finding.kind, true);
        field(out, indent + 2, "expression", &finding.expression, true);
        field(out, indent + 2, "probe_kind", &finding.probe_kind, true);
        field(out, indent + 2, "source_id", &finding.source_id, true);
        field(
            out,
            indent + 2,
            "evidence_record_ref",
            &finding.evidence_record_ref,
            false,
        );
        out.push_str(&format!("{sp}}}"));
        if index + 1 != raw_findings.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", "  ".repeat(indent)));
}

fn presentation_text_json(
    out: &mut String,
    presentation_text: &FindingAlignmentPresentationText,
    indent: usize,
) {
    out.push_str(&format!(
        "{}\"presentation_text\": {{\n",
        "  ".repeat(indent)
    ));
    field(
        out,
        indent + 1,
        "constant_name",
        &presentation_text.constant_name,
        true,
    );
    out.push_str(&format!("{}\"text_literal\": ", "  ".repeat(indent + 1)));
    if let Some(text_literal) = &presentation_text.text_literal {
        out.push_str(&format!("\"{}\",\n", escape(text_literal)));
    } else {
        out.push_str("null,\n");
    }
    field(
        out,
        indent + 1,
        "visibility",
        &presentation_text.visibility,
        true,
    );
    field(
        out,
        indent + 1,
        "observer",
        &presentation_text.observer,
        true,
    );
    field(
        out,
        indent + 1,
        "actionability",
        &presentation_text.actionability,
        true,
    );
    field(
        out,
        indent + 1,
        "source_kind",
        &presentation_text.source_kind,
        true,
    );
    field(
        out,
        indent + 1,
        "canonical_group_reason",
        &presentation_text.canonical_group_reason,
        true,
    );
    field(
        out,
        indent + 1,
        "recommended_observer",
        &presentation_text.recommended_observer,
        false,
    );
    out.push_str(&format!("{}}}", "  ".repeat(indent)));
}

#[cfg(test)]
mod tests {
    use super::{
        GROUP_REASON_DECL_LITERAL, GROUP_REASON_OWNER, parse_presentation_text_declaration,
        parse_string_literal, report_for_findings,
    };
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState,
    };

    #[test]
    fn groups_const_declaration_and_adjacent_literal() -> Result<(), String> {
        let findings = vec![
            finding_at(
                "decl",
                46,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str =",
            ),
            finding_at(
                "literal",
                47,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "presentation text should align".to_string())?;
        assert_eq!(report.summary.raw_signals, 2);
        assert_eq!(report.summary.canonical_items, 1);
        assert_eq!(report.summary.aligned_raw_findings, 2);
        assert_eq!(report.summary.static_limitations, 1);
        let item = &report.items[0];
        assert_eq!(
            item.canonical_gap_id,
            "presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT"
        );
        assert_eq!(item.raw_group_size, 2);
        assert_eq!(item.group_reason, GROUP_REASON_DECL_LITERAL);
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(item.actionability, "inspect_visibility");
        assert_eq!(
            item.presentation_text.text_literal.as_deref(),
            Some("apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane")
        );
        Ok(())
    }

    #[test]
    fn canonical_id_is_stable_across_line_movement() -> Result<(), String> {
        let before = vec![
            finding_at(
                "before-decl",
                42,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const HELP_MOVED_LABEL: &str =",
            ),
            finding_at(
                "before-literal",
                43,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Help label\";",
            ),
        ];
        let after = vec![
            finding_at(
                "after-decl",
                57,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const HELP_MOVED_LABEL: &str =",
            ),
            finding_at(
                "after-literal",
                58,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Help label\";",
            ),
        ];

        let before_report =
            report_for_findings(&before).ok_or_else(|| "before should align".to_string())?;
        let after_report =
            report_for_findings(&after).ok_or_else(|| "after should align".to_string())?;

        assert_eq!(
            before_report.items[0].canonical_gap_id,
            after_report.items[0].canonical_gap_id
        );
        assert_eq!(
            after_report.items[0].canonical_gap_id,
            "presentation_text::HELP_MOVED_LABEL"
        );
        Ok(())
    }

    #[test]
    fn similar_text_in_different_constants_does_not_collide() -> Result<(), String> {
        let findings = vec![
            finding_at(
                "first-decl",
                31,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const APPLE_DEVICE_LABEL: &str =",
            ),
            finding_at(
                "first-literal",
                32,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Apple CPU/NEON lane\";",
            ),
            finding_at(
                "second-decl",
                35,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const APPLE_REPORT_LABEL: &str =",
            ),
            finding_at(
                "second-literal",
                36,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Apple CPU/NEON lane\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "presentation text should align".to_string())?;

        assert_eq!(report.summary.canonical_items, 2);
        assert_eq!(
            report.items[0].canonical_gap_id,
            "presentation_text::APPLE_DEVICE_LABEL"
        );
        assert_eq!(
            report.items[1].canonical_gap_id,
            "presentation_text::APPLE_REPORT_LABEL"
        );
        Ok(())
    }

    #[test]
    fn string_literal_without_text_constant_does_not_create_item() {
        let findings = vec![finding_at(
            "literal",
            47,
            ExposureClass::StaticUnknown,
            ProbeFamily::StaticUnknown,
            "\"apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane\";",
        )];

        assert!(report_for_findings(&findings).is_none());
    }

    #[test]
    fn non_presentation_string_constant_does_not_create_item() {
        let findings = vec![finding_at(
            "decl",
            12,
            ExposureClass::Exposed,
            ProbeFamily::FieldConstruction,
            "pub const CACHE_KEY: &str = \"apple-m3-air-cpu-neon\";",
        )];

        assert!(report_for_findings(&findings).is_none());
    }

    #[test]
    fn declaration_without_literal_uses_owner_identity_group() -> Result<(), String> {
        let findings = vec![finding_at(
            "decl",
            31,
            ExposureClass::Exposed,
            ProbeFamily::FieldConstruction,
            "pub const APPLE_DEVICE_LABEL: &str =",
        )];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "presentation text should align".to_string())?;

        assert_eq!(report.items[0].raw_group_size, 1);
        assert_eq!(report.items[0].group_reason, GROUP_REASON_OWNER);
        assert!(report.items[0].presentation_text.text_literal.is_none());
        Ok(())
    }

    #[test]
    fn visible_help_text_without_supported_observer_is_actionable() -> Result<(), String> {
        let lexical_only = related_test(
            "mentions_help_label_without_observer",
            "tests/help_labels.rs",
            17,
            OracleKind::Unknown,
            OracleStrength::None,
        );
        let findings = vec![
            finding_in_file_with_related(
                "src/help.rs",
                "decl",
                18,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const HELP_DEVICE_LABEL: &str =",
                vec![lexical_only],
            ),
            finding_in_file(
                "src/help.rs",
                "literal",
                19,
                ExposureClass::WeaklyExposed,
                ProbeFamily::StaticUnknown,
                "\"Device label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "visible help text should align".to_string())?;
        let item = &report.items[0];

        assert_eq!(report.summary.actionable_gaps, 1);
        assert_eq!(report.summary.static_limitations, 0);
        assert_eq!(item.canonical_item_kind, "gap");
        assert_eq!(item.gap_state, "actionable");
        assert_eq!(item.actionability, "add_output_observer");
        assert_eq!(item.presentation_text.visibility, "user_visible");
        assert_eq!(item.presentation_text.observer, "none");
        assert_eq!(
            item.presentation_text.recommended_observer,
            "cli_help_output"
        );
        assert!(item.related_test.is_none());
        assert!(!item.recommended_repair.contains("mutation"));
        Ok(())
    }

    #[test]
    fn visible_report_text_with_golden_observer_is_already_observed() -> Result<(), String> {
        let golden = related_test(
            "report_golden_observes_label",
            "tests/golden/report_output.rs",
            22,
            OracleKind::Snapshot,
            OracleStrength::Strong,
        );
        let findings = vec![
            finding_in_file_with_related(
                "src/report.rs",
                "decl",
                27,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const REPORT_DEVICE_LABEL: &str =",
                vec![golden],
            ),
            finding_in_file(
                "src/report.rs",
                "literal",
                28,
                ExposureClass::Exposed,
                ProbeFamily::StaticUnknown,
                "\"Report label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "visible report text should align".to_string())?;
        let item = &report.items[0];

        assert_eq!(report.summary.already_observed, 1);
        assert_eq!(report.summary.actionable_gaps, 0);
        assert_eq!(item.canonical_item_kind, "observed");
        assert_eq!(item.gap_state, "already_observed");
        assert_eq!(item.actionability, "already_observed");
        assert_eq!(item.presentation_text.visibility, "user_visible");
        assert_eq!(item.presentation_text.observer, "golden");
        assert_eq!(item.presentation_text.actionability, "already_observed");
        assert_eq!(
            item.related_test.as_ref().map(|test| test.name.as_str()),
            Some("report_golden_observes_label")
        );
        assert_eq!(item.recommended_repair, "No new RIPR action.");
        Ok(())
    }

    #[test]
    fn internal_only_label_is_no_action() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/proof_lanes.rs",
                "decl",
                12,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const INTERNAL_PROOF_LANE_LABEL: &str =",
            ),
            finding_in_file(
                "src/proof_lanes.rs",
                "literal",
                13,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"internal proof lane\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "internal label should align".to_string())?;
        let item = &report.items[0];

        assert_eq!(report.summary.internal_no_action, 1);
        assert_eq!(report.summary.static_limitations, 0);
        assert_eq!(item.canonical_item_kind, "no_action");
        assert_eq!(item.gap_state, "internal_only");
        assert_eq!(item.actionability, "no_action");
        assert_eq!(item.presentation_text.visibility, "internal_only");
        assert_eq!(item.presentation_text.observer, "none");
        assert_eq!(item.presentation_text.actionability, "no_action_internal");
        assert!(item.static_limitations.is_empty());
        Ok(())
    }

    #[test]
    fn help_named_text_without_supported_sink_stays_visibility_unknown() -> Result<(), String> {
        let findings = vec![finding_in_file(
            "src/opaque.rs",
            "decl",
            33,
            ExposureClass::Exposed,
            ProbeFamily::FieldConstruction,
            "pub const HELP_DEVICE_LABEL: &str = \"Device label\";",
        )];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "opaque help label should align".to_string())?;
        let item = &report.items[0];

        assert_eq!(report.summary.static_limitations, 1);
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(item.actionability, "inspect_visibility");
        assert_eq!(item.presentation_text.visibility, "unknown");
        assert_eq!(item.presentation_text.observer, "unknown");
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.category.as_str()),
            Some("presentation_text_visibility_unknown")
        );
        Ok(())
    }

    #[test]
    fn parses_presentation_text_const_declaration() -> Result<(), String> {
        let declaration = parse_presentation_text_declaration(
            "pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str = \"value\";",
        )
        .ok_or_else(|| "declaration should parse".to_string())?;

        assert_eq!(declaration.constant_name, "APPLE_M3_AIR_DEVICE_LABELS_TEXT");
        assert_eq!(declaration.inline_literal.as_deref(), Some("value"));
        Ok(())
    }

    #[test]
    fn parses_escaped_string_literal() {
        assert_eq!(
            parse_string_literal("\"Help \\\"quoted\\\" label\";").as_deref(),
            Some("Help \"quoted\" label")
        );
    }

    fn finding_at(
        id_suffix: &str,
        line: usize,
        class: ExposureClass,
        family: ProbeFamily,
        expression: &str,
    ) -> Finding {
        finding_in_file(
            "src/device_labels.rs",
            id_suffix,
            line,
            class,
            family,
            expression,
        )
    }

    fn finding_in_file(
        file: &str,
        id_suffix: &str,
        line: usize,
        class: ExposureClass,
        family: ProbeFamily,
        expression: &str,
    ) -> Finding {
        finding_in_file_with_related(file, id_suffix, line, class, family, expression, vec![])
    }

    fn finding_in_file_with_related(
        file: &str,
        id_suffix: &str,
        line: usize,
        class: ExposureClass,
        family: ProbeFamily,
        expression: &str,
        related_tests: Vec<RelatedTest>,
    ) -> Finding {
        let probe_id = format!("probe:src_device_labels_rs:{line}:{id_suffix}");
        Finding {
            id: probe_id.clone(),
            probe: Probe {
                id: ProbeId(probe_id),
                location: SourceLocation::new(file, line, 1),
                owner: None,
                family,
                delta: DeltaKind::Value,
                before: None,
                after: None,
                expression: expression.to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class,
            ripr: RiprEvidence {
                reach: stage("presentation text raw signal"),
                infect: stage("presentation text raw signal"),
                propagate: stage("presentation text raw signal"),
                reveal: RevealEvidence {
                    observe: stage("presentation text raw signal"),
                    discriminate: stage("presentation text raw signal"),
                },
            },
            confidence: 0.2,
            evidence: vec![],
            missing: vec![],
            flow_sinks: vec![],
            activation: ActivationEvidence::default(),
            stop_reasons: vec![],
            related_tests,
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
        }
    }

    fn related_test(
        name: &str,
        file: &str,
        line: usize,
        oracle_kind: OracleKind,
        oracle_strength: OracleStrength,
    ) -> RelatedTest {
        RelatedTest {
            name: name.to_string(),
            file: file.into(),
            line,
            oracle: None,
            oracle_kind,
            oracle_strength,
        }
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Low, summary)
    }
}
