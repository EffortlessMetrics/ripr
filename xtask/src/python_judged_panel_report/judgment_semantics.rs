//! The semantic contract one judgment must satisfy (RIPR-SPEC-0092):
//! enforced on the CLI request and re-checked per stored judgment at report
//! time, so a stored record can never carry what the CLI would reject.

use crate::python_judged_panel::{
    KNOWN_CLASSIFICATIONS, KNOWN_LIMITATION_QUALITIES, direction_admits_error,
};

use super::REVIEWER_ENV;

/// The semantic contract one judgment must satisfy. Enforced on the CLI
/// request (`validate_request`) and re-checked per stored judgment at report
/// time, so a stored record can never carry what the CLI would reject
/// (FIX f2XZU) and carryover rows (null expected_classification,
/// robustness-only) can never be adjudicated at all (FIX f2TIz).
pub(super) struct JudgmentSemantics<'a> {
    pub(super) role: &'a str,
    pub(super) identity: &'a str,
    pub(super) verdict: &'a str,
    pub(super) false_actionable: Option<bool>,
    pub(super) false_exposed: Option<bool>,
    pub(super) evidence_references: &'a [String],
    pub(super) limitation_quality: Option<&'a str>,
    pub(super) direction: &'a str,
    pub(super) carryover: bool,
}

pub(super) fn validate_judgment_semantics(judgment: JudgmentSemantics<'_>) -> Result<(), String> {
    if judgment.carryover {
        return Err("carryover rows (null expected_classification, robustness-only) cannot be adjudicated; select a current case instead".to_string());
    }
    if !KNOWN_CLASSIFICATIONS.contains(&judgment.verdict) {
        return Err(format!(
            "verdict `{}` is not in the conservative static vocabulary (one of {})",
            judgment.verdict,
            KNOWN_CLASSIFICATIONS.join(", ")
        ));
    }
    if judgment.role.trim().is_empty() {
        return Err(
            "role is required and must not be blank; the two-distinct-recorded-roles rule keys on it"
                .to_string(),
        );
    }
    if judgment.identity.trim().is_empty() {
        return Err(format!(
            "reviewer identity is required: pass --reviewer <identity> or set env {REVIEWER_ENV}"
        ));
    }
    // FIX fA8C (devin #3682): `" alice"` and `"alice"` are different people
    // only to a string comparison — padding must never be able to borrow a
    // clean identity's record or dodge its independence conflict.
    if judgment.role.trim() != judgment.role || judgment.identity.trim() != judgment.identity {
        return Err(
            "reviewer role and identity must not carry leading or trailing whitespace".to_string(),
        );
    }
    if judgment.evidence_references.is_empty()
        || judgment
            .evidence_references
            .iter()
            .any(|reference| reference.trim().is_empty())
    {
        return Err("evidence is required at least once with a non-blank reference; a judgment without the adjudicator's own evidence citations is not an independent judgment (do not copy the cited RIPR candidate classification as ground truth)".to_string());
    }
    if matches!(judgment.false_actionable, Some(true))
        && matches!(judgment.false_exposed, Some(true))
    {
        return Err(
            "false_actionable and false_exposed cannot both be true for one terminal adjudication"
                .to_string(),
        );
    }
    for (label, value) in [
        ("false_actionable", judgment.false_actionable),
        ("false_exposed", judgment.false_exposed),
    ] {
        if matches!(value, Some(true)) && !direction_admits_error(judgment.direction, label) {
            return Err(format!(
                "{label} true is not admitted by direction `{}` per the SPEC-0092 outcome table",
                judgment.direction
            ));
        }
    }
    // FIX f2TIA: verdict-to-error coherence mirrors the retained-panel
    // validator's outcome table: crediting `exposed` on a row whose direction
    // expects a gap or a fail-closed limitation is an over-credit by
    // definition and must be recorded as false_exposed true.
    if judgment.direction != "should_stay_quiet"
        && judgment.verdict == "exposed"
        && judgment.false_exposed != Some(true)
    {
        return Err(format!(
            "verdict `exposed` on a `{}` row is an over-credit and requires false_exposed true",
            judgment.direction
        ));
    }
    if let Some(quality) = judgment.limitation_quality {
        if judgment.direction != "should_limit" {
            return Err(format!(
                "limitation-quality applies only to `should_limit` rows, found `{}`",
                judgment.direction
            ));
        }
        if !KNOWN_LIMITATION_QUALITIES.contains(&quality) {
            return Err(format!(
                "limitation-quality `{quality}` is not one of {}",
                KNOWN_LIMITATION_QUALITIES.join(", ")
            ));
        }
    }
    Ok(())
}
