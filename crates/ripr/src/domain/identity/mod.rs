//! Governed identity registry: vocabulary, compatibility map, and invariants.
//!
//! This module records existing identifier authorities. It does not migrate
//! consumers or mint replacement types owned by #4805–#4809.

mod catalog;
mod fields;
mod invariants;
mod kinds;
mod record;
mod render;

#[cfg(test)]
mod tests;

pub use fields::{GOVERNED_IDENTITY_SURFACES, identity_field_disposition};

use catalog::{adjacent_fields, production_records};
use invariants::registry_violations;
use render::{render_canonical_json, render_markdown};

/// Required #1932 taxonomy identities. Sibling serialized identities may also
/// appear in the registry so every governed field has a disposition.
pub const REQUIRED_TAXONOMY_KINDS: &[&str] = &[
    "CanonicalItemId",
    "InstructionSemanticId",
    "InstructionInstanceId",
    "ActionId",
    "EditInstructionId",
    "AnalysisAttemptId",
    "CompletedAnalysisSnapshotId",
    "InputIdentity",
    "DiagnosticResultId",
    "ContinuationId",
    "CommandId",
    "RepairAttemptId",
    "ReceiptId",
];

/// Invariant violations for the production catalog. Empty means the catalog
/// is internally consistent; it does not prove consumers have migrated.
pub fn identity_registry_violations() -> Vec<String> {
    let mut violations = registry_violations(production_records(), adjacent_fields());
    invariants::require_catalog_surfaces_are_governed(
        production_records(),
        adjacent_fields(),
        GOVERNED_IDENTITY_SURFACES,
        &mut violations,
    );
    violations.sort();
    violations.dedup();
    violations
}

/// Human relationship table generated from the production catalog.
pub fn identity_registry_markdown() -> String {
    render_markdown(production_records(), adjacent_fields())
}

/// Byte-stable machine-readable registry generated from the production catalog.
pub fn identity_registry_canonical_json() -> String {
    render_canonical_json(production_records(), adjacent_fields())
}

pub const IDENTITY_REGISTRY_MARKDOWN_PATH: &str = render::REGISTRY_MARKDOWN_PATH;
pub const IDENTITY_REGISTRY_JSON_PATH: &str = render::REGISTRY_JSON_PATH;
