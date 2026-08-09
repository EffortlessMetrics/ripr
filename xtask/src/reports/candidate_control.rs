//! Candidate-relative release state owned by the temporary #2766 control lens.
//!
//! This is deliberately separate from the open-PR disposition report. A
//! complete PR inventory can be useful while candidate scope is still
//! unresolved, and an open-PR count is never a hard-cut predicate.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(crate) const SELECTION_SCHEMA_VERSION: &str = "0.1";

const RESOLUTIONS: &[&str] = &[
    "pending",
    "landed",
    "accepted_defer",
    "candidate_exclusion",
    "failed",
];

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateSelection {
    pub(crate) schema_version: String,
    #[serde(default)]
    pub(crate) selected_cut_sha: Option<String>,
    #[serde(default)]
    pub(crate) selected_claims: Vec<SelectedClaim>,
    #[serde(default)]
    pub(crate) candidate_exclusions: Vec<CandidateExclusion>,
    #[serde(default)]
    pub(crate) known_candidate_defects: Vec<KnownCandidateDefect>,
    #[serde(default)]
    #[serde(
        rename = "denominator_decisions_remaining",
        alias = "denominator_decisions_remaining_through_provisional_cutoff"
    )]
    pub(crate) denominator_decisions_remaining_through_provisional_cutoff: Option<u64>,
    #[serde(default)]
    pub(crate) denominator_decisions_remaining_through_selected_cut: Option<u64>,
    #[serde(default)]
    pub(crate) final_cut_authority: Option<FinalCutAuthority>,
    #[serde(default)]
    pub(crate) projection: CandidateProjection,
    #[serde(default)]
    pub(crate) qualification: QualificationInputs,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FinalCutAuthority {
    pub(crate) cut_sha: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) record_set_digest: Option<String>,
    pub(crate) provisional_decisions_remaining: u64,
    pub(crate) unreviewed_post_provisional_records_through_cut: u64,
    pub(crate) final_cut_decisions_remaining: u64,
    pub(crate) reviewed_through_selected_cut: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectedClaim {
    pub(crate) claim_id: String,
    pub(crate) owner_issue: u64,
    pub(crate) required_for_candidate: bool,
    pub(crate) resolution: String,
    #[serde(default)]
    pub(crate) evidence_refs: Vec<String>,
    #[serde(default)]
    pub(crate) commit_refs: Vec<String>,
    #[serde(default)]
    pub(crate) artifact_refs: Vec<String>,
    pub(crate) candidate_effect: String,
    #[serde(default)]
    pub(crate) non_claim: Option<String>,
    pub(crate) reviewed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateExclusion {
    pub(crate) exclusion_id: String,
    pub(crate) claim_id: String,
    pub(crate) reason: String,
    pub(crate) non_claim: String,
    #[serde(default)]
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) reviewed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KnownCandidateDefect {
    pub(crate) defect_id: String,
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) resolved: bool,
    #[serde(default)]
    pub(crate) evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateProjection {
    #[serde(default)]
    pub(crate) reproducible: bool,
    #[serde(default)]
    pub(crate) candidate_tree_sha: Option<String>,
    #[serde(default)]
    pub(crate) candidate_tree_parent_sha: Option<String>,
    #[serde(default)]
    pub(crate) exclusion_digest: Option<String>,
    #[serde(default)]
    pub(crate) expected_exclusion_digest: Option<String>,
    #[serde(default)]
    pub(crate) preservation_digest: Option<String>,
    #[serde(default)]
    pub(crate) expected_preservation_digest: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QualificationInputs {
    #[serde(default)]
    pub(crate) immutable_candidate_ref: Option<String>,
    #[serde(default)]
    pub(crate) manifest_candidate_tree_sha: Option<String>,
    #[serde(default)]
    pub(crate) required_instruments_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CandidateState {
    pub(crate) status: String,
    pub(crate) selected_candidate_claims: u64,
    pub(crate) candidate_required_claims_pending: u64,
    pub(crate) candidate_claims_landed: u64,
    pub(crate) candidate_claims_excluded: u64,
    pub(crate) candidate_claims_deferred: u64,
    pub(crate) candidate_defects_unresolved: u64,
    #[serde(rename = "denominator_decisions_remaining")]
    pub(crate) denominator_decisions_remaining_through_provisional_cutoff: Option<u64>,
    pub(crate) denominator_decisions_remaining_through_selected_cut: Option<u64>,
    pub(crate) final_cut_authority_present: bool,
    pub(crate) final_cut_reviewed_through_selected_cut: bool,
    pub(crate) final_cut_decisions_remaining: Option<u64>,
    pub(crate) candidate_cut_selected: bool,
    pub(crate) candidate_ref_created: bool,
    pub(crate) projection_reproducible: bool,
    pub(crate) candidate_tree_present: bool,
    pub(crate) candidate_tree_parent_matches_cut: bool,
    pub(crate) exclusion_digests_match: bool,
    pub(crate) preservation_digests_match: bool,
    pub(crate) manifest_matches_candidate_tree: bool,
    pub(crate) qualification_instruments_available: bool,
    pub(crate) reasons: Vec<String>,
}

pub(crate) fn evaluate(selection: Option<&CandidateSelection>) -> CandidateState {
    let Some(selection) = selection else {
        return CandidateState {
            status: "scope_pending".to_string(),
            selected_candidate_claims: 0,
            candidate_required_claims_pending: 0,
            candidate_claims_landed: 0,
            candidate_claims_excluded: 0,
            candidate_claims_deferred: 0,
            candidate_defects_unresolved: 0,
            denominator_decisions_remaining_through_provisional_cutoff: None,
            denominator_decisions_remaining_through_selected_cut: None,
            final_cut_authority_present: false,
            final_cut_reviewed_through_selected_cut: false,
            final_cut_decisions_remaining: None,
            candidate_cut_selected: false,
            candidate_ref_created: false,
            projection_reproducible: false,
            candidate_tree_present: false,
            candidate_tree_parent_matches_cut: false,
            exclusion_digests_match: false,
            preservation_digests_match: false,
            manifest_matches_candidate_tree: false,
            qualification_instruments_available: false,
            reasons: vec!["candidate selection authority is missing".to_string()],
        };
    };

    let mut reasons = Vec::new();
    let mut claim_ids = BTreeSet::new();
    let mut selected_candidate_claims = 0;
    let mut candidate_required_claims_pending = 0;
    let mut candidate_claims_landed = 0;
    let mut candidate_claims_excluded = 0;
    let mut candidate_claims_deferred = 0;
    let mut scope_closed = true;

    if selection.schema_version != SELECTION_SCHEMA_VERSION {
        reasons.push(format!(
            "candidate selection schema_version must be {SELECTION_SCHEMA_VERSION}, got {}",
            selection.schema_version
        ));
        scope_closed = false;
    }
    if selection.selected_claims.is_empty() {
        reasons.push("selected candidate claim set is empty".to_string());
        scope_closed = false;
    }

    for claim in &selection.selected_claims {
        selected_candidate_claims += 1;
        let unique = claim_ids.insert(claim.claim_id.clone());
        if !unique || claim.claim_id.trim().is_empty() {
            reasons.push(format!(
                "selected claim identity is missing or duplicated: `{}`",
                claim.claim_id
            ));
            scope_closed = false;
        }
        if claim.owner_issue == 0 {
            reasons.push(format!(
                "selected claim `{}` has no owner issue",
                claim.claim_id
            ));
            scope_closed = false;
        }
        if !RESOLUTIONS.contains(&claim.resolution.as_str()) {
            reasons.push(format!(
                "selected claim `{}` has invalid resolution `{}`",
                claim.claim_id, claim.resolution
            ));
            scope_closed = false;
        }
        if claim.candidate_effect.trim().is_empty() || !claim.reviewed {
            reasons.push(format!(
                "selected claim `{}` lacks reviewed candidate effect",
                claim.claim_id
            ));
            scope_closed = false;
        }
        if matches!(
            claim.resolution.as_str(),
            "accepted_defer" | "candidate_exclusion"
        ) && claim
            .non_claim
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            reasons.push(format!(
                "selected claim `{}` lacks its release non-claim",
                claim.claim_id
            ));
            scope_closed = false;
        }
        if claim.resolution == "landed"
            && !references_are_non_blank(&[&claim.commit_refs, &claim.artifact_refs])
        {
            reasons.push(format!(
                "landed selected claim `{}` has no commit or artifact binding; a bare issue reference cannot establish delivery",
                claim.claim_id
            ));
            scope_closed = false;
        }
        match claim.resolution.as_str() {
            "landed" => candidate_claims_landed += 1,
            "accepted_defer" => candidate_claims_deferred += 1,
            "candidate_exclusion" => candidate_claims_excluded += 1,
            "pending" => {
                scope_closed = false;
                if claim.required_for_candidate {
                    candidate_required_claims_pending += 1;
                }
            }
            "failed" => {
                scope_closed = false;
                if claim.required_for_candidate {
                    candidate_required_claims_pending += 1;
                }
                reasons.push(format!(
                    "selected claim `{}` failed and must be resolved or accepted as a defer",
                    claim.claim_id
                ));
            }
            _ => {}
        }
    }

    let mut exclusion_ids = BTreeSet::new();
    for exclusion in &selection.candidate_exclusions {
        if !exclusion_ids.insert(exclusion.exclusion_id.clone())
            || exclusion.exclusion_id.trim().is_empty()
        {
            reasons.push(format!(
                "candidate exclusion identity is missing or duplicated: `{}`",
                exclusion.exclusion_id
            ));
            scope_closed = false;
        }
        if !claim_ids.contains(&exclusion.claim_id) {
            reasons.push(format!(
                "candidate exclusion `{}` references unknown claim `{}`",
                exclusion.exclusion_id, exclusion.claim_id
            ));
            scope_closed = false;
        }
        if exclusion.reason.trim().is_empty()
            || exclusion.non_claim.trim().is_empty()
            || !exclusion.reviewed
        {
            reasons.push(format!(
                "candidate exclusion `{}` lacks reviewed reason/non-claim evidence",
                exclusion.exclusion_id
            ));
            scope_closed = false;
        }
    }

    for claim in &selection.selected_claims {
        if claim.resolution == "candidate_exclusion"
            && !selection
                .candidate_exclusions
                .iter()
                .any(|exclusion| exclusion.claim_id == claim.claim_id)
        {
            reasons.push(format!(
                "candidate exclusion resolution for `{}` has no exclusion record",
                claim.claim_id
            ));
            scope_closed = false;
        }
    }

    let candidate_defects_unresolved = selection
        .known_candidate_defects
        .iter()
        .filter(|defect| !defect.resolved)
        .count() as u64;
    let candidate_cut_selected = selection.selected_cut_sha.as_deref().is_some_and(is_sha);
    if selection.selected_cut_sha.is_some() && !candidate_cut_selected {
        reasons.push("selected cut SHA is not a 40-character hexadecimal SHA".to_string());
    }
    let projection_reproducible = selection.projection.reproducible;
    let candidate_tree_present = selection
        .projection
        .candidate_tree_sha
        .as_deref()
        .is_some_and(is_sha);
    if selection.projection.candidate_tree_sha.is_some() && !candidate_tree_present {
        reasons.push("candidate tree SHA is not a 40-character hexadecimal SHA".to_string());
    }
    if selection
        .projection
        .candidate_tree_parent_sha
        .as_deref()
        .is_some_and(|value| !is_sha(value))
    {
        reasons.push("candidate tree parent SHA is not a 40-character hexadecimal SHA".to_string());
    }
    let candidate_tree_parent_matches_cut = selection
        .projection
        .candidate_tree_parent_sha
        .as_deref()
        .zip(selection.selected_cut_sha.as_deref())
        .is_some_and(|(parent, cut)| parent == cut);
    let exclusion_digests_match = matching_non_blank_digests(
        selection.projection.exclusion_digest.as_deref(),
        selection.projection.expected_exclusion_digest.as_deref(),
    );
    let preservation_digests_match = matching_non_blank_digests(
        selection.projection.preservation_digest.as_deref(),
        selection.projection.expected_preservation_digest.as_deref(),
    );
    let candidate_materialized = projection_reproducible
        && candidate_tree_present
        && candidate_tree_parent_matches_cut
        && exclusion_digests_match
        && preservation_digests_match;
    let candidate_ref_created = selection
        .qualification
        .immutable_candidate_ref
        .as_deref()
        .is_some_and(is_immutable_candidate_ref);
    let manifest_matches_candidate_tree = selection
        .qualification
        .manifest_candidate_tree_sha
        .as_deref()
        .zip(selection.projection.candidate_tree_sha.as_deref())
        .is_some_and(|(manifest, tree)| manifest == tree);
    let qualification_instruments_available =
        selection.qualification.required_instruments_available;
    let final_cut_authority_present = selection.final_cut_authority.is_some();
    let final_cut_reviewed_through_selected_cut = selection
        .final_cut_authority
        .as_ref()
        .is_some_and(|authority| authority.reviewed_through_selected_cut);
    let final_cut_decisions_remaining = selection
        .final_cut_authority
        .as_ref()
        .map(|authority| authority.final_cut_decisions_remaining);
    let final_cut_authority_valid =
        selection
            .final_cut_authority
            .as_ref()
            .is_some_and(|authority| {
                Some(authority.cut_sha.as_str()) == selection.selected_cut_sha.as_deref()
                    && authority.provisional_decisions_remaining == 0
                    && authority.unreviewed_post_provisional_records_through_cut == 0
                    && authority.final_cut_decisions_remaining == 0
                    && authority.reviewed_through_selected_cut
                    && authority
                        .record_set_digest
                        .as_deref()
                        .is_some_and(is_record_set_digest)
            });

    let hard_cut_blocked = candidate_required_claims_pending > 0
        || candidate_defects_unresolved > 0
        || selection.denominator_decisions_remaining_through_provisional_cutoff != Some(0)
        || selection.denominator_decisions_remaining_through_selected_cut != Some(0)
        || !final_cut_authority_valid
        || !candidate_cut_selected
        || !projection_reproducible;
    let status = if !scope_closed {
        "scope_pending"
    } else if hard_cut_blocked {
        "scope_closed"
    } else if !candidate_materialized {
        "hard_cut_eligible"
    } else if !candidate_ref_created
        || !manifest_matches_candidate_tree
        || !qualification_instruments_available
    {
        "candidate_materialized"
    } else {
        "qualification_eligible"
    };

    if candidate_required_claims_pending > 0 {
        reasons.push("required selected claims remain pending or failed".to_string());
    }
    if candidate_defects_unresolved > 0 {
        reasons.push("known candidate defects remain unresolved".to_string());
    }
    if selection.denominator_decisions_remaining_through_provisional_cutoff != Some(0) {
        reasons
            .push("denominator decisions remain through the provisional review cutoff".to_string());
    }
    if candidate_cut_selected
        && selection.denominator_decisions_remaining_through_selected_cut != Some(0)
    {
        reasons.push("denominator decisions remain through the selected cut".to_string());
    }
    if !final_cut_authority_valid {
        reasons.push(
            "final-cut authority is missing, unbound, unreviewed, or retains decisions".to_string(),
        );
    }
    if !candidate_cut_selected {
        reasons.push("development cut C is not selected".to_string());
    }
    if !projection_reproducible {
        reasons.push("candidate projection from C plus exclusions is not reproducible".to_string());
    }
    if status == "hard_cut_eligible" {
        reasons.push("candidate tree T has not been materialized".to_string());
    }
    if status == "candidate_materialized" && !candidate_ref_created {
        reasons.push("immutable candidate ref has not been created".to_string());
    }
    if status == "candidate_materialized" && !manifest_matches_candidate_tree {
        reasons.push("candidate manifest does not identify the materialized tree".to_string());
    }
    if status == "candidate_materialized" && !qualification_instruments_available {
        reasons.push("required qualification instruments are unavailable".to_string());
    }

    CandidateState {
        status: status.to_string(),
        selected_candidate_claims,
        candidate_required_claims_pending,
        candidate_claims_landed,
        candidate_claims_excluded,
        candidate_claims_deferred,
        candidate_defects_unresolved,
        denominator_decisions_remaining_through_provisional_cutoff: selection
            .denominator_decisions_remaining_through_provisional_cutoff,
        denominator_decisions_remaining_through_selected_cut: selection
            .denominator_decisions_remaining_through_selected_cut,
        final_cut_authority_present,
        final_cut_reviewed_through_selected_cut,
        final_cut_decisions_remaining,
        candidate_cut_selected,
        candidate_ref_created,
        projection_reproducible,
        candidate_tree_present,
        candidate_tree_parent_matches_cut,
        exclusion_digests_match,
        preservation_digests_match,
        manifest_matches_candidate_tree,
        qualification_instruments_available,
        reasons,
    }
}

fn is_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn is_record_set_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

fn references_are_non_blank(references: &[&[String]]) -> bool {
    references.iter().any(|group| !group.is_empty())
        && references
            .iter()
            .all(|group| group.iter().all(|reference| !reference.trim().is_empty()))
}

fn matching_non_blank_digests(left: Option<&str>, right: Option<&str>) -> bool {
    left.zip(right)
        .is_some_and(|(left, right)| !left.trim().is_empty() && left == right)
}

fn is_immutable_candidate_ref(value: &str) -> bool {
    let Some(suffix) = value.trim().strip_prefix("refs/ripr/candidate-") else {
        return false;
    };
    !suffix.is_empty()
        && suffix.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

#[cfg(test)]
mod tests {
    use super::{CandidateExclusion, CandidateSelection, KnownCandidateDefect, evaluate};
    use serde::Deserialize;
    use serde_json::json;

    fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
        condition.then_some(()).ok_or(message.into())
    }

    fn selection() -> Result<CandidateSelection, String> {
        serde_json::from_value(json!({
            "schema_version": "0.1",
            "selected_cut_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "selected_claims": [{
                "claim_id": "claim:lifecycle",
                "owner_issue": 2822,
                "required_for_candidate": true,
                "resolution": "landed",
                "evidence_refs": ["https://github.com/EffortlessMetrics/ripr-swarm/issues/2822"],
                "commit_refs": ["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],
                "artifact_refs": [],
                "candidate_effect": "include",
                "non_claim": null,
                "reviewed": true
            }],
            "candidate_exclusions": [],
            "known_candidate_defects": [],
            "denominator_decisions_remaining_through_provisional_cutoff": 0,
            "denominator_decisions_remaining_through_selected_cut": 0,
            "final_cut_authority": {
                "cut_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "record_set_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "provisional_decisions_remaining": 0,
                "unreviewed_post_provisional_records_through_cut": 0,
                "final_cut_decisions_remaining": 0,
                "reviewed_through_selected_cut": true
            },
            "projection": {
                "reproducible": true,
                "candidate_tree_sha": "cccccccccccccccccccccccccccccccccccccccc",
                "candidate_tree_parent_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "exclusion_digest": "sha256:exclusions",
                "expected_exclusion_digest": "sha256:exclusions",
                "preservation_digest": "sha256:preservation",
                "expected_preservation_digest": "sha256:preservation"
            },
            "qualification": {
                "immutable_candidate_ref": "refs/ripr/candidate-0.11",
                "manifest_candidate_tree_sha": "cccccccccccccccccccccccccccccccccccccccc",
                "required_instruments_available": true
            }
        }))
        .map_err(|error| format!("failed to build candidate selection: {error}"))
    }

    #[test]
    fn schema_01_preserves_legacy_denominator_field_on_serialization() -> Result<(), String> {
        let value = serde_json::to_value(selection()?).map_err(|error| error.to_string())?;
        if value
            .get("denominator_decisions_remaining")
            .and_then(serde_json::Value::as_u64)
            != Some(0)
        {
            return Err("schema 0.1 legacy denominator field was not retained".to_string());
        }
        if value
            .get("denominator_decisions_remaining_through_provisional_cutoff")
            .is_some()
        {
            return Err("schema 0.1 emitted the post-migration denominator field".to_string());
        }
        Ok(())
    }

    #[test]
    fn final_cut_authority_rejects_unreviewed_post_cutoff_records() -> Result<(), String> {
        let mut candidate = selection()?;
        candidate
            .final_cut_authority
            .as_mut()
            .ok_or_else(|| "selection fixture lost final-cut authority".to_string())?
            .unreviewed_post_provisional_records_through_cut = 1;
        let state = evaluate(Some(&candidate));
        if state.status != "scope_closed"
            || !state
                .reasons
                .iter()
                .any(|reason| reason.contains("final-cut authority"))
        {
            return Err("unreviewed post-cutoff record was treated as hard-cut ready".to_string());
        }
        Ok(())
    }

    #[test]
    fn final_cut_authority_requires_record_set_provenance() -> Result<(), String> {
        let mut candidate = selection()?;
        candidate
            .final_cut_authority
            .as_mut()
            .ok_or_else(|| "selection fixture lost final-cut authority".to_string())?
            .record_set_digest = None;
        let state = evaluate(Some(&candidate));
        require(
            state.status == "scope_closed"
                && state
                    .reasons
                    .iter()
                    .any(|reason| reason.contains("final-cut authority")),
            "final-cut authority without record-set provenance was accepted",
        )
    }

    #[test]
    fn missing_denominator_counts_fail_closed() -> Result<(), String> {
        let mut candidate = selection()?;
        candidate.denominator_decisions_remaining_through_provisional_cutoff = None;
        candidate.denominator_decisions_remaining_through_selected_cut = None;
        let state = evaluate(Some(&candidate));
        require(
            state.status == "scope_closed"
                && state.reasons.iter().any(|reason| {
                    reason.contains("denominator decisions remain through the provisional")
                }),
            "missing denominator counts did not block the hard cut",
        )
    }

    #[test]
    fn staged_state_advances_only_after_each_boundary() -> Result<(), String> {
        let mut value = selection()?;
        let ready = evaluate(Some(&value));
        if ready.status != "qualification_eligible" || !ready.reasons.is_empty() {
            return Err(format!(
                "complete selection should be qualification_eligible with no reasons, got {} / {:?}",
                ready.status, ready.reasons
            ));
        }
        value.qualification = Default::default();
        let state = evaluate(Some(&value));
        if state.status != "candidate_materialized" {
            return Err(format!(
                "expected candidate_materialized, got {}",
                state.status
            ));
        }
        value.projection.candidate_tree_sha = None;
        let state = evaluate(Some(&value));
        if state.status != "hard_cut_eligible" {
            return Err(format!("expected hard_cut_eligible, got {}", state.status));
        }
        value.denominator_decisions_remaining_through_provisional_cutoff = Some(1);
        let state = evaluate(Some(&value));
        if state.status != "scope_closed" {
            return Err(format!("expected scope_closed, got {}", state.status));
        }
        Ok(())
    }

    #[test]
    fn missing_candidate_selection_is_not_ready() -> Result<(), String> {
        let state = evaluate(None);
        if state.status != "scope_pending" || state.candidate_cut_selected {
            return Err("missing selection must remain scope_pending".to_string());
        }
        Ok(())
    }

    #[test]
    fn unresolved_claim_is_not_hard_cut_eligible() -> Result<(), String> {
        let mut value = selection()?;
        value.selected_claims[0].resolution = "pending".to_string();
        let state = evaluate(Some(&value));
        if state.status != "scope_pending" || state.candidate_required_claims_pending != 1 {
            return Err("pending required claim did not block hard cut".to_string());
        }
        Ok(())
    }

    #[test]
    fn unresolved_defect_is_not_hard_cut_eligible() -> Result<(), String> {
        let mut value = selection()?;
        value
            .known_candidate_defects
            .push(super::KnownCandidateDefect {
                defect_id: "defect:one".to_string(),
                description: "known candidate defect".to_string(),
                resolved: false,
                evidence_refs: vec!["issue:1".to_string()],
            });
        let state = evaluate(Some(&value));
        if state.status != "scope_closed" || state.candidate_defects_unresolved != 1 {
            return Err("unresolved defect did not block hard cut".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_cut_and_projection_are_not_hard_cut_eligible() -> Result<(), String> {
        let mut value = selection()?;
        value.selected_cut_sha = None;
        value.projection.reproducible = false;
        let state = evaluate(Some(&value));
        if state.status != "scope_closed"
            || state.candidate_cut_selected
            || state.projection_reproducible
        {
            return Err("missing cut or projection incorrectly passed hard cut".to_string());
        }
        Ok(())
    }

    #[test]
    fn mismatched_materialization_and_manifest_do_not_qualify() -> Result<(), String> {
        let mut value = selection()?;
        value.projection.candidate_tree_parent_sha =
            Some("dddddddddddddddddddddddddddddddddddddddd".to_string());
        let state = evaluate(Some(&value));
        if state.status != "hard_cut_eligible" {
            return Err("mismatched tree parent should stop before materialization".to_string());
        }
        value.projection.candidate_tree_parent_sha = value.selected_cut_sha.clone();
        value.qualification.manifest_candidate_tree_sha =
            Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string());
        let state = evaluate(Some(&value));
        if state.status != "candidate_materialized" || state.manifest_matches_candidate_tree {
            return Err("mismatched manifest should not qualify".to_string());
        }
        Ok(())
    }

    #[test]
    fn blank_landed_reference_is_not_scope_closed() -> Result<(), String> {
        let mut value = selection()?;
        value.selected_claims[0].evidence_refs = vec![" ".to_string()];
        value.selected_claims[0].commit_refs.clear();
        let state = evaluate(Some(&value));
        if state.status != "scope_pending" {
            return Err(format!(
                "blank landed reference should remain scope_pending, got {}",
                state.status
            ));
        }
        Ok(())
    }

    #[test]
    fn landed_claim_with_only_an_issue_reference_is_not_scope_closed() -> Result<(), String> {
        let mut value = selection()?;
        value.selected_claims[0].commit_refs.clear();
        value.selected_claims[0].artifact_refs.clear();
        let state = evaluate(Some(&value));
        if state.status != "scope_pending"
            || !state
                .reasons
                .iter()
                .any(|reason| reason.contains("no commit or artifact binding"))
        {
            return Err(format!(
                "issue-only landed claim should remain scope_pending with a binding reason, got {}: {:?}",
                state.status, state.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn landed_claim_with_an_artifact_binding_alone_closes_scope() -> Result<(), String> {
        let mut value = selection()?;
        value.selected_claims[0].commit_refs.clear();
        value.selected_claims[0].artifact_refs =
            vec!["target/ripr/reports/release-readiness.json".to_string()];
        let bound = evaluate(Some(&value));
        let mut baseline = selection()?;
        baseline.selected_claims[0].artifact_refs =
            vec!["target/ripr/reports/release-readiness.json".to_string()];
        let baseline = evaluate(Some(&baseline));
        if bound.status == "scope_pending" || bound.status != baseline.status {
            return Err(format!(
                "artifact-bound landed claim should close scope like the commit-bound baseline, got {} vs {}",
                bound.status, baseline.status
            ));
        }
        Ok(())
    }

    #[test]
    fn whitespace_non_claim_is_not_scope_closed() -> Result<(), String> {
        let mut value = selection()?;
        value.selected_claims[0].resolution = "accepted_defer".to_string();
        value.selected_claims[0].candidate_effect = "exclude".to_string();
        value.selected_claims[0].non_claim = Some(" \t".to_string());
        let state = evaluate(Some(&value));
        if state.status != "scope_pending" {
            return Err(format!(
                "whitespace-only non-claim should remain scope_pending, got {}",
                state.status
            ));
        }
        Ok(())
    }

    #[test]
    fn optional_failed_claim_is_not_scope_closed() -> Result<(), String> {
        let mut value = selection()?;
        value.selected_claims[0].required_for_candidate = false;
        value.selected_claims[0].resolution = "failed".to_string();
        let state = evaluate(Some(&value));
        if state.status != "scope_pending" || state.candidate_required_claims_pending != 0 {
            return Err(format!(
                "optional failed claim should remain scope_pending without required pending count, got {} / {}",
                state.status, state.candidate_required_claims_pending
            ));
        }
        Ok(())
    }

    #[test]
    fn blank_projection_digests_do_not_materialize() -> Result<(), String> {
        let mut value = selection()?;
        value.projection.exclusion_digest = Some(String::new());
        value.projection.expected_exclusion_digest = Some(String::new());
        let state = evaluate(Some(&value));
        if state.status != "hard_cut_eligible" || state.exclusion_digests_match {
            return Err(format!(
                "blank matching digests should not materialize, got {}",
                state.status
            ));
        }
        Ok(())
    }

    #[test]
    fn mutable_candidate_ref_does_not_qualify() -> Result<(), String> {
        let mut value = selection()?;
        value.qualification.immutable_candidate_ref = Some("refs/heads/main".to_string());
        let state = evaluate(Some(&value));
        if state.status != "candidate_materialized" || state.candidate_ref_created {
            return Err(format!(
                "mutable candidate ref should not qualify, got {}",
                state.status
            ));
        }
        Ok(())
    }

    const NEGATIVE_FIXTURE_VERSION: &str = "candidate_state_negative.v1";

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct NegativeFixture {
        schema_version: String,
        base_selection: CandidateSelection,
        cases: Vec<NegativeCase>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct NegativeCase {
        name: String,
        expected_status: String,
        #[serde(default)]
        selected_claim_resolution: Option<String>,
        #[serde(default)]
        selected_claim_required_for_candidate: Option<bool>,
        #[serde(default)]
        selected_claim_non_claim: Option<String>,
        #[serde(default)]
        selected_claim_evidence_refs: Option<Vec<String>>,
        #[serde(default)]
        clear_landed_binding: bool,
        #[serde(default)]
        unresolved_defect: bool,
        #[serde(default)]
        #[serde(alias = "denominator_decisions_remaining")]
        denominator_decisions_remaining_through_provisional_cutoff: Option<u64>,
        #[serde(default)]
        denominator_decisions_remaining_through_selected_cut: Option<u64>,
        #[serde(default)]
        clear_selected_cut: bool,
        #[serde(default)]
        projection_reproducible: Option<bool>,
        #[serde(default)]
        clear_candidate_tree: bool,
        #[serde(default)]
        clear_immutable_ref: bool,
        #[serde(default)]
        immutable_candidate_ref: Option<String>,
        #[serde(default)]
        blank_exclusion_digest: bool,
        #[serde(default)]
        blank_preservation_digest: bool,
        #[serde(default)]
        manifest_tree_sha: Option<String>,
        #[serde(default)]
        exclusion_mutation: Option<String>,
    }

    #[test]
    fn negative_candidate_state_fixture_covers_each_false_ready_boundary() -> Result<(), String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("release_control")
            .join("candidate-state-negative.json");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let fixture: NegativeFixture = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        if fixture.schema_version != NEGATIVE_FIXTURE_VERSION {
            return Err(format!(
                "fixture schema_version must be {NEGATIVE_FIXTURE_VERSION}, got {}",
                fixture.schema_version
            ));
        }
        if fixture.base_selection.selected_claims.is_empty() {
            return Err(
                "negative fixture base selection must contain a selected claim".to_string(),
            );
        }
        if fixture.cases.is_empty() {
            return Err("negative fixture must contain at least one case".to_string());
        }
        for case in fixture.cases {
            let mut selection = fixture.base_selection.clone();
            if let Some(resolution) = case.selected_claim_resolution {
                selection.selected_claims[0].resolution = resolution;
            }
            if let Some(required) = case.selected_claim_required_for_candidate {
                selection.selected_claims[0].required_for_candidate = required;
            }
            if let Some(non_claim) = case.selected_claim_non_claim {
                selection.selected_claims[0].non_claim = Some(non_claim);
            }
            if let Some(evidence_refs) = case.selected_claim_evidence_refs {
                selection.selected_claims[0].evidence_refs = evidence_refs;
            }
            if case.clear_landed_binding {
                selection.selected_claims[0].commit_refs.clear();
                selection.selected_claims[0].artifact_refs.clear();
            }
            if case.unresolved_defect {
                selection
                    .known_candidate_defects
                    .push(KnownCandidateDefect {
                        defect_id: format!("defect:{}", case.name),
                        description: case.name.clone(),
                        resolved: false,
                        evidence_refs: vec!["fixture:negative".to_string()],
                    });
            }
            if case
                .denominator_decisions_remaining_through_provisional_cutoff
                .is_some()
            {
                selection.denominator_decisions_remaining_through_provisional_cutoff =
                    case.denominator_decisions_remaining_through_provisional_cutoff;
            }
            if case
                .denominator_decisions_remaining_through_selected_cut
                .is_some()
            {
                selection.denominator_decisions_remaining_through_selected_cut =
                    case.denominator_decisions_remaining_through_selected_cut;
            }
            if case.clear_selected_cut {
                selection.selected_cut_sha = None;
            }
            if let Some(reproducible) = case.projection_reproducible {
                selection.projection.reproducible = reproducible;
            }
            if case.clear_candidate_tree {
                selection.projection.candidate_tree_sha = None;
            }
            if case.clear_immutable_ref {
                selection.qualification.immutable_candidate_ref = None;
            }
            if let Some(candidate_ref) = case.immutable_candidate_ref {
                selection.qualification.immutable_candidate_ref = Some(candidate_ref);
            }
            if case.blank_exclusion_digest {
                selection.projection.exclusion_digest = Some(String::new());
                selection.projection.expected_exclusion_digest = Some(String::new());
            }
            if case.blank_preservation_digest {
                selection.projection.preservation_digest = Some(String::new());
                selection.projection.expected_preservation_digest = Some(String::new());
            }
            if let Some(tree_sha) = case.manifest_tree_sha {
                selection.qualification.manifest_candidate_tree_sha = Some(tree_sha);
            }
            match case.exclusion_mutation.as_deref() {
                Some("blank_id") => selection.candidate_exclusions.push(CandidateExclusion {
                    exclusion_id: " ".to_string(),
                    claim_id: "claim:lifecycle".to_string(),
                    reason: "reviewed reason".to_string(),
                    non_claim: "reviewed non-claim".to_string(),
                    evidence_refs: vec!["fixture:negative".to_string()],
                    reviewed: true,
                }),
                Some("duplicate_id") => {
                    let exclusion = CandidateExclusion {
                        exclusion_id: "exclusion:duplicate".to_string(),
                        claim_id: "claim:lifecycle".to_string(),
                        reason: "reviewed reason".to_string(),
                        non_claim: "reviewed non-claim".to_string(),
                        evidence_refs: vec!["fixture:negative".to_string()],
                        reviewed: true,
                    };
                    selection.candidate_exclusions.push(exclusion.clone());
                    selection.candidate_exclusions.push(exclusion);
                }
                Some("unknown_claim") => selection.candidate_exclusions.push(CandidateExclusion {
                    exclusion_id: "exclusion:unknown".to_string(),
                    claim_id: "claim:unknown".to_string(),
                    reason: "reviewed reason".to_string(),
                    non_claim: "reviewed non-claim".to_string(),
                    evidence_refs: vec!["fixture:negative".to_string()],
                    reviewed: true,
                }),
                Some("blank_reason") | Some("blank_non_claim") | Some("unreviewed") => {
                    selection.candidate_exclusions.push(CandidateExclusion {
                        exclusion_id: "exclusion:invalid".to_string(),
                        claim_id: "claim:lifecycle".to_string(),
                        reason: if case.exclusion_mutation.as_deref() == Some("blank_reason") {
                            " ".to_string()
                        } else {
                            "reviewed reason".to_string()
                        },
                        non_claim: if case.exclusion_mutation.as_deref() == Some("blank_non_claim")
                        {
                            "\t".to_string()
                        } else {
                            "reviewed non-claim".to_string()
                        },
                        evidence_refs: vec!["fixture:negative".to_string()],
                        reviewed: case.exclusion_mutation.as_deref() != Some("unreviewed"),
                    });
                }
                Some("candidate_resolution_without_record") => {
                    selection.selected_claims[0].resolution = "candidate_exclusion".to_string();
                }
                Some(other) => return Err(format!("unknown exclusion mutation `{other}`")),
                None => {}
            }
            let state = evaluate(Some(&selection));
            if state.status != case.expected_status {
                return Err(format!(
                    "case `{}` expected {}, got {}",
                    case.name, case.expected_status, state.status
                ));
            }
        }
        Ok(())
    }
}
