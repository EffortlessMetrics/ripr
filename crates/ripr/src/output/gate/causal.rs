use crate::domain::DeltaAttribution;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(super) const DEFAULT_CANONICAL_DELTA: &str = "target/ripr/pr/canonical-delta.json";

/// The gate's narrow authority view over the producer-owned canonical delta.
///
/// This deliberately does not infer attribution from a candidate's path, line,
/// class, or prose. An artifact with incomplete coverage is useful context but
/// cannot authorize a causal gate count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CausalDeltaAuthority {
    by_gap_id: BTreeMap<String, DeltaAttribution>,
    complete: bool,
    ambiguous_items: usize,
    unknown_items: usize,
}

impl CausalDeltaAuthority {
    pub(super) fn load(root: &Path) -> Result<Option<Self>, String> {
        let path = root.join(DEFAULT_CANONICAL_DELTA);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("read canonical delta failed: {error}")),
        };
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("parse canonical delta failed: {error}"))?;
        let schema_version = value
            .get("schema_version")
            .and_then(Value::as_str)
            .ok_or_else(|| "canonical delta missing schema_version".to_string())?;
        if schema_version != "0.1" {
            return Err(format!(
                "unsupported canonical delta schema_version `{schema_version}`"
            ));
        }
        let coverage = value
            .get("coverage")
            .ok_or_else(|| "canonical delta missing coverage".to_string())?;
        let mut ambiguous_items = coverage
            .get("ambiguous_items")
            .and_then(Value::as_u64)
            .ok_or_else(|| "canonical delta coverage missing ambiguous_items".to_string())?
            as usize;
        let mut unknown_items = coverage
            .get("unknown_items")
            .and_then(Value::as_u64)
            .ok_or_else(|| "canonical delta coverage missing unknown_items".to_string())?
            as usize;
        let producer_claimed_complete = coverage
            .get("complete")
            .and_then(Value::as_bool)
            .unwrap_or(ambiguous_items == 0 && unknown_items == 0);
        let mut by_gap_id = BTreeMap::new();
        for delta in value
            .get("deltas")
            .and_then(Value::as_array)
            .ok_or_else(|| "canonical delta missing deltas array".to_string())?
        {
            let Some(gap_id) = delta
                .get("canonical_gap_id")
                .and_then(Value::as_str)
                .filter(|gap_id| !gap_id.trim().is_empty())
            else {
                unknown_items += 1;
                continue;
            };
            let (attribution, recognized) = parse_attribution(
                delta
                    .get("delta_attribution")
                    .and_then(Value::as_str)
                    .unwrap_or("comparison_unknown"),
            );
            if !recognized {
                unknown_items += 1;
            }
            if let Some(previous) = by_gap_id.insert(gap_id.to_string(), attribution)
                && previous != attribution
            {
                ambiguous_items += 1;
                by_gap_id.insert(gap_id.to_string(), DeltaAttribution::ComparisonUnknown);
            }
        }
        let complete = producer_claimed_complete && ambiguous_items == 0 && unknown_items == 0;
        Ok(Some(Self {
            by_gap_id,
            complete,
            ambiguous_items,
            unknown_items,
        }))
    }

    pub(super) fn attribution_for(&self, canonical_gap_id: Option<&str>) -> DeltaAttribution {
        if !self.complete {
            return DeltaAttribution::ComparisonUnknown;
        }
        canonical_gap_id
            .and_then(|gap_id| self.by_gap_id.get(gap_id).copied())
            .unwrap_or(DeltaAttribution::ComparisonUnknown)
    }

    pub(super) fn allows_blocking(attribution: DeltaAttribution) -> bool {
        matches!(
            attribution,
            DeltaAttribution::IntroducedByChange
                | DeltaAttribution::WeakenedByChange
                | DeltaAttribution::ReintroducedByChange
        )
    }

    pub(super) fn is_complete(&self) -> bool {
        self.complete
    }

    pub(super) fn disclosure(&self) -> String {
        if self.complete {
            "canonical delta comparison is complete".to_string()
        } else {
            format!(
                "canonical delta comparison is incomplete; {} ambiguous and {} unknown items remain",
                self.ambiguous_items, self.unknown_items
            )
        }
    }
}

fn parse_attribution(value: &str) -> (DeltaAttribution, bool) {
    match value {
        "introduced_by_change" => (DeltaAttribution::IntroducedByChange, true),
        "weakened_by_change" => (DeltaAttribution::WeakenedByChange, true),
        "reintroduced_by_change" => (DeltaAttribution::ReintroducedByChange, true),
        "resolved_by_change" => (DeltaAttribution::ResolvedByChange, true),
        "changed_surface_existing" => (DeltaAttribution::ChangedSurfaceExisting, true),
        "adjacent_preexisting" => (DeltaAttribution::AdjacentPreexisting, true),
        "baseline_existing" => (DeltaAttribution::BaselineExisting, true),
        "comparison_unknown" => (DeltaAttribution::ComparisonUnknown, true),
        _ => (DeltaAttribution::ComparisonUnknown, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn only_causal_change_classes_can_block() {
        for attribution in [
            DeltaAttribution::IntroducedByChange,
            DeltaAttribution::WeakenedByChange,
            DeltaAttribution::ReintroducedByChange,
        ] {
            assert!(CausalDeltaAuthority::allows_blocking(attribution));
        }
        for attribution in [
            DeltaAttribution::ResolvedByChange,
            DeltaAttribution::ChangedSurfaceExisting,
            DeltaAttribution::AdjacentPreexisting,
            DeltaAttribution::BaselineExisting,
            DeltaAttribution::ComparisonUnknown,
        ] {
            assert!(!CausalDeltaAuthority::allows_blocking(attribution));
        }
    }

    #[test]
    fn incomplete_coverage_fails_closed_for_every_identity() {
        let authority = CausalDeltaAuthority {
            by_gap_id: BTreeMap::from([(
                "gap:introduced".to_string(),
                DeltaAttribution::IntroducedByChange,
            )]),
            complete: false,
            ambiguous_items: 1,
            unknown_items: 2,
        };
        assert_eq!(
            authority.attribution_for(Some("gap:introduced")),
            DeltaAttribution::ComparisonUnknown
        );
        assert!(authority.disclosure().contains("1 ambiguous"));
        assert!(authority.disclosure().contains("2 unknown"));
    }

    #[test]
    fn unknown_or_missing_identity_is_not_causal() {
        let authority = CausalDeltaAuthority {
            by_gap_id: BTreeMap::from([(
                "gap:introduced".to_string(),
                DeltaAttribution::IntroducedByChange,
            )]),
            complete: true,
            ambiguous_items: 0,
            unknown_items: 0,
        };
        assert_eq!(
            authority.attribution_for(Some("gap:other")),
            DeltaAttribution::ComparisonUnknown
        );
        assert_eq!(
            authority.attribution_for(None),
            DeltaAttribution::ComparisonUnknown
        );
    }

    #[test]
    fn loader_reads_versioned_delta_and_preserves_non_causal_classes() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-causal-gate-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("clock before epoch: {error}"))?
                .as_nanos()
        ));
        let path = root.join(DEFAULT_CANONICAL_DELTA);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("create fixture: {error}"))?;
        }
        fs::write(
            &path,
            r#"{
              "schema_version":"0.1",
              "coverage":{"ambiguous_items":0,"unknown_items":0},
              "deltas":[
                {"canonical_gap_id":"gap:introduced","delta_attribution":"introduced_by_change"},
                {"canonical_gap_id":"gap:resolved","delta_attribution":"resolved_by_change"}
              ]
            }"#,
        )
        .map_err(|error| format!("write fixture: {error}"))?;
        let authority = CausalDeltaAuthority::load(&root)?.ok_or("expected authority")?;
        if !CausalDeltaAuthority::allows_blocking(authority.attribution_for(Some("gap:introduced")))
        {
            return Err("introduced class was not accepted by gate authority".to_string());
        }
        if CausalDeltaAuthority::allows_blocking(authority.attribution_for(Some("gap:resolved"))) {
            return Err("resolved class was accepted by gate authority".to_string());
        }
        fs::remove_dir_all(&root).map_err(|error| format!("remove fixture: {error}"))?;
        Ok(())
    }
}
