//! Crate-private seam model per `docs/specs/RIPR-SPEC-0005-repo-seam-inventory.md`.
//!
//! This module introduces the seam evidence data types — `RepoSeam`,
//! `SeamId`, `SeamKind`, `RequiredDiscriminator`, `ExpectedSink`,
//! `SeamGripClass` — but does not walk source, attach evidence,
//! classify, or render output. Those
//! responsibilities land in subsequent work items
//! (`analysis/repo-seam-inventory-v1`, `analysis/test-grip-evidence-v1`,
//! `analysis/repo-ripr-classification-v1`, `output/repo-exposure-report-v1`).
//!
//! All items are `pub(crate)`. `policy/public_api.txt` is intentionally
//! unchanged: the seam model is internal until a real consumer contract
//! exists.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Stable seam identifier.
///
/// Deterministic across runs and across input file walk reorderings.
/// Format: 16 lowercase hex chars, the FNV-1a 64-bit hash of the canonical
/// fields (file, owner, kind, byte offset).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct SeamId(String);

impl SeamId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Behavior seam category. The initial set is syntax-backed; per
/// RIPR-SPEC-0005 § Non-Goals, MIR/trait-resolution kinds may be added
/// later. `ValidationBranch` from the spec is intentionally absent
/// until `analysis/test-grip-evidence-v1` adds detection — the model
/// admits new variants additively.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum SeamKind {
    PredicateBoundary,
    ErrorVariant,
    ReturnValue,
    FieldConstruction,
    SideEffect,
    MatchArm,
    CallPresence,
}

impl SeamKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SeamKind::PredicateBoundary => "predicate_boundary",
            SeamKind::ErrorVariant => "error_variant",
            SeamKind::ReturnValue => "return_value",
            SeamKind::FieldConstruction => "field_construction",
            SeamKind::SideEffect => "side_effect",
            SeamKind::MatchArm => "match_arm",
            SeamKind::CallPresence => "call_presence",
        }
    }

    /// Test-only round-trip helper. The walker constructs `SeamKind`
    /// values directly from probe shape strings via
    /// `seam_inventory::seam_kind_from_probe_shape`; nothing in
    /// production code parses kind discriminants back into the enum.
    #[cfg(test)]
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "predicate_boundary" => SeamKind::PredicateBoundary,
            "error_variant" => SeamKind::ErrorVariant,
            "return_value" => SeamKind::ReturnValue,
            "field_construction" => SeamKind::FieldConstruction,
            "side_effect" => SeamKind::SideEffect,
            "match_arm" => SeamKind::MatchArm,
            "call_presence" => SeamKind::CallPresence,
            _ => return None,
        })
    }
}

/// What a test would need to observe to grip this seam.
///
/// The variant set tracks what the inventory walker can currently
/// emit. Spec variants (e.g. `BranchTaken` for validation branches)
/// will be added when `analysis/test-grip-evidence-v1` introduces
/// the corresponding detection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RequiredDiscriminator {
    BoundaryValue { description: String },
    ErrorVariant { variant: String },
    ReturnValue { description: String },
    FieldValue { field: String },
    Effect { sink: String },
    MatchArmTaken { arm: String },
    CallSite { target: String },
}

impl RequiredDiscriminator {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            RequiredDiscriminator::BoundaryValue { .. } => "boundary_value",
            RequiredDiscriminator::ErrorVariant { .. } => "error_variant",
            RequiredDiscriminator::ReturnValue { .. } => "return_value",
            RequiredDiscriminator::FieldValue { .. } => "field_value",
            RequiredDiscriminator::Effect { .. } => "effect",
            RequiredDiscriminator::MatchArmTaken { .. } => "match_arm_taken",
            RequiredDiscriminator::CallSite { .. } => "call_site",
        }
    }
}

/// Where a seam's effect would manifest — the sink class a test must
/// observe to discriminate the changed behavior. Subsequent inventory
/// and classification PRs populate this from existing flow-sink facts.
/// `Unknown` will be added back when a kind without a determinable
/// sink is detected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum ExpectedSink {
    ReturnValue,
    OutputField,
    ErrorChannel,
    SideEffect,
}

impl ExpectedSink {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ExpectedSink::ReturnValue => "return_value",
            ExpectedSink::OutputField => "output_field",
            ExpectedSink::ErrorChannel => "error_channel",
            ExpectedSink::SideEffect => "side_effect",
        }
    }
}

/// Classification of how strongly current tests grip a seam.
///
/// The full set is locked in RIPR-SPEC-0005. The headline-eligibility
/// table on `is_headline_eligible` mirrors the spec's
/// "Headline Count vs Visible-Only Mapping" section and is consumed
/// by the upcoming `output/repo-exposure-report-v1` work item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum SeamGripClass {
    StronglyGripped,
    WeaklyGripped,
    Ungripped,
    ReachableUnrevealed,
    ActivationUnknown,
    PropagationUnknown,
    ObservationUnknown,
    DiscriminationUnknown,
    Opaque,
    Intentional,
    Suppressed,
}

impl SeamGripClass {
    /// Every grip class, in spec declaration order. Used by future
    /// renderers to enumerate per-class metric buckets.
    pub(crate) const ALL: [SeamGripClass; 11] = [
        SeamGripClass::StronglyGripped,
        SeamGripClass::WeaklyGripped,
        SeamGripClass::Ungripped,
        SeamGripClass::ReachableUnrevealed,
        SeamGripClass::ActivationUnknown,
        SeamGripClass::PropagationUnknown,
        SeamGripClass::ObservationUnknown,
        SeamGripClass::DiscriminationUnknown,
        SeamGripClass::Opaque,
        SeamGripClass::Intentional,
        SeamGripClass::Suppressed,
    ];

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SeamGripClass::StronglyGripped => "strongly_gripped",
            SeamGripClass::WeaklyGripped => "weakly_gripped",
            SeamGripClass::Ungripped => "ungripped",
            SeamGripClass::ReachableUnrevealed => "reachable_unrevealed",
            SeamGripClass::ActivationUnknown => "activation_unknown",
            SeamGripClass::PropagationUnknown => "propagation_unknown",
            SeamGripClass::ObservationUnknown => "observation_unknown",
            SeamGripClass::DiscriminationUnknown => "discrimination_unknown",
            SeamGripClass::Opaque => "opaque",
            SeamGripClass::Intentional => "intentional",
            SeamGripClass::Suppressed => "suppressed",
        }
    }

    /// Whether this class counts toward the headline badge per
    /// RIPR-SPEC-0005 § "Headline Count vs Visible-Only Mapping".
    /// `Opaque`'s headline treatment is decided by badge policy at
    /// render time; this method returns `false` so the model itself
    /// stays policy-free.
    pub(crate) fn is_headline_eligible(&self) -> bool {
        matches!(
            self,
            SeamGripClass::Ungripped
                | SeamGripClass::WeaklyGripped
                | SeamGripClass::ReachableUnrevealed
                | SeamGripClass::ActivationUnknown
                | SeamGripClass::PropagationUnknown
                | SeamGripClass::ObservationUnknown
                | SeamGripClass::DiscriminationUnknown
        )
    }
}

/// A first-class behavior seam discovered in a production file.
///
/// The `id` is computed from the canonical fields by `RepoSeam::new`; do
/// not assemble seams via field literals at call sites, because that would
/// allow constructing a seam whose `id` does not match its fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RepoSeam {
    id: SeamId,
    kind: SeamKind,
    file: PathBuf,
    owner: String,
    byte_offset: usize,
    display_line: usize,
    expression: String,
    required_discriminator: RequiredDiscriminator,
    expected_sink: ExpectedSink,
}

impl RepoSeam {
    /// Construct a seam, computing a deterministic ID from the canonical
    /// fields per RIPR-SPEC-0005 § "Stable Seam ID Rules".
    ///
    /// `expression` is the source-code text at the seam origin and is
    /// surfaced verbatim in human/JSON output. It is *not* part of the
    /// canonical ID hash, so reformatting whitespace within an expression
    /// does not change `SeamId`.
    // Eight fields are intrinsic to a seam's identity and presentation;
    // grouping them into nested structs would force every call site
    // through extra constructors without making the data simpler.
    #[expect(
        clippy::too_many_arguments,
        reason = "Eight fields are intrinsic to a seam's identity and presentation; grouping them into nested structs forces every call site through extra constructors without simplifying the data."
    )]
    pub(crate) fn new(
        file: impl AsRef<Path>,
        owner: impl Into<String>,
        kind: SeamKind,
        byte_offset: usize,
        display_line: usize,
        expression: impl Into<String>,
        required_discriminator: RequiredDiscriminator,
        expected_sink: ExpectedSink,
    ) -> Self {
        let file_normalized = normalize_path(file.as_ref());
        let owner = owner.into();
        let id = compute_seam_id(&file_normalized, &owner, kind, byte_offset);
        RepoSeam {
            id,
            kind,
            file: PathBuf::from(file_normalized),
            owner,
            byte_offset,
            display_line,
            expression: expression.into(),
            required_discriminator,
            expected_sink,
        }
    }

    pub(crate) fn id(&self) -> &SeamId {
        &self.id
    }
    pub(crate) fn kind(&self) -> SeamKind {
        self.kind
    }
    pub(crate) fn file(&self) -> &Path {
        &self.file
    }
    pub(crate) fn owner(&self) -> &str {
        &self.owner
    }
    pub(crate) fn byte_offset(&self) -> usize {
        self.byte_offset
    }
    pub(crate) fn display_line(&self) -> usize {
        self.display_line
    }
    pub(crate) fn expression(&self) -> &str {
        &self.expression
    }
    pub(crate) fn required_discriminator(&self) -> &RequiredDiscriminator {
        &self.required_discriminator
    }
    pub(crate) fn expected_sink(&self) -> ExpectedSink {
        self.expected_sink
    }
}

/// Repo-root-relative path normalization: Unix separators, no leading `./`.
/// Used inside the ID hash so `src/x.rs`, `./src/x.rs`, and `src\x.rs`
/// produce the same seam ID across platforms.
fn normalize_path(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    s.strip_prefix("./").unwrap_or(&s).to_string()
}

/// FNV-1a 64-bit hash of the canonical seam fields, encoded as a 16-char
/// lowercase hex string.
///
/// FNV-1a is chosen because it is simple, has no third-party dependency,
/// and is stable across Rust versions — unlike
/// `std::collections::hash_map::DefaultHasher`, which is intentionally not
/// stable across releases. The hash never reads time, walk order, process
/// ID, or any other ambient state.
fn compute_seam_id(file: &str, owner: &str, kind: SeamKind, byte_offset: usize) -> SeamId {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    // Null byte separator avoids collisions: `\0` cannot appear in POSIX
    // or Windows file paths, in Rust module/identifier names that make up
    // owner symbols, in our static kind discriminants, or in the decimal
    // stringification of `byte_offset`.
    let offset_str = byte_offset.to_string();
    let parts: [&[u8]; 7] = [
        file.as_bytes(),
        b"\0",
        owner.as_bytes(),
        b"\0",
        kind.as_str().as_bytes(),
        b"\0",
        offset_str.as_bytes(),
    ];
    let mut hash: u64 = FNV_OFFSET;
    for part in parts {
        for byte in part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    SeamId(format!("{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_seam(file: &str, owner: &str, kind: SeamKind, off: usize) -> RepoSeam {
        RepoSeam::new(
            file,
            owner,
            kind,
            off,
            1,
            "amount >= threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        )
    }

    #[test]
    fn seam_id_is_deterministic_for_identical_inputs() {
        let a = make_seam(
            "src/pricing.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            88,
        );
        let b = make_seam(
            "src/pricing.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            88,
        );
        assert_eq!(a.id(), b.id());
    }

    #[test]
    fn seam_id_differs_when_any_canonical_field_differs() {
        let base = make_seam(
            "src/pricing.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            88,
        );
        let other_file = make_seam(
            "src/checkout.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            88,
        );
        let other_owner = make_seam(
            "src/pricing.rs",
            "pricing::compute",
            SeamKind::PredicateBoundary,
            88,
        );
        let other_kind = make_seam(
            "src/pricing.rs",
            "pricing::quote",
            SeamKind::ReturnValue,
            88,
        );
        let other_offset = make_seam(
            "src/pricing.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            89,
        );

        assert_ne!(base.id(), other_file.id());
        assert_ne!(base.id(), other_owner.id());
        assert_ne!(base.id(), other_kind.id());
        assert_ne!(base.id(), other_offset.id());
    }

    #[test]
    fn seam_ids_do_not_depend_on_construction_order() -> Result<(), String> {
        let inputs = [
            ("src/a.rs", "a::f", SeamKind::PredicateBoundary, 10),
            ("src/b.rs", "b::g", SeamKind::ErrorVariant, 20),
            ("src/c.rs", "c::h", SeamKind::ReturnValue, 30),
        ];

        let forward: Vec<String> = inputs
            .iter()
            .map(|(f, o, k, off)| make_seam(f, o, *k, *off).id().as_str().to_string())
            .collect();

        let mut reversed: Vec<String> = inputs
            .iter()
            .rev()
            .map(|(f, o, k, off)| make_seam(f, o, *k, *off).id().as_str().to_string())
            .collect();
        reversed.reverse();

        if forward != reversed {
            return Err("seam IDs depend on construction order".to_string());
        }
        Ok(())
    }

    #[test]
    fn seam_id_normalizes_windows_path_separators() {
        let unix = make_seam(
            "src/pricing.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            88,
        );
        let windows = make_seam(
            "src\\pricing.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            88,
        );
        assert_eq!(unix.id(), windows.id());
    }

    #[test]
    fn seam_id_normalizes_leading_dot_slash() {
        let plain = make_seam(
            "src/pricing.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            88,
        );
        let dotted = make_seam(
            "./src/pricing.rs",
            "pricing::quote",
            SeamKind::PredicateBoundary,
            88,
        );
        assert_eq!(plain.id(), dotted.id());
    }

    #[test]
    fn seam_id_is_16_lowercase_hex_chars() -> Result<(), String> {
        let seam = make_seam("src/x.rs", "x::y", SeamKind::PredicateBoundary, 0);
        let id = seam.id().as_str();
        if id.len() != 16 {
            return Err(format!(
                "seam id should be 16 chars, got {}: {id}",
                id.len()
            ));
        }
        for c in id.chars() {
            if !c.is_ascii_hexdigit() {
                return Err(format!("seam id should be hex, got: {id}"));
            }
            if c.is_ascii_alphabetic() && !c.is_ascii_lowercase() {
                return Err(format!("seam id hex should be lowercase, got: {id}"));
            }
        }
        Ok(())
    }

    #[test]
    fn seam_kind_round_trips_through_str() -> Result<(), String> {
        let all = [
            SeamKind::PredicateBoundary,
            SeamKind::ErrorVariant,
            SeamKind::ReturnValue,
            SeamKind::FieldConstruction,
            SeamKind::SideEffect,
            SeamKind::MatchArm,
            SeamKind::CallPresence,
        ];
        for kind in all {
            let s = kind.as_str();
            let parsed = SeamKind::from_str(s)
                .ok_or_else(|| format!("SeamKind::from_str rejected its own as_str: {s}"))?;
            if parsed != kind {
                return Err(format!("round-trip failed for {s}"));
            }
        }
        if SeamKind::from_str("nonsense").is_some() {
            return Err("SeamKind::from_str should reject unknown strings".to_string());
        }
        Ok(())
    }

    #[test]
    fn required_discriminator_carries_kind_via_as_str() {
        let cases: &[(RequiredDiscriminator, &str)] = &[
            (
                RequiredDiscriminator::BoundaryValue {
                    description: "amount >= threshold".to_string(),
                },
                "boundary_value",
            ),
            (
                RequiredDiscriminator::ErrorVariant {
                    variant: "QuoteError::Insolvent".to_string(),
                },
                "error_variant",
            ),
            (
                RequiredDiscriminator::ReturnValue {
                    description: "non-zero discount".to_string(),
                },
                "return_value",
            ),
            (
                RequiredDiscriminator::FieldValue {
                    field: "Discount.amount".to_string(),
                },
                "field_value",
            ),
            (
                RequiredDiscriminator::Effect {
                    sink: "log::error".to_string(),
                },
                "effect",
            ),
            (
                RequiredDiscriminator::MatchArmTaken {
                    arm: "Pricing::Premium".to_string(),
                },
                "match_arm_taken",
            ),
            (
                RequiredDiscriminator::CallSite {
                    target: "metrics::record".to_string(),
                },
                "call_site",
            ),
        ];
        for (case, expected) in cases {
            assert_eq!(case.as_str(), *expected);
        }
    }

    #[test]
    fn expected_sink_str_covers_all_variants() {
        let all = [
            (ExpectedSink::ReturnValue, "return_value"),
            (ExpectedSink::OutputField, "output_field"),
            (ExpectedSink::ErrorChannel, "error_channel"),
            (ExpectedSink::SideEffect, "side_effect"),
        ];
        for (sink, expected) in all {
            assert_eq!(sink.as_str(), expected);
        }
    }

    #[test]
    fn repo_seam_accessors_round_trip_construction_inputs() -> Result<(), String> {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::check_discount",
            SeamKind::PredicateBoundary,
            1234,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        assert_eq!(seam.kind(), SeamKind::PredicateBoundary);
        assert_eq!(seam.owner(), "pricing::check_discount");
        assert_eq!(seam.byte_offset(), 1234);
        assert_eq!(seam.display_line(), 88);
        assert_eq!(seam.expression(), "amount >= discount_threshold");
        assert_eq!(seam.expected_sink(), ExpectedSink::ReturnValue);
        assert_eq!(seam.file().to_string_lossy(), "src/pricing.rs");
        match seam.required_discriminator() {
            RequiredDiscriminator::BoundaryValue { description } => {
                assert_eq!(description, "amount >= discount_threshold");
                Ok(())
            }
            other => Err(format!("expected BoundaryValue, got {}", other.as_str())),
        }
    }
}
