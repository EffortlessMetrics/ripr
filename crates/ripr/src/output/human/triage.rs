use crate::app::{CheckOutput, FindingNavigation};
use crate::config::RiprConfig;
use crate::domain::{ExposureClass, Finding};
use crate::output::preview_actionability::preview_actionability_for;
use std::collections::BTreeSet;

use super::sections::render_finding_digest_with_config;

pub(crate) struct HumanTriage<'a> {
    pub(crate) state: HumanTriageState,
    pub(crate) selected: Option<&'a Finding>,
    pub(crate) omitted_findings: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HumanTriageState {
    TopGap,
    NoActionableGap,
    StaticLimited,
    PreviewLimited,
    MissingScope,
}

impl HumanTriageState {
    fn as_str(self) -> &'static str {
        match self {
            Self::TopGap => "top_gap",
            Self::NoActionableGap => "no_actionable_gap",
            Self::StaticLimited => "static_limited",
            Self::PreviewLimited => "preview_limited",
            Self::MissingScope => "missing_scope",
        }
    }
}

pub(crate) fn select_human_triage<'a>(
    output: &'a CheckOutput,
    _config: &RiprConfig,
) -> HumanTriage<'a> {
    let suppressed_ids: BTreeSet<&str> = output
        .suppression
        .iter()
        .flat_map(|outcome| {
            outcome
                .suppressed
                .iter()
                .map(|entry| entry.finding_id.as_str())
        })
        .collect();
    let mut selected = None;
    let mut visible_findings: usize = 0;
    for finding in &output.findings {
        if suppressed_ids.contains(finding.id.as_str()) {
            continue;
        }
        visible_findings += 1;
        if selected.is_none_or(|current| triage_rank(finding) < triage_rank(current)) {
            selected = Some(finding);
        }
    }
    let state = selected.map_or_else(
        || {
            if output.no_scope_provided {
                HumanTriageState::MissingScope
            } else if !output.findings.is_empty() && visible_findings == 0 {
                HumanTriageState::NoActionableGap
            } else {
                HumanTriageState::StaticLimited
            }
        },
        |finding| {
            if is_preview_limited(finding) {
                HumanTriageState::PreviewLimited
            } else if finding.class == ExposureClass::Exposed {
                HumanTriageState::NoActionableGap
            } else if is_static_limited(finding) {
                HumanTriageState::StaticLimited
            } else {
                HumanTriageState::TopGap
            }
        },
    );
    HumanTriage {
        state,
        selected,
        omitted_findings: visible_findings.saturating_sub(usize::from(selected.is_some())),
    }
}

pub(crate) fn render_human_triage(
    out: &mut String,
    triage: &HumanTriage<'_>,
    output: &CheckOutput,
    config: &RiprConfig,
    navigation: Option<&FindingNavigation>,
) {
    out.push_str("Start here:\n");
    out.push_str(&format!("  State: {}\n", triage.state.as_str()));
    match triage.state {
        HumanTriageState::TopGap => out.push_str(
            "  Safe next action: inspect or repair the selected non-exposed gap; this is static advisory evidence only.\n",
        ),
        HumanTriageState::NoActionableGap => {
            if triage.selected.is_none() && !output.findings.is_empty() {
                out.push_str(
                    "  Safe next action: all findings are suppressed by policy; review the suppression block before treating this run as actionable.\n",
                );
            } else {
                out.push_str(
                    "  Safe next action: no non-exposed diff finding was selected. This is not runtime proof, coverage adequacy, or mutation confirmation.\n",
                );
            }
        }
        HumanTriageState::StaticLimited => out.push_str(
            "  Safe next action: inspect the named static limitation before treating this as repair-ready.\n",
        ),
        HumanTriageState::PreviewLimited => {
            // #2273: the shared repair-packet validator is the only authority
            // on packet completeness, and the line must name the real blocker:
            // a complete packet stays advisory (do not tell the operator to
            // complete fields that are already present); a blocked packet with
            // no missing fields AND a structured static-limit kind is held by
            // that named limitation, not by absent fields; anything else has
            // genuinely missing fields. Languages without a structured preview
            // packet (for example Python) fall through to the generic line.
            match triage.selected.and_then(preview_actionability_for) {
                Some(actionability) if actionability.repair_packet_ready => out.push_str(
                    "  Safe next action: preview-language evidence is advisory; the repair packet is complete but remains advisory, so verify independently before acting.\n",
                ),
                Some(actionability)
                    if actionability.missing_actionability_fields.is_empty()
                        && triage
                            .selected
                            .is_some_and(|finding| finding.static_limit_kind.is_some()) =>
                {
                    out.push_str(
                        "  Safe next action: preview-language evidence is advisory; the repair packet is blocked by the named static limitation, not by missing fields; resolve the limitation and rerun preview evidence before acting.\n",
                    );
                }
                _ => out.push_str(
                    "  Safe next action: preview-language evidence is advisory; complete the missing repair-packet fields before acting.\n",
                ),
            }
        }
        HumanTriageState::MissingScope => out.push_str(
            "  Safe next action: provide an analysis scope; this empty output is not an all-clear.\n",
        ),
    }
    if let Some(finding) = triage.selected {
        out.push_str(&render_finding_digest_with_config(finding, config));
        if let Some(navigation) = navigation {
            out.push_str("\nNext: drill into the top finding:\n");
            out.push_str(&format!("  {}\n", navigation.explain_command(&finding.id)));
            out.push_str(&format!("  {}\n", navigation.context_command(&finding.id)));
        }
    }
    // #2567: the default human render is the release-facing surface, so it must
    // not claim a hidden remainder that does not exist. `Hidden:` plus a literal
    // `0 lower-priority finding(s) omitted` reads as suppressed evidence and was
    // the dominant case in fixture output. When nothing is omitted, keep only the
    // format pointers under a `More:` heading; the count line stays for the real
    // truncation case, where it is the whole point of the section.
    if triage.omitted_findings == 0 {
        out.push_str("\nMore:\n");
    } else {
        out.push_str("\nHidden:\n");
        out.push_str(&format!(
            "  {} lower-priority finding(s) omitted from default human output.\n",
            triage.omitted_findings
        ));
    }
    out.push_str("  Full evidence: rerun with --format human-full\n");
    out.push_str("  Machine data: rerun with --format json\n\n");
}

fn triage_rank(finding: &Finding) -> (u8, u8, u8, u8, u8, i32, &std::path::Path, usize) {
    let class_rank = match finding.class {
        ExposureClass::ReachableUnrevealed => 2,
        ExposureClass::WeaklyExposed => 3,
        ExposureClass::NoStaticPath => 4,
        ExposureClass::InfectionUnknown
        | ExposureClass::PropagationUnknown
        | ExposureClass::StaticUnknown => 5,
        ExposureClass::Exposed => 9,
    };
    let preview_rank = u8::from(is_preview_limited(finding));
    let repair_rank = if finding.class != ExposureClass::Exposed
        && !is_preview_limited(finding)
        && has_repair_route(finding)
    {
        0
    } else {
        class_rank
    };
    (
        preview_rank,
        repair_rank,
        u8::from(finding.canonical_gap.is_none()),
        u8::from(finding.related_tests.is_empty()),
        u8::from(finding.missing.is_empty()),
        -(finding.confidence * 100.0) as i32,
        finding.probe.location.file.as_path(),
        finding.probe.location.line,
    )
}

fn has_repair_route(finding: &Finding) -> bool {
    finding.recommended_next_step.is_some()
        || finding
            .evidence
            .iter()
            .any(|line| line.starts_with("suggested_verify_command: "))
}

fn is_static_limited(finding: &Finding) -> bool {
    matches!(
        finding.class,
        ExposureClass::NoStaticPath
            | ExposureClass::InfectionUnknown
            | ExposureClass::PropagationUnknown
            | ExposureClass::StaticUnknown
    )
}

fn is_preview_limited(finding: &Finding) -> bool {
    finding
        .language_status
        .is_some_and(|status| status.as_str() == "preview")
}
