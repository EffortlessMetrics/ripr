//! Private badge summary model and renderer.
//!
//! This module is the rendering substrate for the `ripr` and (future)
//! `ripr+` badges. Its types are intentionally crate-private — the public
//! contract is the JSON wire shape, not the Rust types. See
//! [`docs/BADGE_POLICY.md`](../../../../../../docs/BADGE_POLICY.md) for the
//! locked semantics, color thresholds, and JSON shape.
//!
//! Both `ripr` (exposure-gap count) and `ripr+` (exposure + actionable
//! test-efficiency, minus declared intent) badge formats are supported.
//! Suppressions, CI artifacts, and the published Shields endpoint live
//! in their own scoped PRs.

mod model;
mod render;
mod summaries;
mod test_efficiency;

pub(crate) use model::{BadgeKind, BadgePolicy, BadgeSummary};
#[cfg(test)]
pub(crate) use model::{BadgeScope, BadgeStatus};
pub(crate) use render::{render_native_json, render_shields_json};
pub(crate) use summaries::{
    repo_gap_ledger_badge_summary_from_json, ripr_badge_summary_with_suppressions,
    ripr_canonical_actionable_gap_badge_summary,
};
#[cfg(test)]
pub(crate) use summaries::{ripr_badge_summary, ripr_seam_badge_summary};
#[cfg(test)]
pub(crate) use test_efficiency::ripr_plus_badge_summary;
pub(crate) use test_efficiency::{
    DiffRelatedTests, TestEfficiencyAggregationScope, parse_test_efficiency_badge_summary,
    ripr_plus_badge_summary_with_suppressions, ripr_plus_canonical_actionable_gap_badge_summary,
};
#[cfg(test)]
pub(crate) use test_efficiency::{TestEfficiencyBadgeEntry, TestEfficiencyBadgeSummary};

#[cfg(test)]
use model::BADGE_REASON_KEYS;
#[cfg(test)]
use summaries::badge_status_color;

#[cfg(test)]
mod tests;
