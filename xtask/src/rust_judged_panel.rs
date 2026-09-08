use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use ra_ap_syntax::{Edition, SourceFile, SyntaxKind};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

mod host_run;
mod packet;
mod subject;

pub(crate) const MANIFEST_PATH: &str = "metrics/rust-judged-behavior-panel/manifest.json";
const DIFF_ROOT: &str = "metrics/rust-judged-behavior-panel/diffs";
const RERUN_COMMAND: &str = "cargo xtask rust-judged-panel check";
const REQUIRED_DIRECTIONS: [&str; 3] = ["should_gap", "should_stay_quiet", "should_limit"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RustJudgedPanelManifest {
    schema_version: String,
    kind: String,
    authority: String,
    tier: String,
    description: String,
    selection_status: String,
    limits: Vec<String>,
    required_directions: Vec<String>,
    items: Vec<RustJudgedPanelItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustJudgedPanelItem {
    id: String,
    repository: String,
    #[serde(default)]
    base: Nullable<String>,
    #[serde(default)]
    head: Nullable<String>,
    #[serde(default)]
    tree_identity: Nullable<String>,
    diff_path: String,
    expected_direction: String,
    behavior_family: String,
    anchor: RustJudgedPanelAnchor,
    test_evidence: RustJudgedPanelTestEvidence,
    expected_classification: String,
    #[serde(default)]
    expected_static_limit_kind: Nullable<String>,
    expected_actionability: String,
    selection_dimensions: RustJudgedPanelSelectionDimensions,
    labels: RustJudgedPanelLabels,
    runtime_calibration: RustJudgedPanelRuntimeCalibration,
    #[serde(default)]
    judgment_source: Nullable<String>,
    #[serde(default)]
    judged_at: Nullable<String>,
    judged_by: Vec<String>,
    disposition: String,
    must_not_claim: Vec<String>,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustJudgedPanelAnchor {
    file: String,
    line: u64,
    owner: String,
    changed_behavior: String,
    required_discriminator: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustJudgedPanelTestEvidence {
    relation_basis: String,
    oracle_kind: String,
    oracle_strength: String,
    observed_inputs: Vec<String>,
    #[serde(default)]
    aligned_observer: Nullable<String>,
    #[serde(default)]
    missing: Nullable<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustJudgedPanelSelectionDimensions {
    relation_basis: String,
    oracle_family: String,
    propagation_witness: String,
    target_kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustJudgedPanelLabels {
    #[serde(default)]
    structural_judgment: Nullable<bool>,
    #[serde(default)]
    false_actionable: Nullable<bool>,
    #[serde(default)]
    false_exposed: Nullable<bool>,
    #[serde(default)]
    static_under_credit: Nullable<bool>,
    #[serde(default)]
    wrong_target: Nullable<bool>,
    #[serde(default)]
    limitation_correct: Nullable<bool>,
}

impl RustJudgedPanelLabels {
    fn all_explicitly_null(&self) -> bool {
        self.structural_judgment.is_null()
            && self.false_actionable.is_null()
            && self.false_exposed.is_null()
            && self.static_under_credit.is_null()
            && self.wrong_target.is_null()
            && self.limitation_correct.is_null()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustJudgedPanelRuntimeCalibration {
    status: String,
    #[serde(default)]
    outcome: Nullable<String>,
    #[serde(default)]
    evidence_ref: Nullable<String>,
}

#[derive(Debug, Default)]
enum Nullable<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<T> Nullable<T> {
    fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    fn value(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Missing | Self::Null => None,
        }
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

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
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

fn parse_json_without_duplicate_keys(body: &str) -> Result<serde_json::Value, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(body);
    let StrictJson(value) = StrictJson::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    match args {
        [subcommand] if subcommand == "check" => {
            let manifest = check_at(Path::new("."))?;
            println!(
                "Rust judged panel seed, subjects, and portable packets valid: manifest={MANIFEST_PATH} items={} directions={}",
                manifest.items.len(),
                manifest.required_directions.join(",")
            );
            Ok(())
        }
        [subcommand] if subcommand == "replay" => {
            let manifest = check_seed_at(Path::new("."))?;
            host_run::run(Path::new("."), &manifest, None)
        }
        [subcommand, flag, output] if subcommand == "replay" && flag == "--out" => {
            let manifest = check_seed_at(Path::new("."))?;
            host_run::run(Path::new("."), &manifest, Some(output))
        }
        [subcommand] if subcommand == "packet" => {
            let manifest = check_seed_at(Path::new("."))?;
            packet::publish(Path::new("."), &manifest, packet::DEFAULT_HOST_CURRENT)
        }
        [subcommand, flag, current] if subcommand == "packet" && flag == "--host-current" => {
            let manifest = check_seed_at(Path::new("."))?;
            packet::publish(Path::new("."), &manifest, current)
        }
        [] => Err(format!(
            "rust-judged-panel requires `check`, `replay [--out target/ripr/<path>]`, or `packet [--host-current target/ripr/<path>/current.json]`\nrerun: {RERUN_COMMAND}"
        )),
        _ => Err(format!(
            "unknown rust-judged-panel arguments `{}`\nrerun: {RERUN_COMMAND}",
            args.join(" ")
        )),
    }
}

pub(crate) fn check_canonical() -> Result<(), String> {
    check_at(Path::new(".")).map(|_| ())
}

fn check_at(root: &Path) -> Result<RustJudgedPanelManifest, String> {
    let manifest = check_seed_at(root)?;
    packet::validate_at(root, &manifest)?;
    Ok(manifest)
}

fn check_seed_at(root: &Path) -> Result<RustJudgedPanelManifest, String> {
    let manifest = load_and_validate_at(root, Path::new(MANIFEST_PATH))?;
    subject::validate_at(root, &manifest)?;
    Ok(manifest)
}

pub(crate) fn load_and_validate_at(
    root: &Path,
    manifest_path: &Path,
) -> Result<RustJudgedPanelManifest, String> {
    let display = normalize_path(manifest_path);
    let body = fs::read_to_string(root.join(manifest_path))
        .map_err(|error| format!("read Rust judged panel manifest `{display}`: {error}"))?;
    let value = parse_json_without_duplicate_keys(&body)
        .map_err(|error| format!("parse Rust judged panel manifest `{display}`: {error}"))?;
    let manifest: RustJudgedPanelManifest = serde_json::from_value(value)
        .map_err(|error| format!("parse Rust judged panel manifest `{display}`: {error}"))?;
    let mut violations = validate_manifest(root, &manifest);
    violations.sort();
    violations.dedup();
    if violations.is_empty() {
        Ok(manifest)
    } else {
        Err(format!(
            "Rust judged panel manifest `{display}` has {} semantic violation(s):\n- {}\nrerun: {RERUN_COMMAND}",
            violations.len(),
            violations.join("\n- ")
        ))
    }
}

fn validate_manifest(root: &Path, manifest: &RustJudgedPanelManifest) -> Vec<String> {
    let mut violations = Vec::new();
    require_equal(
        &mut violations,
        "manifest.schema_version",
        &manifest.schema_version,
        "0.1",
    );
    require_equal(
        &mut violations,
        "manifest.kind",
        &manifest.kind,
        "rust_judged_behavior_panel_manifest",
    );
    require_equal(
        &mut violations,
        "manifest.authority",
        &manifest.authority,
        "EffortlessMetrics/ripr-swarm#3164",
    );
    require_equal(&mut violations, "manifest.tier", &manifest.tier, "seed");
    require_equal(
        &mut violations,
        "manifest.selection_status",
        &manifest.selection_status,
        "complete",
    );
    require_non_empty(
        &mut violations,
        "manifest.description",
        &manifest.description,
    );
    if manifest.limits.is_empty() || manifest.limits.iter().any(|limit| limit.trim().is_empty()) {
        violations.push("manifest.limits: require non-empty seed non-claims".to_string());
    }
    validate_required_directions(&manifest.required_directions, &mut violations);
    if manifest.items.is_empty() {
        violations.push("manifest.items: selected denominator must not be empty".to_string());
    }

    let required = manifest
        .required_directions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut selected_counts = BTreeMap::<&str, usize>::new();
    for (index, item) in manifest.items.iter().enumerate() {
        let subject = if item.id.trim().is_empty() {
            format!("items[{index}]")
        } else {
            format!("item `{}`", item.id)
        };
        if item.id.trim().is_empty() {
            violations.push(format!("{subject}.id: must not be blank"));
        } else if !ids.insert(item.id.as_str()) {
            violations.push(format!("{subject}.id: duplicate selected item identity"));
        }
        if !REQUIRED_DIRECTIONS.contains(&item.expected_direction.as_str()) {
            violations.push(format!(
                "{subject}.expected_direction: unknown direction `{}`",
                item.expected_direction
            ));
        } else {
            *selected_counts
                .entry(item.expected_direction.as_str())
                .or_default() += 1;
            if !required.contains(item.expected_direction.as_str()) {
                violations.push(format!(
                    "{subject}.expected_direction: `{}` is absent from manifest.required_directions",
                    item.expected_direction
                ));
            }
        }
        validate_item(root, item, &subject, &mut violations);
    }
    for direction in REQUIRED_DIRECTIONS {
        if selected_counts.get(direction).copied().unwrap_or(0) == 0 {
            violations.push(format!(
                "manifest.items: selected denominator is missing `{direction}`"
            ));
        }
    }
    violations
}

fn validate_required_directions(directions: &[String], violations: &mut Vec<String>) {
    let mut counts = BTreeMap::<&str, usize>::new();
    for direction in directions {
        *counts.entry(direction.as_str()).or_default() += 1;
        if !REQUIRED_DIRECTIONS.contains(&direction.as_str()) {
            violations.push(format!(
                "manifest.required_directions: unknown direction `{direction}`"
            ));
        }
    }
    for direction in REQUIRED_DIRECTIONS {
        match counts.get(direction).copied().unwrap_or(0) {
            0 => violations.push(format!(
                "manifest.required_directions: missing `{direction}`"
            )),
            1 => {}
            count => violations.push(format!(
                "manifest.required_directions: `{direction}` occurs {count} times"
            )),
        }
    }
}

fn validate_item(
    root: &Path,
    item: &RustJudgedPanelItem,
    subject: &str,
    violations: &mut Vec<String>,
) {
    require_non_empty(
        violations,
        &format!("{subject}.repository"),
        &item.repository,
    );
    require_non_empty(
        violations,
        &format!("{subject}.behavior_family"),
        &item.behavior_family,
    );
    require_non_empty(violations, &format!("{subject}.reason"), &item.reason);
    if item.must_not_claim.is_empty()
        || item
            .must_not_claim
            .iter()
            .any(|claim| claim.trim().is_empty())
    {
        violations.push(format!(
            "{subject}.must_not_claim: require at least one non-empty guard"
        ));
    }
    if !item.base.is_null() || !item.head.is_null() || !item.tree_identity.is_null() {
        violations.push(format!(
            "{subject}.identity: seed phase requires null base, head, and tree_identity"
        ));
    }
    validate_evidence_contract(item, subject, violations);
    validate_direction_contract(item, subject, violations);
    validate_seed_judgment(item, subject, violations);
    validate_runtime(item, subject, violations);
    validate_anchor(root, item, subject, violations);
}

fn validate_evidence_contract(
    item: &RustJudgedPanelItem,
    subject: &str,
    violations: &mut Vec<String>,
) {
    let evidence = &item.test_evidence;
    let dimensions = &item.selection_dimensions;
    for (field, value) in [
        ("test_evidence.relation_basis", &evidence.relation_basis),
        ("test_evidence.oracle_kind", &evidence.oracle_kind),
        ("test_evidence.oracle_strength", &evidence.oracle_strength),
        (
            "selection_dimensions.relation_basis",
            &dimensions.relation_basis,
        ),
        (
            "selection_dimensions.oracle_family",
            &dimensions.oracle_family,
        ),
        (
            "selection_dimensions.propagation_witness",
            &dimensions.propagation_witness,
        ),
        ("selection_dimensions.target_kind", &dimensions.target_kind),
    ] {
        require_non_empty(violations, &format!("{subject}.{field}"), value);
    }
    if evidence
        .observed_inputs
        .iter()
        .any(|input| input.trim().is_empty())
    {
        violations.push(format!(
            "{subject}.test_evidence.observed_inputs: entries must not be blank"
        ));
    }
    if evidence.oracle_kind != "exact_value" || evidence.oracle_strength != "strong" {
        violations.push(format!(
            "{subject}.test_evidence: seed requires exact_value/strong oracle vocabulary"
        ));
    }
    let expected = match item.expected_direction.as_str() {
        "should_gap" => Some((
            "direct_owner_call",
            "exact_value",
            "direct_return",
            "existing_test",
            true,
            true,
        )),
        "should_stay_quiet" => Some((
            "direct_owner_call",
            "exact_value",
            "direct_return",
            "existing_test",
            false,
            true,
        )),
        "should_limit" => Some((
            "macro_witness",
            "exact_value_candidate",
            "unresolved",
            "none",
            true,
            false,
        )),
        _ => None,
    };
    let Some((relation, oracle, propagation, target, missing, observer)) = expected else {
        return;
    };
    for (field, actual, required) in [
        (
            "selection_dimensions.relation_basis",
            dimensions.relation_basis.as_str(),
            relation,
        ),
        (
            "selection_dimensions.oracle_family",
            dimensions.oracle_family.as_str(),
            oracle,
        ),
        (
            "selection_dimensions.propagation_witness",
            dimensions.propagation_witness.as_str(),
            propagation,
        ),
        (
            "selection_dimensions.target_kind",
            dimensions.target_kind.as_str(),
            target,
        ),
    ] {
        require_equal(violations, &format!("{subject}.{field}"), actual, required);
    }
    let expected_test_relation = if item.expected_direction == "should_limit" {
        "macro_wrapped_test_call_candidate"
    } else {
        "direct_owner_call"
    };
    require_equal(
        violations,
        &format!("{subject}.test_evidence.relation_basis"),
        &evidence.relation_basis,
        expected_test_relation,
    );
    validate_nullable_text(
        &evidence.missing,
        missing,
        &format!("{subject}.test_evidence.missing"),
        violations,
    );
    validate_nullable_text(
        &evidence.aligned_observer,
        observer,
        &format!("{subject}.test_evidence.aligned_observer"),
        violations,
    );
    if item.expected_direction == "should_limit" && !evidence.observed_inputs.is_empty() {
        violations.push(format!(
            "{subject}.test_evidence.observed_inputs: unresolved limit requires an empty list"
        ));
    } else if item.expected_direction != "should_limit" && evidence.observed_inputs.is_empty() {
        violations.push(format!(
            "{subject}.test_evidence.observed_inputs: direct owner evidence requires at least one input"
        ));
    }
}

fn validate_nullable_text(
    value: &Nullable<String>,
    required: bool,
    field: &str,
    violations: &mut Vec<String>,
) {
    match (required, value) {
        (true, Nullable::Value(text)) if !text.trim().is_empty() => {}
        (true, _) => violations.push(format!("{field}: requires a non-empty value")),
        (false, Nullable::Null) => {}
        (false, _) => violations.push(format!("{field}: requires explicit null")),
    }
}

fn validate_direction_contract(
    item: &RustJudgedPanelItem,
    subject: &str,
    violations: &mut Vec<String>,
) {
    let expected = match item.expected_direction.as_str() {
        "should_gap" => Some(("weakly_exposed", "repair_candidate", false)),
        "should_stay_quiet" => Some(("exposed", "no_action", false)),
        "should_limit" => Some(("no_static_path", "inspect_static_limitation", true)),
        _ => None,
    };
    let Some((class, actionability, limit_required)) = expected else {
        return;
    };
    if item.expected_classification != class {
        violations.push(format!(
            "{subject}.expected_classification: `{}` requires `{class}`, found `{}`",
            item.expected_direction, item.expected_classification
        ));
    }
    if item.expected_actionability != actionability {
        violations.push(format!(
            "{subject}.expected_actionability: `{}` requires `{actionability}`, found `{}`",
            item.expected_direction, item.expected_actionability
        ));
    }
    let has_limit = item
        .expected_static_limit_kind
        .value()
        .is_some_and(|value| !value.trim().is_empty());
    if limit_required != has_limit {
        let requirement = if limit_required {
            "a named static limit"
        } else {
            "a null static limit"
        };
        violations.push(format!(
            "{subject}.expected_static_limit_kind: `{}` requires {requirement}",
            item.expected_direction
        ));
    }
}

fn validate_seed_judgment(item: &RustJudgedPanelItem, subject: &str, violations: &mut Vec<String>) {
    if matches!(item.labels.false_actionable, Nullable::Value(true))
        && matches!(item.labels.false_exposed, Nullable::Value(true))
    {
        violations.push(format!(
            "{subject}.labels: false_actionable and false_exposed cannot both be true"
        ));
    }
    if !item.labels.all_explicitly_null() {
        violations.push(format!(
            "{subject}.labels: seed labels must remain null; null means unjudged, not false/pass"
        ));
    }
    if item.disposition != "unjudged" {
        violations.push(format!(
            "{subject}.disposition: seed requires `unjudged`, found `{}`",
            item.disposition
        ));
    }
    if !item.judgment_source.is_null() || !item.judged_at.is_null() || !item.judged_by.is_empty() {
        violations.push(format!(
            "{subject}.judgment_identity: seed requires null source/time and no reviewers"
        ));
    }
}

fn validate_runtime(item: &RustJudgedPanelItem, subject: &str, violations: &mut Vec<String>) {
    if item.runtime_calibration.status != "not_run" {
        violations.push(format!(
            "{subject}.runtime_calibration.status: seed requires `not_run`, found `{}`",
            item.runtime_calibration.status
        ));
    }
    if !item.runtime_calibration.outcome.is_null()
        || !item.runtime_calibration.evidence_ref.is_null()
    {
        violations.push(format!(
            "{subject}.runtime_calibration: `not_run` requires null outcome and evidence_ref"
        ));
    }
}

fn validate_anchor(
    root: &Path,
    item: &RustJudgedPanelItem,
    subject: &str,
    violations: &mut Vec<String>,
) {
    require_non_empty(
        violations,
        &format!("{subject}.anchor.file"),
        &item.anchor.file,
    );
    require_non_empty(
        violations,
        &format!("{subject}.anchor.owner"),
        &item.anchor.owner,
    );
    require_non_empty(
        violations,
        &format!("{subject}.anchor.changed_behavior"),
        &item.anchor.changed_behavior,
    );
    require_non_empty(
        violations,
        &format!("{subject}.anchor.required_discriminator"),
        &item.anchor.required_discriminator,
    );
    if item.anchor.line == 0 {
        violations.push(format!("{subject}.anchor.line: must be positive"));
    }
    let anchor_file = Path::new(&item.anchor.file);
    if normalize_path(anchor_file) != item.anchor.file || !is_confined_relative_path(anchor_file) {
        violations.push(format!(
            "{subject}.anchor.file: `{}` must be a normalized repository-relative path",
            item.anchor.file
        ));
        return;
    }

    let diff_path = Path::new(&item.diff_path);
    if normalize_path(diff_path) != item.diff_path || !is_confined_diff_path(diff_path) {
        violations.push(format!(
            "{subject}.diff_path: `{}` must be a relative file under `{DIFF_ROOT}` without parent traversal",
            item.diff_path
        ));
        return;
    }
    let canonical_root = match fs::canonicalize(root) {
        Ok(path) => path,
        Err(error) => {
            violations.push(format!(
                "{subject}.diff_path: failed to resolve repository root: {error}"
            ));
            return;
        }
    };
    let full_path = root.join(diff_path);
    if !full_path.is_file() {
        violations.push(format!(
            "{subject}.diff_path: `{}` is missing or is not a file",
            item.diff_path
        ));
        return;
    }
    let confined_root = match fs::canonicalize(root.join(DIFF_ROOT)) {
        Ok(path) => path,
        Err(error) => {
            violations.push(format!(
                "{subject}.diff_path: failed to resolve governed diff root: {error}"
            ));
            return;
        }
    };
    if !confined_root.starts_with(&canonical_root) {
        violations.push(format!(
            "{subject}.diff_path: governed diff root resolves outside the repository root"
        ));
        return;
    }
    let resolved = match fs::canonicalize(&full_path) {
        Ok(path) => path,
        Err(error) => {
            violations.push(format!(
                "{subject}.diff_path: failed to resolve `{}`: {error}",
                item.diff_path
            ));
            return;
        }
    };
    if !resolved.starts_with(&confined_root) {
        violations.push(format!(
            "{subject}.diff_path: `{}` resolves outside `{DIFF_ROOT}`",
            item.diff_path
        ));
        return;
    }
    let body = match fs::read_to_string(&full_path) {
        Ok(body) => body,
        Err(error) => {
            violations.push(format!(
                "{subject}.diff_path: failed to read `{}`: {error}",
                item.diff_path
            ));
            return;
        }
    };
    match added_line_at(&body, &item.anchor.file, item.anchor.line) {
        Ok(Some(line)) if contains_rust_token_sequence(line, &item.anchor.changed_behavior) => {}
        Ok(Some(line)) => violations.push(format!(
            "{subject}.anchor.changed_behavior: added line {} in `{}` is `{}`, which does not contain the Rust token sequence `{}`",
            item.anchor.line, item.anchor.file, line.trim(), item.anchor.changed_behavior
        )),
        Ok(None) => violations.push(format!(
            "{subject}.anchor: `{}` line {} is not an added-file line in `{}`",
            item.anchor.file, item.anchor.line, item.diff_path
        )),
        Err(error) => violations.push(format!("{subject}.diff_path: {error}")),
    }
}

fn is_confined_diff_path(path: &Path) -> bool {
    if !is_confined_relative_path(path) {
        return false;
    }
    path.starts_with(Path::new(DIFF_ROOT))
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

fn added_line_at<'a>(
    diff: &'a str,
    anchor_file: &str,
    anchor_line: u64,
) -> Result<Option<&'a str>, String> {
    let expected_target = format!("b/{anchor_file}");
    let mut target_matches = false;
    let mut target_headers = 0_usize;
    let mut matched_line = None;
    let mut hunk = None;
    let mut source_header_seen = false;
    let mut section_bound = false;
    for line in diff.lines() {
        if line.starts_with("diff --cc ")
            || line.starts_with("diff --combined ")
            || line.starts_with("GIT binary patch")
            || line.starts_with("Binary files ")
        {
            return Err("combined or binary diffs are unsupported".to_string());
        }
        if line.starts_with("diff --git ") {
            source_header_seen = false;
            section_bound = false;
            target_matches = false;
            finish_hunk(&mut hunk)?;
            continue;
        }
        if line.starts_with("--- ") {
            source_header_seen = true;
            section_bound = false;
            target_matches = false;
            finish_hunk(&mut hunk)?;
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
            target_matches = target.trim() == expected_target;
            if target_matches {
                target_headers += 1;
            }
            source_header_seen = false;
            section_bound = true;
            finish_hunk(&mut hunk)?;
            continue;
        }
        if line.starts_with("@@ ") {
            if !section_bound {
                return Err("hunk is not bound to a `---`/`+++` file section".to_string());
            }
            finish_hunk(&mut hunk)?;
            hunk = Some(parse_hunk_header(line)?);
            continue;
        }
        let Some(state) = hunk.as_mut() else {
            continue;
        };
        if line.starts_with('\\') {
            continue;
        }
        if let Some(added) = line.strip_prefix('+') {
            if state.new_remaining == 0 {
                return Err("hunk contains more new lines than declared".to_string());
            }
            if target_matches
                && state.new_line == anchor_line
                && matched_line.replace(added).is_some()
            {
                return Err(format!(
                    "anchor target `{anchor_file}` line {anchor_line} is ambiguous across hunks"
                ));
            }
            state.consume_new()?;
        } else if line.starts_with('-') {
            state.consume_old()?;
        } else if line.starts_with(' ') {
            state.consume_old()?;
            state.consume_new()?;
        } else {
            return Err(format!("unsupported line inside unified hunk `{line}`"));
        }
    }
    finish_hunk(&mut hunk)?;
    if target_headers > 1 {
        return Err(format!(
            "target `b/{anchor_file}` occurs in {target_headers} file sections"
        ));
    }
    Ok(matched_line)
}

fn contains_rust_token_sequence(line: &str, expected: &str) -> bool {
    let line_tokens = rust_code_tokens(line);
    let expected_tokens = rust_code_tokens(expected);
    !expected_tokens.is_empty()
        && line_tokens
            .windows(expected_tokens.len())
            .any(|window| window == expected_tokens)
}

fn rust_code_tokens(source: &str) -> Vec<String> {
    let wrapped = format!("fn __ripr_anchor() {{ {source} }}");
    let parsed = SourceFile::parse(&wrapped, Edition::CURRENT);
    let tokens = parsed
        .syntax_node()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| {
            !matches!(
                token.kind(),
                SyntaxKind::WHITESPACE
                    | SyntaxKind::COMMENT
                    | SyntaxKind::STRING
                    | SyntaxKind::BYTE_STRING
                    | SyntaxKind::C_STRING
                    | SyntaxKind::CHAR
                    | SyntaxKind::BYTE
            )
        })
        .map(|token| token.text().to_string())
        .collect::<Vec<_>>();
    tokens
        .get(5..tokens.len().saturating_sub(1))
        .unwrap_or_default()
        .to_vec()
}

#[derive(Debug)]
struct HunkState {
    old_remaining: u64,
    new_remaining: u64,
    new_line: u64,
}

impl HunkState {
    fn consume_old(&mut self) -> Result<(), String> {
        self.old_remaining = self
            .old_remaining
            .checked_sub(1)
            .ok_or_else(|| "hunk contains more old lines than declared".to_string())?;
        Ok(())
    }

    fn consume_new(&mut self) -> Result<(), String> {
        self.new_remaining = self
            .new_remaining
            .checked_sub(1)
            .ok_or_else(|| "hunk contains more new lines than declared".to_string())?;
        self.new_line = self
            .new_line
            .checked_add(1)
            .ok_or_else(|| "hunk new-line counter overflowed".to_string())?;
        Ok(())
    }
}

fn finish_hunk(hunk: &mut Option<HunkState>) -> Result<(), String> {
    if let Some(state) = hunk.take()
        && (state.old_remaining != 0 || state.new_remaining != 0)
    {
        return Err(format!(
            "hunk ended before declared extents were consumed (old remaining {}, new remaining {})",
            state.old_remaining, state.new_remaining
        ));
    }
    Ok(())
}

fn parse_hunk_header(line: &str) -> Result<HunkState, String> {
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
    let (_, old_count) = parse_hunk_range(old, '-')?;
    let (new_line, new_count) = parse_hunk_range(new, '+')?;
    Ok(HunkState {
        old_remaining: old_count,
        new_remaining: new_count,
        new_line,
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

fn require_equal(violations: &mut Vec<String>, field: &str, actual: &str, expected: &str) {
    if actual != expected {
        violations.push(format!("{field}: expected `{expected}`, found `{actual}`"));
    }
}

fn require_non_empty(violations: &mut Vec<String>, field: &str, value: &str) {
    if value.trim().is_empty() {
        violations.push(format!("{field}: must not be blank"));
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::{Value, json};

    use super::{MANIFEST_PATH, load_and_validate_at};

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
                "ripr-rust-judged-panel-{name}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("metrics/rust-judged-behavior-panel/diffs"))
                .map_err(|error| format!("create test fixture: {error}"))?;
            Ok(Self { root })
        }

        fn write_diff(
            &self,
            name: &str,
            target: &str,
            added_behavior: &str,
        ) -> Result<String, String> {
            let relative = format!("metrics/rust-judged-behavior-panel/diffs/{name}.diff");
            let body = format!(
                "--- a/{target}\n+++ b/{target}\n@@ -5,3 +5,3 @@\n context\n-old_behavior()\n+{added_behavior}\n context\n"
            );
            fs::write(self.root.join(&relative), body)
                .map_err(|error| format!("write test diff: {error}"))?;
            Ok(relative)
        }

        fn write_diff_with_later_hunk(
            &self,
            name: &str,
            target: &str,
            added_behavior: &str,
        ) -> Result<String, String> {
            let relative = format!("metrics/rust-judged-behavior-panel/diffs/{name}.diff");
            let body = format!(
                "--- a/{target}\n+++ b/{target}\n@@ -1,1 +1,1 @@\n-before()\n+after()\n@@ -40,1 +40,3 @@\n context\n+{added_behavior}\n+second_added_line()\n"
            );
            fs::write(self.root.join(&relative), body)
                .map_err(|error| format!("write test diff: {error}"))?;
            Ok(relative)
        }

        fn write_manifest(&self, value: &Value) -> Result<(), String> {
            let path = self.root.join(MANIFEST_PATH);
            let parent = path
                .parent()
                .ok_or_else(|| "test manifest path has no parent".to_string())?;
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            let body = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
            fs::write(path, body).map_err(|error| error.to_string())
        }
    }

    impl Drop for TempFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    fn symlink_file(source: &Path, target: &Path) -> Result<bool, String> {
        std::os::unix::fs::symlink(source, target)
            .map(|_| true)
            .map_err(|error| error.to_string())
    }

    #[cfg(windows)]
    fn symlink_file(source: &Path, target: &Path) -> Result<bool, String> {
        match std::os::windows::fs::symlink_file(source, target) {
            Ok(()) => Ok(true),
            Err(error) if symlink_unsupported(&error) => Ok(false),
            Err(error) => Err(error.to_string()),
        }
    }

    #[cfg(unix)]
    fn symlink_dir(source: &Path, target: &Path) -> Result<bool, String> {
        std::os::unix::fs::symlink(source, target)
            .map(|_| true)
            .map_err(|error| error.to_string())
    }

    #[cfg(windows)]
    fn symlink_dir(source: &Path, target: &Path) -> Result<bool, String> {
        match std::os::windows::fs::symlink_dir(source, target) {
            Ok(()) => Ok(true),
            Err(error) if symlink_unsupported(&error) => Ok(false),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Windows maps `ERROR_PRIVILEGE_NOT_HELD` (1314) to an `ErrorKind` that
    /// is not `PermissionDenied` on this toolchain, so both the kind and the
    /// raw code must read as "symlinks unsupported" for the
    /// `*_when_supported` tests to skip instead of fail (#3289).
    #[cfg(windows)]
    fn symlink_unsupported(error: &std::io::Error) -> bool {
        error.kind() == std::io::ErrorKind::PermissionDenied
            || error.raw_os_error() == Some(WINDOWS_ERROR_PRIVILEGE_NOT_HELD)
    }

    /// `ERROR_PRIVILEGE_NOT_HELD`.
    #[cfg(windows)]
    const WINDOWS_ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

    fn valid_item(
        id: &str,
        direction: &str,
        diff_path: String,
        target: &str,
        changed_behavior: &str,
    ) -> Value {
        let (class, limit, actionability) = match direction {
            "should_gap" => ("weakly_exposed", Value::Null, "repair_candidate"),
            "should_stay_quiet" => ("exposed", Value::Null, "no_action"),
            _ => (
                "no_static_path",
                Value::String("macro_path_unresolved".to_string()),
                "inspect_static_limitation",
            ),
        };
        let (
            test_relation,
            observed_inputs,
            observer,
            missing,
            selection_relation,
            oracle_family,
            propagation,
            target_kind,
        ) = match direction {
            "should_gap" => (
                "direct_owner_call",
                json!(["alternate_owner(1)"]),
                json!("alternate exact observer"),
                json!("alternate missing discriminator"),
                "direct_owner_call",
                "exact_value",
                "direct_return",
                "existing_test",
            ),
            "should_stay_quiet" => (
                "direct_owner_call",
                json!(["alternate_owner(2)"]),
                json!("alternate exact observer"),
                Value::Null,
                "direct_owner_call",
                "exact_value",
                "direct_return",
                "existing_test",
            ),
            _ => (
                "macro_wrapped_test_call_candidate",
                json!([]),
                Value::Null,
                json!("alternate unresolved owner-observer path"),
                "macro_witness",
                "exact_value_candidate",
                "unresolved",
                "none",
            ),
        };
        json!({
            "id": id,
            "repository": "alternate-seed-repository",
            "base": null,
            "head": null,
            "tree_identity": null,
            "diff_path": diff_path,
            "expected_direction": direction,
            "behavior_family": "alternate_family",
            "anchor": {
                "file": target,
                "line": 6,
                "owner": "alternate_owner",
                "changed_behavior": changed_behavior,
                "required_discriminator": "alternate discriminator"
            },
            "test_evidence": {
                "relation_basis": test_relation,
                "oracle_kind": "exact_value",
                "oracle_strength": "strong",
                "observed_inputs": observed_inputs,
                "aligned_observer": observer,
                "missing": missing
            },
            "expected_classification": class,
            "expected_static_limit_kind": limit,
            "expected_actionability": actionability,
            "selection_dimensions": {
                "relation_basis": selection_relation,
                "oracle_family": oracle_family,
                "propagation_witness": propagation,
                "target_kind": target_kind
            },
            "labels": {
                "structural_judgment": null,
                "false_actionable": null,
                "false_exposed": null,
                "static_under_credit": null,
                "wrong_target": null,
                "limitation_correct": null
            },
            "runtime_calibration": {
                "status": "not_run",
                "outcome": null,
                "evidence_ref": null
            },
            "judgment_source": null,
            "judged_at": null,
            "judged_by": [],
            "disposition": "unjudged",
            "must_not_claim": ["alternate fixture is not a result"],
            "reason": "alternate load-bearing selection reason"
        })
    }

    fn valid_alternate_manifest(fixture: &TempFixture) -> Result<Value, String> {
        let limit_path =
            fixture.write_diff("alternate-limit", "src/limit.rs", "limit_behavior()")?;
        let gap_path = fixture.write_diff("alternate-gap", "src/gap.rs", "gap_behavior()")?;
        let quiet_path =
            fixture.write_diff("alternate-quiet", "src/quiet.rs", "quiet_behavior()")?;
        let second_gap_path = fixture.write_diff_with_later_hunk(
            "alternate-second-gap",
            "src/second_gap.rs",
            "second_gap_behavior()",
        )?;
        let mut second_gap = valid_item(
            "alternate-second-gap-id",
            "should_gap",
            second_gap_path,
            "src/second_gap.rs",
            "second_gap_behavior()",
        );
        second_gap["anchor"]["line"] = json!(41);
        Ok(json!({
            "schema_version": "0.1",
            "kind": "rust_judged_behavior_panel_manifest",
            "authority": "EffortlessMetrics/ripr-swarm#3164",
            "tier": "seed",
            "description": "Alternate valid seed proving contract validation.",
            "selection_status": "complete",
            "limits": ["alternate seed remains unjudged"],
            "required_directions": ["should_limit", "should_gap", "should_stay_quiet"],
            "items": [
                valid_item("alternate-limit-id", "should_limit", limit_path, "src/limit.rs", "limit_behavior()"),
                valid_item("alternate-gap-id", "should_gap", gap_path, "src/gap.rs", "gap_behavior()"),
                valid_item("alternate-quiet-id", "should_stay_quiet", quiet_path, "src/quiet.rs", "quiet_behavior()"),
                second_gap
            ]
        }))
    }

    fn expect_rejection(
        fixture: &TempFixture,
        value: &Value,
        expected: &[&str],
    ) -> Result<(), String> {
        fixture.write_manifest(value)?;
        let error = load_and_validate_at(&fixture.root, Path::new(MANIFEST_PATH))
            .err()
            .ok_or_else(|| "malformed judged-panel fixture was accepted".to_string())?;
        for fragment in expected {
            if !error.contains(fragment) {
                return Err(format!(
                    "expected rejection to contain `{fragment}`; actual: {error}"
                ));
            }
        }
        if !error.contains("rerun: cargo xtask rust-judged-panel check") {
            return Err(format!("rejection omitted direct rerun command: {error}"));
        }
        Ok(())
    }

    #[test]
    fn canonical_seed_manifest_is_semantically_valid() -> Result<(), String> {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest has no repository parent".to_string())?;
        let manifest = load_and_validate_at(repository_root, Path::new(MANIFEST_PATH))?;
        if manifest.items.len() != 3 {
            return Err(format!(
                "canonical selected denominator must contain 3 items, found {}",
                manifest.items.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn alternate_valid_seed_proves_validator_is_not_hard_coded() -> Result<(), String> {
        let fixture = TempFixture::new("alternate-valid")?;
        let manifest = valid_alternate_manifest(&fixture)?;
        fixture.write_manifest(&manifest)?;
        load_and_validate_at(&fixture.root, Path::new(MANIFEST_PATH)).map(|_| ())
    }

    #[test]
    fn denominator_rejects_duplicate_id_and_missing_direction() -> Result<(), String> {
        let fixture = TempFixture::new("denominator")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["required_directions"] = json!(["should_gap", "should_gap", "should_limit"]);
        manifest["items"][1]["id"] = manifest["items"][0]["id"].clone();
        expect_rejection(
            &fixture,
            &manifest,
            &[
                "duplicate selected item identity",
                "missing `should_stay_quiet`",
                "occurs 2 times",
            ],
        )
    }

    #[test]
    fn diff_path_rejects_parent_escape_and_missing_file() -> Result<(), String> {
        let fixture = TempFixture::new("paths")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]["diff_path"] =
            json!("metrics/rust-judged-behavior-panel/diffs/../escape.diff");
        manifest["items"][1]["diff_path"] =
            json!("metrics/rust-judged-behavior-panel/diffs/missing.diff");
        expect_rejection(
            &fixture,
            &manifest,
            &["without parent traversal", "missing or is not a file"],
        )
    }

    #[test]
    fn diff_path_rejects_cross_platform_separator_and_duplicate_target() -> Result<(), String> {
        let fixture = TempFixture::new("diff-ambiguity")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]["diff_path"] =
            json!("metrics\\rust-judged-behavior-panel\\diffs\\alternate-limit.diff");
        let duplicate_relative = "metrics/rust-judged-behavior-panel/diffs/duplicate-target.diff";
        fs::write(
            fixture.root.join(duplicate_relative),
            "--- a/src/gap.rs\n+++ b/src/gap.rs\n@@ -5 +5 @@\n-old()\n+gap_behavior()\n--- a/src/gap.rs\n+++ b/src/gap.rs\n@@ -9 +9 @@\n-old()\n+other()\n",
        )
        .map_err(|error| error.to_string())?;
        manifest["items"][1]["diff_path"] = json!(duplicate_relative);
        expect_rejection(
            &fixture,
            &manifest,
            &["without parent traversal", "occurs in 2 file sections"],
        )
    }

    #[test]
    fn diff_path_rejects_curdir_doubled_separator_and_prefix_lookalike() -> Result<(), String> {
        let fixture = TempFixture::new("path-spelling")?;
        for value in [
            "metrics/rust-judged-behavior-panel/diffs/./alternate-limit.diff",
            "metrics/rust-judged-behavior-panel/diffs//alternate-limit.diff",
            "metrics/rust-judged-behavior-panel/diffs-lookalike/alternate-limit.diff",
        ] {
            let mut manifest = valid_alternate_manifest(&fixture)?;
            manifest["items"][0]["diff_path"] = json!(value);
            expect_rejection(&fixture, &manifest, &["must be a relative file under"])?;
        }
        Ok(())
    }

    #[test]
    fn diff_path_rejects_absolute_drive_unc_and_backslash_forms() -> Result<(), String> {
        let fixture = TempFixture::new("path-platform-forms")?;
        let drive_absolute = format!("{}:/outside.diff", 'C');
        for value in [
            "/absolute.diff",
            &drive_absolute,
            "//server/share/outside.diff",
            "metrics\\rust-judged-behavior-panel\\diffs\\outside.diff",
        ] {
            let mut manifest = valid_alternate_manifest(&fixture)?;
            manifest["items"][0]["diff_path"] = json!(value);
            expect_rejection(&fixture, &manifest, &["must be a relative file under"])?;
        }
        Ok(())
    }

    #[test]
    fn diff_path_rejects_interior_symlink_escape_when_supported() -> Result<(), String> {
        let fixture = TempFixture::new("interior-symlink")?;
        let outside = TempFixture::new("interior-symlink-outside")?;
        let external = outside.root.join("external.diff");
        fs::write(
            &external,
            "--- a/src/limit.rs\n+++ b/src/limit.rs\n@@ -5,1 +5,1 @@\n-old()\n+limit_behavior()\n",
        )
        .map_err(|error| error.to_string())?;
        let relative = "metrics/rust-judged-behavior-panel/diffs/symlink.diff";
        if !symlink_file(&external, &fixture.root.join(relative))? {
            return Ok(());
        }
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]["diff_path"] = json!(relative);
        expect_rejection(&fixture, &manifest, &["resolves outside"])
    }

    #[test]
    fn diff_path_rejects_governed_root_symlink_escape_when_supported() -> Result<(), String> {
        let fixture = TempFixture::new("root-symlink")?;
        let outside = TempFixture::new("root-symlink-outside")?;
        let governed = fixture
            .root
            .join("metrics/rust-judged-behavior-panel/diffs");
        fs::remove_dir_all(&governed).map_err(|error| error.to_string())?;
        if !symlink_dir(&outside.root, &governed)? {
            return Ok(());
        }
        let relative = "metrics/rust-judged-behavior-panel/diffs/escape.diff";
        fs::write(
            outside.root.join("escape.diff"),
            "--- a/src/limit.rs\n+++ b/src/limit.rs\n@@ -5,1 +5,1 @@\n-old()\n+limit_behavior()\n",
        )
        .map_err(|error| error.to_string())?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]["diff_path"] = json!(relative);
        expect_rejection(
            &fixture,
            &manifest,
            &["governed diff root resolves outside the repository root"],
        )
    }

    #[test]
    fn diff_parser_rejects_combined_binary_and_quoted_forms() -> Result<(), String> {
        let fixture = TempFixture::new("unsupported-diffs")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        for (index, (name, body)) in [
            (
                "combined.diff",
                "diff --cc src/limit.rs\n+++ b/src/limit.rs\n@@@ -1,-1 +1 @@@\n+limit_behavior()\n",
            ),
            (
                "binary.diff",
                "GIT binary patch\n+++ b/src/gap.rs\n@@ -5 +5 @@\n+gap_behavior()\n",
            ),
            (
                "quoted.diff",
                "--- a/src/quiet.rs\n+++ \"b/src/quiet.rs\"\n@@ -5 +5 @@\n+quiet_behavior()\n",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let relative = format!("metrics/rust-judged-behavior-panel/diffs/{name}");
            fs::write(fixture.root.join(&relative), body).map_err(|error| error.to_string())?;
            manifest["items"][index]["diff_path"] = json!(relative);
        }
        expect_rejection(
            &fixture,
            &manifest,
            &[
                "combined or binary diffs are unsupported",
                "quoted or metadata-bearing",
            ],
        )
    }

    #[test]
    fn diff_parser_rejects_false_hunk_extents_truncation_and_rename() -> Result<(), String> {
        let fixture = TempFixture::new("hunk-extents")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        for (index, (name, body)) in [
            (
                "overflow.diff",
                "--- a/src/limit.rs\n+++ b/src/limit.rs\n@@ -1,1 +1,1 @@\n context\n-old()\n+limit_behavior()\n context\n",
            ),
            (
                "truncated.diff",
                "--- a/src/gap.rs\n+++ b/src/gap.rs\n@@ -5,3 +5,3 @@\n context\n-old()\n+gap_behavior()\n",
            ),
            (
                "rename.diff",
                "similarity index 90%\nrename from old.rs\nrename to src/quiet.rs\n--- a/old.rs\n+++ b/src/quiet.rs\n@@ -5 +5 @@\n-old()\n+quiet_behavior()\n",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let relative = format!("metrics/rust-judged-behavior-panel/diffs/{name}");
            fs::write(fixture.root.join(&relative), body).map_err(|error| error.to_string())?;
            manifest["items"][index]["diff_path"] = json!(relative);
        }
        expect_rejection(
            &fixture,
            &manifest,
            &[
                "more old lines than declared",
                "hunk ended before declared extents",
                "rename and copy diffs are unsupported",
            ],
        )
    }

    #[test]
    fn diff_parser_rejects_copy_metadata_independently() -> Result<(), String> {
        let fixture = TempFixture::new("copy-diff")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        let relative = "metrics/rust-judged-behavior-panel/diffs/copy.diff";
        fs::write(
            fixture.root.join(relative),
            "similarity index 90%\ncopy from old.rs\ncopy to src/limit.rs\n--- a/old.rs\n+++ b/src/limit.rs\n@@ -5 +5 @@\n-old()\n+limit_behavior()\n",
        )
        .map_err(|error| error.to_string())?;
        manifest["items"][0]["diff_path"] = json!(relative);
        expect_rejection(
            &fixture,
            &manifest,
            &["rename and copy diffs are unsupported"],
        )
    }

    #[test]
    fn canonical_body_rejects_mutated_one_line_hunk_declaration() -> Result<(), String> {
        let canonical = include_str!(
            "../../metrics/rust-judged-behavior-panel/diffs/boundary_missing_equality.diff"
        );
        let mutated = canonical.replace("@@ -1,7 +1,7 @@", "@@ -1,1 +1,1 @@");
        let error = super::added_line_at(&mutated, "src/lib.rs", 2)
            .err()
            .ok_or_else(|| "canonical false hunk extent was accepted".to_string())?;
        if !error.contains("more old lines than declared")
            && !error.contains("more new lines than declared")
        {
            return Err(format!("unexpected false-extent error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn evidence_contract_rejects_empty_typo_missing_and_direction_drift() -> Result<(), String> {
        let fixture = TempFixture::new("evidence-contract")?;
        for (name, mutate, fragment) in [
            ("empty", 0_usize, "missing field `relation_basis`"),
            ("typo", 1_usize, "unknown field `oracle_family_typo`"),
            ("missing", 2_usize, "missing field `oracle_strength`"),
        ] {
            let mut manifest = valid_alternate_manifest(&fixture)?;
            match mutate {
                0 => manifest["items"][0]["test_evidence"] = json!({}),
                1 => {
                    manifest["items"][1]["selection_dimensions"] = json!({
                        "relation_basis": "direct_owner_call",
                        "oracle_family_typo": "exact_value",
                        "propagation_witness": "direct_return",
                        "target_kind": "existing_test"
                    });
                }
                _ => {
                    manifest["items"][2]["test_evidence"]
                        .as_object_mut()
                        .ok_or_else(|| "test evidence fixture must be an object".to_string())?
                        .remove("oracle_strength");
                }
            }
            fixture.write_manifest(&manifest)?;
            let error = load_and_validate_at(&fixture.root, Path::new(MANIFEST_PATH))
                .err()
                .ok_or_else(|| format!("invalid typed evidence `{name}` was accepted"))?;
            if !error.contains(fragment) {
                return Err(format!(
                    "typed evidence rejection omitted `{fragment}`: {error}"
                ));
            }
        }

        let mut coherent = valid_alternate_manifest(&fixture)?;
        coherent["items"][1]["test_evidence"]["missing"] = Value::Null;
        coherent["items"][2]["test_evidence"]["aligned_observer"] = Value::Null;
        coherent["items"][0]["selection_dimensions"]["target_kind"] = json!("existing_test");
        expect_rejection(
            &fixture,
            &coherent,
            &[
                "test_evidence.missing: requires a non-empty value",
                "test_evidence.aligned_observer: requires a non-empty value",
                "selection_dimensions.target_kind: expected `none`",
            ],
        )
    }

    #[test]
    fn anchor_rejects_wrong_file_line_and_nearby_behavior() -> Result<(), String> {
        let fixture = TempFixture::new("anchor")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]["anchor"]["file"] = json!("src/wrong.rs");
        manifest["items"][1]["anchor"]["line"] = json!(5);
        manifest["items"][2]["anchor"]["changed_behavior"] = json!("quiet_behavior_similar()");
        expect_rejection(
            &fixture,
            &manifest,
            &[
                "is not an added-file line",
                "does not contain the Rust token sequence",
            ],
        )
    }

    #[test]
    fn token_anchor_rejects_comments_strings_and_identifier_prefixes() -> Result<(), String> {
        let fixture = TempFixture::new("token-anchor")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        let diff_path = fixture.write_diff(
            "token-tricks",
            "src/token_tricks.rs",
            "quiet_behavior_similar(); let note = \"quiet_behavior()\"; // quiet_behavior()",
        )?;
        manifest["items"][2]["diff_path"] = json!(diff_path);
        manifest["items"][2]["anchor"]["file"] = json!("src/token_tricks.rs");
        expect_rejection(
            &fixture,
            &manifest,
            &["does not contain the Rust token sequence `quiet_behavior()`"],
        )
    }

    #[test]
    fn seed_rejects_missing_field_distinct_from_explicit_null() -> Result<(), String> {
        let fixture = TempFixture::new("missing-null")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]
            .as_object_mut()
            .ok_or_else(|| "fixture item must be an object".to_string())?
            .remove("base");
        expect_rejection(&fixture, &manifest, &["seed phase requires null base"])
    }

    #[test]
    fn parser_rejects_duplicate_and_unknown_keys() -> Result<(), String> {
        let fixture = TempFixture::new("duplicate-key")?;
        let path = fixture.root.join(MANIFEST_PATH);
        fs::write(&path, r#"{"schema_version":"0.1","schema_version":"0.1"}"#)
            .map_err(|error| error.to_string())?;
        let duplicate = load_and_validate_at(&fixture.root, Path::new(MANIFEST_PATH))
            .err()
            .ok_or_else(|| "duplicate key was accepted".to_string())?;
        if !duplicate.contains("duplicate object key `schema_version`") {
            return Err(format!("unexpected duplicate-key error: {duplicate}"));
        }

        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["typo_field"] = json!(true);
        fixture.write_manifest(&manifest)?;
        let unknown = load_and_validate_at(&fixture.root, Path::new(MANIFEST_PATH))
            .err()
            .ok_or_else(|| "unknown key was accepted".to_string())?;
        if !unknown.contains("unknown field `typo_field`") {
            return Err(format!("unexpected unknown-key error: {unknown}"));
        }
        Ok(())
    }

    #[test]
    fn direction_contract_rejects_repairable_limit_and_actionable_quiet() -> Result<(), String> {
        let fixture = TempFixture::new("direction")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]["expected_static_limit_kind"] = Value::Null;
        manifest["items"][0]["expected_actionability"] = json!("repair_candidate");
        manifest["items"][2]["expected_actionability"] = json!("repair_candidate");
        expect_rejection(
            &fixture,
            &manifest,
            &[
                "requires a named static limit",
                "requires `inspect_static_limitation`",
                "requires `no_action`",
            ],
        )
    }

    #[test]
    fn seed_rejects_non_null_judgment_and_double_error() -> Result<(), String> {
        let fixture = TempFixture::new("judgment")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]["labels"]["false_actionable"] = json!(true);
        manifest["items"][0]["labels"]["false_exposed"] = json!(true);
        manifest["items"][0]["disposition"] = json!("judged");
        manifest["items"][0]["judgment_source"] = json!("copied_static_output");
        expect_rejection(
            &fixture,
            &manifest,
            &[
                "cannot both be true",
                "null means unjudged",
                "seed requires `unjudged`",
                "seed requires null source/time",
            ],
        )
    }

    #[test]
    fn seed_rejects_replay_identity_and_inconsistent_runtime() -> Result<(), String> {
        let fixture = TempFixture::new("runtime")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["items"][0]["base"] = json!("0123456789012345678901234567890123456789");
        manifest["items"][0]["runtime_calibration"]["outcome"] = json!("caught");
        manifest["items"][0]["runtime_calibration"]["evidence_ref"] = json!("receipt.json");
        expect_rejection(
            &fixture,
            &manifest,
            &[
                "seed phase requires null base",
                "`not_run` requires null outcome",
            ],
        )
    }

    #[test]
    fn reports_all_independent_violations_deterministically() -> Result<(), String> {
        let fixture = TempFixture::new("deterministic")?;
        let mut manifest = valid_alternate_manifest(&fixture)?;
        manifest["schema_version"] = json!("9.9");
        manifest["items"][0]["reason"] = json!("");
        manifest["items"][0]["must_not_claim"] = json!([]);
        fixture.write_manifest(&manifest)?;
        let first = load_and_validate_at(&fixture.root, Path::new(MANIFEST_PATH))
            .err()
            .ok_or_else(|| "invalid fixture passed first validation".to_string())?;
        let second = load_and_validate_at(&fixture.root, Path::new(MANIFEST_PATH))
            .err()
            .ok_or_else(|| "invalid fixture passed second validation".to_string())?;
        if first != second {
            return Err("semantic violations are not deterministic".to_string());
        }
        for fragment in ["manifest.schema_version", ".reason", ".must_not_claim"] {
            if !first.contains(fragment) {
                return Err(format!(
                    "all-violations output omitted `{fragment}`: {first}"
                ));
            }
        }
        Ok(())
    }
}
