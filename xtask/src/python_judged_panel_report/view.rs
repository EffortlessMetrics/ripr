//! Adjudication views: the two-independent-roles derivation over one stored
//! record, projected onto the validated row (RIPR-SPEC-0092).

use std::collections::BTreeSet;

use crate::python_judged_panel::{PythonJudgedPanelItem, direction_admits_error};

use super::adjudication::{AdjudicationJudgment, AdjudicationRecord};

// ---------------------------------------------------------------------------
// Adjudication views: the two-independent-roles derivation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum AdjudicationState {
    /// Fewer than two independent roles have recorded a judgment.
    PendingSecondRole,
    /// Two or more independent roles disagree on the terminal judgment.
    Disputed,
    /// Independent roles agree, but no direction-admitted error axis is
    /// decided — a real recorded state, never a pass.
    Inconclusive,
    /// Two or more independent roles agree with at least one admitted error
    /// axis decided.
    Adjudicated,
    /// The record's echoed row identity no longer matches the validated row;
    /// excluded from every adjudicated count and disclosed per case.
    StaleRow,
}

impl AdjudicationState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::PendingSecondRole => "pending_second_role",
            Self::Disputed => "disputed",
            Self::Inconclusive => "inconclusive",
            Self::Adjudicated => "adjudicated",
            Self::StaleRow => "stale_row",
        }
    }
}

/// One judgment axis reduced across the record's roles.
pub(super) enum AxisValue<T> {
    /// Fewer than two independent roles — no axis decision exists yet.
    NotIndependent,
    /// The independent roles disagree on this axis.
    Disputed,
    /// The independent roles agree; `None` is an explicit undecided.
    Agreed(Option<T>),
}

impl<T: PartialEq> AxisValue<T> {
    fn from_judgments(
        judgments: &[AdjudicationJudgment],
        read: impl Fn(&AdjudicationJudgment) -> Option<T>,
    ) -> Self {
        if judgments.len() < 2 {
            return Self::NotIndependent;
        }
        let mut agreed: Option<Option<T>> = None;
        for judgment in judgments {
            let value = read(judgment);
            match &agreed {
                None => agreed = Some(value),
                Some(previous) if *previous != value => return Self::Disputed,
                Some(_) => {}
            }
        }
        match agreed {
            Some(agreed) => Self::Agreed(agreed),
            None => Self::NotIndependent,
        }
    }

    pub(super) fn agreed_decided(&self) -> Option<T>
    where
        T: Clone,
    {
        match self {
            Self::Agreed(Some(value)) => Some(value.clone()),
            _ => None,
        }
    }
}

pub(super) fn bool_token(axis: &AxisValue<bool>) -> &'static str {
    match axis {
        AxisValue::Agreed(Some(true)) => "true",
        AxisValue::Agreed(Some(false)) => "false",
        AxisValue::Agreed(None) => "undecided",
        AxisValue::Disputed => "disputed",
        AxisValue::NotIndependent => "not_independent",
    }
}

pub(super) struct AdjudicationView {
    pub(super) state: AdjudicationState,
    pub(super) roles: Vec<String>,
    pub(super) verdict_agreed: Option<String>,
    pub(super) false_actionable: AxisValue<bool>,
    pub(super) false_exposed: AxisValue<bool>,
    pub(super) wrong_target: AxisValue<bool>,
    pub(super) invalid_command: AxisValue<bool>,
    pub(super) limitation_quality: AxisValue<String>,
    /// FIX f2XZN: the row revision the record was bound to at adjudication
    /// time, disclosed against the current revision on drift.
    pub(super) row_revision_stored: String,
    pub(super) row_revision_current: String,
}

/// Projects one adjudication record onto the validated row: the
/// two-distinct-recorded-roles rule, the terminal lattice agreement, and the
/// stale-row drift check against the current row revision. Independence is
/// approximated by distinct recorded roles/identities — it is self-claimed,
/// not mechanically verified. The terminal SPEC-0092 lattice is disputed
/// exactly when the verdict or either error axis disagrees across roles.
pub(super) fn derive_adjudication_view(
    record: &AdjudicationRecord,
    item: &PythonJudgedPanelItem,
    row_revision: &str,
) -> AdjudicationView {
    // FIX f2XZN: the bound row-revision digest subsumes the earlier
    // must_not_claim/envelope/direction echo checks — any row or diff change
    // makes the stored digest stale.
    let stale_row = record.row_revision_sha256 != row_revision;
    let judgments = &record.judgments;
    let roles = judgments
        .iter()
        .map(|judgment| judgment.reviewer_role.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let identities = judgments
        .iter()
        .map(|judgment| judgment.reviewer_identity.clone())
        .collect::<BTreeSet<_>>();
    let independent = roles.len() >= 2 && identities.len() >= 2;
    let verdict = AxisValue::from_judgments(judgments, |judgment| Some(judgment.verdict.clone()));
    let false_actionable =
        AxisValue::from_judgments(judgments, |judgment| judgment.false_actionable);
    let false_exposed = AxisValue::from_judgments(judgments, |judgment| judgment.false_exposed);
    let disputed = matches!(
        (&verdict, &false_actionable, &false_exposed),
        (_, AxisValue::Disputed, _) | (AxisValue::Disputed, ..) | (_, _, AxisValue::Disputed)
    );
    let lattice_agrees = independent && !disputed;

    let state = if stale_row {
        AdjudicationState::StaleRow
    } else if !independent {
        AdjudicationState::PendingSecondRole
    } else if disputed {
        AdjudicationState::Disputed
    } else {
        let decided = usize::from(
            direction_admits_error(&item.expected_direction, "false_actionable")
                && false_actionable.agreed_decided().is_some(),
        ) + usize::from(
            direction_admits_error(&item.expected_direction, "false_exposed")
                && false_exposed.agreed_decided().is_some(),
        );
        if decided == 0 {
            AdjudicationState::Inconclusive
        } else {
            AdjudicationState::Adjudicated
        }
    };

    AdjudicationView {
        state,
        verdict_agreed: if lattice_agrees {
            verdict.agreed_decided()
        } else {
            None
        },
        roles,
        false_actionable,
        false_exposed,
        wrong_target: AxisValue::from_judgments(judgments, |judgment| judgment.wrong_target),
        invalid_command: AxisValue::from_judgments(judgments, |judgment| judgment.invalid_command),
        limitation_quality: AxisValue::from_judgments(judgments, |judgment| {
            judgment.limitation_quality.clone()
        }),
        row_revision_stored: record.row_revision_sha256.clone(),
        row_revision_current: row_revision.to_string(),
    }
}
