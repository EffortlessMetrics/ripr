//! Driver binding validation for the governed Python repair-trust corpus —
//! `cargo xtask python-repair-trust check-driver` (RIPR-SPEC-0176, #3569).
//!
//! The two-phase external-edit driver (#2443) binds one durable repair
//! attempt (#2927) to one accepted selection row (#3568) by digests and
//! retains a typed binding record inside the attempt. This validator is the
//! offline promotion-side counterpart: given the accepted selection manifest
//! and a set of retained driver binding records, it re-checks every digest
//! anchor and claim boundary the records assert, so a promotion pass can
//! trust the retained evidence without re-running the driver.
//!
//! What the check enforces, fail closed:
//!
//! - the record binds to the ACCEPTED selection manifest by exact-bytes
//!   sha256 — a changed manifest makes every retained record stale;
//! - the record binds one selection row by its recomputed canonical
//!   `selection_digest` over the retained preimage — a replaced or edited row
//!   makes the record stale;
//! - every identity the record copies from the selected row (case, subject,
//!   repository, base, head, optional tree/currentness/limitation, family,
//!   owner, discriminator, relation, oracle) agrees exactly with the accepted
//!   row — a rewritten copy fails even while the digest anchors stay valid;
//! - the record's target identity agrees with the row (`target_path`,
//!   `target_state`); the driver binds only `existing` targets, and the
//!   normalized target path is what is compared and recorded;
//! - the driver issued the binding only under explicit operator/agent
//!   authorization (`status: granted`, named authority, the
//!   explicit-operator-flags method) — an inferred or absent authorization
//!   fails;
//! - the standing non-claims ride on every record: the driver claims no
//!   verification result, no static movement, and no closure. Records carry
//!   no lifecycle, movement, or execution fields at all (deny-unknown), so a
//!   driver record can never appear completed;
//! - apply-phase records additionally carry the durable attempt identity
//!   (#2927 shape), the prepare-record digest chain, the patch digest, the
//!   changed-file set, the edit-cage decision, and the resulting head; every
//!   apply record must chain to exactly one supplied prepare record by its
//!   exact-byte digest AND agree with that prepare record on every
//!   prepare-bound identity field (trust attempt, selection digest, seam,
//!   prepare-time head, target, input digests, edit surface, authorization) —
//!   an apply that names another attempt's prepare digest fails, naming both
//!   records and the disagreeing field. A compliant record must include the
//!   selected target within the declared cage. A `violated` record is
//!   accepted as the producer's typed failed result: its escaped paths are
//!   validated structurally (portable spellings) and retained verbatim as
//!   failure evidence, and the report renders the cage decision so
//!   `compliant` and failed(`violated`) dispositions stay distinguishable.
//! - production/generated/vendor/environment edit surfaces fail: a record
//!   whose target falls under a denied surface prefix is rejected even when
//!   the row declared it `unsafe`, because the driver binds only test-only,
//!   existing, unambiguous targets.
//!
//! Verdict vocabulary: `valid`/`inconsistent`/`not_run` (no records
//! supplied). Violations take precedence: any violation makes the verdict
//! `inconsistent` and the command exits nonzero — an invalid-only binding
//! set never passes. `not_run` is reserved for an empty input with no
//! violations and is not a pass. None of the three is a support-tier,
//! repair-correctness, gate, badge, or promotion claim; #3570 owns the
//! verification phase.
//!
//! Focused modules: `cli` (entry and publication), `schema` (DTOs and closed
//! key sets), `validate` (identity and currentness), `aggregate` (joins and
//! verdict), `render` (JSON/Markdown). Tests remain
//! `python_repair_driver_binding`.

mod aggregate;
mod cli;
mod render;
mod schema;
mod validate;

#[cfg(test)]
#[path = "python_repair_driver/tests.rs"]
mod python_repair_driver_binding;

pub(crate) use cli::run_check_driver;

// Re-imported at the facade so `tests.rs` keeps reaching the moved items
// through `super::` exactly as it did when this module was one file.
#[cfg(test)]
pub(crate) use super::python_repair_trust::{
    KNOWN_SPEC, SelectionManifest, canonical_selection_digest, validate_selection_manifest,
};
#[cfg(test)]
pub(crate) use aggregate::check_driver_artifacts;
#[cfg(test)]
pub(crate) use schema::{
    BINDING_KIND, BINDING_NON_CLAIMS, BINDING_SCHEMA_VERSION, BINDING_SPEC, DriverBindingRecord,
};
#[cfg(test)]
pub(crate) use serde_json::{Value, json};
#[cfg(test)]
pub(crate) use validate::validate_binding_record;
