//! One evidence line for the `discriminate` stage, shared by the human and
//! JSON surfaces.
//!
//! The producer's `discriminate` stage grades the strongest related oracle.
//! On a finding that is not `exposed`, printing that grade as
//! "discriminator yes" reads as "a discriminator exists" and contradicts the
//! finding's own class. The line keeps the oracle grade but leads with the
//! missing discriminating input the producer named, or says the discriminator
//! is not established (the same wording the editor hover uses).

use crate::domain::{ExposureClass, Finding, StageState};

pub(crate) fn discriminator_evidence_line(finding: &Finding) -> String {
    let stage = &finding.ripr.reveal.discriminate;
    if finding.class == ExposureClass::Exposed || stage.state != StageState::Yes {
        return format!("discriminator {}: {}", stage.state.as_str(), stage.summary);
    }
    let missing = finding
        .activation
        .missing_discriminators
        .iter()
        .map(|fact| format!("`{}`", fact.value))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        format!(
            "discriminator not established ({}); related oracle: {}",
            finding.class.as_str(),
            stage.summary
        )
    } else {
        format!(
            "discriminator missing: {}; related oracle: {}",
            missing.join(", "),
            stage.summary
        )
    }
}
