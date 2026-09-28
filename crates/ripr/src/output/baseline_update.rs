use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: &str = "0.1";
const BASELINE_KIND: &str = "gate_baseline";
const LIMITS_NOTE: &str = "Baseline identity refresh over static RIPR gate evidence; update removes resolved reviewed debt only with --remove-resolved, migrates reviewed legacy identities only with --migrate-legacy-identities, and never adopts new current debt.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BaselineUpdateInput {
    pub(crate) baseline_path: String,
    pub(crate) current_gate_decision_path: String,
    pub(crate) baseline_json: String,
    pub(crate) current_gate_decision_json: String,
    /// Shrink-only removal of resolved reviewed debt (issue #1964, review):
    /// without this flag unmatched entries are preserved, so a
    /// migration-only run cannot silently shrink the reviewed baseline.
    pub(crate) remove_resolved: bool,
    /// Deterministic legacy-to-canonical identity migration (issue #1964):
    /// reviewed fallback-only entries joined unambiguously to a current
    /// decision that carries a canonical gap id gain that canonical identity.
    /// Never adopts new current debt; never overwrites a reviewed canonical.
    pub(crate) migrate_legacy_identities: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BaselineUpdateReport {
    updated_baseline: Value,
    before_entries: usize,
    after_entries: usize,
    removed_resolved: usize,
    preserved_invalid: usize,
    preserved_stale: usize,
    ignored_new_current: usize,
    migrated_legacy_identities: usize,
    warnings: Vec<String>,
}

/// One deterministic legacy migration decision (issue #1964).
enum LegacyMigration {
    /// Replace the reviewed legacy identity with this canonical gap id.
    Apply {
        canonical_gap_id: String,
        legacy_identity: String,
    },
    /// Preserve the entry unchanged and say why.
    Refuse(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Identity {
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    source_id: Option<String>,
    id: Option<String>,
    dedupe_key: Option<String>,
    fallback: Option<String>,
}

impl Identity {
    fn has_stable_value(&self) -> bool {
        self.canonical_gap_id.is_some()
            || self.seam_id.is_some()
            || self.source_id.is_some()
            || self.id.is_some()
            || self.dedupe_key.is_some()
            || self.fallback.is_some()
    }

    fn sort_key(&self) -> String {
        match self
            .canonical_gap_id
            .as_deref()
            .or(self.seam_id.as_deref())
            .or(self.source_id.as_deref())
            .or(self.id.as_deref())
            .or(self.dedupe_key.as_deref())
            .or(self.fallback.as_deref())
        {
            Some(value) => value.to_string(),
            None => String::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct CurrentRecord {
    identity: Identity,
}

#[derive(Clone, Debug)]
struct CurrentIndexes {
    canonical_gap_id: BTreeMap<String, Vec<usize>>,
    seam_id: BTreeMap<String, Vec<usize>>,
    source_id: BTreeMap<String, Vec<usize>>,
    id: BTreeMap<String, Vec<usize>>,
    dedupe_key: BTreeMap<String, Vec<usize>>,
    fallback: BTreeMap<String, Vec<usize>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MatchResult {
    Match { index: usize, matched_by: String },
    Ambiguous { matched_by: String, count: usize },
    None,
}

struct UpdateBaselineValueInput<'a> {
    kept_entries: Vec<Value>,
    valid_entries: usize,
    invalid_entries: usize,
    current_gate_decision_path: &'a str,
    remove_resolved: bool,
    removed_resolved: usize,
    ignored_new_current: usize,
    migrated_legacy_identities: &'a [Value],
    warnings: &'a [String],
}

pub(crate) fn build_baseline_update_remove_resolved(
    input: BaselineUpdateInput,
) -> Result<BaselineUpdateReport, String> {
    let mut baseline = parse_json(
        "baseline",
        &input.baseline_path,
        &input.baseline_json,
        BASELINE_KIND,
    )?;
    let current = parse_json(
        "current gate-decision",
        &input.current_gate_decision_path,
        &input.current_gate_decision_json,
        "",
    )?;
    let baseline_entries = baseline
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "baseline update input {} is missing entries array",
                input.baseline_path
            )
        })?;
    let current_decisions = current
        .get("decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "baseline update current gate-decision input {} is missing decisions array",
                input.current_gate_decision_path
            )
        })?;

    let current_records = current_decisions
        .iter()
        .filter_map(current_record_from_value)
        .collect::<Vec<_>>();
    let current_root = string_field(current.get("root"));
    let indexes = build_current_indexes(&current_records);
    let mut matched_current = BTreeSet::new();
    let mut kept_entries = Vec::new();
    let mut removed_resolved = 0usize;
    let mut preserved_invalid = 0usize;
    let mut preserved_stale = 0usize;
    let mut migrated = Vec::new();
    let mut warnings = Vec::new();

    for entry in baseline_entries {
        let identity = baseline_identity_from_value(entry);
        if !identity.has_stable_value() {
            preserved_invalid += 1;
            warnings.push(
                "preserved malformed baseline entry without stable identity during shrink-only update"
                    .to_string(),
            );
            kept_entries.push(entry.clone());
            continue;
        }

        match match_current_decision(&identity, &indexes) {
            MatchResult::Match { index, matched_by } if matched_current.contains(&index) => {
                preserved_stale += 1;
                warnings.push(format!(
                    "preserved baseline entry {} because it also matched an already joined current decision by {matched_by}",
                    identity.sort_key()
                ));
                kept_entries.push(entry.clone());
            }
            MatchResult::Match { index, matched_by } => {
                // STALE-JOIN REFUSAL (issue #1964, review): the delta surface
                // refuses diverged and cross-root fallback joins as stale, so
                // the update must agree instead of consuming the current
                // decision as historical. The entry is preserved unchanged
                // and the current index stays unmatched.
                if matched_by == "fallback"
                    && let Some(current_record) = current_records.get(index)
                    && let Some(stale_reason) =
                        stale_fallback_join_reason(entry, current_record, current_root.as_deref())
                {
                    preserved_stale += 1;
                    warnings.push(format!(
                        "preserved baseline entry {} unchanged: {stale_reason}",
                        identity.sort_key()
                    ));
                    kept_entries.push(entry.clone());
                    continue;
                }
                if matched_by == "fallback" {
                    warnings.push(format!(
                        "preserved baseline entry {} using fallback path/line/static_class identity",
                        identity.sort_key()
                    ));
                }
                matched_current.insert(index);
                let mut kept = entry.clone();
                // DETERMINISTIC MIGRATION (issue #1964): an unambiguous
                // fallback join against a current decision that carries a
                // canonical gap id can replace the reviewed legacy identity
                // with the canonical one. Path, line, decision, evidence, and
                // review travel untouched; no new current debt is adopted.
                if input.migrate_legacy_identities
                    && matched_by == "fallback"
                    && let Some(current_record) = current_records.get(index)
                {
                    match legacy_migration_for(entry, current_record, current_root.as_deref()) {
                        Some(LegacyMigration::Apply {
                            canonical_gap_id,
                            legacy_identity,
                        }) => {
                            migrate_entry_identity_to_canonical(&mut kept, &canonical_gap_id);
                            warnings.push(format!(
                                "migrated baseline entry {legacy_identity} from legacy fallback identity to canonical gap id `{canonical_gap_id}`"
                            ));
                            migrated.push(json!({
                                "legacy_identity": legacy_identity,
                                "canonical_gap_id": canonical_gap_id,
                                "path": string_field(entry.get("path")),
                            }));
                        }
                        Some(LegacyMigration::Refuse(reason)) => warnings.push(format!(
                            "preserved baseline entry {} without legacy migration: {reason}",
                            identity.sort_key()
                        )),
                        None => {}
                    }
                }
                kept_entries.push(kept);
            }
            MatchResult::Ambiguous { matched_by, count } => {
                preserved_stale += 1;
                warnings.push(format!(
                    "preserved baseline entry {} because it matched {count} current decisions by {matched_by}",
                    identity.sort_key()
                ));
                kept_entries.push(entry.clone());
            }
            MatchResult::None => {
                // HONEST SHRINK (issue #1964, review): removal happens only
                // with --remove-resolved. A migration-only run preserves
                // unmatched entries instead of silently shrinking the
                // reviewed baseline.
                if input.remove_resolved {
                    removed_resolved += 1;
                } else {
                    warnings.push(format!(
                        "preserved baseline entry {} without --remove-resolved; rerun with --remove-resolved to drop resolved reviewed debt",
                        identity.sort_key()
                    ));
                    kept_entries.push(entry.clone());
                }
            }
        }
    }

    let ignored_new_current = current_records
        .iter()
        .enumerate()
        .filter(|(index, _record)| !matched_current.contains(index))
        .count();
    let before_entries = baseline_entries.len();
    let after_entries = kept_entries.len();
    let valid_entries = kept_entries
        .iter()
        .filter(|entry| baseline_identity_from_value(entry).has_stable_value())
        .count();
    let invalid_entries = after_entries.saturating_sub(valid_entries);
    update_baseline_value(
        &mut baseline,
        UpdateBaselineValueInput {
            kept_entries,
            valid_entries,
            invalid_entries,
            current_gate_decision_path: &input.current_gate_decision_path,
            remove_resolved: input.remove_resolved,
            removed_resolved,
            ignored_new_current,
            migrated_legacy_identities: &migrated,
            warnings: &warnings,
        },
    )?;

    Ok(BaselineUpdateReport {
        updated_baseline: baseline,
        before_entries,
        after_entries,
        removed_resolved,
        preserved_invalid,
        preserved_stale,
        ignored_new_current,
        migrated_legacy_identities: migrated.len(),
        warnings,
    })
}

pub(crate) fn render_baseline_update_json(report: &BaselineUpdateReport) -> Result<String, String> {
    serde_json::to_string_pretty(&report.updated_baseline)
        .map_err(|err| format!("failed to render updated baseline JSON: {err}"))
}

pub(crate) fn baseline_update_before_entry_count(report: &BaselineUpdateReport) -> usize {
    report.before_entries
}

pub(crate) fn baseline_update_after_entry_count(report: &BaselineUpdateReport) -> usize {
    report.after_entries
}

pub(crate) fn baseline_update_removed_resolved_count(report: &BaselineUpdateReport) -> usize {
    report.removed_resolved
}

pub(crate) fn baseline_update_ignored_new_current_count(report: &BaselineUpdateReport) -> usize {
    report.ignored_new_current
}

pub(crate) fn baseline_update_migrated_count(report: &BaselineUpdateReport) -> usize {
    report.migrated_legacy_identities
}

pub(crate) fn baseline_update_warning_count(report: &BaselineUpdateReport) -> usize {
    report.warnings.len()
}

pub(crate) use crate::output::path::display_path;

fn parse_json(
    label: &str,
    path: &str,
    json_text: &str,
    expected_kind: &str,
) -> Result<Value, String> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|err| format!("parse {label} {path} failed: {err}"))?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} {path} missing schema_version"))?;
    if schema_version != SCHEMA_VERSION {
        return Err(format!(
            "{label} {path} has unsupported schema_version `{schema_version}`; expected `{SCHEMA_VERSION}`"
        ));
    }
    if !expected_kind.is_empty() {
        let kind = value
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{label} {path} missing kind"))?;
        if kind != expected_kind {
            return Err(format!(
                "{label} {path} has unsupported kind `{kind}`; expected `{expected_kind}`"
            ));
        }
    }
    Ok(value)
}

fn update_baseline_value(
    baseline: &mut Value,
    input: UpdateBaselineValueInput<'_>,
) -> Result<(), String> {
    let entry_count = input.kept_entries.len();
    let Some(object) = baseline.as_object_mut() else {
        return Err("baseline update input must be a JSON object".to_string());
    };
    object.insert("entries".to_string(), Value::Array(input.kept_entries));
    object.insert(
        "update".to_string(),
        json!({
            "remove_resolved": input.remove_resolved,
            "current_gate_decision": input.current_gate_decision_path,
            "removed_resolved": input.removed_resolved,
            "ignored_new_current": input.ignored_new_current,
            "migrated_legacy_identities": input.migrated_legacy_identities,
        }),
    );
    object.insert(
        "limits_note".to_string(),
        Value::String(LIMITS_NOTE.to_string()),
    );

    let summary = object
        .entry("summary".to_string())
        .or_insert_with(|| json!({}));
    if !summary.is_object() {
        *summary = json!({});
    }
    let Some(summary_object) = summary.as_object_mut() else {
        return Err("baseline update summary must be a JSON object".to_string());
    };
    summary_object.insert("entries".to_string(), json!(entry_count));
    summary_object.insert("included".to_string(), json!(input.valid_entries));
    let skipped = summary_object
        .entry("skipped".to_string())
        .or_insert_with(|| json!({}));
    if !skipped.is_object() {
        *skipped = json!({});
    }
    let Some(skipped_object) = skipped.as_object_mut() else {
        return Err("baseline update skipped summary must be a JSON object".to_string());
    };
    skipped_object.insert("malformed".to_string(), json!(input.invalid_entries));

    let warnings_value = object
        .entry("warnings".to_string())
        .or_insert_with(|| json!([]));
    if !warnings_value.is_array() {
        *warnings_value = json!([]);
    }
    let Some(warnings_array) = warnings_value.as_array_mut() else {
        return Err("baseline update warnings must be a JSON array".to_string());
    };
    warnings_array.extend(input.warnings.iter().cloned().map(Value::String));

    Ok(())
}

fn current_record_from_value(value: &Value) -> Option<CurrentRecord> {
    let path = string_field(value.pointer("/placement/path"));
    let line = value.pointer("/placement/line").and_then(Value::as_u64);
    let static_class = string_field(value.get("static_class"));
    let identity = Identity {
        canonical_gap_id: canonical_gap_id_from_value(value),
        seam_id: string_field(value.get("seam_id")),
        source_id: string_field(value.get("source_id")),
        id: string_field(value.get("id")),
        dedupe_key: string_field(value.get("dedupe_key")),
        fallback: fallback_identity(path.as_deref(), line, static_class.as_deref()),
    };
    if !identity.has_stable_value() {
        return None;
    }
    Some(CurrentRecord { identity })
}

fn baseline_identity_from_value(value: &Value) -> Identity {
    let identity_value = match value.get("identity") {
        Some(identity) => identity,
        None => value,
    };
    let path =
        string_field(value.get("path")).or_else(|| string_field(value.pointer("/placement/path")));
    let line = value
        .get("line")
        .and_then(Value::as_u64)
        .or_else(|| value.pointer("/placement/line").and_then(Value::as_u64));
    let static_class = string_field(value.get("static_class"));
    Identity {
        canonical_gap_id: canonical_gap_id_from_value(value),
        seam_id: string_field(identity_value.get("seam_id"))
            .or_else(|| string_field(value.get("seam_id"))),
        source_id: string_field(identity_value.get("source_id"))
            .or_else(|| string_field(value.get("source_id"))),
        id: string_field(identity_value.get("id")).or_else(|| string_field(value.get("id"))),
        dedupe_key: string_field(identity_value.get("dedupe_key"))
            .or_else(|| string_field(value.get("dedupe_key"))),
        fallback: string_field(identity_value.get("fallback"))
            .or_else(|| fallback_identity(path.as_deref(), line, static_class.as_deref())),
    }
}

fn build_current_indexes(records: &[CurrentRecord]) -> CurrentIndexes {
    let mut indexes = CurrentIndexes {
        canonical_gap_id: BTreeMap::new(),
        seam_id: BTreeMap::new(),
        source_id: BTreeMap::new(),
        id: BTreeMap::new(),
        dedupe_key: BTreeMap::new(),
        fallback: BTreeMap::new(),
    };
    for (index, record) in records.iter().enumerate() {
        push_index(
            &mut indexes.canonical_gap_id,
            record.identity.canonical_gap_id.as_ref(),
            index,
        );
        push_index(
            &mut indexes.seam_id,
            record.identity.seam_id.as_ref(),
            index,
        );
        push_index(
            &mut indexes.source_id,
            record.identity.source_id.as_ref(),
            index,
        );
        push_index(&mut indexes.id, record.identity.id.as_ref(), index);
        push_index(
            &mut indexes.dedupe_key,
            record.identity.dedupe_key.as_ref(),
            index,
        );
        push_index(
            &mut indexes.fallback,
            record.identity.fallback.as_ref(),
            index,
        );
    }
    indexes
}

fn push_index(index: &mut BTreeMap<String, Vec<usize>>, key: Option<&String>, value: usize) {
    if let Some(key) = key {
        index.entry(key.clone()).or_default().push(value);
    }
}

fn match_current_decision(identity: &Identity, indexes: &CurrentIndexes) -> MatchResult {
    let mut first_ambiguity = None;
    for (method, key, index) in [
        (
            "canonical_gap_id",
            identity.canonical_gap_id.as_ref(),
            &indexes.canonical_gap_id,
        ),
        ("seam_id", identity.seam_id.as_ref(), &indexes.seam_id),
        ("source_id", identity.source_id.as_ref(), &indexes.source_id),
        ("id", identity.id.as_ref(), &indexes.id),
        (
            "dedupe_key",
            identity.dedupe_key.as_ref(),
            &indexes.dedupe_key,
        ),
        ("fallback", identity.fallback.as_ref(), &indexes.fallback),
    ] {
        if let Some(key) = key
            && let Some(matches) = index.get(key)
        {
            if matches.len() == 1 {
                return MatchResult::Match {
                    index: matches[0],
                    matched_by: method.to_string(),
                };
            }
            if first_ambiguity.is_none() {
                first_ambiguity = Some(MatchResult::Ambiguous {
                    matched_by: method.to_string(),
                    count: matches.len(),
                });
            }
        }
    }
    first_ambiguity.unwrap_or(MatchResult::None)
}

fn fallback_identity(
    path: Option<&str>,
    line: Option<u64>,
    static_class: Option<&str>,
) -> Option<String> {
    match (path, line, static_class) {
        (Some(path), Some(line), Some(static_class)) => Some(format!(
            "{}:{line}:{static_class}",
            path.replace('\\', "/").trim_start_matches("./")
        )),
        _ => None,
    }
}

/// Decide the deterministic legacy migration for one fallback-joined baseline
/// entry (issue #1964). `None` means no migration applies and the preserved
/// entry needs no extra warning beyond the standard fallback notice.
fn legacy_migration_for(
    entry: &Value,
    current: &CurrentRecord,
    current_root: Option<&str>,
) -> Option<LegacyMigration> {
    let identity = baseline_identity_from_value(entry);
    let legacy_identity = identity.fallback?;
    if let Some(reviewed) = identity.canonical_gap_id {
        return Some(LegacyMigration::Refuse(format!(
            "entry already carries reviewed canonical gap id `{reviewed}`; refusing to overwrite it"
        )));
    }
    let canonical_gap_id = current.identity.canonical_gap_id.clone()?;
    if roots_disagree(string_field(entry.get("root")).as_deref(), current_root) {
        let entry_root = string_field(entry.get("root")).unwrap_or_default();
        let current_root = current_root.unwrap_or_default();
        return Some(LegacyMigration::Refuse(format!(
            "entry belongs to another repository root ({entry_root} -> {current_root}); cross-root migration is refused"
        )));
    }
    Some(LegacyMigration::Apply {
        canonical_gap_id,
        legacy_identity,
    })
}

/// True when both roots are present and name different repositories after
/// spelling normalization (issue #1964, review). Shared by the stale-join
/// refusal and the migration refusal so delta and update agree.
fn roots_disagree(entry_root: Option<&str>, current_root: Option<&str>) -> bool {
    match (entry_root, current_root) {
        (Some(entry_root), Some(current_root)) => {
            super::baseline_delta::normalize_root_for_comparison(entry_root)
                != super::baseline_delta::normalize_root_for_comparison(current_root)
        }
        _ => false,
    }
}

/// The stale-join reason when a fallback-joined update pair must not consume
/// the current decision (issue #1964, review): canonical divergence or a
/// cross-root join. Mirrors the delta refusal so `baseline diff` and
/// `baseline update` tell the same story about the same evidence.
fn stale_fallback_join_reason(
    entry: &Value,
    current: &CurrentRecord,
    current_root: Option<&str>,
) -> Option<String> {
    let identity = baseline_identity_from_value(entry);
    match (
        identity.canonical_gap_id.as_deref(),
        current.identity.canonical_gap_id.as_deref(),
    ) {
        (Some(old), Some(new)) if old != new => {
            return Some(format!(
                "canonical identity diverged ({old} -> {new}); refresh the baseline identity instead of treating the current gap as historical"
            ));
        }
        _ => {}
    }
    if roots_disagree(string_field(entry.get("root")).as_deref(), current_root) {
        let entry_root = string_field(entry.get("root")).unwrap_or_default();
        let current_root = current_root.unwrap_or_default();
        return Some(format!(
            "entry belongs to another repository root ({entry_root} -> {current_root}); the current gap is evaluated on this repository's own history"
        ));
    }
    None
}

/// Write the migrated canonical gap id into a kept baseline entry, preserving
/// every other field (path, line, decision, evidence, review, and the legacy
/// fallback string for traceability). Handles both the `identity`-object and
/// the flat legacy entry shapes.
fn migrate_entry_identity_to_canonical(entry: &mut Value, canonical_gap_id: &str) {
    // Explicit object insert, never subscript assignment (review): the
    // identity object of a legacy entry typically lacks `canonical_gap_id`,
    // which is exactly what migration adds.
    if let Some(object) = entry.get_mut("identity").and_then(Value::as_object_mut) {
        object.insert(
            "canonical_gap_id".to_string(),
            Value::String(canonical_gap_id.to_string()),
        );
        return;
    }
    if let Some(object) = entry.as_object_mut() {
        object.insert(
            "canonical_gap_id".to_string(),
            Value::String(canonical_gap_id.to_string()),
        );
    }
}

fn canonical_gap_id_from_value(value: &Value) -> Option<String> {
    string_field(value.get("canonical_gap_id"))
        .or_else(|| string_field(value.pointer("/identity/canonical_gap_id")))
        .or_else(|| string_field(value.pointer("/evidence_record/canonical_gap_id")))
}

fn string_field(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{
        BaselineUpdateInput, baseline_update_after_entry_count, baseline_update_before_entry_count,
        baseline_update_ignored_new_current_count, baseline_update_migrated_count,
        baseline_update_removed_resolved_count, baseline_update_warning_count,
        build_baseline_update_remove_resolved, render_baseline_update_json,
    };

    #[test]
    fn baseline_update_removes_resolved_without_adopting_new_current_debt() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "tool": "ripr",
          "kind": "gate_baseline",
          "created_at": "unix_ms:1",
          "source_report": "old-gate.json",
          "mode": "baseline-check",
          "reviewed": false,
          "summary": {
            "entries": 3,
            "included": 2,
            "skipped": {"suppressed": 0, "not_applicable": 0, "malformed": 1, "other": 0}
          },
          "entries": [
            {
              "identity": {"seam_id": "same"},
              "path": "src/same.rs",
              "line": 1,
              "static_class": "weakly_gripped",
              "review": {
                "owner": "test-platform",
                "reason": "known baseline debt",
                "created_at": "2026-05-08T00:00:00Z",
                "review_after": "2026-08-08T00:00:00Z",
                "source": "target/ripr/reports/gate-decision.json"
              }
            },
            {"identity": {"seam_id": "gone"}, "path": "src/gone.rs", "line": 2, "static_class": "weakly_gripped"},
            {"identity": {}, "decision": "advisory"}
          ],
          "warnings": []
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {"decision": "advisory", "seam_id": "same", "static_class": "weakly_gripped", "placement": {"path": "src/same.rs", "line": 1}, "evidence": {}},
            {"decision": "blocking", "seam_id": "new", "static_class": "weakly_gripped", "placement": {"path": "src/new.rs", "line": 9}, "evidence": {}}
          ]
        }"#;

        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: ".ripr/gate-baseline.json".to_string(),
            current_gate_decision_path: "target/ripr/reports/gate-decision.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: true,
            migrate_legacy_identities: false,
        })?;
        let rendered = render_baseline_update_json(&report)?;
        assert_eq!(baseline_update_before_entry_count(&report), 3);
        assert_eq!(baseline_update_after_entry_count(&report), 2);
        assert_eq!(baseline_update_removed_resolved_count(&report), 1);
        assert_eq!(baseline_update_ignored_new_current_count(&report), 1);
        assert!(rendered.contains("\"seam_id\": \"same\""));
        assert!(rendered.contains("\"owner\": \"test-platform\""));
        assert!(rendered.contains("\"review_after\": \"2026-08-08T00:00:00Z\""));
        assert!(rendered.contains("\"source\": \"target/ripr/reports/gate-decision.json\""));
        assert!(!rendered.contains("\"seam_id\": \"gone\""));
        assert!(!rendered.contains("\"seam_id\": \"new\""));
        assert!(rendered.contains("\"entries\": 2"));
        assert!(rendered.contains("\"included\": 1"));
        assert!(rendered.contains("\"malformed\": 1"));
        assert!(rendered.contains("\"removed_resolved\": 1"));
        assert!(rendered.contains("\"ignored_new_current\": 1"));
        assert!(rendered.contains("preserved malformed baseline entry"));
        Ok(())
    }

    #[test]
    fn baseline_update_preserves_ambiguous_and_fallback_matches() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "kind": "gate_baseline",
          "entries": [
            {"identity": {"fallback": "src/fallback.rs:7:weakly_gripped"}, "path": "src/fallback.rs", "line": 7, "static_class": "weakly_gripped"},
            {"identity": {"fallback": "src/ambiguous.rs:8:weakly_gripped"}, "path": "src/ambiguous.rs", "line": 8, "static_class": "weakly_gripped"}
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/fallback.rs", "line": 7}},
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/ambiguous.rs", "line": 8}},
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/ambiguous.rs", "line": 8}}
          ]
        }"#;

        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: true,
            migrate_legacy_identities: false,
        })?;
        let rendered = render_baseline_update_json(&report)?;
        assert_eq!(baseline_update_removed_resolved_count(&report), 0);
        assert_eq!(baseline_update_warning_count(&report), 2);
        assert!(rendered.contains("src/fallback.rs:7:weakly_gripped"));
        assert!(rendered.contains("fallback path/line/static_class"));
        assert!(rendered.contains("matched 2 current decisions by fallback"));
        Ok(())
    }

    #[test]
    fn baseline_update_preserves_refactored_entry_matched_by_canonical_gap_id() -> Result<(), String>
    {
        let baseline = r#"{
          "schema_version": "0.1",
          "kind": "gate_baseline",
          "entries": [
            {
              "identity": {
                "canonical_gap_id": "pricing::discount::threshold_equality",
                "seam_id": "old-seam"
              },
              "path": "src/old.rs",
              "line": 10,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {
              "decision": "advisory",
              "seam_id": "new-seam",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/new.rs", "line": 88},
              "evidence_record": {
                "canonical_gap_id": "pricing::discount::threshold_equality"
              }
            }
          ]
        }"#;

        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: true,
            migrate_legacy_identities: false,
        })?;
        let rendered = render_baseline_update_json(&report)?;
        assert_eq!(baseline_update_removed_resolved_count(&report), 0);
        assert!(
            rendered.contains("\"canonical_gap_id\": \"pricing::discount::threshold_equality\"")
        );
        assert!(rendered.contains("\"seam_id\": \"old-seam\""));
        Ok(())
    }

    #[test]
    fn baseline_update_uses_unique_seam_after_shared_canonical_gap_is_ambiguous()
    -> Result<(), String> {
        let baseline = r#"{"schema_version":"0.1","kind":"gate_baseline","entries":[
          {"identity":{"canonical_gap_id":"gap:shared","seam_id":"seam:a"}},
          {"identity":{"canonical_gap_id":"gap:shared","seam_id":"seam:b"}}
        ]}"#;
        let current = r#"{"schema_version":"0.1","decisions":[
          {"decision":"advisory","canonical_gap_id":"gap:shared","seam_id":"seam:a","static_class":"weakly_gripped","placement":{"path":"src/a.rs","line":1}},
          {"decision":"advisory","canonical_gap_id":"gap:shared","seam_id":"seam:b","static_class":"weakly_gripped","placement":{"path":"src/b.rs","line":2}}
        ]}"#;
        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: true,
            migrate_legacy_identities: false,
        })?;
        assert_eq!(baseline_update_before_entry_count(&report), 2);
        assert_eq!(baseline_update_after_entry_count(&report), 2);
        assert_eq!(baseline_update_removed_resolved_count(&report), 0);
        assert_eq!(baseline_update_ignored_new_current_count(&report), 0);
        assert_eq!(baseline_update_warning_count(&report), 0);
        Ok(())
    }

    #[test]
    fn baseline_update_rejects_invalid_inputs() {
        let valid_baseline = r#"{"schema_version":"0.1","kind":"gate_baseline","entries":[]}"#;
        let valid_current = r#"{"schema_version":"0.1","decisions":[]}"#;
        for (baseline_json, current_json, expected) in [
            ("{", valid_current, "parse baseline baseline.json failed"),
            (
                r#"{"schema_version":"9","kind":"gate_baseline","entries":[]}"#,
                valid_current,
                "unsupported schema_version",
            ),
            (
                r#"{"schema_version":"0.1","kind":"other","entries":[]}"#,
                valid_current,
                "unsupported kind",
            ),
            (
                r#"{"schema_version":"0.1","kind":"gate_baseline"}"#,
                valid_current,
                "missing entries array",
            ),
            (
                valid_baseline,
                r#"{"schema_version":"0.1"}"#,
                "missing decisions array",
            ),
            (
                valid_baseline,
                r#"{"schema_version":"9","decisions":[]}"#,
                "unsupported schema_version",
            ),
        ] {
            let result = build_baseline_update_remove_resolved(BaselineUpdateInput {
                baseline_path: "baseline.json".to_string(),
                current_gate_decision_path: "current.json".to_string(),
                baseline_json: baseline_json.to_string(),
                current_gate_decision_json: current_json.to_string(),
                remove_resolved: true,
                migrate_legacy_identities: false,
            });
            assert!(
                matches!(result, Err(ref message) if message.contains(expected)),
                "{result:?}"
            );
        }
    }

    #[test]
    fn baseline_update_migrates_legacy_identity_without_adopting_new_debt() -> Result<(), String> {
        // Issue #1964, fixture 7 + migration route: the update removes
        // resolved debt, migrates the reviewed legacy identity to the
        // unambiguously joined canonical gap id, preserves everything else,
        // and never adopts new current debt.
        let baseline = r#"{
          "schema_version": "0.1",
          "kind": "gate_baseline",
          "entries": [
            {
              "identity": {"fallback": "src/legacy.rs:7:weakly_gripped"},
              "path": "src/legacy.rs",
              "line": 7,
              "static_class": "weakly_gripped",
              "decision": "advisory",
              "evidence": {"missing_discriminator": "legacy == 7"},
              "review": {"reviewed": true, "owner": "test-platform", "reason": "known debt"}
            },
            {
              "identity": {"seam_id": "gone"},
              "path": "src/gone.rs",
              "line": 2,
              "static_class": "weakly_gripped"
            },
            {
              "identity": {"fallback": "src/plain.rs:9:weakly_gripped"},
              "path": "src/plain.rs",
              "line": 9,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "root": ".",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/legacy.rs", "line": 7},
              "evidence_record": {"canonical_gap_id": "legacy::gap::seven"},
              "evidence": {}
            },
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/plain.rs", "line": 9},
              "evidence": {}
            },
            {
              "decision": "blocking",
              "seam_id": "brand-new",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/new.rs", "line": 1},
              "evidence": {}
            }
          ]
        }"#;

        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: true,
            migrate_legacy_identities: true,
        })?;
        let rendered = render_baseline_update_json(&report)?;
        // Resolved debt removed, legacy entry migrated, plain fallback entry
        // preserved untouched, brand-new current debt ignored.
        assert_eq!(baseline_update_before_entry_count(&report), 3);
        assert_eq!(baseline_update_after_entry_count(&report), 2);
        assert_eq!(baseline_update_removed_resolved_count(&report), 1);
        assert_eq!(baseline_update_ignored_new_current_count(&report), 1);
        assert_eq!(baseline_update_migrated_count(&report), 1);
        assert!(
            rendered.contains("\"canonical_gap_id\": \"legacy::gap::seven\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"fallback\": \"src/legacy.rs:7:weakly_gripped\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"owner\": \"test-platform\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"missing_discriminator\": \"legacy == 7\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("migrated baseline entry src/legacy.rs:7:weakly_gripped"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"migrated_legacy_identities\": ["),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"legacy_identity\": \"src/legacy.rs:7:weakly_gripped\""),
            "{rendered}"
        );
        assert!(!rendered.contains("\"seam_id\": \"gone\""), "{rendered}");
        assert!(
            !rendered.contains("\"seam_id\": \"brand-new\""),
            "{rendered}"
        );
        // The unmigratable plain entry is preserved as reviewed: the only
        // canonical gap id in the output is the migrated one (kept entry +
        // review ledger), and the plain fallback entry is untouched.
        assert!(
            rendered.contains("\"fallback\": \"src/plain.rs:9:weakly_gripped\""),
            "{rendered}"
        );
        assert_eq!(
            rendered
                .matches("\"canonical_gap_id\": \"legacy::gap::seven\"")
                .count(),
            2,
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn baseline_update_migration_is_off_by_default() -> Result<(), String> {
        // Without the explicit compatibility flag the reviewed legacy identity
        // is preserved as-is: migration is opt-in, never silent default.
        let baseline = r#"{
          "schema_version": "0.1",
          "kind": "gate_baseline",
          "entries": [
            {"identity": {"fallback": "src/legacy.rs:7:weakly_gripped"}, "path": "src/legacy.rs", "line": 7, "static_class": "weakly_gripped"}
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/legacy.rs", "line": 7},
              "evidence_record": {"canonical_gap_id": "legacy::gap::seven"},
              "evidence": {}
            }
          ]
        }"#;

        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: true,
            migrate_legacy_identities: false,
        })?;
        let rendered = render_baseline_update_json(&report)?;
        assert_eq!(baseline_update_migrated_count(&report), 0);
        assert!(!rendered.contains("legacy::gap::seven"), "{rendered}");
        assert!(!rendered.contains("migrated baseline entry"), "{rendered}");
        assert!(
            rendered.contains("\"migrated_legacy_identities\": []"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn baseline_update_migration_refuses_conflicts_and_foreign_roots() -> Result<(), String> {
        // A reviewed canonical identity is never overwritten: the conflict
        // entry joins by fallback (the current candidate names no canonical)
        // and migration refuses with a loud reason. A foreign root never
        // joins at all: the stale-join refusal preserves the entry unchanged
        // and leaves the current decision unmatched.
        let baseline = r#"{
          "schema_version": "0.1",
          "kind": "gate_baseline",
          "entries": [
            {
              "identity": {
                "canonical_gap_id": "reviewed::canonical",
                "fallback": "src/conflict.rs:1:weakly_gripped"
              },
              "path": "src/conflict.rs",
              "line": 1,
              "static_class": "weakly_gripped"
            },
            {
              "identity": {"fallback": "src/foreign.rs:2:weakly_gripped"},
              "path": "src/foreign.rs",
              "line": 2,
              "static_class": "weakly_gripped",
              "root": "/other/repo"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "root": ".",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/conflict.rs", "line": 1},
              "evidence": {}
            },
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/foreign.rs", "line": 2},
              "evidence_record": {"canonical_gap_id": "foreign::gap"},
              "evidence": {}
            }
          ]
        }"#;

        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: true,
            migrate_legacy_identities: true,
        })?;
        let rendered = render_baseline_update_json(&report)?;
        assert_eq!(baseline_update_migrated_count(&report), 0);
        assert_eq!(baseline_update_after_entry_count(&report), 2);
        assert!(
            rendered.contains("\"canonical_gap_id\": \"reviewed::canonical\""),
            "{rendered}"
        );
        assert!(!rendered.contains("foreign::gap"), "{rendered}");
        assert!(rendered.contains("refusing to overwrite it"), "{rendered}");
        assert!(rendered.contains("another repository root"), "{rendered}");
        assert!(rendered.contains("/other/repo -> ."), "{rendered}");
        assert_eq!(baseline_update_ignored_new_current_count(&report), 1);
        Ok(())
    }

    #[test]
    fn baseline_update_migrate_only_preserves_unmatched_entries() -> Result<(), String> {
        // Issue #1964, review: a migration-only run (no --remove-resolved)
        // migrates what it can and preserves everything else. The reviewed
        // baseline never shrinks without the explicit shrink flag.
        let baseline = r#"{
          "schema_version": "0.1",
          "kind": "gate_baseline",
          "entries": [
            {
              "identity": {"fallback": "src/legacy.rs:7:weakly_gripped"},
              "path": "src/legacy.rs",
              "line": 7,
              "static_class": "weakly_gripped"
            },
            {
              "identity": {"seam_id": "gone"},
              "path": "src/gone.rs",
              "line": 2,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/legacy.rs", "line": 7},
              "evidence_record": {"canonical_gap_id": "legacy::gap::seven"},
              "evidence": {}
            }
          ]
        }"#;

        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: false,
            migrate_legacy_identities: true,
        })?;
        let rendered = render_baseline_update_json(&report)?;
        assert_eq!(baseline_update_before_entry_count(&report), 2);
        assert_eq!(baseline_update_after_entry_count(&report), 2);
        assert_eq!(baseline_update_removed_resolved_count(&report), 0);
        assert_eq!(baseline_update_migrated_count(&report), 1);
        assert!(
            rendered.contains("\"canonical_gap_id\": \"legacy::gap::seven\""),
            "{rendered}"
        );
        assert!(rendered.contains("\"seam_id\": \"gone\""), "{rendered}");
        assert!(
            rendered.contains("\"remove_resolved\": false"),
            "{rendered}"
        );
        assert!(rendered.contains("without --remove-resolved"), "{rendered}");
        Ok(())
    }

    #[test]
    fn baseline_update_refuses_diverged_fallback_join_as_stale() -> Result<(), String> {
        // Issue #1964, review: the update agrees with the delta refusal. A
        // diverged fallback join preserves the entry unchanged and leaves
        // the current decision unmatched (ignored, never adopted).
        let baseline = r#"{
          "schema_version": "0.1",
          "kind": "gate_baseline",
          "entries": [
            {
              "identity": {
                "canonical_gap_id": "pricing::discount::old_rule",
                "fallback": "src/pricing.rs:88:weakly_gripped"
              },
              "path": "src/pricing.rs",
              "line": 88,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/pricing.rs", "line": 88},
              "evidence_record": {"canonical_gap_id": "pricing::discount::new_rule"},
              "evidence": {}
            }
          ]
        }"#;

        let report = build_baseline_update_remove_resolved(BaselineUpdateInput {
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: baseline.to_string(),
            current_gate_decision_json: current.to_string(),
            remove_resolved: true,
            migrate_legacy_identities: true,
        })?;
        let rendered = render_baseline_update_json(&report)?;
        assert_eq!(baseline_update_after_entry_count(&report), 1);
        assert_eq!(baseline_update_removed_resolved_count(&report), 0);
        assert_eq!(baseline_update_ignored_new_current_count(&report), 1);
        assert_eq!(baseline_update_migrated_count(&report), 0);
        assert!(
            rendered.contains("\"canonical_gap_id\": \"pricing::discount::old_rule\""),
            "{rendered}"
        );
        // The new canonical appears only in the disclosure warning, never as
        // a joined identity: the entry keeps the old canonical.
        assert!(
            !rendered.contains("\"canonical_gap_id\": \"pricing::discount::new_rule\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("canonical identity diverged"),
            "{rendered}"
        );
        Ok(())
    }
}
