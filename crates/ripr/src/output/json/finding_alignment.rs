use crate::domain::Finding;

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

        let raw_findings = raw_indices
            .iter()
            .map(|raw_index| raw_finding_for(&findings[*raw_index]))
            .collect::<Vec<_>>();
        items.push(presentation_text_visibility_unknown_item(
            &declaration.constant_name,
            literal,
            raw_findings,
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

fn presentation_text_visibility_unknown_item(
    constant_name: &str,
    text_literal: Option<String>,
    raw_findings: Vec<FindingAlignmentRawFinding>,
) -> FindingAlignmentItem {
    let group_reason = if raw_findings.len() > 1 {
        GROUP_REASON_DECL_LITERAL
    } else {
        GROUP_REASON_OWNER
    };

    FindingAlignmentItem {
        canonical_gap_id: format!("presentation_text::{constant_name}"),
        canonical_item_kind: "limitation".to_string(),
        evidence_class: PRESENTATION_TEXT_CLASS.to_string(),
        gap_state: "static_limitation".to_string(),
        actionability: "inspect_visibility".to_string(),
        raw_group_size: raw_findings.len(),
        group_reason: group_reason.to_string(),
        why: "Changed presentation text could not be traced to or away from a user-visible output sink.".to_string(),
        recommended_repair:
            "Trace the string constant to a rendered output path or confirm it is internal-only."
                .to_string(),
        related_test: None,
        verify_command: "cargo xtask evidence-quality-scorecard".to_string(),
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
        raw_findings,
        presentation_text: FindingAlignmentPresentationText {
            constant_name: constant_name.to_string(),
            text_literal,
            visibility: "unknown".to_string(),
            observer: "unknown".to_string(),
            actionability: "static_limitation_visibility_unknown".to_string(),
            source_kind: "const_decl".to_string(),
            canonical_group_reason: group_reason.to_string(),
            recommended_observer: "unknown".to_string(),
        },
    }
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
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, Probe, ProbeFamily,
        ProbeId, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
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
        let probe_id = format!("probe:src_device_labels_rs:{line}:{id_suffix}");
        Finding {
            id: probe_id.clone(),
            probe: Probe {
                id: ProbeId(probe_id),
                location: SourceLocation::new("src/device_labels.rs", line, 1),
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
            related_tests: vec![],
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
        }
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Low, summary)
    }
}
