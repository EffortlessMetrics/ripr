use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct JsonInput {
    pub(super) state: InputState,
    pub(super) value: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InputState {
    Present,
    Missing,
    Invalid(String),
}

impl std::fmt::Display for InputState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Present => f.write_str("present"),
            Self::Missing => f.write_str("missing"),
            Self::Invalid(err) => write!(
                f,
                "invalid: {}",
                crate::reports::pr_evidence_summary::util::md_escape(err)
            ),
        }
    }
}

// ── In-memory output model ────────────────────────────────────────────────────
//
// These types carry the computed fields before JSON/MD serialization.
// They use plain Rust types (no serde derives) because xtask does not
// take a serde dep; rendering to JSON is done via serde_json::json! in json.rs.

/// Top-level in-memory representation of the PR evidence summary.
pub(super) struct PrEvidenceSummaryJson {
    /// `"complete"`, `"seam_limit_applied"`,
    /// `"diff_complete_full_repo_limited"`, or `"unknown"`.
    pub(super) run_status: String,
    pub(super) changed_surfaces: U64OrNotAvailable,
    pub(super) gaps: GapCounts,
    pub(super) limitations: Vec<LimitationEntry>,
    pub(super) missing_receipts: U64OrNotAvailable,
    pub(super) receipt_status: ReceiptStatusCounts,
    pub(super) top_repair: Option<TopRepair>,
    /// Human-readable state when top_repair is None.
    pub(super) top_repair_state: Option<String>,
    pub(super) top_limitation: Option<TopLimitation>,
    pub(super) local_reproduction_commands: Vec<String>,
}

/// Six-count receipt-status object surfaced in the PR evidence summary.
///
/// Two fields are derivable from gap-ledger summary counts today.
/// Four are `NotAvailable` because the gap-decision-ledger build path does not
/// carry the per-record signals needed to classify them, and emitting `0` would
/// be a fake zero — no real condition can produce a non-zero count, so a `0`
/// would falsely claim "we checked and found none" (see #1130 adversarial review):
///
/// - `orphan_receipts`: a receipt file exists but no matching gap record was found —
///   requires a `target/ripr/receipts/` dir sweep that is not performed during
///   summary derivation.
/// - `stale_receipts`: a receipt is stale — the genuine staleness signal lives in
///   `swarm_ingest` (`staleness_status`), a separate artifact the gap-ledger build
///   does not consume; the gap-ledger never writes `receipt.state == "receipt_stale"`
///   in production.
/// - `gap_mismatch_receipts`: a receipt references a different gap id than the
///   ledger record — requires reading each receipt file to compare its own
///   recorded `canonical_gap_id` against the attached gap; the ledger ingest
///   does not surface the receipt's own gap id field.
/// - `verify_failed_receipts`: a receipt's verify failed — the genuine verify
///   pass/fail signal lives in `swarm_ingest` (`verify.passed`/`failed`), a
///   separate artifact the gap-ledger build does not consume; the gap-ledger
///   never writes `receipt.state == "receipt_verify_failed"` in production.
pub(super) struct ReceiptStatusCounts {
    /// Gap-ledger records that carry receipt evidence:
    /// `summary.receipt_improved_total + summary.receipt_unchanged_after_attempt_total`.
    pub(super) receipts_present: U64OrNotAvailable,
    /// Actionable gaps without a receipt:
    /// mirrors the top-level `missing_receipts` field.
    pub(super) missing_receipts: U64OrNotAvailable,
    /// NOT DERIVABLE — requires a `target/ripr/receipts/` dir sweep vs. ledger records.
    pub(super) orphan_receipts: U64OrNotAvailable,
    /// NOT DERIVABLE — real staleness signal lives in `swarm_ingest`, not the
    /// gap-ledger build; emitting 0 would be a fake zero (#1130).
    pub(super) stale_receipts: U64OrNotAvailable,
    /// NOT DERIVABLE — requires reading each receipt file to compare its own
    /// `canonical_gap_id` against the attached gap record.
    pub(super) gap_mismatch_receipts: U64OrNotAvailable,
    /// NOT DERIVABLE — real verify pass/fail signal lives in `swarm_ingest`, not
    /// the gap-ledger build; emitting 0 would be a fake zero (#1130).
    pub(super) verify_failed_receipts: U64OrNotAvailable,
}

/// Gap counts. Delta fields are `null` when no `--baseline` is given.
pub(super) struct GapCounts {
    pub(super) total_actionable: U64OrNotAvailable,
    pub(super) total_static_limitation: U64OrNotAvailable,
    pub(super) new_actionable: NullableU64,
    pub(super) resolved: NullableU64,
    pub(super) regressed: NullableU64,
    /// Explanation when delta fields are null.
    pub(super) gap_delta_note: Option<String>,
}

pub(super) struct LimitationEntry {
    pub(super) category: String,
    pub(super) repair_route: String,
}

pub(super) struct TopRepair {
    pub(super) canonical_gap_id: String,
    pub(super) language: String,
    /// repair.route from start-here selected.
    pub(super) repair_kind: String,
    /// repair.target_file from start-here selected.
    pub(super) target: String,
    pub(super) verify_command: String,
    pub(super) receipt_command: String,
    pub(super) receipt_state: String,
}

pub(super) struct TopLimitation {
    pub(super) category: String,
    pub(super) repair_route: String,
    pub(super) why_not_actionable: String,
}

/// Either a concrete `u64` value or `"not_available"` in JSON.
pub(super) enum U64OrNotAvailable {
    Value(u64),
    NotAvailable,
}

/// Either a `u64` or JSON `null`.
pub(super) enum NullableU64 {
    Value(u64),
    Null,
}
