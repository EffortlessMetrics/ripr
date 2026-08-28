//! Producer-owned base/head comparison for PR evidence.
//!
//! This module materializes each revision once, reads the canonical repo
//! exposure snapshot produced by the same ripr binary, and then delegates
//! attribution rules to ripr::domain::compare_fixture_delta. Renderers and
//! gate policy must consume the resulting artifact rather than infer
//! causality from paths or line proximity.

use super::write_parented_file;
use crate::run::{
    capture_output_with_timeout, run_output_owned, run_output_owned_with_timeout,
    tool_build_timeout,
};
use ripr::domain::{
    AttributionBasis, CanonicalEvidenceState, ComparisonConfidence, ComparisonCoverage,
    DeltaAttribution, GapState, OracleStrength, compare_fixture_delta,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) const CANONICAL_DELTA_JSON: &str = "target/ripr/pr/canonical-delta.json";
const CANONICAL_DELTA_SCHEMA_VERSION: &str = "0.1";
const SNAPSHOT_TIMEOUT: Duration = Duration::from_mins(2);

pub(crate) fn write_canonical_delta(
    repo: &Path,
    base: &str,
    head: &str,
    changed_files: &[String],
    root: &str,
) -> Result<(), String> {
    let artifact = build_canonical_delta(repo, base, head, changed_files, root);
    let rendered = serde_json::to_string_pretty(&artifact)
        .map_err(|err| format!("serialize canonical delta: {err}"))?;
    write_parented_file(
        &repo.join(CANONICAL_DELTA_JSON),
        CANONICAL_DELTA_JSON,
        format!("{rendered}\n"),
    )?;
    println!("Wrote {CANONICAL_DELTA_JSON}");
    Ok(())
}

fn build_canonical_delta(
    repo: &Path,
    base: &str,
    head: &str,
    changed_files: &[String],
    root: &str,
) -> Value {
    let binary = resolve_ripr_binary(repo);
    let changed_files = changed_files
        .iter()
        .map(|path| normalize_path(path))
        .collect::<BTreeSet<_>>();

    let (base_snapshot, head_snapshot) = match binary {
        Ok(binary) => (
            materialize_snapshot(repo, base, "base", &binary, root),
            materialize_snapshot(repo, head, "head", &binary, root),
        ),
        Err(err) => (
            Snapshot::unavailable(format!("RIPR binary unavailable: {err}")),
            Snapshot::unavailable(format!("RIPR binary unavailable: {err}")),
        ),
    };

    compare_snapshots(base, head, &base_snapshot, &head_snapshot, &changed_files)
}

fn resolve_ripr_binary(repo: &Path) -> Result<String, String> {
    if let Ok(binary) = env::var("RIPR_BIN") {
        if binary.trim().is_empty() {
            return Err("RIPR_BIN is set but empty".to_string());
        }
        return Ok(binary);
    }

    let build_args = vec![
        "build".to_string(),
        "--manifest-path".to_string(),
        repo.join("Cargo.toml").display().to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--quiet".to_string(),
    ];
    run_output_owned_with_timeout(
        "cargo",
        &build_args,
        tool_build_timeout()?,
        "cargo build of the ripr binary for the canonical PR delta",
    )?;

    let cwd = env::current_dir().map_err(|err| format!("resolve current directory: {err}"))?;
    let target_dir = match env::var_os("CARGO_TARGET_DIR") {
        Some(value) if !value.is_empty() => {
            let value = PathBuf::from(value);
            if value.is_absolute() {
                value
            } else {
                cwd.join(value)
            }
        }
        _ => repo.join("target"),
    };
    let name = if cfg!(windows) { "ripr.exe" } else { "ripr" };
    Ok(target_dir.join("debug").join(name).display().to_string())
}

fn materialize_snapshot(
    repo: &Path,
    revision: &str,
    label: &str,
    binary: &str,
    root: &str,
) -> Snapshot {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let snapshot_dir = repo
        .join("target/ripr/pr/canonical-snapshots")
        .join(format!("{label}-{}-{nonce}", std::process::id()));
    let output_dir = snapshot_dir.join("output");
    let worktree = snapshot_dir.join("worktree");
    let worktree_args = vec![
        "-C".to_string(),
        repo.display().to_string(),
        "worktree".to_string(),
        "add".to_string(),
        "--detach".to_string(),
        worktree.display().to_string(),
        revision.to_string(),
    ];

    let result = (|| {
        run_output_owned("git", &worktree_args)
            .map_err(|err| format!("materialize {label} revision {revision}: {err}"))?;
        let args = vec![
            "pilot".to_string(),
            "--root".to_string(),
            snapshot_root(&worktree, root),
            "--out".to_string(),
            output_dir.display().to_string(),
            "--mode".to_string(),
            "ready".to_string(),
            "--max-seams".to_string(),
            "2000".to_string(),
            "--timeout-ms".to_string(),
            SNAPSHOT_TIMEOUT.as_millis().to_string(),
        ];
        let output = capture_output_with_timeout(
            binary,
            &args,
            &[
                ("RIPR_REPO_EXPOSURE_SEAM_LIMIT", "0"),
                ("RIPR_PILOT_SEAM_BUDGET", "0"),
            ],
            SNAPSHOT_TIMEOUT,
            "canonical repo exposure snapshot",
        )?;
        if output.timed_out {
            return Err(format!("{label} snapshot timed out"));
        }
        if output.status.is_none_or(|status| !status.success()) {
            return Err(format!(
                "{label} snapshot failed\nstdout:\n{}\nstderr:\n{}",
                output.stdout.trim(),
                output.stderr.trim()
            ));
        }
        let path = output_dir.join("repo-exposure.json");
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("read {label} snapshot {}: {err}", path.display()))?;
        parse_snapshot(&text, label)
    })();

    let cleanup = run_output_owned(
        "git",
        &[
            "-C".to_string(),
            repo.display().to_string(),
            "worktree".to_string(),
            "remove".to_string(),
            "--force".to_string(),
            worktree.display().to_string(),
        ],
    );
    let _ = fs::remove_dir_all(&snapshot_dir);

    match (result, cleanup) {
        (Ok(snapshot), Ok(_)) => snapshot,
        (Ok(_), Err(err)) => Snapshot::unavailable(format!("cleanup {label} snapshot: {err}")),
        (Err(err), _) => Snapshot::unavailable(err),
    }
}

fn snapshot_root(worktree: &Path, root: &str) -> String {
    let root_path = Path::new(root);
    if root_path.is_absolute() {
        root.to_string()
    } else {
        worktree.join(root_path).display().to_string()
    }
}

#[derive(Clone, Debug)]
struct Snapshot {
    available: bool,
    status: String,
    limitations: Vec<String>,
    items: Vec<SnapshotItem>,
    unknown_items: usize,
}

impl Snapshot {
    fn unavailable(reason: String) -> Self {
        Self {
            available: false,
            status: "unavailable".to_string(),
            limitations: vec![reason],
            items: Vec::new(),
            unknown_items: 0,
        }
    }

    fn comparable(&self) -> bool {
        self.available && self.unknown_items == 0 && self.limitations.is_empty()
    }
}

#[derive(Clone, Debug)]
struct SnapshotItem {
    gap_id: String,
    semantic_key: String,
    state: CanonicalEvidenceState,
    file: String,
    line: usize,
    seam_id: String,
}

fn parse_snapshot(text: &str, label: &str) -> Result<Snapshot, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|err| format!("parse {label} canonical snapshot: {err}"))?;
    let status = value
        .get("run_status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let limitations = value
        .get("limitations")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.get("category")
                        .and_then(Value::as_str)
                        .unwrap_or("unnamed_limitation")
                        .to_string()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut limitations = limitations;
    if status != "complete" && status != "seam_limit_applied" {
        limitations.push(format!("{label} run status: {status}"));
    }
    let mut items = Vec::new();
    let mut unknown_items = 0usize;
    for seam in value
        .get("seams")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label} snapshot has no seams array"))?
    {
        match snapshot_item(seam) {
            Ok(item) => items.push(item),
            Err(_) => unknown_items += 1,
        }
    }
    Ok(Snapshot {
        available: true,
        status,
        limitations,
        items,
        unknown_items,
    })
}

fn snapshot_item(value: &Value) -> Result<SnapshotItem, String> {
    let record = value
        .get("evidence_record")
        .ok_or_else(|| "missing evidence_record".to_string())?;
    let canonical = record
        .get("canonical_item")
        .ok_or_else(|| "missing canonical_item".to_string())?;
    let gap_id = canonical
        .get("canonical_gap_id")
        .or_else(|| record.get("canonical_gap_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "missing canonical gap id".to_string())?
        .to_string();
    let owner = record
        .get("owner")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing canonical owner".to_string())?;
    let seam_kind = record
        .get("seam_kind")
        .and_then(Value::as_str)
        .or_else(|| value.get("kind").and_then(Value::as_str))
        .unwrap_or("unknown");
    let item_kind = canonical
        .get("canonical_item_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let evidence_class = canonical
        .get("evidence_class")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let discriminator_identity = discriminator_identity(value, canonical, evidence_class);
    let semantic_key = format!("{owner}|{seam_kind}|{item_kind}|{discriminator_identity}");
    let state = CanonicalEvidenceState::new(
        owner,
        format!("{seam_kind}|{item_kind}"),
        discriminator_identity,
        GapState::from(
            canonical
                .get("gap_state")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
        ),
        strongest_oracle_strength(value),
    );
    Ok(SnapshotItem {
        gap_id,
        semantic_key,
        state,
        file: value
            .get("file")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>")
            .to_string(),
        line: value.get("line").and_then(Value::as_u64).unwrap_or(0) as usize,
        seam_id: value
            .get("seam_id")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>")
            .to_string(),
    })
}

fn discriminator_identity(value: &Value, canonical: &Value, evidence_class: &str) -> String {
    let mut parts = vec![
        evidence_class.to_string(),
        canonical
            .get("canonical_item_kind")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
    ];
    if let Some(rows) = value
        .get("missing_discriminators")
        .and_then(Value::as_array)
    {
        for row in rows {
            let item = format!(
                "{}:{}",
                row.get("value")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                row.get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            );
            parts.push(item);
        }
    }
    parts.sort();
    parts.join("|")
}

fn strongest_oracle_strength(value: &Value) -> OracleStrength {
    let mut best = OracleStrength::Unknown;
    if let Some(rows) = value.get("related_tests").and_then(Value::as_array) {
        for row in rows {
            let candidate = match row.get("oracle_strength").and_then(Value::as_str) {
                Some("strong") | Some("Strong") => OracleStrength::Strong,
                Some("medium") | Some("Medium") => OracleStrength::Medium,
                Some("weak") | Some("Weak") => OracleStrength::Weak,
                Some("smoke") | Some("Smoke") => OracleStrength::Smoke,
                Some("none") | Some("None") => OracleStrength::None,
                _ => OracleStrength::Unknown,
            };
            if candidate.rank() > best.rank() {
                best = candidate;
            }
        }
    }
    best
}

fn compare_snapshots(
    base_revision: &str,
    head_revision: &str,
    base: &Snapshot,
    head: &Snapshot,
    changed_files: &BTreeSet<String>,
) -> Value {
    let mut base_by_gap = HashMap::<String, usize>::new();
    for (index, item) in base.items.iter().enumerate() {
        base_by_gap.insert(item.gap_id.clone(), index);
    }
    let mut used_base = HashSet::new();
    let mut deltas = Vec::new();
    let mut coverage = ComparisonCoverage {
        base_items: base.items.len(),
        head_items: head.items.len(),
        ..ComparisonCoverage::default()
    };
    let mut ambiguous = 0usize;
    let mut unknown = base.unknown_items
        + head.unknown_items
        + usize::from(!base.available)
        + usize::from(!head.available);

    for head_item in &head.items {
        let exact = base_by_gap
            .get(&head_item.gap_id)
            .copied()
            .filter(|index| !used_base.contains(index));
        let (base_item, mapped) = if let Some(index) = exact {
            used_base.insert(index);
            (Some(&base.items[index]), false)
        } else {
            let candidates = base
                .items
                .iter()
                .enumerate()
                .filter(|(index, item)| {
                    !used_base.contains(index) && item.semantic_key == head_item.semantic_key
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [index] => {
                    used_base.insert(*index);
                    (Some(&base.items[*index]), true)
                }
                [] => (None, false),
                _ => {
                    ambiguous += 1;
                    let mut delta = compare_fixture_delta(
                        head_item.gap_id.clone(),
                        base.comparable(),
                        head.comparable(),
                        None,
                        Some(head_item.state.clone()),
                    );
                    delta.delta_attribution = DeltaAttribution::ComparisonUnknown;
                    delta.comparison_confidence = ComparisonConfidence::Unknown;
                    delta.attribution_basis = vec![AttributionBasis::IdentityAmbiguous];
                    deltas.push(delta_value(delta, None, Some(head_item), "ambiguous"));
                    continue;
                }
            }
        };

        let Some(base_item) = base_item else {
            let mut delta = compare_fixture_delta(
                head_item.gap_id.clone(),
                base.comparable(),
                head.comparable(),
                None,
                Some(head_item.state.clone()),
            );
            delta.delta_attribution = DeltaAttribution::ComparisonUnknown;
            delta.comparison_confidence = ComparisonConfidence::Unknown;
            delta.attribution_basis = vec![AttributionBasis::IdentityAmbiguous];
            unknown += 1;
            deltas.push(delta_value(delta, None, Some(head_item), "head_only"));
            continue;
        };

        let mut delta = compare_fixture_delta(
            base_item.gap_id.clone(),
            base.available,
            head.available,
            Some(base_item.state.clone()),
            Some(head_item.state.clone()),
        );
        if mapped || base_item.file != head_item.file || base_item.line != head_item.line {
            delta
                .attribution_basis
                .push(AttributionBasis::RenameOrMoveMapped);
        }
        if delta.delta_attribution == DeltaAttribution::ChangedSurfaceExisting
            && !changed_files.contains(&normalize_path(&head_item.file))
        {
            delta.delta_attribution = DeltaAttribution::BaselineExisting;
            delta
                .attribution_basis
                .push(AttributionBasis::BaselineReceipt);
        }
        if delta.delta_attribution == DeltaAttribution::ComparisonUnknown {
            unknown += 1;
        }
        coverage.matched_items += 1;
        deltas.push(delta_value(
            delta,
            Some(base_item),
            Some(head_item),
            if mapped { "rename_or_move" } else { "exact" },
        ));
    }

    for (index, base_item) in base.items.iter().enumerate() {
        if used_base.contains(&index) {
            continue;
        }
        let delta = compare_fixture_delta(
            base_item.gap_id.clone(),
            base.comparable(),
            head.comparable(),
            Some(base_item.state.clone()),
            None,
        );
        if delta.delta_attribution == DeltaAttribution::ComparisonUnknown {
            unknown += 1;
        }
        deltas.push(delta_value(delta, Some(base_item), None, "base_only"));
    }

    deltas.sort_by(|left, right| {
        left.get("canonical_gap_id")
            .and_then(Value::as_str)
            .cmp(&right.get("canonical_gap_id").and_then(Value::as_str))
    });
    coverage.ambiguous_items = ambiguous;
    coverage.unknown_items = unknown;
    let mut attribution_counts = BTreeMap::<String, usize>::new();
    for delta in &deltas {
        if let Some(attribution) = delta.get("delta_attribution").and_then(Value::as_str) {
            *attribution_counts
                .entry(attribution.to_string())
                .or_default() += 1;
        }
    }
    let coverage_complete = coverage.is_complete()
        && base.limitations.is_empty()
        && head.limitations.is_empty()
        && base.available
        && head.available;
    json!({
        "schema_version": CANONICAL_DELTA_SCHEMA_VERSION,
        "base": {"revision": base_revision, "status": base.status, "available": base.available},
        "head": {"revision": head_revision, "status": head.status, "available": head.available},
        "coverage": {
            "base_items": coverage.base_items,
            "head_items": coverage.head_items,
            "matched_items": coverage.matched_items,
            "ambiguous_items": coverage.ambiguous_items,
            "unknown_items": coverage.unknown_items,
            "complete": coverage_complete,
            "low_coverage_disclosed": !coverage_complete,
        },
        "summary": {"attribution_counts": attribution_counts},
        "limitations": base.limitations.iter().chain(head.limitations.iter()).collect::<Vec<_>>(),
        "deltas": deltas,
    })
}

fn delta_value(
    delta: ripr::domain::CanonicalDelta,
    base: Option<&SnapshotItem>,
    head: Option<&SnapshotItem>,
    match_kind: &str,
) -> Value {
    let mut value = serde_json::to_value(delta).unwrap_or_else(|_| {
        json!({
            "delta_attribution": "comparison_unknown",
            "comparison_confidence": "unknown"
        })
    });
    if let Some(object) = value.as_object_mut() {
        object.insert("match_kind".to_string(), json!(match_kind));
        object.insert("base_location".to_string(), location_value(base));
        object.insert("head_location".to_string(), location_value(head));
        object.insert(
            "base_seam_id".to_string(),
            base.map(|item| json!(item.seam_id)).unwrap_or(Value::Null),
        );
        object.insert(
            "head_seam_id".to_string(),
            head.map(|item| json!(item.seam_id)).unwrap_or(Value::Null),
        );
    }
    value
}

fn location_value(item: Option<&SnapshotItem>) -> Value {
    item.map(|item| json!({"file": item.file, "line": item.line}))
        .unwrap_or(Value::Null)
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(
        gap_id: &str,
        owner: &str,
        file: &str,
        gap_state: &str,
        oracle: OracleStrength,
    ) -> SnapshotItem {
        SnapshotItem {
            gap_id: gap_id.to_string(),
            semantic_key: format!("{owner}|predicate|boundary|predicate_boundary|boundary"),
            state: CanonicalEvidenceState::new(
                owner,
                "predicate|boundary",
                "predicate_boundary|boundary",
                GapState::from(gap_state),
                oracle,
            ),
            file: file.to_string(),
            line: 10,
            seam_id: format!("seam-{gap_id}"),
        }
    }

    fn snapshot(items: Vec<SnapshotItem>) -> Snapshot {
        Snapshot {
            available: true,
            status: "complete".to_string(),
            limitations: Vec::new(),
            items,
            unknown_items: 0,
        }
    }

    #[test]
    fn base_head_comparison_does_not_use_line_proximity() -> Result<(), String> {
        let base = snapshot(vec![item(
            "gap:base",
            "owner",
            "src/base.rs",
            "actionable",
            OracleStrength::Strong,
        )]);
        let head = snapshot(vec![item(
            "gap:head",
            "owner",
            "tests/head.rs",
            "actionable",
            OracleStrength::Weak,
        )]);
        let value = compare_snapshots("base", "head", &base, &head, &BTreeSet::new());
        let deltas = value
            .get("deltas")
            .and_then(Value::as_array)
            .ok_or("missing deltas")?;
        if deltas[0]["delta_attribution"] != "weakened_by_change" {
            return Err(format!(
                "expected semantic match to be weakened, got {}",
                deltas[0]
            ));
        }
        Ok(())
    }

    #[test]
    fn unavailable_base_is_comparison_unknown() -> Result<(), String> {
        let mut base = snapshot(Vec::new());
        base.available = false;
        base.status = "unavailable".to_string();
        base.limitations
            .push("base snapshot unavailable".to_string());
        let head = snapshot(vec![item(
            "gap:head",
            "owner",
            "src/lib.rs",
            "actionable",
            OracleStrength::Weak,
        )]);
        let value = compare_snapshots("base", "head", &base, &head, &BTreeSet::new());
        if value["deltas"][0]["delta_attribution"] != "comparison_unknown"
            || value["coverage"]["low_coverage_disclosed"] != true
        {
            return Err(format!("unavailable base did not fail closed: {value}"));
        }
        Ok(())
    }

    #[test]
    fn ambiguous_semantic_mapping_fails_closed() -> Result<(), String> {
        let base = snapshot(vec![
            item(
                "gap:one",
                "owner",
                "src/a.rs",
                "actionable",
                OracleStrength::Weak,
            ),
            item(
                "gap:two",
                "owner",
                "src/b.rs",
                "actionable",
                OracleStrength::Weak,
            ),
        ]);
        let head = snapshot(vec![item(
            "gap:head",
            "owner",
            "src/c.rs",
            "actionable",
            OracleStrength::Weak,
        )]);
        let value = compare_snapshots("base", "head", &base, &head, &BTreeSet::new());
        if value["coverage"]["ambiguous_items"] != 1
            || value["deltas"][0]["delta_attribution"] != "comparison_unknown"
            || !value["deltas"][0]["base_state"].is_null()
        {
            return Err(format!("ambiguous comparison did not fail closed: {value}"));
        }
        Ok(())
    }

    #[test]
    fn different_owner_cannot_be_mapped_as_a_move() -> Result<(), String> {
        let base = snapshot(vec![item(
            "gap:base",
            "owner-a",
            "src/old.rs",
            "actionable",
            OracleStrength::Weak,
        )]);
        let head = snapshot(vec![item(
            "gap:head",
            "owner-b",
            "src/new.rs",
            "actionable",
            OracleStrength::Weak,
        )]);
        let value = compare_snapshots("base", "head", &base, &head, &BTreeSet::new());
        let delta = value["deltas"]
            .as_array()
            .and_then(|deltas| {
                deltas
                    .iter()
                    .find(|delta| delta["match_kind"] == "head_only")
            })
            .ok_or("missing head-only owner-mismatch delta")?;
        if delta["delta_attribution"] != "comparison_unknown" {
            return Err(format!("owner mismatch was mapped: {value}"));
        }
        Ok(())
    }

    #[test]
    fn unmatched_base_identity_is_comparison_unknown() -> Result<(), String> {
        let base = snapshot(vec![item(
            "gap:base",
            "owner",
            "src/base.rs",
            "actionable",
            OracleStrength::Weak,
        )]);
        let mut head_item = item(
            "gap:head",
            "owner",
            "src/head.rs",
            "different_behavior",
            OracleStrength::Weak,
        );
        head_item.semantic_key = "owner|different-behavior".to_string();
        let head = snapshot(vec![head_item]);
        let value = compare_snapshots("base", "head", &base, &head, &BTreeSet::new());
        let delta = value["deltas"]
            .as_array()
            .and_then(|deltas| {
                deltas
                    .iter()
                    .find(|delta| delta["match_kind"] == "head_only")
            })
            .ok_or("missing head-only unmatched-base delta")?;
        if delta["delta_attribution"] != "comparison_unknown"
            || delta["comparison_confidence"] != "unknown"
        {
            return Err(format!("unmatched base was treated as causal: {value}"));
        }
        Ok(())
    }

    #[test]
    fn snapshot_root_preserves_relative_and_absolute_scope() {
        let worktree = Path::new("snapshot-worktree");
        assert_eq!(
            snapshot_root(worktree, "crates/ripr"),
            worktree.join("crates/ripr").display().to_string()
        );
        let absolute = std::env::temp_dir().join("snapshot-workspace/crates/ripr");
        let absolute_string = absolute.display().to_string();
        assert_eq!(snapshot_root(worktree, &absolute_string), absolute_string);
    }

    #[test]
    fn moved_identity_is_explicitly_recorded() -> Result<(), String> {
        let base = snapshot(vec![item(
            "gap:stable",
            "owner",
            "src/old.rs",
            "actionable",
            OracleStrength::Weak,
        )]);
        let head = snapshot(vec![item(
            "gap:stable",
            "owner",
            "src/new.rs",
            "actionable",
            OracleStrength::Weak,
        )]);
        let value = compare_snapshots("base", "head", &base, &head, &BTreeSet::new());
        if value["deltas"][0]["attribution_basis"]
            .as_array()
            .is_none_or(|bases| !bases.iter().any(|basis| basis == "rename_or_move_mapped"))
        {
            return Err(format!("move basis missing: {value}"));
        }
        Ok(())
    }
}
