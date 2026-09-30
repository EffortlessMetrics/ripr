use crate::app::{CheckOutput, FindingDrillIn};
use crate::config::RiprConfig;
use crate::domain::{ExposureClass, Finding, LanguageId};
use crate::output::path::display_path;
use crate::output::preview_actionability::preview_actionability_for;
use crate::output::python_repair_card::python_repair_card;
use crate::output::typescript_packet_projection::typescript_gap_record_for;
use std::collections::BTreeSet;

use super::sections::{one_line, render_finding_digest_with_config};

pub(crate) struct HumanTriage<'a> {
    pub(crate) state: HumanTriageState,
    pub(crate) selected: Option<&'a Finding>,
    /// Every considered (non-suppressed) finding the default human render does
    /// not show: the lower-ranked candidates plus, when nothing was
    /// candidate-actionable, all of them. #4320 names them in `Hidden:` by
    /// `file:line (class)` so a reader can confirm coverage without a rerun;
    /// #4395(b) summarizes preview / non-Rust identity inline.
    pub(crate) omitted: Vec<&'a Finding>,
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

    /// The state in words; the stable id follows it in parentheses.
    fn plain_label(self) -> &'static str {
        match self {
            Self::TopGap => "a test gap to inspect or repair",
            Self::NoActionableGap => "no gap selected for repair",
            Self::StaticLimited => "limited by static analysis",
            Self::PreviewLimited => "preview language, advisory only",
            Self::MissingScope => "nothing in scope",
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
    let mut considered: Vec<&'a Finding> = Vec::new();
    let mut visible_findings: usize = 0;
    for finding in &output.findings {
        if suppressed_ids.contains(finding.id.as_str()) {
            continue;
        }
        considered.push(finding);
        // Candidate-actionable eligibility (#3281): "Start here" names a
        // current candidate-side obligation. Base-side evidence and
        // unresolved subjects remain counted findings (the hidden-count
        // denominator below keeps them) but never become the top gap.
        if !finding.is_candidate_actionable() {
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
    // #4320: the hidden set is every considered finding except the one
    // selected finding (identity by finding id, not value equality).
    HumanTriage {
        state,
        selected,
        omitted: considered
            .into_iter()
            .filter(|finding| selected.is_none_or(|chosen| chosen.id != finding.id))
            .collect(),
    }
}

pub(crate) fn render_human_triage(
    out: &mut String,
    triage: &HumanTriage<'_>,
    output: &CheckOutput,
    config: &RiprConfig,
    drill_in: Option<&FindingDrillIn>,
) {
    out.push_str("Start here:\n");
    out.push_str(&format!(
        "  State: {} ({})\n",
        triage.state.plain_label(),
        triage.state.as_str()
    ));
    match triage.state {
        HumanTriageState::TopGap => out.push_str(
            "  Safe next action: inspect or repair the selected non-exposed gap; this is static advisory evidence only.\n",
        ),
        HumanTriageState::NoActionableGap => {
            if triage.selected.is_none() && !triage.omitted.is_empty() {
                // #4320: findings exist but the #3281 candidate filter hid
                // every one — the run is not a policy suppression. Claiming
                // suppression contradicts the suppression block (which listed
                // nothing) and misleads the reader about what happened to the
                // findings. #4320 review: the framing must also keep the
                // currentness distinction — `unresolved_subject` is the
                // explicit unknown, not base-side evidence.
                out.push_str(&no_selection_safe_action_line(&triage.omitted));
            } else if triage.selected.is_none() && !output.findings.is_empty() {
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
                Some(actionability) if actionability.repair_packet_ready => {
                    out.push_str(&complete_packet_preview_safe_action(
                        triage.selected,
                        &actionability.repair_route,
                    ));
                }
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
                not_ready => match triage.selected {
                    Some(finding) if finding.language == Some(LanguageId::Python) => {
                        out.push_str(&python_preview_safe_action(finding));
                    }
                    Some(finding) if finding.class == ExposureClass::Exposed => {
                        out.push_str(EXPOSED_PREVIEW_SAFE_ACTION);
                    }
                    Some(finding) if let Some(actionability) = not_ready.as_ref() => {
                        out.push_str(&packet_closed_preview_safe_action(
                            finding,
                            &actionability.why_not_actionable,
                        ));
                    }
                    _ => out.push_str(
                        "  Safe next action: preview-language evidence is advisory; complete the missing repair-packet fields before acting.\n",
                    ),
                },
            }
        }
        // #4012: on an established-but-empty range the scope was provided
        // (a default base was resolved and compared) — the honest action is
        // to change something, not to provide a scope.
        HumanTriageState::MissingScope => {
            if let Some(base) = output.base.as_deref() {
                out.push_str(&format!(
                    "  Safe next action: no changed files were compared against `{base}`; commit a change and re-run, or add `--worktree` to include uncommitted edits.\n"
                ));
            } else {
                out.push_str(
                    "  Safe next action: provide an analysis scope; this empty output is not an all-clear.\n",
                );
            }
        }
    }
    if let Some(finding) = triage.selected {
        out.push_str(&render_finding_digest_with_config(finding, config));
        match drill_in {
            Some(FindingDrillIn::Commands(navigation)) => {
                out.push_str("\nNext: drill into the top finding:\n");
                out.push_str(&format!("  {}\n", navigation.explain_command(&finding.id)));
                out.push_str(&format!("  {}\n", navigation.context_command(&finding.id)));
            }
            // #4321: a `--worktree` run without `--write-artifact` has no
            // artifact for sibling commands to replay; say so and name the
            // route instead of dropping the block silently.
            Some(FindingDrillIn::WorktreeReplayNeedsArtifact) => {
                out.push_str(&format!(
                    "\n{}\n",
                    FindingDrillIn::worktree_replay_note(&finding.id)
                ));
            }
            None => {}
        }
    }
    // #2567: the default human render is the release-facing surface, so it must
    // not claim a hidden remainder that does not exist. `Hidden:` plus a literal
    // `0 lower-priority finding(s) omitted` reads as suppressed evidence and was
    // the dominant case in fixture output. When nothing is omitted, keep only the
    // format pointers under a `More:` heading; the count line stays for the real
    // truncation case, where it is the whole point of the section.
    //
    // #4320: when something IS hidden, the block names it — by
    // `file:line (class)`, so a reader can confirm a file they care about was
    // covered without a rerun — and distinguishes the all-base-side case, where
    // nothing was candidate-actionable and a lower-priority framing would
    // misdescribe the run.
    if triage.omitted.is_empty() {
        out.push_str("\nMore:\n");
    } else {
        out.push_str("\nHidden:\n");
        if triage.selected.is_none() {
            out.push_str(&no_selection_hidden_line(&triage.omitted));
        } else {
            out.push_str(&format!(
                "  {} lower-priority finding(s) omitted from default human output{}.\n",
                triage.omitted.len(),
                omitted_identity_suffix(&triage.omitted)
            ));
        }
        let listed = triage.omitted.len().min(HIDDEN_FINDINGS_LISTED);
        for finding in triage.omitted.iter().take(listed) {
            out.push_str(&format!(
                "    - {}:{} ({})\n",
                display_path(&finding.probe.location.file),
                finding.probe.location.line,
                finding.class.as_str()
            ));
        }
        let remaining = triage.omitted.len() - listed;
        if remaining > 0 {
            out.push_str(&format!(
                "    - … and {remaining} more omitted finding(s); every identity is in --format json.\n"
            ));
        }
    }
    out.push_str("  Full evidence: rerun with --format human-full\n");
    out.push_str("  Machine data: rerun with --format json\n\n");
}

/// #4320: the `Hidden:` list names omitted findings so the reader can confirm
/// coverage without a rerun, but the default surface stays bounded: beyond
/// this window the list discloses the remainder instead of printing every
/// identity.
const HIDDEN_FINDINGS_LISTED: usize = 20;

/// The currentness mix of a no-selection run. `unresolved_subject` is the
/// explicit unknown (#3281) — not base-side evidence — so the #4320
/// no-selection framing must name the actual mix instead of promoting every
/// unselected finding to a base-side claim (#4320 review).
enum NoSelectionMix {
    AllBaseSide,
    AllUnresolved,
    Mixed { base_side: usize, unresolved: usize },
}

fn no_selection_mix(omitted: &[&Finding]) -> NoSelectionMix {
    let base_side = omitted
        .iter()
        .filter(|finding| {
            matches!(
                finding.source_currentness,
                crate::domain::SourceCurrentness::BaseDeleted
                    | crate::domain::SourceCurrentness::MovedOrRenamed
            )
        })
        .count();
    let unresolved = omitted
        .iter()
        .filter(|finding| {
            finding.source_currentness == crate::domain::SourceCurrentness::UnresolvedSubject
        })
        .count();
    match (base_side, unresolved) {
        (0, 0) => NoSelectionMix::AllBaseSide,
        (_, 0) => NoSelectionMix::AllBaseSide,
        (0, _) => NoSelectionMix::AllUnresolved,
        (b, u) => NoSelectionMix::Mixed {
            base_side: b,
            unresolved: u,
        },
    }
}

/// The `Hidden:` count line for a run where nothing was candidate-actionable.
fn no_selection_hidden_line(omitted: &[&Finding]) -> String {
    let suffix = omitted_identity_suffix(omitted);
    match no_selection_mix(omitted) {
        NoSelectionMix::AllBaseSide => format!(
            "  All {} finding(s) are base-side evidence, not candidate edit targets — rerun with --format human-full for the full evidence{}.\n",
            omitted.len(),
            suffix
        ),
        NoSelectionMix::AllUnresolved => format!(
            "  All {} finding(s) have unresolved subject currentness — not established base-side or candidate edit targets; rerun with --format human-full for the full evidence{}.\n",
            omitted.len(),
            suffix
        ),
        NoSelectionMix::Mixed {
            base_side,
            unresolved,
        } => format!(
            "  None of the {} finding(s) is a candidate edit target ({} base-side, {} unresolved currentness) — rerun with --format human-full for the full evidence{}.\n",
            omitted.len(),
            base_side,
            unresolved,
            suffix
        ),
    }
}

/// The safe-next-action line for a run where nothing was candidate-actionable.
fn no_selection_safe_action_line(omitted: &[&Finding]) -> String {
    match no_selection_mix(omitted) {
        NoSelectionMix::AllBaseSide => {
            "  Safe next action: all findings are base-side evidence, not candidate edit targets; rerun with --format human-full to inspect the full evidence before treating this run as actionable.\n".to_string()
        }
        NoSelectionMix::AllUnresolved => {
            "  Safe next action: no finding is resolved to the candidate (subject currentness unresolved); rerun with --format human-full to inspect the full evidence before treating this run as actionable.\n".to_string()
        }
        NoSelectionMix::Mixed { base_side, unresolved } => format!(
            "  Safe next action: no finding is a candidate edit target ({} base-side, {} unresolved currentness); rerun with --format human-full to inspect the full evidence before treating this run as actionable.\n",
            base_side, unresolved
        ),
    }
}

/// #4395(b): the Hidden count line names omitted preview / non-Rust identity
/// from fields already on those findings. This is not the #2615 availability
/// projection. Rust-only remainder stays the count line alone.
fn omitted_identity_suffix(omitted: &[&Finding]) -> String {
    match omitted_language_identity(omitted) {
        Some(identity) => format!(" ({identity})"),
        None => String::new(),
    }
}

fn omitted_language_identity(omitted: &[&Finding]) -> Option<String> {
    if !omitted.iter().any(|finding| {
        is_preview_limited(finding) || finding.language.is_some_and(|id| id != LanguageId::Rust)
    }) {
        return None;
    }

    let mut parts = Vec::new();
    for language in LanguageId::ALL {
        let count = omitted
            .iter()
            .filter(|finding| finding.language == Some(language))
            .count();
        if count == 0 {
            continue;
        }
        let preview = omitted
            .iter()
            .any(|finding| finding.language == Some(language) && is_preview_limited(finding));
        if preview {
            parts.push(format!("{} preview: {count}", language.display_name()));
        } else {
            parts.push(format!("{}: {count}", language.display_name()));
        }
    }
    let unlabeled_preview = omitted
        .iter()
        .filter(|finding| finding.language.is_none() && is_preview_limited(finding))
        .count();
    if unlabeled_preview > 0 {
        parts.push(format!("preview-language: {unlabeled_preview}"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join(", "))
}

/// A complete (validator-approved) preview packet: name the packet's own
/// repair action, test file, and verify command so the operator can act on
/// it, rather than a bare "verify independently" with no route. Falls back to
/// the generic advisory line when the projected record lacks a test file or
/// verify command (for example a synthetic Perl packet). Readiness is only
/// read here, never decided.
fn complete_packet_preview_safe_action(finding: Option<&Finding>, repair_route: &str) -> String {
    const GENERIC: &str = "  Safe next action: preview-language evidence is advisory; the repair packet is complete but remains advisory, so verify independently before acting.\n";
    let Some(record) = finding.and_then(typescript_gap_record_for) else {
        return GENERIC.to_string();
    };
    let target_file = record
        .repair_route
        .as_ref()
        .and_then(|route| route.target_file.as_deref())
        .filter(|file| !file.trim().is_empty());
    let verify = record
        .verification_commands
        .first()
        .map(String::as_str)
        .filter(|command| !command.trim().is_empty());
    let (Some(target_file), Some(verify)) = (target_file, verify) else {
        return GENERIC.to_string();
    };
    // The action is the packet's own instruction and carries the concrete
    // assertion shape, so it is collapsed to one line but not truncated.
    let action = repair_route
        .replacen(" in the related test", "", 1)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "  Safe next action: preview-language evidence is advisory; the repair packet is complete: in `{target_file}`, {action}; run `{verify}`, then rerun `ripr check`.\n"
    )
}

/// An exposed preview finding has nothing to repair, in any preview language.
const EXPOSED_PREVIEW_SAFE_ACTION: &str = "  Safe next action: preview-language evidence is advisory; a related test appears to observe this change, so there is no repair to make; verify independently before relying on it.\n";

/// #4216 (TS/JS arm of row 1): the shared validator behind
/// `preview_actionability_for` kept this preview finding's repair packet
/// closed, so `check` emits no repair packet, the gap ledger carries no repair
/// route (#4224), and no ripr command routes it. "Complete the missing
/// repair-packet fields" asked for something the operator cannot supply. The
/// line is terminal instead: it quotes `preview_actionability_for`'s
/// `why_not_actionable` (in most closed-packet cases the validator never
/// ran) and names the manual step. Readiness is only read here, never
/// decided.
fn packet_closed_preview_safe_action(finding: &Finding, why_not_actionable: &str) -> String {
    let language = finding
        .language
        .map_or("preview-language", LanguageId::display_name);
    let manual_step = match finding.class {
        ExposureClass::NoStaticPath => "add a test that calls it by hand",
        // An unknown class is a visibility limit (for example the Bun bridge's
        // cross-language gap), not a known weak test: ask for the same check
        // the Python static-limit line names, not a test edit.
        ExposureClass::StaticUnknown
        | ExposureClass::InfectionUnknown
        | ExposureClass::PropagationUnknown => "check by hand whether a test observes this change",
        _ => "add or strengthen a test by hand",
    };
    // Only the quoted reason is bounded; the routing and manual-step parts
    // stay whole.
    // The authority's reason opens with a generic preview preamble and ends
    // with `validator: <specific cause>`; under the line budget the specific
    // cause is the part the user can act on, so show it when present.
    // The cause itself may still open with the fixed eligibility phrase;
    // drop it so the remedy ("derive an input ...") fits the budget.
    let specific = why_not_actionable
        .split_once("validator: ")
        .map_or(why_not_actionable, |(_, cause)| cause);
    let specific = specific
        .strip_prefix("is not agent-packet eligible: ")
        .unwrap_or(specific);
    let reason = one_line(specific);
    format!(
        "  Safe next action: this {language} preview finding's repair packet is not ready ({reason}); `ripr pilot`, `ripr agent repair` and `ripr first-pr` will not route it; {manual_step}, then rerun `ripr check`.\n"
    )
}

/// #4216 row 1: Python has no structured preview packet, so the generic
/// "complete the missing repair-packet fields" line told the operator to do
/// something they cannot, and pilot, first-pr and status each routed back to
/// another command. The Python repair card (`python_repair_card`) is the one
/// authority on whether a Python finding carries a repair route: with a card,
/// its suggested test and verify command are the route; without one, no ripr
/// command will route the finding, so the line says why and names the only
/// step left, a manual test or check. An exposed finding has nothing to repair.
fn python_preview_safe_action(finding: &Finding) -> String {
    if finding.class == ExposureClass::Exposed {
        return EXPOSED_PREVIEW_SAFE_ACTION.to_string();
    }
    if python_repair_card(finding).is_some() {
        return "  Safe next action: preview-language evidence is advisory; apply the next step below to the suggested test and run its verify command before relying on it.\n".to_string();
    }
    if let Some(kind) = finding.static_limit_kind.as_ref() {
        return format!(
            "  Safe next action: static limitation `{}` keeps this Python preview finding from a repair card, so `ripr pilot`, `ripr agent repair` and `ripr first-pr` will not route it; check by hand whether a test observes this change, then rerun `ripr check`.\n",
            kind.as_str()
        );
    }
    let (reason, manual_step) = if finding.class == ExposureClass::NoStaticPath {
        (
            "no Python test reaches this code",
            "add a test that calls it by hand",
        )
    } else if finding.activation.missing_discriminators.is_empty() {
        (
            "static evidence names no concrete missing discriminator",
            "add or strengthen a test by hand",
        )
    } else {
        (
            "its test placement or related-test evidence is incomplete",
            "add or strengthen a test by hand",
        )
    };
    format!(
        "  Safe next action: this Python preview finding has no repair card ({reason}), so `ripr pilot`, `ripr agent repair` and `ripr first-pr` will not route it; {manual_step}, then rerun `ripr check`.\n"
    )
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
    let repair_rank = if finding.class != ExposureClass::Exposed && has_ranked_repair_route(finding)
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

/// A stable finding ranks by its repair route; a preview finding only by the
/// repair authority for its language (#4216 rc rehearsal): a Python finding
/// with a repair card outranks one without, so Start here does not pick a
/// card-less finding that no ripr command routes over one pilot and first-pr
/// do route. Other preview languages keep their class rank. Classification is
/// unchanged; only the selection order moves.
fn has_ranked_repair_route(finding: &Finding) -> bool {
    if !is_preview_limited(finding) {
        return has_repair_route(finding);
    }
    finding.language == Some(LanguageId::Python) && python_repair_card(finding).is_some()
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
