//! Python judged PR panel (RIPR-SPEC-0092, issue #3555 PR A): the typed loader
//! and semantic validator for the retained Tier B fixture inventory — three
//! immutable historical JSON envelopes under `fixtures/python-judged-pr-panel/`
//! (the unjudged hand-vetted seed plus two manually judged historical panels).
//!
//! Structural rot is rejected at load: unknown fields (deny-unknown-fields),
//! duplicate JSON keys, missing required fields, non-UTF-8 bytes, and JSON
//! parse errors. Contract rot is rejected semantically: unknown schema/kind/
//! tier/spec/direction values, unknown judgment and classification vocabulary,
//! duplicate or empty case ids, missing direction coverage, impossible judgment
//! combinations, hand-entered totals that disagree with derived rows, and
//! diff/anchor proofs. Historical evidence is validated, never rewritten; the
//! retained-data accommodations are documented at each rule: judged rows may
//! point at the sibling Tier A sweep directory (the governed diff root is the
//! retained `fixtures/` tree); carryover rows with a null
//! `expected_classification` (the sqlalchemy timeout row) declare no anchor,
//! claim no error, and stay outside the judged denominator exactly as the
//! retained measurement summaries state; and a retained anchor line within one
//! line of a hand-trimmed sweep rendition's changed position warns instead of
//! failing, because replay must re-prove anchors against pinned bases.
//!
//! Aggregate totals, per-direction coverage, per-repository counts, and the
//! false-actionable/false-`exposed` numerators and rates are derived here as
//! pure functions of the validated rows only (`derive_aggregates`); hand-entered
//! envelope totals must agree with the derivation or the check fails.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Retained, immutable panel inventory. Never written by this module.
pub(crate) const INVENTORY_PATHS: [&str; 3] = [
    "fixtures/python-judged-pr-panel/manifest.json",
    "fixtures/python-judged-pr-panel/starter-judged.json",
    "fixtures/python-judged-pr-panel/scaled-judged.json",
];
const RERUN_COMMAND: &str = "cargo xtask python-judged-panel check";
const REQUIRED_DIRECTIONS: [&str; 3] = ["should_gap", "should_stay_quiet", "should_limit"];
const KNOWN_SCHEMA_VERSION: &str = "0.1";
const KNOWN_KIND: &str = "python_judged_pr_panel_manifest";
const KNOWN_SPEC: &str = "RIPR-SPEC-0092";
const KNOWN_TIER: &str = "B";
const KNOWN_AUTHORITY_BOUNDARY: &str = "review_advisory_only";
/// The repo-wide conservative static vocabulary (AGENTS.md language rules).
/// Shared with the #3555 adjudication workflow: a recorded judgment must use
/// the same vocabulary, never a private one.
pub(crate) const KNOWN_CLASSIFICATIONS: [&str; 7] = [
    "exposed",
    "weakly_exposed",
    "reachable_unrevealed",
    "no_static_path",
    "infection_unknown",
    "propagation_unknown",
    "static_unknown",
];
const KNOWN_ORACLE_ALIGNMENTS: [&str; 3] = ["changed_sink_token", "orthogonal", "unknown"];
pub(crate) const KNOWN_LIMITATION_QUALITIES: [&str; 4] =
    ["precise", "imprecise", "wrong_kind", "over_limited"];
/// The product-contract StaticLimitKind vocabulary, mirrored exactly from
/// `crates/ripr/src/domain/language.rs` (`StaticLimitKind::as_str`). Do not
/// invent kinds here; extend the product enum first, then this mirror.
const KNOWN_STATIC_LIMIT_KINDS: [&str; 17] = [
    "dynamic_dispatch",
    "metaprogramming",
    "missing_import_graph",
    "decorator_indirection",
    "mocked_module",
    "opaque_custom_assertion_helper",
    "property_based_test",
    "unresolved_pytest_fixture",
    "unsupported_syntax",
    "cross_language_oracle_visibility_unresolved",
    "rust_transitive_reach_unresolved",
    "rust_integration_public_api_path_unresolved",
    "rust_macro_reach_unresolved",
    "rust_macro_wrapped_test_call_unresolved",
    "rust_macro_wrapped_assertion_unresolved",
    "rust_value_propagation_unresolved",
    "rust_subprocess_binary_reach_unresolved",
];
/// Historical sweep renditions are hand-trimmed; a retained anchor line within
/// this distance of the rendition's changed position is disclosed as a warning.
const ANCHOR_DRIFT_WARNING: u64 = 1;
const FIXTURE_DIFF_ROOT: &str = "fixtures";

/// Ground-truth classifications admitted per direction; the retained rows
/// extend SPEC-0092 with `no_static_path` for `should_gap` (starter click).
fn admitted_classifications(direction: &str) -> &'static [&'static str] {
    match direction {
        "should_gap" => &["weakly_exposed", "reachable_unrevealed", "no_static_path"],
        "should_stay_quiet" => &["exposed"],
        "should_limit" => &["static_unknown"],
        _ => &[],
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PythonJudgedPanelEnvelope {
    schema_version: String,
    kind: String,
    spec: String,
    tier: String,
    description: String,
    #[serde(default)]
    pub(crate) measurement_summary: Option<PythonJudgedPanelMeasurementSummary>,
    limits: Vec<String>,
    pub(crate) items: Vec<PythonJudgedPanelItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PythonJudgedPanelMeasurementSummary {
    items_judged: u64,
    false_exposed_count: u64,
    false_actionable_count: u64,
    note: String,
    #[serde(default)]
    pub(crate) judged_against: Nullable<String>,
    #[serde(default)]
    updated: Nullable<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PythonJudgedPanelItem {
    pub(crate) id: String,
    pub(crate) repo: String,
    #[serde(default)]
    base: Nullable<String>,
    #[serde(default)]
    head: Nullable<String>,
    pub(crate) diff_path: String,
    pub(crate) shape: Vec<String>,
    pub(crate) expected_direction: String,
    pub(crate) anchor: PythonJudgedPanelAnchor,
    #[serde(default)]
    pub(crate) expected_classification: Nullable<String>,
    #[serde(default)]
    pub(crate) expected_static_limit_kind: Nullable<String>,
    #[serde(default)]
    pub(crate) actual_classification: Nullable<String>,
    #[serde(default)]
    pub(crate) actual_oracle_alignment: Nullable<String>,
    labels: PythonJudgedPanelLabels,
    #[serde(default)]
    pub(crate) judgment_source: Nullable<String>,
    #[serde(default)]
    judged_at: Nullable<String>,
    #[serde(default)]
    judged_by: Nullable<String>,
    authority_boundary: String,
    repair_packet_ready: bool,
    #[serde(default)]
    pub(crate) must_not_claim: Nullable<Vec<String>>,
    reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PythonJudgedPanelAnchor {
    #[serde(default)]
    pub(crate) file: Nullable<String>,
    #[serde(default)]
    pub(crate) line: Nullable<u64>,
    pub(crate) owner: String,
    boundary: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PythonJudgedPanelLabels {
    #[serde(default)]
    top_card_useful: Nullable<bool>,
    #[serde(default)]
    false_actionable: Nullable<bool>,
    #[serde(default)]
    false_exposed: Nullable<bool>,
    #[serde(default)]
    verify_command_valid: Nullable<bool>,
    #[serde(default)]
    suggested_location_valid: Nullable<bool>,
    #[serde(default)]
    packet_boundaries_safe: Nullable<bool>,
    #[serde(default)]
    limitation_quality: Nullable<String>,
}

impl PythonJudgedPanelLabels {
    fn all_explicitly_null(&self) -> bool {
        self.top_card_useful.is_null()
            && self.false_actionable.is_null()
            && self.false_exposed.is_null()
            && self.verify_command_valid.is_null()
            && self.suggested_location_valid.is_null()
            && self.packet_boundaries_safe.is_null()
            && self.limitation_quality.is_null()
    }
}

/// Distinguishes a missing key from an explicit JSON null.
#[derive(Debug, Default)]
pub(crate) enum Nullable<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<T> Nullable<T> {
    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Self::Null | Self::Missing)
    }

    pub(crate) fn value(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Missing | Self::Null => None,
        }
    }
}

impl<T: Serialize> Serialize for Nullable<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            // Missing and Null both serialize as null: they carry the same
            // row content for revision-digest purposes.
            Self::Null | Self::Missing => serializer.serialize_none(),
        }
    }
}

impl Nullable<String> {
    pub(crate) fn non_blank_value(&self) -> Option<&str> {
        self.value()
            .map(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Nullable<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(|value| match value {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

/// Row status: unjudged seed rows, completed usefulness judgments, and Tier A
/// robustness carryover rows excluded from the judged denominator.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum RowKind {
    Seed,
    Judged,
    Carryover,
}

pub(crate) fn row_kind(item: &PythonJudgedPanelItem) -> RowKind {
    // A blank judgment_source is not a declared source: the row stays
    // unjudged instead of entering the judged denominator.
    if item.judgment_source.non_blank_value().is_none() {
        RowKind::Seed
    } else if item.expected_classification.non_blank_value().is_some() {
        RowKind::Judged
    } else {
        RowKind::Carryover
    }
}

struct StrictJson(serde_json::Value);

impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictJsonVisitor)
    }
}

struct StrictJsonVisitor;

impl<'de> Visitor<'de> for StrictJsonVisitor {
    type Value = StrictJson;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictJson(value.into()))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(StrictJson(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(StrictJson(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .map(StrictJson)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(StrictJson(value.into()))
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(StrictJson(serde_json::Value::Null))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(StrictJson(serde_json::Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(StrictJson(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(StrictJson(serde_json::Value::Array(values)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let StrictJson(value) = map.next_value()?;
            if values.insert(key.clone(), value).is_some() {
                return Err(de::Error::custom(format!("duplicate object key `{key}`")));
            }
        }
        Ok(StrictJson(serde_json::Value::Object(values)))
    }
}

pub(crate) fn parse_json_without_duplicate_keys(
    body: &str,
) -> Result<serde_json::Value, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(body);
    let StrictJson(value) = StrictJson::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

fn load_envelope_at(root: &Path, display: &str) -> Result<PythonJudgedPanelEnvelope, String> {
    let body = fs::read_to_string(root.join(display)).map_err(|error| {
        format!("read Python judged panel envelope `{display}`: {error}\nrerun: {RERUN_COMMAND}")
    })?;
    let value = parse_json_without_duplicate_keys(&body).map_err(|error| {
        format!("parse Python judged panel envelope `{display}`: {error}\nrerun: {RERUN_COMMAND}")
    })?;
    serde_json::from_value(value).map_err(|error| {
        format!("parse Python judged panel envelope `{display}`: {error}\nrerun: {RERUN_COMMAND}")
    })
}

pub(crate) struct LoadedEnvelope {
    pub(crate) display: String,
    pub(crate) envelope: PythonJudgedPanelEnvelope,
}

/// One validated inventory: row-derived aggregates plus non-failing warnings.
pub(crate) struct CheckReport {
    aggregates: PanelAggregates,
    warnings: Vec<String>,
}

/// Aggregate derivation as a pure function of validated rows only; the
/// report-shaped core the later #3555 slice will render.
#[derive(Debug, Default, Eq, PartialEq)]
struct PanelAggregates {
    items_total: usize,
    seed_items: usize,
    judged_items: usize,
    carryover_items: usize,
    false_actionable_numerator: usize,
    false_exposed_numerator: usize,
    per_direction: BTreeMap<String, DirectionAggregate>,
    per_repo: BTreeMap<String, RepoAggregate>,
    directions_covered: BTreeSet<String>,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct DirectionAggregate {
    selected: usize,
    judged: usize,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct RepoAggregate {
    selected: usize,
    judged: usize,
}

impl PanelAggregates {
    /// No denominator means no rate.
    fn false_actionable_rate(&self) -> Option<f64> {
        rate(self.false_actionable_numerator, self.judged_items)
    }

    fn false_exposed_rate(&self) -> Option<f64> {
        rate(self.false_exposed_numerator, self.judged_items)
    }
}

fn rate(numerator: usize, denominator: usize) -> Option<f64> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

fn derive_aggregates(loaded: &[LoadedEnvelope]) -> PanelAggregates {
    let mut aggregates = PanelAggregates::default();
    for file in loaded {
        for item in &file.envelope.items {
            aggregates.items_total += 1;
            let kind = row_kind(item);
            let judged = kind == RowKind::Judged;
            match kind {
                RowKind::Seed => aggregates.seed_items += 1,
                RowKind::Judged => aggregates.judged_items += 1,
                RowKind::Carryover => aggregates.carryover_items += 1,
            }
            let direction = aggregates
                .per_direction
                .entry(item.expected_direction.clone())
                .or_default();
            direction.selected += 1;
            direction.judged += usize::from(judged);
            let repo = aggregates.per_repo.entry(item.repo.clone()).or_default();
            repo.selected += 1;
            repo.judged += usize::from(judged);
            aggregates.false_actionable_numerator += usize::from(matches!(
                item.labels.false_actionable,
                Nullable::Value(true)
            ));
            aggregates.false_exposed_numerator +=
                usize::from(matches!(item.labels.false_exposed, Nullable::Value(true)));
            aggregates
                .directions_covered
                .insert(item.expected_direction.clone());
        }
    }
    aggregates
}

fn missing_directions(aggregates: &PanelAggregates) -> Vec<&'static str> {
    REQUIRED_DIRECTIONS
        .iter()
        .filter(|direction| !aggregates.directions_covered.contains(**direction))
        .copied()
        .collect()
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("check") if args.len() == 1 || args == ["check", "--check"] => {
            print_report(&check_inventory_at(Path::new("."), &INVENTORY_PATHS)?);
            Ok(())
        }
        Some("replay") => super::python_judged_panel_replay::run(&args[1..]),
        Some("report") => super::python_judged_panel_report::run_report(&args[1..]),
        Some("adjudicate") => super::python_judged_panel_report::run_adjudicate(&args[1..]),
        Some("feedback") => super::python_judged_panel_feedback::run_feedback(&args[1..]),
        _ => Err(format!(
            "python-judged-panel requires `check [--check]`, `replay [--check] [--limit <n>] [--network]`, `report [--records <dir>] [--adjudications <dir>] [--threshold-policy <path>] [--out <dir>] [--check]`, `feedback [--records <dir>] [--adjudications <dir>] [--out <dir>] [--check]`, or `adjudicate --case <id> --verdict <classification> --role <role> (--reviewer <identity> | env RIPR_PANEL_ADJUDICATOR) --evidence <ref> [--adjudications <dir>] [--records <dir>]`\nrerun: {RERUN_COMMAND}"
        )),
    }
}

/// Precommit alias (`cargo xtask check-python-judged-panel`): the same
/// retained-inventory validation without the pass report.
pub(crate) fn check_canonical() -> Result<(), String> {
    check_inventory_at(Path::new("."), &INVENTORY_PATHS).map(|_| ())
}

fn print_report(report: &CheckReport) {
    let aggregates = &report.aggregates;
    println!(
        "Python judged PR panel inventory valid: items={} seed={} judged={} carryover={}",
        aggregates.items_total,
        aggregates.seed_items,
        aggregates.judged_items,
        aggregates.carryover_items,
    );
    let coverage = REQUIRED_DIRECTIONS
        .iter()
        .map(|direction| {
            let entry = aggregates.per_direction.get(*direction);
            format!(
                "{direction}={} (judged {})",
                entry.map(|value| value.selected).unwrap_or(0),
                entry.map(|value| value.judged).unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let coverage_state = if missing_directions(aggregates).is_empty() {
        "all three directions present"
    } else {
        "coverage incomplete"
    };
    println!("direction coverage: {coverage} ({coverage_state})");
    for (repo, entry) in &aggregates.per_repo {
        println!(
            "repo coverage: {repo}: selected={} judged={}",
            entry.selected, entry.judged
        );
    }
    println!(
        "derived from rows: false_actionable={}/{} (rate {}) false_exposed={}/{} (rate {}); rates are disclosed only with a judged denominator",
        aggregates.false_actionable_numerator,
        aggregates.judged_items,
        disclosed_rate(aggregates.false_actionable_rate()),
        aggregates.false_exposed_numerator,
        aggregates.judged_items,
        disclosed_rate(aggregates.false_exposed_rate()),
    );
    for warning in &report.warnings {
        println!("warning: {warning}");
    }
    println!("rerun: {RERUN_COMMAND}");
}

fn disclosed_rate(value: Option<f64>) -> String {
    match value {
        Some(rate) => format!("{rate:.3}"),
        None => "none: no judged denominator".to_string(),
    }
}

/// Validates the given repo-relative envelope files as one inventory under
/// `root`; every declared diff path must resolve under `root`/`fixtures/`.
pub(crate) fn check_inventory_at(root: &Path, displays: &[&str]) -> Result<CheckReport, String> {
    let loaded = load_validated_inventory(root, displays)?;
    let (_, mut warnings) = validate_inventory(root, &loaded);
    warnings.sort();
    warnings.dedup();
    Ok(CheckReport {
        aggregates: derive_aggregates(&loaded),
        warnings,
    })
}

/// The one semantic loader behind `check` and `replay` (#3555): strict parse,
/// full semantic validation, then the validated rows. Replay consumes only
/// rows this loader accepted; it never re-parses envelopes in parallel.
pub(crate) fn load_validated_inventory(
    root: &Path,
    displays: &[&str],
) -> Result<Vec<LoadedEnvelope>, String> {
    let loaded = displays
        .iter()
        .map(|display| {
            Ok(LoadedEnvelope {
                display: (*display).to_string(),
                envelope: load_envelope_at(root, display)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let (mut violations, _) = validate_inventory(root, &loaded);
    violations.sort();
    violations.dedup();
    if !violations.is_empty() {
        return Err(format!(
            "Python judged PR panel inventory under `{}` has {} semantic violation(s):\n- {}\nrerun: {RERUN_COMMAND}",
            root.display(),
            violations.len(),
            violations.join("\n- ")
        ));
    }
    Ok(loaded)
}

fn validate_inventory(root: &Path, loaded: &[LoadedEnvelope]) -> (Vec<String>, Vec<String>) {
    let mut violations = Vec::new();
    let mut warnings = Vec::new();
    let mut seen_ids = BTreeMap::<String, String>::new();
    let mut seen_subjects = BTreeMap::<String, String>::new();
    for file in loaded {
        validate_envelope(file, &mut violations);
        for (index, item) in file.envelope.items.iter().enumerate() {
            let subject = if item.id.trim().is_empty() {
                format!("{} items[{index}]", file.display)
            } else {
                format!("{} item `{}`", file.display, item.id)
            };
            if item.id.trim().is_empty() {
                violations.push(format!("{subject}.id: must not be blank"));
            } else if let Some(previous) = seen_ids.get(item.id.as_str()) {
                violations.push(format!(
                    "inventory.item `{}`: duplicate case id also declared in `{previous}`",
                    item.id
                ));
            } else {
                seen_ids.insert(item.id.clone(), file.display.clone());
            }
            let logical_subject = format!(
                "repo `{}` base `{}` head `{}` diff `{}` anchor `{}:{}`",
                item.repo,
                item.base.value().cloned().unwrap_or_default(),
                item.head.value().cloned().unwrap_or_default(),
                item.diff_path,
                item.anchor.file.value().cloned().unwrap_or_default(),
                item.anchor
                    .line
                    .value()
                    .map(|line| line.to_string())
                    .unwrap_or_default(),
            );
            if let Some(previous) = seen_subjects.get(&logical_subject) {
                violations.push(format!(
                    "inventory.item `{}`: duplicate logical subject identity ({logical_subject}) also declared in `{previous}`",
                    item.id
                ));
            } else {
                seen_subjects.insert(logical_subject, file.display.clone());
            }
            validate_item(root, item, &subject, &mut violations, &mut warnings);
        }
        validate_measurement_summary(file, &mut violations);
    }
    for direction in missing_directions(&derive_aggregates(loaded)) {
        violations.push(format!(
            "inventory: missing required direction coverage `{direction}`; the combined panel must contain all three directions"
        ));
    }
    (violations, warnings)
}

fn validate_envelope(file: &LoadedEnvelope, violations: &mut Vec<String>) {
    let display = &file.display;
    let envelope = &file.envelope;
    for (field, value, expected) in [
        (
            "schema_version",
            &envelope.schema_version,
            KNOWN_SCHEMA_VERSION,
        ),
        ("kind", &envelope.kind, KNOWN_KIND),
        ("spec", &envelope.spec, KNOWN_SPEC),
        ("tier", &envelope.tier, KNOWN_TIER),
    ] {
        require_equal(violations, &format!("{display}.{field}"), value, expected);
    }
    require_non_empty(
        violations,
        &format!("{display}.description"),
        &envelope.description,
    );
    if envelope.limits.is_empty() || envelope.limits.iter().any(|limit| limit.trim().is_empty()) {
        violations.push(format!(
            "{display}.limits: require at least one non-empty panel non-claim"
        ));
    }
    if envelope.items.is_empty() {
        violations.push(format!(
            "{display}.items: panel denominator must not be empty"
        ));
    }
}

fn validate_measurement_summary(file: &LoadedEnvelope, violations: &mut Vec<String>) {
    let Some(summary) = &file.envelope.measurement_summary else {
        // An envelope with judged rows must declare its measurement summary;
        // seed-only (and carryover-only) envelopes may omit it.
        if file
            .envelope
            .items
            .iter()
            .any(|item| row_kind(item) == RowKind::Judged)
        {
            violations.push(format!(
                "{display}.measurement_summary: envelopes with judged rows require a measurement summary; the judged denominator must be declared",
                display = file.display
            ));
        }
        return;
    };
    let display = &file.display;
    require_non_empty(
        violations,
        &format!("{display}.measurement_summary.note"),
        &summary.note,
    );
    for (field, value) in [
        ("judged_against", &summary.judged_against),
        ("updated", &summary.updated),
    ] {
        if !value.is_null() && value.non_blank_value().is_none() {
            violations.push(format!(
                "{display}.measurement_summary.{field}: must not be blank when declared"
            ));
        }
    }
    let mut judged = 0_usize;
    let mut actionable = 0_usize;
    let mut exposed = 0_usize;
    for item in &file.envelope.items {
        judged += usize::from(row_kind(item) == RowKind::Judged);
        actionable += usize::from(matches!(
            item.labels.false_actionable,
            Nullable::Value(true)
        ));
        exposed += usize::from(matches!(item.labels.false_exposed, Nullable::Value(true)));
    }
    if summary.items_judged as usize != judged {
        violations.push(format!(
            "{display}.measurement_summary.items_judged: hand-entered total {} disagrees with {judged} derived judged rows",
            summary.items_judged
        ));
    }
    if summary.false_actionable_count as usize != actionable {
        violations.push(format!(
            "{display}.measurement_summary.false_actionable_count: hand-entered total {} disagrees with {actionable} derived rows",
            summary.false_actionable_count
        ));
    }
    if summary.false_exposed_count as usize != exposed {
        violations.push(format!(
            "{display}.measurement_summary.false_exposed_count: hand-entered total {} disagrees with {exposed} derived rows",
            summary.false_exposed_count
        ));
    }
}

fn validate_item(
    root: &Path,
    item: &PythonJudgedPanelItem,
    subject: &str,
    violations: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    require_non_empty(violations, &format!("{subject}.repo"), &item.repo);
    require_non_empty(violations, &format!("{subject}.reason"), &item.reason);
    if item.shape.is_empty() || item.shape.iter().any(|shape| shape.trim().is_empty()) {
        violations.push(format!(
            "{subject}.shape: require at least one non-empty Tier A shape entry"
        ));
    }
    require_equal(
        violations,
        &format!("{subject}.authority_boundary"),
        &item.authority_boundary,
        KNOWN_AUTHORITY_BOUNDARY,
    );
    if item.repair_packet_ready {
        violations.push(format!(
            "{subject}.repair_packet_ready: must remain false; the panel is review-advisory only"
        ));
    }
    for (field, value) in [("base", &item.base), ("head", &item.head)] {
        // A present-but-blank revision is a broken pinned identity, not an
        // unpinned row: it fails the same sha-shape rule as any other pin.
        if value
            .value()
            .is_some_and(|sha| !(7..=40).contains(&sha.len()))
            || value
                .value()
                .is_some_and(|sha| !sha.chars().all(|c| c.is_ascii_hexdigit()))
        {
            violations.push(format!(
                "{subject}.{field}: must be a non-empty hex commit sha when pinned"
            ));
        }
    }
    validate_direction(item, subject, violations);
    validate_vocabularies(item, subject, violations);
    validate_judgment_identity(item, subject, violations);
    validate_labels(item, subject, violations);
    validate_anchor_and_diff(root, item, subject, violations, warnings);
}

fn validate_direction(item: &PythonJudgedPanelItem, subject: &str, violations: &mut Vec<String>) {
    if !REQUIRED_DIRECTIONS.contains(&item.expected_direction.as_str()) {
        violations.push(format!(
            "{subject}.expected_direction: unknown direction `{}`",
            item.expected_direction
        ));
        return;
    }
    if row_kind(item) == RowKind::Seed && item.expected_classification.non_blank_value().is_none() {
        violations.push(format!(
            "{subject}.expected_classification: seed rows require the conservative ground-truth verdict"
        ));
    }
    if let Some(classification) = item.expected_classification.non_blank_value() {
        let admitted = admitted_classifications(&item.expected_direction);
        if !admitted.contains(&classification) {
            violations.push(format!(
                "{subject}.expected_classification: `{}` requires one of {}, found `{classification}`",
                item.expected_direction,
                admitted.join(", ")
            ));
        }
    }
    if item.expected_static_limit_kind.non_blank_value().is_some()
        && item.expected_direction != "should_limit"
    {
        violations.push(format!(
            "{subject}.expected_static_limit_kind: a non-null limit kind requires `should_limit`, found `{}`",
            item.expected_direction
        ));
    }
    // A should_limit row names its limitation. Grandfathers, both retained:
    // carryover rows (the sqlalchemy robustness-carryover row) and judged rows
    // that explicitly grade their limitation record via a non-null
    // limitation_quality (the retained six row, whose limitation is recorded
    // as `imprecise`). New should_limit rows must name their kind.
    if item.expected_direction == "should_limit"
        && item.expected_static_limit_kind.non_blank_value().is_none()
    {
        let grandfathered = row_kind(item) == RowKind::Carryover
            || (row_kind(item) == RowKind::Judged
                && item.labels.limitation_quality.non_blank_value().is_some());
        if !grandfathered {
            violations.push(format!(
                "{subject}.expected_static_limit_kind: `should_limit` rows require a registered static-limit kind"
            ));
        }
    }
    if let Some(kind) = item.expected_static_limit_kind.non_blank_value() {
        // The retained sqlalchemy carryover row records a Tier A sweep timeout
        // as its limit kind. That is a robustness carryover, not a Python
        // StaticLimitKind; it is grandfathered on carryover rows only (which
        // are excluded from the judged denominator) and must not be read as a
        // registered Python static-limit kind.
        let grandfathered_carryover_timeout =
            row_kind(item) == RowKind::Carryover && kind == "timeout";
        if !KNOWN_STATIC_LIMIT_KINDS.contains(&kind) && !grandfathered_carryover_timeout {
            violations.push(format!(
                "{subject}.expected_static_limit_kind: unknown static-limit kind `{kind}`; must be a registered StaticLimitKind from the product contract"
            ));
        }
    }
}

fn validate_vocabularies(
    item: &PythonJudgedPanelItem,
    subject: &str,
    violations: &mut Vec<String>,
) {
    for (field, value, known, label) in [
        (
            "actual_classification",
            item.actual_classification.non_blank_value(),
            KNOWN_CLASSIFICATIONS.as_slice(),
            "classification",
        ),
        (
            "actual_oracle_alignment",
            item.actual_oracle_alignment.non_blank_value(),
            KNOWN_ORACLE_ALIGNMENTS.as_slice(),
            "oracle-alignment",
        ),
        (
            "judgment_source",
            item.judgment_source.non_blank_value(),
            &["manual_review"],
            "judgment",
        ),
        (
            "labels.limitation_quality",
            item.labels.limitation_quality.non_blank_value(),
            KNOWN_LIMITATION_QUALITIES.as_slice(),
            "limitation-quality",
        ),
    ] {
        if let Some(value) = value
            && !known.contains(&value)
        {
            violations.push(format!(
                "{subject}.{field}: unknown {label} value `{value}`"
            ));
        }
    }
}

fn validate_judgment_identity(
    item: &PythonJudgedPanelItem,
    subject: &str,
    violations: &mut Vec<String>,
) {
    if row_kind(item) == RowKind::Seed {
        if item.judged_at.value().is_some() || item.judged_by.value().is_some() {
            violations.push(format!(
                "{subject}.judgment_identity: unjudged rows cannot carry judged_at/judged_by identity"
            ));
        }
        let claims_held = matches!(
            &item.must_not_claim,
            Nullable::Value(claims)
                if !claims.is_empty() && claims.iter().all(|claim| !claim.trim().is_empty())
        );
        if !claims_held {
            violations.push(format!(
                "{subject}.must_not_claim: seed items require at least one non-empty null-honesty non-claim"
            ));
        }
        return;
    }
    if item.judged_at.non_blank_value().is_none() || item.judged_by.non_blank_value().is_none() {
        violations.push(format!(
            "{subject}.judgment_identity: judged rows require non-empty judged_at and judged_by; absent currentness identity cannot be represented as current"
        ));
    }
    if row_kind(item) == RowKind::Judged {
        if item.actual_classification.non_blank_value().is_none() {
            violations.push(format!(
                "{subject}.actual_classification: judged rows require the observed verdict; a claimed judgment without it cannot enter the denominator"
            ));
        }
        if item.actual_oracle_alignment.non_blank_value().is_none() {
            violations.push(format!(
                "{subject}.actual_oracle_alignment: judged rows require the observed oracle alignment"
            ));
        }
    }
    if row_kind(item) == RowKind::Carryover
        && (!item.anchor.file.is_null() || item.anchor.line.value().is_some())
    {
        violations.push(format!(
            "{subject}.anchor: carryover rows (null expected_classification) cannot declare an anchor"
        ));
    }
}

/// RIPR-SPEC-0092 outcome lattice: the error labels a direction can carry as
/// true. `should_gap` measures `false_exposed` (ripr stayed quiet or
/// over-credited); `should_stay_quiet` measures `false_actionable` (ripr
/// routed against discriminated behavior); `should_limit` measures BOTH —
/// `false_actionable` when ripr routes past the limitation and
/// `false_exposed` when it credits exposed past the limitation.
pub(crate) fn direction_admits_error(direction: &str, label: &str) -> bool {
    matches!(
        (direction, label),
        ("should_gap", "false_exposed")
            | ("should_stay_quiet", "false_actionable")
            | ("should_limit", "false_actionable")
            | ("should_limit", "false_exposed")
    )
}

fn validate_labels(item: &PythonJudgedPanelItem, subject: &str, violations: &mut Vec<String>) {
    if matches!(item.labels.false_actionable, Nullable::Value(true))
        && matches!(item.labels.false_exposed, Nullable::Value(true))
    {
        violations.push(format!(
            "{subject}.labels: false_actionable and false_exposed cannot both be true for one terminal adjudication"
        ));
    }
    for (label, value) in [
        ("false_actionable", &item.labels.false_actionable),
        ("false_exposed", &item.labels.false_exposed),
    ] {
        if matches!(value, Nullable::Value(true))
            && !direction_admits_error(&item.expected_direction, label)
        {
            violations.push(format!(
                "{subject}.labels.{label}: `{}` cannot carry a true {label} per the SPEC-0092 outcome table",
                item.expected_direction
            ));
        }
    }
    // FIX B coherence: crediting `exposed` on a row whose direction expects a
    // gap or a fail-closed limitation is an over-credit by definition, so the
    // row must record `false_exposed: true`. A `should_stay_quiet` row is
    // excluded: `exposed` there is the correct verdict (the retained structlog
    // row), so no error label is owed.
    if row_kind(item) == RowKind::Judged
        && item.expected_direction != "should_stay_quiet"
        && item.actual_classification.non_blank_value() == Some("exposed")
        && !matches!(item.labels.false_exposed, Nullable::Value(true))
    {
        violations.push(format!(
            "{subject}.labels.false_exposed: actual_classification `exposed` on a `{}` row is an over-credit and requires false_exposed true",
            item.expected_direction
        ));
    }
    match row_kind(item) {
        RowKind::Seed => {
            for (key, value) in [
                ("top_card_useful", &item.labels.top_card_useful),
                ("false_actionable", &item.labels.false_actionable),
                ("false_exposed", &item.labels.false_exposed),
                ("verify_command_valid", &item.labels.verify_command_valid),
                (
                    "suggested_location_valid",
                    &item.labels.suggested_location_valid,
                ),
                (
                    "packet_boundaries_safe",
                    &item.labels.packet_boundaries_safe,
                ),
            ] {
                if matches!(value, Nullable::Missing) {
                    violations.push(format!(
                        "{subject}.labels.{key}: seed rows must declare every label key explicitly null"
                    ));
                }
            }
            if matches!(item.labels.limitation_quality, Nullable::Missing) {
                violations.push(format!(
                    "{subject}.labels.limitation_quality: seed rows must declare every label key explicitly null"
                ));
            }
            if !item.labels.all_explicitly_null() {
                violations.push(format!(
                    "{subject}.labels: seed labels must remain null; null means unjudged, not false/pass"
                ));
            }
        }
        RowKind::Judged => {
            for (flag, decided) in [
                ("false_actionable", !item.labels.false_actionable.is_null()),
                ("false_exposed", !item.labels.false_exposed.is_null()),
            ] {
                if !decided {
                    violations.push(format!(
                        "{subject}.labels.{flag}: judged rows must record an explicit decision; null is inconclusive, not a pass"
                    ));
                }
            }
        }
        RowKind::Carryover => {
            if matches!(item.labels.false_actionable, Nullable::Value(true))
                || matches!(item.labels.false_exposed, Nullable::Value(true))
            {
                violations.push(format!(
                    "{subject}.labels: carryover rows cannot claim false_actionable/false_exposed"
                ));
            }
        }
    }
}

/// Diff/anchor proofs: the diff is a confined retained fixture that parses as
/// a strict unified diff; the anchor file is touched and the anchor line maps
/// to a changed position within the allowed historical-rendition drift.
fn validate_anchor_and_diff(
    root: &Path,
    item: &PythonJudgedPanelItem,
    subject: &str,
    violations: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    for (field, value) in [
        ("owner", &item.anchor.owner),
        ("boundary", &item.anchor.boundary),
    ] {
        require_non_empty(violations, &format!("{subject}.anchor.{field}"), value);
    }
    match (item.anchor.file.non_blank_value(), item.anchor.line.value()) {
        (Some(_), Some(_)) | (None, None) => {}
        (Some(file), None) => violations.push(format!(
            "{subject}.anchor: file `{file}` is declared without a line; file and line must be null together or set together"
        )),
        (None, Some(line)) => violations.push(format!(
            "{subject}.anchor: line {line} is declared without a file; file and line must be null together or set together"
        )),
    }
    if item.anchor.line.value().is_some_and(|line| *line == 0) {
        violations.push(format!("{subject}.anchor.line: must be positive"));
    }
    let Some(diff_body) = load_diff_body(root, &item.diff_path, subject, violations) else {
        return;
    };
    let parsed = match parse_unified_diff(&diff_body) {
        Ok(parsed) => parsed,
        Err(error) => {
            violations.push(format!(
                "{subject}.anchor: diff `{}` does not parse as a strict unified diff: {error}",
                item.diff_path
            ));
            return;
        }
    };
    let Some(anchor_file) = item.anchor.file.non_blank_value() else {
        return;
    };
    let sections = parsed
        .sections
        .iter()
        .filter(|section| {
            section.old_path.as_deref() == Some(anchor_file)
                || section.new_path.as_deref() == Some(anchor_file)
        })
        .collect::<Vec<_>>();
    if sections.is_empty() {
        violations.push(format!(
            "{subject}.anchor: target-file mismatch: `{}` does not touch `{anchor_file}`",
            item.diff_path
        ));
        return;
    }
    if sections.len() > 1 {
        violations.push(format!(
            "{subject}.anchor: `{anchor_file}` occurs in {} file sections of `{}`",
            sections.len(),
            item.diff_path
        ));
        return;
    }
    let changed = sections
        .iter()
        .flat_map(|section| &section.hunks)
        .flat_map(|hunk| hunk.added.iter().chain(hunk.deleted.iter()).copied())
        .collect::<BTreeSet<u64>>();
    let anchor_line = item.anchor.line.value().copied().unwrap_or(0);
    let Some(nearest) = changed
        .iter()
        .copied()
        .min_by_key(|position| position.abs_diff(anchor_line))
    else {
        violations.push(format!(
            "{subject}.anchor: `{anchor_file}` declares no changed lines in `{}`",
            item.diff_path
        ));
        return;
    };
    match nearest.abs_diff(anchor_line) {
        0 => {}
        delta if delta <= ANCHOR_DRIFT_WARNING => warnings.push(format!(
            "{subject}: anchor line {anchor_line} sits {delta} line from the changed position {nearest} in `{}`; historical sweep renditions are hand-trimmed, replay must re-prove the anchor",
            item.diff_path
        )),
        _ => violations.push(format!(
            "{subject}.anchor: line {anchor_line} does not map to an added/deleted position in `{}` (nearest changed position {nearest}; allowed historical-rendition drift {ANCHOR_DRIFT_WARNING})",
            item.diff_path
        )),
    }
}

/// Confined diff loading: repo-relative, no traversal, under `fixtures/`.
pub(crate) fn load_diff_body(
    root: &Path,
    diff_path: &str,
    subject: &str,
    violations: &mut Vec<String>,
) -> Option<String> {
    let relative = Path::new(diff_path);
    if normalize_path(relative) != diff_path || !is_confined_fixture_diff_path(relative) {
        violations.push(format!(
            "{subject}.diff_path: `{diff_path}` must be a relative .diff file under `{FIXTURE_DIFF_ROOT}/` without parent traversal"
        ));
        return None;
    }
    let full_path = root.join(relative);
    if !full_path.is_file() {
        violations.push(format!(
            "{subject}.diff_path: `{diff_path}` is missing or is not a file"
        ));
        return None;
    }
    let mut canonical = |path: &Path, what: &str| -> Option<PathBuf> {
        match fs::canonicalize(path) {
            Ok(resolved) => Some(resolved),
            Err(error) => {
                violations.push(format!(
                    "{subject}.diff_path: failed to resolve {what}: {error}"
                ));
                None
            }
        }
    };
    let canonical_root = canonical(root, "the repository root")?;
    let fixture_root = canonical(&root.join(FIXTURE_DIFF_ROOT), "the retained fixture root")?;
    let resolved = canonical(&full_path, &format!("`{diff_path}`"))?;
    if !resolved.starts_with(&fixture_root) || !fixture_root.starts_with(&canonical_root) {
        violations.push(format!(
            "{subject}.diff_path: `{diff_path}` resolves outside the retained `{FIXTURE_DIFF_ROOT}/` fixture tree"
        ));
        return None;
    }
    match fs::read_to_string(&full_path) {
        Ok(body) => Some(body),
        Err(error) => {
            violations.push(format!(
                "{subject}.diff_path: failed to read `{diff_path}`: {error}"
            ));
            None
        }
    }
}

fn is_confined_fixture_diff_path(path: &Path) -> bool {
    is_confined_relative_path(path)
        && path.starts_with(Path::new(FIXTURE_DIFF_ROOT))
        && path.extension().and_then(|extension| extension.to_str()) == Some("diff")
}

fn is_confined_relative_path(path: &Path) -> bool {
    let raw = normalize_path(path);
    !raw.contains(':')
        && !raw.contains('\\')
        && !raw.contains("//")
        && !raw
            .split('/')
            .any(|segment| segment == "." || segment.is_empty())
        && !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::RootDir
            )
        })
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Parses a unified diff into file sections with per-hunk added (new-file
/// numbering) and deleted (old-file numbering) positions, rejecting combined,
/// binary, rename, copy, quoted, unbound-hunk, and false-extent forms.
/// Empty lines inside a hunk are blank context lines (the retained seed
/// rendition of `decorator_indirection_limit.diff` ends its hunk with two).
/// Hunk line text is retained (`DiffHunk.lines`) so replay can reconstruct
/// the proved base/head content (#3555 PR B).
pub(crate) fn parse_unified_diff(body: &str) -> Result<ParsedDiff, String> {
    let mut sections = Vec::new();
    let mut current: Option<DiffSection> = None;
    let mut hunk: Option<DiffHunk> = None;
    let mut source_header_seen = false;
    // Tracks whether the immediately preceding physical line inside a hunk
    // was the no-newline marker, so a doubled marker is rejected (fxi3X
    // round-3 review).
    let mut marker_seen = false;
    for raw in body.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.starts_with("diff --cc ")
            || line.starts_with("diff --combined ")
            || line.starts_with("GIT binary patch")
            || line.starts_with("Binary files ")
        {
            return Err("combined or binary diffs are unsupported".to_string());
        }
        if line.starts_with("diff --git ") {
            finish_hunk(&mut hunk, &mut current)?;
            push_current(&mut current, &mut sections);
            source_header_seen = false;
            continue;
        }
        if let Some(source) = line.strip_prefix("--- ") {
            finish_hunk(&mut hunk, &mut current)?;
            push_current(&mut current, &mut sections);
            source_header_seen = true;
            current = Some(DiffSection {
                old_path: parse_diff_header_path(source),
                new_path: None,
                hunks: Vec::new(),
            });
            continue;
        }
        if line.starts_with("rename from ")
            || line.starts_with("rename to ")
            || line.starts_with("copy from ")
            || line.starts_with("copy to ")
        {
            return Err("rename and copy diffs are unsupported".to_string());
        }
        if let Some(target) = line.strip_prefix("+++ ") {
            if !source_header_seen {
                return Err("`+++` target is not paired with a preceding `---` source".to_string());
            }
            if target.starts_with('"') || target.contains('\t') {
                return Err("quoted or metadata-bearing `+++` targets are unsupported".to_string());
            }
            let section = current
                .as_mut()
                .ok_or("`+++` target is not paired with a preceding `---` source")?;
            section.new_path = parse_diff_header_path(target);
            source_header_seen = false;
            continue;
        }
        if line.starts_with("@@ ") {
            let bound = current
                .as_ref()
                .is_some_and(|section| section.new_path.is_some());
            if !bound {
                return Err("hunk is not bound to a `---`/`+++` file section".to_string());
            }
            finish_hunk(&mut hunk, &mut current)?;
            hunk = Some(parse_hunk_header(line)?);
            continue;
        }
        let Some(state) = hunk.as_mut() else {
            continue;
        };
        if line.starts_with('\\') {
            // FIX fxi3X: the `\ No newline at end of file` marker flags the
            // immediately preceding hunk line as lacking the trailing
            // newline on the side(s) it feeds. Any other backslash line is
            // a malformed diff, the marker with no preceding hunk line is
            // likewise malformed, and a doubled marker is malformed too —
            // the second cannot attach to a line that already has one
            // (fx75U, round-3 review).
            if line != "\\ No newline at end of file" {
                return Err("malformed diff: unexpected backslash line inside hunk".to_string());
            }
            if marker_seen {
                return Err("malformed diff: repeated no-newline marker".to_string());
            }
            let Some(last) = state.lines.last_mut() else {
                return Err(
                    "malformed diff: no-newline marker with no preceding hunk line".to_string(),
                );
            };
            if last.no_newline {
                return Err("malformed diff: repeated no-newline marker".to_string());
            }
            last.no_newline = true;
            marker_seen = true;
            continue;
        }
        marker_seen = false;
        if let Some(text) = line.strip_prefix('+') {
            state.consume_new()?;
            state.added.insert(state.next_new - 1);
            state.lines.push(DiffLine {
                kind: DiffLineKind::Added,
                text: text.to_string(),
                no_newline: false,
            });
        } else if let Some(text) = line.strip_prefix('-') {
            state.consume_old()?;
            state.deleted.insert(state.next_old - 1);
            state.lines.push(DiffLine {
                kind: DiffLineKind::Deleted,
                text: text.to_string(),
                no_newline: false,
            });
        } else if line.starts_with(' ') || line.is_empty() {
            state.consume_old()?;
            state.consume_new()?;
            let text = line.strip_prefix(' ').unwrap_or("");
            state.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                text: text.to_string(),
                no_newline: false,
            });
        } else {
            return Err(format!("unsupported line inside unified hunk `{line}`"));
        }
    }
    finish_hunk(&mut hunk, &mut current)?;
    push_current(&mut current, &mut sections);
    Ok(ParsedDiff { sections })
}

#[derive(Debug, Default)]
pub(crate) struct ParsedDiff {
    pub(crate) sections: Vec<DiffSection>,
}

#[derive(Debug)]
pub(crate) struct DiffSection {
    pub(crate) old_path: Option<String>,
    pub(crate) new_path: Option<String>,
    pub(crate) hunks: Vec<DiffHunk>,
}

#[derive(Debug)]
pub(crate) struct DiffHunk {
    pub(crate) old_start: u64,
    pub(crate) new_start: u64,
    old_remaining: u64,
    new_remaining: u64,
    pub(crate) next_old: u64,
    pub(crate) next_new: u64,
    pub(crate) added: BTreeSet<u64>,
    pub(crate) deleted: BTreeSet<u64>,
    /// Hunk body in document order with the leading mode character stripped.
    pub(crate) lines: Vec<DiffLine>,
}

/// One retained hunk body line: its diff role plus the text after the role
/// character. FIX fxi3X: a trailing `\ No newline at end of file`
/// marker is retained as `no_newline` on the line it follows; the flag
/// applies to the side(s) that line feeds (old for deleted, new for
/// added, both for context).
#[derive(Debug)]
pub(crate) struct DiffLine {
    pub(crate) kind: DiffLineKind,
    pub(crate) text: String,
    pub(crate) no_newline: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DiffLineKind {
    Context,
    Added,
    Deleted,
}

impl DiffHunk {
    fn consume_old(&mut self) -> Result<(), String> {
        if self.old_remaining == 0 {
            return Err("hunk contains more old lines than declared".to_string());
        }
        self.old_remaining -= 1;
        self.next_old = self
            .next_old
            .checked_add(1)
            .ok_or_else(|| "diff hunk line count overflows".to_string())?;
        Ok(())
    }

    fn consume_new(&mut self) -> Result<(), String> {
        if self.new_remaining == 0 {
            return Err("hunk contains more new lines than declared".to_string());
        }
        self.new_remaining -= 1;
        self.next_new = self
            .next_new
            .checked_add(1)
            .ok_or_else(|| "diff hunk line count overflows".to_string())?;
        Ok(())
    }
}

fn push_current(current: &mut Option<DiffSection>, sections: &mut Vec<DiffSection>) {
    if let Some(section) = current.take() {
        sections.push(section);
    }
}

fn parse_diff_header_path(raw: &str) -> Option<String> {
    let value = raw.split('\t').next().unwrap_or(raw).trim();
    if value == "/dev/null" {
        return None;
    }
    let stripped = value
        .strip_prefix("a/")
        .or_else(|| value.strip_prefix("b/"))
        .unwrap_or(value);
    Some(stripped.to_string())
}

fn parse_hunk_header(line: &str) -> Result<DiffHunk, String> {
    let mut parts = line.split_whitespace();
    if parts.next() != Some("@@") {
        return Err(format!("malformed unified hunk header `{line}`"));
    }
    let old = parts
        .next()
        .filter(|part| part.starts_with('-'))
        .ok_or_else(|| format!("malformed unified hunk header `{line}`"))?;
    let new = parts
        .next()
        .filter(|part| part.starts_with('+'))
        .ok_or_else(|| format!("malformed unified hunk header `{line}`"))?;
    if parts.next() != Some("@@") {
        return Err(format!("malformed unified hunk header `{line}`"));
    }
    let (old_start, old_count) = parse_hunk_range(old, '-')?;
    let (new_start, new_count) = parse_hunk_range(new, '+')?;
    Ok(DiffHunk {
        old_start,
        new_start,
        old_remaining: old_count,
        new_remaining: new_count,
        next_old: old_start,
        next_new: new_start,
        added: BTreeSet::new(),
        deleted: BTreeSet::new(),
        lines: Vec::new(),
    })
}

fn parse_hunk_range(value: &str, prefix: char) -> Result<(u64, u64), String> {
    let value = value
        .strip_prefix(prefix)
        .ok_or_else(|| format!("invalid hunk range `{value}`"))?;
    let mut pieces = value.split(',');
    let start = pieces
        .next()
        .and_then(|part| part.parse().ok())
        .ok_or_else(|| format!("invalid hunk range `{value}`"))?;
    let count = pieces
        .next()
        .map(str::parse)
        .transpose()
        .map_err(|error| format!("invalid hunk range `{value}`: {error}"))?
        .unwrap_or(1);
    if pieces.next().is_some() {
        return Err(format!("invalid hunk range `{value}`"));
    }
    Ok((start, count))
}

/// Validates the in-progress hunk and files it into its section.
fn finish_hunk(
    hunk: &mut Option<DiffHunk>,
    current: &mut Option<DiffSection>,
) -> Result<(), String> {
    if let Some(state) = hunk.take() {
        if state.old_remaining != 0 || state.new_remaining != 0 {
            return Err(format!(
                "hunk ended before declared extents were consumed (old remaining {}, new remaining {})",
                state.old_remaining, state.new_remaining
            ));
        }
        if let Some(section) = current.as_mut() {
            section.hunks.push(state);
        }
    }
    Ok(())
}

fn require_equal(violations: &mut Vec<String>, field: &str, actual: &str, expected: &str) {
    if actual != expected {
        violations.push(format!(
            "{field}: unknown value `{actual}`; expected `{expected}`"
        ));
    }
}

fn require_non_empty(violations: &mut Vec<String>, field: &str, value: &str) {
    if value.trim().is_empty() {
        violations.push(format!("{field}: must not be blank"));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::{Value, json};

    use super::{
        INVENTORY_PATHS, LoadedEnvelope, PanelAggregates, PythonJudgedPanelEnvelope,
        check_inventory_at, derive_aggregates, missing_directions, parse_unified_diff,
    };

    const PANEL_DIR: &str = "fixtures/python-judged-pr-panel";

    struct TempFixture {
        root: PathBuf,
    }

    impl TempFixture {
        fn new(name: &str) -> Result<Self, String> {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "ripr-python-judged-panel-{name}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(root.join(format!("{PANEL_DIR}/diffs")))
                .map_err(|error| format!("create test fixture: {error}"))?;
            Ok(Self { root })
        }

        /// Writes a diff whose only changed line is new-file line 6.
        fn write_diff(&self, name: &str, target: &str, added: &str) -> Result<String, String> {
            let relative = format!("{PANEL_DIR}/diffs/{name}.diff");
            let body = format!(
                "--- a/{target}\n+++ b/{target}\n@@ -5,3 +5,3 @@\n context\n-old_behavior()\n+{added}\n context\n"
            );
            fs::write(self.root.join(&relative), body)
                .map_err(|error| format!("write test diff: {error}"))?;
            Ok(relative)
        }

        fn write_envelope(&self, name: &str, value: &Value) -> Result<String, String> {
            let relative = format!("{PANEL_DIR}/{name}");
            let body = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
            fs::write(self.root.join(&relative), body).map_err(|error| error.to_string())?;
            Ok(relative)
        }
    }

    impl Drop for TempFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn object_mut(value: &mut Value) -> Result<&mut serde_json::Map<String, Value>, String> {
        match value {
            Value::Object(map) => Ok(map),
            other => Err(format!("fixture value must be an object, found {other:?}")),
        }
    }

    fn seed_item(id: &str, direction: &str, diff_path: &str, target: &str) -> Value {
        json!({
            "id": id,
            "repo": "alt-synthetic-repo",
            "diff_path": diff_path,
            "shape": ["pytest_library"],
            "expected_direction": direction,
            "anchor": {
                "file": target,
                "line": 6,
                "owner": "alt_owner",
                "boundary": "alt changed sink"
            },
            "expected_classification": match direction {
                "should_gap" => "weakly_exposed",
                "should_stay_quiet" => "exposed",
                _ => "static_unknown",
            },
            "expected_static_limit_kind": if direction == "should_limit" {
                Value::String("decorator_indirection".to_string())
            } else {
                Value::Null
            },
            "labels": {
                "top_card_useful": null,
                "false_actionable": null,
                "false_exposed": null,
                "verify_command_valid": null,
                "suggested_location_valid": null,
                "packet_boundaries_safe": null,
                "limitation_quality": null
            },
            "authority_boundary": "review_advisory_only",
            "repair_packet_ready": false,
            "must_not_claim": ["Do not treat a null label as a passing judgment."],
            "reason": "alternate load-bearing selection reason"
        })
    }

    fn judged_item(
        id: &str,
        direction: &str,
        diff_path: &str,
        target: &str,
        actionable: Value,
    ) -> Result<Value, String> {
        let mut item = seed_item(id, direction, diff_path, target);
        let object = object_mut(&mut item)?;
        object.remove("must_not_claim");
        object.insert("actual_classification".to_string(), json!("static_unknown"));
        object.insert("actual_oracle_alignment".to_string(), json!("unknown"));
        object["labels"]["false_actionable"] = actionable;
        object["labels"]["false_exposed"] = json!(false);
        object["labels"]["packet_boundaries_safe"] = json!(true);
        object.insert("judgment_source".to_string(), json!("manual_review"));
        object.insert("judged_at".to_string(), json!("2026-06-13"));
        object.insert("judged_by".to_string(), json!("campaign"));
        Ok(item)
    }

    fn carryover_item(id: &str, diff_path: &str) -> Result<Value, String> {
        let mut item = judged_item(id, "should_limit", diff_path, "unused.py", json!(false))?;
        let object = object_mut(&mut item)?;
        object["anchor"]["file"] = Value::Null;
        object["anchor"]["line"] = Value::Null;
        object["expected_classification"] = Value::Null;
        object["actual_classification"] = Value::Null;
        object["actual_oracle_alignment"] = Value::Null;
        object.insert("expected_static_limit_kind".to_string(), json!("timeout"));
        Ok(item)
    }

    fn measurement(items: u64, exposed: u64, actionable: u64) -> Value {
        json!({
            "items_judged": items,
            "false_exposed_count": exposed,
            "false_actionable_count": actionable,
            "note": "alternate synthetic measurement note"
        })
    }

    fn envelope(items: Vec<Value>, totals: Value) -> Value {
        let mut value = json!({
            "schema_version": "0.1",
            "kind": "python_judged_pr_panel_manifest",
            "spec": "RIPR-SPEC-0092",
            "tier": "B",
            "description": "Alternate synthetic panel proving the validator is not hard coded.",
            "limits": ["alternate synthetic panel remains advisory only"],
            "items": items
        });
        if !totals.is_null() {
            value["measurement_summary"] = totals;
        }
        value
    }

    /// A valid synthetic inventory: seed plus judged panel with carryover.
    fn valid_alternate_inventory(fixture: &TempFixture) -> Result<Vec<(String, Value)>, String> {
        let gap = fixture.write_diff("alt-gap", "pkg/gap.py", "gap_behavior()")?;
        let quiet = fixture.write_diff("alt-quiet", "pkg/quiet.py", "quiet_behavior()")?;
        let limit = fixture.write_diff("alt-limit", "pkg/limit.py", "limit_behavior()")?;
        let judged = fixture.write_diff("alt-judged", "pkg/judged.py", "judged_behavior()")?;
        let drifted = fixture.write_diff("alt-drift", "pkg/drift.py", "drift_behavior()")?;
        let mut drift_row = judged_item(
            "alt-drift-judged",
            "should_gap",
            &drifted,
            "pkg/drift.py",
            json!(false),
        )?;
        let drift_object = object_mut(&mut drift_row)?;
        drift_object["anchor"]["line"] = json!(7);
        drift_object["expected_classification"] = json!("weakly_exposed");
        Ok(vec![
            (
                "alt-seed.json".to_string(),
                envelope(
                    vec![
                        seed_item("alt-seed-gap", "should_gap", &gap, "pkg/gap.py"),
                        seed_item(
                            "alt-seed-quiet",
                            "should_stay_quiet",
                            &quiet,
                            "pkg/quiet.py",
                        ),
                        seed_item("alt-seed-limit", "should_limit", &limit, "pkg/limit.py"),
                    ],
                    Value::Null,
                ),
            ),
            (
                "alt-judged.json".to_string(),
                envelope(
                    vec![
                        judged_item(
                            "alt-judged-quiet",
                            "should_stay_quiet",
                            &judged,
                            "pkg/judged.py",
                            json!(false),
                        )?,
                        drift_row,
                        carryover_item("alt-judged-carryover", &judged)?,
                    ],
                    measurement(2, 0, 0),
                ),
            ),
        ])
    }

    /// Writes the inventory and runs the validator against it.
    fn check(
        fixture: &TempFixture,
        envelopes: &[(String, Value)],
    ) -> Result<super::CheckReport, String> {
        let mut displays = Vec::new();
        for (name, value) in envelopes {
            displays.push(fixture.write_envelope(name, value)?);
        }
        let refs = displays.iter().map(String::as_str).collect::<Vec<_>>();
        check_inventory_at(&fixture.root, &refs)
    }

    fn expect_rejection(
        fixture: &TempFixture,
        envelopes: &[(String, Value)],
        expected: &[&str],
    ) -> Result<(), String> {
        let error = check(fixture, envelopes)
            .err()
            .ok_or("malformed python judged-panel fixture was accepted")?;
        if !error.contains("rerun: cargo xtask python-judged-panel check") {
            return Err(format!("rejection omitted direct rerun command: {error}"));
        }
        if let Some(fragment) = expected.iter().find(|fragment| !error.contains(*fragment)) {
            return Err(format!(
                "expected rejection to contain `{fragment}`; actual: {error}"
            ));
        }
        Ok(())
    }

    #[test]
    fn retained_and_alternate_inventories_validate() -> Result<(), String> {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("xtask manifest has no repository parent")?;
        let report = check_inventory_at(repository_root, &INVENTORY_PATHS)?;
        let aggregates = &report.aggregates;
        if aggregates.items_total != 11
            || aggregates.seed_items != 3
            || aggregates.judged_items != 7
            || aggregates.carryover_items != 1
            || aggregates.false_actionable_numerator != 3
            || aggregates.false_exposed_numerator != 0
        {
            return Err(format!(
                "retained inventory aggregates drifted from the historical n=7 record: {aggregates:?}"
            ));
        }
        if !missing_directions(aggregates).is_empty() || report.warnings.len() != 3 {
            return Err(format!(
                "retained inventory lost coverage or drifted warnings: {:?}",
                report.warnings
            ));
        }

        // Anti-hardcode proof: a completely different synthetic inventory with
        // fresh paths, ids, and repos must also validate.
        let fixture = TempFixture::new("alternate-valid")?;
        let envelopes = valid_alternate_inventory(&fixture)?;
        let alternate = check(&fixture, &envelopes)?;
        let aggregates = &alternate.aggregates;
        if aggregates.items_total != 6
            || aggregates.seed_items != 3
            || aggregates.judged_items != 2
            || aggregates.carryover_items != 1
        {
            return Err("alternate inventory aggregates are wrong".to_string());
        }
        if alternate.warnings.len() != 1 {
            return Err(format!(
                "the one-line drifted anchor must warn exactly once, found {}: {:?}",
                alternate.warnings.len(),
                alternate.warnings
            ));
        }
        Ok(())
    }

    #[test]
    fn loader_rejects_identity_vocabulary_and_parse_rot() -> Result<(), String> {
        let fixture = TempFixture::new("loader-rot")?;
        for (field, value, fragment) in [
            (
                "schema_version",
                json!("9.9"),
                "schema_version: unknown value `9.9`",
            ),
            (
                "kind",
                json!("other_kind"),
                "kind: unknown value `other_kind`",
            ),
            ("tier", json!("A"), "tier: unknown value `A`"),
            (
                "spec",
                json!("RIPR-SPEC-0000"),
                "spec: unknown value `RIPR-SPEC-0000`",
            ),
        ] {
            let mut envelopes = valid_alternate_inventory(&fixture)?;
            envelopes[0].1[field] = value;
            expect_rejection(&fixture, &envelopes, &[fragment])?;
        }

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["expected_direction"] = json!("should_pass");
        envelopes[1].1["items"][0]["judgment_source"] = json!("copied_static_output");
        envelopes[1].1["items"][0]["actual_oracle_alignment"] = json!("vibes");
        envelopes[1].1["items"][0]["labels"]["limitation_quality"] = json!("fine");
        envelopes[0].1["items"][1]["expected_classification"] = json!("bogus_verdict");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "expected_direction: unknown direction `should_pass`",
                "unknown judgment value `copied_static_output`",
                "unknown oracle-alignment value `vibes`",
                "unknown limitation-quality value `fine`",
                "`should_stay_quiet` requires one of exposed, found `bogus_verdict`",
            ],
        )?;

        // Missing field, unknown field, duplicate JSON key, non-UTF-8 bytes.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0]
            .1
            .as_object_mut()
            .ok_or("seed envelope must be an object")?
            .remove("items");
        let error = check(&fixture, &envelopes)
            .err()
            .ok_or("missing field was accepted")?;
        if !error.contains("missing field `items`") {
            return Err(format!("unexpected missing-field error: {error}"));
        }

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["typo_field"] = json!(true);
        expect_rejection(&fixture, &envelopes, &["unknown field `typo_field`"])?;

        let envelopes = valid_alternate_inventory(&fixture)?;
        check(&fixture, &envelopes)?;
        fs::write(
            fixture.root.join(format!("{PANEL_DIR}/alt-seed.json")),
            r#"{"tier":"B","tier":"B"}"#,
        )
        .map_err(|error| error.to_string())?;
        let refs = [
            "fixtures/python-judged-pr-panel/alt-seed.json",
            "fixtures/python-judged-pr-panel/alt-judged.json",
        ];
        let error = check_inventory_at(&fixture.root, &refs)
            .err()
            .ok_or("duplicate key was accepted")?;
        if !error.contains("duplicate object key `tier`") {
            return Err(format!("unexpected duplicate-key error: {error}"));
        }

        let bytes_fixture = TempFixture::new("non-utf8")?;
        let envelopes = valid_alternate_inventory(&bytes_fixture)?;
        check(&bytes_fixture, &envelopes)?;
        fs::write(
            bytes_fixture
                .root
                .join(format!("{PANEL_DIR}/alt-seed.json")),
            [0xFF_u8, 0xFE, 0x00],
        )
        .map_err(|error| error.to_string())?;
        let refs = [
            "fixtures/python-judged-pr-panel/alt-seed.json",
            "fixtures/python-judged-pr-panel/alt-judged.json",
        ];
        let error = check_inventory_at(&bytes_fixture.root, &refs)
            .err()
            .ok_or("non-UTF-8 bytes were accepted")?;
        if !error.contains("read Python judged panel envelope") || !error.contains("UTF-8") {
            return Err(format!("unexpected non-UTF-8 error: {error}"));
        }

        // Independent violations must all surface, deterministically ordered.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["schema_version"] = json!("9.9");
        envelopes[0].1["items"][0]["reason"] = json!("  ");
        envelopes[0].1["items"][1]["id"] = json!("alt-seed-gap");
        let first = check(&fixture, &envelopes)
            .err()
            .ok_or("invalid fixture passed the first validation")?;
        let second = check(&fixture, &envelopes)
            .err()
            .ok_or("invalid fixture passed the second validation")?;
        if first != second {
            return Err("semantic violations are not deterministic".to_string());
        }
        for fragment in ["schema_version", ".reason", "duplicate case id"] {
            if !first.contains(fragment) {
                return Err(format!(
                    "all-violations output omitted `{fragment}`: {first}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn panel_contract_rejects_inventory_judgment_and_totals_drift() -> Result<(), String> {
        let fixture = TempFixture::new("contract")?;
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][0]["id"] = envelopes[0].1["items"][0]["id"].clone();
        expect_rejection(
            &fixture,
            &envelopes,
            &["duplicate case id also declared in"],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][2]["expected_direction"] = json!("should_gap");
        envelopes[1].1["items"][2]["expected_direction"] = json!("should_gap");
        expect_rejection(
            &fixture,
            &envelopes,
            &["missing required direction coverage `should_limit`"],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["id"] = json!("   ");
        expect_rejection(&fixture, &envelopes, &["items[0].id: must not be blank"])?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][0]["labels"]["false_actionable"] = json!(true);
        envelopes[1].1["items"][0]["labels"]["false_exposed"] = json!(true);
        expect_rejection(
            &fixture,
            &envelopes,
            &["cannot both be true for one terminal adjudication"],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][0]["labels"]["false_actionable"] = Value::Null;
        envelopes[1].1["measurement_summary"]["items_judged"] = json!(3);
        envelopes[1].1["measurement_summary"]["false_actionable_count"] = json!(1);
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "labels.false_actionable: judged rows must record an explicit decision; null is inconclusive, not a pass",
                "measurement_summary.items_judged: hand-entered total 3 disagrees with 2 derived judged rows",
                "measurement_summary.false_actionable_count: hand-entered total 1 disagrees with 0 derived rows",
            ],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["labels"]["false_actionable"] = json!(false);
        envelopes[0].1["items"][0]["must_not_claim"] = json!([]);
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "seed labels must remain null; null means unjudged, not false/pass",
                "require at least one non-empty null-honesty non-claim",
            ],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["repair_packet_ready"] = json!(true);
        envelopes[0].1["items"][1]["authority_boundary"] = json!("auto_promote");
        envelopes[0].1["items"][1]["expected_static_limit_kind"] = json!("decorator_indirection");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "repair_packet_ready: must remain false; the panel is review-advisory only",
                "authority_boundary: unknown value `auto_promote`; expected `review_advisory_only`",
                "a non-null limit kind requires `should_limit`, found `should_stay_quiet`",
            ],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][0]["judged_at"] = Value::Null;
        envelopes[1].1["items"][0]["judged_by"] = Value::Null;
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "judged rows require non-empty judged_at and judged_by; absent currentness identity cannot be represented as current",
            ],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["judged_at"] = json!("2026-06-13");
        envelopes[0].1["items"][0]["judged_by"] = json!("campaign");
        expect_rejection(
            &fixture,
            &envelopes,
            &["unjudged rows cannot carry judged_at/judged_by identity"],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][2]["anchor"]["file"] = json!("pkg/judged.py");
        envelopes[1].1["items"][2]["anchor"]["line"] = json!(6);
        envelopes[1].1["items"][2]["labels"]["false_exposed"] = json!(true);
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "carryover rows (null expected_classification) cannot declare an anchor",
                "carryover rows cannot claim false_actionable/false_exposed",
            ],
        )
    }

    /// Review-round strictness (#3669): direction-compatible error labels,
    /// judged-denominator integrity, explicit-null seed labels, the mirrored
    /// StaticLimitKind vocabulary with the carryover-timeout grandfather, and
    /// blank pinned revisions.
    #[test]
    fn review_hardening_rejects_direction_label_denominator_and_vocabulary_drift()
    -> Result<(), String> {
        let fixture = TempFixture::new("review-hardening")?;

        // FIX 1: a should_gap row cannot carry false_actionable; a
        // should_stay_quiet row cannot carry false_exposed.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][1]["labels"]["false_actionable"] = json!(true);
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "labels.false_actionable: `should_gap` cannot carry a true false_actionable per the SPEC-0092 outcome table",
            ],
        )?;
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][0]["labels"]["false_exposed"] = json!(true);
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "labels.false_exposed: `should_stay_quiet` cannot carry a true false_exposed per the SPEC-0092 outcome table",
            ],
        )?;

        // FIX 2: a blank judgment_source is not a declared source, so the row
        // falls back to unjudged (and its judged-style labels violate the seed
        // null-honesty rule); a claimed-judged row without the observed
        // verdict cannot enter the denominator.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][0]["judgment_source"] = json!("   ");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "seed labels must remain null; null means unjudged, not false/pass",
                "must_not_claim: seed items require at least one non-empty null-honesty non-claim",
            ],
        )?;
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][0]["actual_classification"] = Value::Null;
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "actual_classification: judged rows require the observed verdict; a claimed judgment without it cannot enter the denominator",
            ],
        )?;

        // FIX 3: a seed row must carry every label key explicitly null.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0]
            .1
            .as_object_mut()
            .ok_or("seed envelope must be an object")?
            .remove("items");
        envelopes[0].1["items"] = json!([seed_item(
            "alt-seed-gap",
            "should_gap",
            "fixtures/python-judged-pr-panel/diffs/alt-gap.diff",
            "pkg/gap.py"
        )]);
        envelopes[0].1["items"][0]["labels"]
            .as_object_mut()
            .ok_or("labels must be an object")?
            .remove("verify_command_valid");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "labels.verify_command_valid: seed rows must declare every label key explicitly null",
            ],
        )?;

        // FIX 4: non-registered kinds are rejected; the carryover-timeout
        // grandfather does not extend to other row kinds.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][2]["expected_static_limit_kind"] = json!("horoscope");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "unknown static-limit kind `horoscope`; must be a registered StaticLimitKind from the product contract",
            ],
        )?;
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][2]["expected_static_limit_kind"] = json!("timeout");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "unknown static-limit kind `timeout`; must be a registered StaticLimitKind from the product contract",
            ],
        )?;

        // FIX 5: a whitespace-only pinned revision is a broken pin, not an
        // unpinned row.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["base"] = json!("   ");
        expect_rejection(
            &fixture,
            &envelopes,
            &["base: must be a non-empty hex commit sha when pinned"],
        )?;

        // FIX A positive: the should_limit lattice admits BOTH error labels,
        // so a judged should_limit row over-credited as false_exposed is a
        // coherent judgment, not a violation.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][1]["expected_direction"] = json!("should_limit");
        envelopes[1].1["items"][1]["expected_classification"] = json!("static_unknown");
        envelopes[1].1["items"][1]["expected_static_limit_kind"] = json!("decorator_indirection");
        envelopes[1].1["items"][1]["labels"]["false_actionable"] = json!(false);
        envelopes[1].1["items"][1]["labels"]["false_exposed"] = json!(true);
        envelopes[1].1["measurement_summary"]["false_exposed_count"] = json!(1);
        let report = check(&fixture, &envelopes)?;
        if report.aggregates.false_exposed_numerator != 1 {
            return Err("the admitted should_limit false_exposed must be derived".to_string());
        }

        // FIX B: crediting `exposed` on a should_gap judged row is an
        // over-credit and must be recorded as false_exposed.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][1]["actual_classification"] = json!("exposed");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "labels.false_exposed: actual_classification `exposed` on a `should_gap` row is an over-credit and requires false_exposed true",
            ],
        )?;

        // FIX C: a synthetic judged should_limit row without a named kind is
        // rejected (the retained grandfathers do not extend to it).
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1].1["items"][1]["expected_direction"] = json!("should_limit");
        envelopes[1].1["items"][1]["expected_classification"] = json!("static_unknown");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "expected_static_limit_kind: `should_limit` rows require a registered static-limit kind",
            ],
        )?;

        // FIX D: an envelope with judged rows must declare its measurement
        // summary.
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[1]
            .1
            .as_object_mut()
            .ok_or("judged envelope must be an object")?
            .remove("measurement_summary");
        expect_rejection(
            &fixture,
            &envelopes,
            &["measurement_summary: envelopes with judged rows require a measurement summary"],
        )
    }

    #[test]
    fn anchor_proofs_reject_traversal_missing_and_target_drift() -> Result<(), String> {
        let fixture = TempFixture::new("anchors")?;
        for value in [
            "fixtures/python-judged-pr-panel/diffs/../escape.diff",
            "/absolute/outside.diff",
            "src/lib.rs.diff",
            "fixtures\\python-judged-pr-panel\\diffs\\alt-gap.diff",
        ] {
            let mut envelopes = valid_alternate_inventory(&fixture)?;
            envelopes[0].1["items"][0]["diff_path"] = json!(value);
            expect_rejection(
                &fixture,
                &envelopes,
                &["must be a relative .diff file under `fixtures/`"],
            )?;
        }
        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["diff_path"] =
            json!("fixtures/python-judged-pr-panel/diffs/missing.diff");
        expect_rejection(&fixture, &envelopes, &["is missing or is not a file"])?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["anchor"]["file"] = json!("pkg/other.py");
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "target-file mismatch: `fixtures/python-judged-pr-panel/diffs/alt-gap.diff` does not touch `pkg/other.py`",
            ],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["anchor"]["line"] = json!(40);
        expect_rejection(
            &fixture,
            &envelopes,
            &[
                "line 40 does not map to an added/deleted position in `fixtures/python-judged-pr-panel/diffs/alt-gap.diff` (nearest changed position 6",
            ],
        )?;

        let mut envelopes = valid_alternate_inventory(&fixture)?;
        envelopes[0].1["items"][0]["anchor"]["line"] = json!(0);
        expect_rejection(&fixture, &envelopes, &["anchor.line: must be positive"])
    }

    #[test]
    fn aggregates_derive_from_rows_only_with_guarded_denominators() -> Result<(), String> {
        let mut item = judged_item(
            "agg-judged",
            "should_stay_quiet",
            "fixtures/python-judged-pr-panel/diffs/alt-judged.diff",
            "pkg/judged.py",
            json!(true),
        )?;
        object_mut(&mut item)?["repo"] = json!("agg-repo");
        let envelope = serde_json::from_value::<PythonJudgedPanelEnvelope>(envelope(
            vec![item],
            measurement(1, 0, 1),
        ))
        .map_err(|error| error.to_string())?;
        let aggregates = derive_aggregates(&[LoadedEnvelope {
            display: "agg.json".to_string(),
            envelope,
        }]);
        if aggregates.items_total != 1
            || aggregates.seed_items != 0
            || aggregates.judged_items != 1
            || aggregates.carryover_items != 0
            || aggregates.false_actionable_numerator != 1
            || aggregates.false_exposed_numerator != 0
            || aggregates.directions_covered != BTreeSet::from(["should_stay_quiet".to_string()])
        {
            return Err(format!(
                "aggregate derivation drifted from rows: {aggregates:?}"
            ));
        }
        let direction = aggregates
            .per_direction
            .get("should_stay_quiet")
            .ok_or("missing per-direction aggregate")?;
        if direction.selected != 1 || direction.judged != 1 {
            return Err(format!("per-direction aggregate drifted: {direction:?}"));
        }
        let repo = aggregates
            .per_repo
            .get("agg-repo")
            .ok_or("missing per-repo aggregate")?;
        if repo.selected != 1 || repo.judged != 1 {
            return Err(format!("per-repo aggregate drifted: {repo:?}"));
        }
        if aggregates.false_actionable_rate().is_none() || aggregates.false_exposed_rate().is_none()
        {
            return Err("rates must exist when a judged denominator exists".to_string());
        }
        let empty = PanelAggregates::default();
        if empty.false_actionable_rate().is_some() || empty.false_exposed_rate().is_some() {
            return Err("rates must stay absent without a judged denominator".to_string());
        }
        if missing_directions(&empty) != ["should_gap", "should_stay_quiet", "should_limit"] {
            return Err("empty aggregate lost the required-direction gap".to_string());
        }
        Ok(())
    }

    /// fxi3X round-3 review: a doubled no-newline marker is malformed.
    #[test]
    fn diff_parser_rejects_doubled_no_newline_marker() -> Result<(), String> {
        let body = concat!(
            "--- a/file.py\n",
            "+++ b/file.py\n",
            "@@ -1 +1 @@\n",
            "-old\n",
            "+new\n",
            r"\ No newline at end of file",
            "\n",
            r"\ No newline at end of file",
            "\n",
        );
        let err = parse_unified_diff(body).err().ok_or("expected rejection")?;
        assert!(
            err.contains("repeated no-newline marker"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn diff_parser_proves_positions_and_rejects_false_extents() -> Result<(), String> {
        let parsed = parse_unified_diff(
            "--- a/pkg/multi.py\n+++ b/pkg/multi.py\n@@ -1,1 +1,1 @@ func ctx\n-a()\n+b()\n@@ -40,1 +40,2 @@\n ctx\n+added_late()\n",
        )
        .map_err(|error| error.to_string())?;
        let section = parsed.sections.first().ok_or("section must exist")?;
        if section.old_path.as_deref() != Some("pkg/multi.py")
            || section.new_path.as_deref() != Some("pkg/multi.py")
        {
            return Err("file paths were not stripped of a/ b/ prefixes".to_string());
        }
        let added = section
            .hunks
            .iter()
            .flat_map(|hunk| hunk.added.iter().copied())
            .collect::<Vec<_>>();
        let deleted = section
            .hunks
            .iter()
            .flat_map(|hunk| hunk.deleted.iter().copied())
            .collect::<Vec<_>>();
        if added != vec![1, 41] || deleted != vec![1] {
            return Err(format!(
                "unexpected changed positions: {added:?} {deleted:?}"
            ));
        }

        // The retained seed rendition ends its hunk with two empty (blank
        // context) lines; the parser must accept them and place the added line
        // at new-file position 3.
        let retained = parse_unified_diff(include_str!(
            "../../fixtures/python-judged-pr-panel/diffs/decorator_indirection_limit.diff"
        ))
        .map_err(|error| error.to_string())?;
        let retained_added = retained
            .sections
            .first()
            .ok_or("retained section missing")?
            .hunks
            .first()
            .ok_or("retained hunk missing")?
            .added
            .iter()
            .copied()
            .collect::<Vec<_>>();
        if retained_added != vec![3] {
            return Err(format!(
                "retained added positions drifted: {retained_added:?}"
            ));
        }

        for (name, body) in [
            (
                "overflow",
                "--- a/pkg/x.py\n+++ b/pkg/x.py\n@@ -1,1 +1,1 @@\n ctx\n-a()\n+b()\n",
            ),
            (
                "truncated",
                "--- a/pkg/x.py\n+++ b/pkg/x.py\n@@ -5,3 +5,3 @@\n ctx\n-a()\n+b()\n",
            ),
            (
                "rename",
                "rename from old.py\nrename to pkg/x.py\n--- a/old.py\n+++ b/pkg/x.py\n@@ -1,1 +1,1 @@\n-a()\n+b()\n",
            ),
            (
                "unbound-hunk",
                "--- a/pkg/x.py\n@@ -1,1 +1,1 @@\n-a()\n+b()\n",
            ),
            (
                "quoted",
                "--- a/pkg/x.py\n+++ \"b/pkg/x.py\"\n@@ -1,1 +1,1 @@\n-a()\n+b()\n",
            ),
        ] {
            let error = parse_unified_diff(body)
                .err()
                .ok_or(format!("invalid `{name}` diff was accepted"))?;
            if error.is_empty() {
                return Err(format!("`{name}` produced an empty error"));
            }
        }

        // u64::MAX hunk starts must overflow the line counters into a named
        // error instead of panicking on position arithmetic.
        for (name, body) in [
            (
                "old-start-overflow",
                "--- a/pkg/x.py\n+++ b/pkg/x.py\n@@ -18446744073709551615,1 +1,1 @@\n-a()\n+b()\n",
            ),
            (
                "new-start-overflow",
                "--- a/pkg/x.py\n+++ b/pkg/x.py\n@@ -1,1 +18446744073709551615,1 @@\n-a()\n+b()\n",
            ),
        ] {
            let error = parse_unified_diff(body)
                .err()
                .ok_or(format!("`{name}` u64::MAX hunk was accepted"))?;
            if !error.contains("diff hunk line count overflows") {
                return Err(format!("unexpected u64::MAX error for `{name}`: {error}"));
            }
        }
        Ok(())
    }
}
