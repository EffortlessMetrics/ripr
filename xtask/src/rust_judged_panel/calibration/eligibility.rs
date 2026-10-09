//! Calibration eligibility for independently judged Rust panel rows (#4795).
//!
//! Eligibility is a static/authorization fact. It does not run a mutant and
//! cannot rewrite a structural judgment.

use super::{JudgedCase, SCOPE_AUTHORIZED};

pub(super) const ELIGIBLE: &str = "eligible";
pub(super) const INELIGIBLE_UNJUDGED: &str = "ineligible_unjudged";
pub(super) const INELIGIBLE_UNAUTHORIZED: &str = "ineligible_unauthorized";
pub(super) const INELIGIBLE_NO_FOCUSED_MUTANT: &str = "ineligible_no_focused_mutant";

#[cfg(test)]
pub(super) const ELIGIBILITY: [&str; 4] = [
    ELIGIBLE,
    INELIGIBLE_UNJUDGED,
    INELIGIBLE_UNAUTHORIZED,
    INELIGIBLE_NO_FOCUSED_MUTANT,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Eligibility {
    Eligible,
    IneligibleUnjudged,
    IneligibleUnauthorized,
    IneligibleNoFocusedMutant,
}

impl Eligibility {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Eligible => ELIGIBLE,
            Self::IneligibleUnjudged => INELIGIBLE_UNJUDGED,
            Self::IneligibleUnauthorized => INELIGIBLE_UNAUTHORIZED,
            Self::IneligibleNoFocusedMutant => INELIGIBLE_NO_FOCUSED_MUTANT,
        }
    }
}

pub(super) fn eligibility_for(case: &JudgedCase) -> Eligibility {
    if case.terminal.is_empty() || case.terminal.starts_with("unjudged") {
        Eligibility::IneligibleUnjudged
    } else if case.scope_authorization != SCOPE_AUTHORIZED {
        Eligibility::IneligibleUnauthorized
    } else if case.no_focused_mutant {
        Eligibility::IneligibleNoFocusedMutant
    } else {
        Eligibility::Eligible
    }
}
