//! Public projection records for the repository-governed test-harness
//! registry (#3532).
//!
//! These records carry what a registration established for one run —
//! harness kind, provenance, subject identity, selector capability, and
//! typed limitations — so CLI, LSP, and packet surfaces can project the
//! registry's facts without reconstructing them from paths, attributes,
//! or names. Empty records (no registrations) project nothing.

pub use crate::analysis::facts::{HarnessLimitationFact, HarnessSubjectFact};
#[cfg(test)]
pub use crate::analysis::facts::{HarnessSelectorCapability, HarnessSubjectClaim};
use std::path::PathBuf;

/// What one registration established for one analysis run.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TestHarnessProjection {
    /// Stable registration identifier from `ripr.toml`.
    pub registration_id: String,
    /// Harness family (`custom_harness` | `registered_attribute`).
    pub harness_kind: String,
    /// Adapter generation (e.g. `libtest_mimic_v1`).
    pub adapter: String,
    /// Exact source marker the registration matched.
    pub marker: String,
    /// Exact registered target file (workspace-relative, forward-slashed).
    pub target: PathBuf,
    /// Trust provenance of the registration.
    pub provenance: String,
    /// Executable test subjects the adapter established, in source order.
    pub subjects: Vec<HarnessSubjectFact>,
    /// Typed limitations the adapter recorded, in source order.
    pub limitations: Vec<HarnessLimitationFact>,
}

impl TestHarnessProjection {
    pub(crate) fn from_registration(registration: &crate::config::TestHarnessRegistration) -> Self {
        Self {
            registration_id: registration.registration_id.clone(),
            harness_kind: registration.kind.as_str().to_string(),
            adapter: registration.adapter.as_str().to_string(),
            marker: registration.marker.clone(),
            target: registration
                .target
                .to_string_lossy()
                .replace('\\', "/")
                .into(),
            provenance: crate::config::TestHarnessRegistration::provenance().to_string(),
            subjects: Vec::new(),
            limitations: Vec::new(),
        }
    }
}

/// Build one projection per registration (registration order preserved)
/// from the typed facts the harness registry left on the index.
pub(crate) fn projections_from_index(
    index: &crate::analysis::facts::RustIndex,
    registrations: &[crate::config::TestHarnessRegistration],
) -> Vec<TestHarnessProjection> {
    if registrations.is_empty() {
        return Vec::new();
    }
    let mut projections: Vec<TestHarnessProjection> = registrations
        .iter()
        .map(TestHarnessProjection::from_registration)
        .collect();
    for subject in &index.harness_subjects {
        if let Some(projection) = projections
            .iter_mut()
            .find(|projection| projection.registration_id == subject.registration_id)
        {
            projection.subjects.push(subject.clone());
        }
    }
    for limitation in &index.harness_limitations {
        if let Some(projection) = projections
            .iter_mut()
            .find(|projection| projection.registration_id == limitation.registration_id)
        {
            projection.limitations.push(limitation.clone());
        }
    }
    projections
}
