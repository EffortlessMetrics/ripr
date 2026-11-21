//! Shared projection of the producer-owned canonical PR delta artifact.
//!
//! This module only reads the versioned artifact produced by `cargo xtask
//! ripr-pr`. It never infers causality from a path, line, class, or message.

use crate::domain::{CanonicalDelta, CanonicalEvidenceState, ComparisonCoverage};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(crate) const DEFAULT_CANONICAL_DELTA: &str = "target/ripr/pr/canonical-delta.json";
const SCHEMA_VERSION: &str = "0.1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CausalComparisonCoverage {
    pub(crate) counts: ComparisonCoverage,
    pub(crate) complete: bool,
    pub(crate) low_coverage_disclosed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CausalDeltaArtifact {
    pub(crate) coverage: CausalComparisonCoverage,
    deltas: BTreeMap<String, CanonicalDelta>,
}

impl CausalDeltaArtifact {
    pub(crate) fn load_optional(root: &Path) -> (Option<Self>, Option<String>) {
        match Self::load(root) {
            Ok(projection) => (projection, None),
            Err(error) => (
                None,
                Some(format!(
                    "causal comparison artifact omitted from projection: {error}"
                )),
            ),
        }
    }

    pub(crate) fn load(root: &Path) -> Result<Option<Self>, String> {
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
        if schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported canonical delta schema_version `{schema_version}`"
            ));
        }

        let coverage_value = value
            .get("coverage")
            .ok_or_else(|| "canonical delta missing coverage".to_string())?;
        let counts = ComparisonCoverage {
            base_items: required_count(coverage_value, "base_items")?,
            head_items: required_count(coverage_value, "head_items")?,
            matched_items: required_count(coverage_value, "matched_items")?,
            ambiguous_items: required_count(coverage_value, "ambiguous_items")?,
            unknown_items: required_count(coverage_value, "unknown_items")?,
        };
        let claimed_complete = coverage_value
            .get("complete")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| counts.is_complete());
        let complete = claimed_complete && counts.is_complete();
        let low_coverage_disclosed = coverage_value
            .get("low_coverage_disclosed")
            .and_then(Value::as_bool)
            .unwrap_or(!complete)
            || !complete;

        let mut deltas = BTreeMap::new();
        for value in value
            .get("deltas")
            .and_then(Value::as_array)
            .ok_or_else(|| "canonical delta missing deltas array".to_string())?
        {
            let delta: CanonicalDelta = serde_json::from_value(value.clone())
                .map_err(|error| format!("canonical delta contains invalid delta: {error}"))?;
            if delta.canonical_gap_id.trim().is_empty() {
                return Err("canonical delta contains an empty canonical_gap_id".to_string());
            }
            if deltas
                .insert(delta.canonical_gap_id.clone(), delta)
                .is_some()
            {
                return Err("canonical delta contains duplicate canonical_gap_id".to_string());
            }
        }

        Ok(Some(Self {
            coverage: CausalComparisonCoverage {
                counts,
                complete,
                low_coverage_disclosed,
            },
            deltas,
        }))
    }

    pub(crate) fn delta_for(&self, canonical_gap_id: Option<&str>) -> Option<&CanonicalDelta> {
        canonical_gap_id.and_then(|id| self.deltas.get(id))
    }

    /// Add the producer-owned typed result without changing its vocabulary or
    /// deriving a replacement from presentation fields.
    pub(crate) fn insert_delta_fields(
        &self,
        object: &mut Map<String, Value>,
        canonical_gap_id: Option<&str>,
    ) {
        let Some(delta) = self.delta_for(canonical_gap_id) else {
            return;
        };
        insert_canonical_delta_fields(object, delta);
    }

    pub(crate) fn insert_comparison_fields(&self, object: &mut Map<String, Value>) {
        object.insert("causal_comparison".to_string(), self.comparison_json());
    }

    pub(crate) fn comparison_json(&self) -> Value {
        json!({
            "available": true,
            "coverage": {
                "base_items": self.coverage.counts.base_items,
                "head_items": self.coverage.counts.head_items,
                "matched_items": self.coverage.counts.matched_items,
                "ambiguous_items": self.coverage.counts.ambiguous_items,
                "unknown_items": self.coverage.counts.unknown_items,
                "complete": self.coverage.complete,
                "low_coverage_disclosed": self.coverage.low_coverage_disclosed,
            },
        })
    }
}

pub(crate) fn insert_canonical_delta_fields(
    object: &mut Map<String, Value>,
    delta: &CanonicalDelta,
) {
    object.insert(
        "delta_attribution".to_string(),
        Value::String(delta.delta_attribution.as_str().to_string()),
    );
    object.insert(
        "base_state".to_string(),
        evidence_state_json(delta.base_state.as_ref()),
    );
    object.insert(
        "head_state".to_string(),
        evidence_state_json(delta.head_state.as_ref()),
    );
    object.insert(
        "attribution_basis".to_string(),
        json!(
            delta
                .attribution_basis
                .iter()
                .map(|basis| basis.as_str())
                .collect::<Vec<_>>()
        ),
    );
    object.insert(
        "comparison_confidence".to_string(),
        Value::String(delta.comparison_confidence.as_str().to_string()),
    );
}

fn required_count(value: &Value, field: &str) -> Result<usize, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .ok_or_else(|| format!("canonical delta coverage missing {field}"))
}

fn evidence_state_json(state: Option<&CanonicalEvidenceState>) -> Value {
    state
        .map(|state| {
            json!({
                "canonical_owner": state.canonical_owner,
                "behavior_identity": state.behavior_identity,
                "discriminator_identity": state.discriminator_identity,
                "gap_state": state.gap_state.as_str(),
                "oracle_strength": state.oracle_strength.as_str(),
            })
        })
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_typed_delta_and_preserves_incomplete_coverage() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-causal-projection-{}",
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
              "coverage":{"base_items":1,"head_items":1,"matched_items":1,"ambiguous_items":0,"unknown_items":1},
              "deltas":[{
                "canonical_gap_id":"gap:a",
                "delta_attribution":"comparison_unknown",
                "base_state":null,
                "head_state":null,
                "attribution_basis":["identity_ambiguous"],
                "comparison_confidence":"unknown"
              }]
            }"#,
        )
        .map_err(|error| format!("write fixture: {error}"))?;
        let artifact = CausalDeltaArtifact::load(&root)?.ok_or("expected artifact")?;
        if artifact.coverage.complete || !artifact.coverage.low_coverage_disclosed {
            return Err("incomplete coverage was not disclosed".to_string());
        }
        let mut object = Map::new();
        artifact.insert_delta_fields(&mut object, Some("gap:a"));
        if object.get("delta_attribution") != Some(&json!("comparison_unknown")) {
            return Err("typed attribution was not projected".to_string());
        }
        if artifact.delta_for(Some("src/example.rs:42")).is_some() {
            return Err("causal projection matched a non-canonical identity".to_string());
        }
        fs::remove_dir_all(&root).map_err(|error| format!("remove fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn missing_artifact_is_optional_context() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-causal-projection-missing-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("clock before epoch: {error}"))?
                .as_nanos()
        ));
        if CausalDeltaArtifact::load(&root)?.is_some() {
            return Err("missing artifact unexpectedly loaded".to_string());
        }
        Ok(())
    }
}
