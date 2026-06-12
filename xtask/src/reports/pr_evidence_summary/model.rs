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
    pub(super) top_repair: Option<TopRepair>,
    /// Human-readable state when top_repair is None.
    pub(super) top_repair_state: Option<String>,
    pub(super) top_limitation: Option<TopLimitation>,
    pub(super) local_reproduction_commands: Vec<String>,
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
