//! Negative controls for release-candidate artifact lifecycle (#3842).
//!
//! Fixtures start from the retained repository artifacts and the checked-in
//! registry, then apply one targeted edit, so each reported rule is
//! attributable to that edit. The numbered controls follow the issue.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use super::{
    ArtifactTree, CANDIDATE_RULE_IDS, CandidateOperation, LifecycleState, PROJECTION_PATH,
    REGISTRY_PATH, RULE_CURRENTNESS, RULE_DIGEST, RULE_PROJECTION, RULE_REGISTRATION,
    RULE_REGISTRY_INPUT, RULE_STATE_IDENTITY, RULE_SUCCESSION, RegistryDocument, ReleaseController,
    evaluate_candidate_registry, read_artifact_tree, registry_json, render_projection,
    resolve_candidate_authority, sha256_hex,
};

const HARD_CUT_JSON: &str = "docs/release-candidates/0.11.0-hard-cut.json";
const HARD_CUT_MD: &str = "docs/release-candidates/0.11.0-hard-cut.md";
const LIVE_HEAD_JSON: &str = "docs/release-candidates/0.11.0-live-head-selection.json";
const LIVE_HEAD_MD: &str = "docs/release-candidates/0.11.0-live-head-selection.md";
const FREEZE_JSON: &str = "docs/release-candidates/0.11.0-replacement-freeze.json";
const PINNED_JSON: &str = "docs/release-candidates/0.11.0-pinned-candidate.json";

const PINNED_SHA: &str = "1111111111111111111111111111111111111111";
const PINNED_TREE: &str = "2222222222222222222222222222222222222222";

fn retained() -> Vec<(&'static str, &'static [u8])> {
    vec![
        (
            HARD_CUT_JSON,
            include_bytes!("../../../../../docs/release-candidates/0.11.0-hard-cut.json"),
        ),
        (
            HARD_CUT_MD,
            include_bytes!("../../../../../docs/release-candidates/0.11.0-hard-cut.md"),
        ),
        (
            LIVE_HEAD_JSON,
            include_bytes!(
                "../../../../../docs/release-candidates/0.11.0-live-head-selection.json"
            ),
        ),
        (
            LIVE_HEAD_MD,
            include_bytes!("../../../../../docs/release-candidates/0.11.0-live-head-selection.md"),
        ),
        (
            FREEZE_JSON,
            include_bytes!("../../../../../docs/release-candidates/0.11.0-replacement-freeze.json"),
        ),
    ]
}

fn bytes_of(path: &str) -> Vec<u8> {
    retained()
        .into_iter()
        .find(|(name, _)| *name == path)
        .map(|(_, bytes)| bytes.to_vec())
        .unwrap_or_default()
}

fn repository_registry() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/release-candidates/index.json"
    ))
    .unwrap_or(Value::Null)
}

fn controllers() -> Vec<ReleaseController> {
    vec![ReleaseController {
        version: "0.11.0".to_string(),
        goal_issue: Some(2379),
    }]
}

/// Build a tree from the retained artifacts, `registry`, and extra files. The
/// README projection is re-derived from the fixture's own rows so a fixture
/// isolates its targeted rule; the projection control edits it explicitly.
fn tree(registry: &Value, extra: &[(&str, Vec<u8>)]) -> ArtifactTree {
    let mut tree = ArtifactTree::default();
    for (path, bytes) in retained() {
        tree.files.insert(path.to_string(), bytes.to_vec());
    }
    for (path, bytes) in extra {
        tree.files.insert((*path).to_string(), bytes.clone());
    }
    let text = serde_json::to_string_pretty(registry).unwrap_or_default();
    if let Ok(document) = serde_json::from_str::<RegistryDocument>(&text) {
        tree.files.insert(
            PROJECTION_PATH.to_string(),
            render_projection(&document.artifacts).into_bytes(),
        );
    }
    tree.files
        .insert(REGISTRY_PATH.to_string(), text.into_bytes());
    tree
}

fn row_mut<'a>(registry: &'a mut Value, path: &str) -> &'a mut Value {
    let index = registry["artifacts"]
        .as_array()
        .and_then(|rows| rows.iter().position(|row| row["path"] == path));
    assert!(index.is_some(), "fixture registry has no row for {path}");
    &mut registry["artifacts"][index.unwrap_or_default()]
}

fn push_row(registry: &mut Value, row: Value) {
    if let Some(rows) = registry["artifacts"].as_array_mut() {
        rows.push(row);
    }
}

fn pinned_artifact() -> Vec<u8> {
    serde_json::to_vec_pretty(&json!({
        "schema_version": "1.0",
        "kind": "ripr_swarm_live_head_release_authority",
        "release_line": "0.11.0",
        "authority_issue": 2379,
        "status": "pinned_exact_head",
        "selected_swarm_parent": PINNED_SHA,
        "template_sha256": sha256_hex(&bytes_of(LIVE_HEAD_JSON)),
    }))
    .unwrap_or_default()
}

/// Control 12's later state: #1609 registers an exact candidate and the
/// template retires. Only the template's own rows and the new row change.
fn retire_template(mut registry: Value) -> Value {
    let template_sha = sha256_hex(&bytes_of(LIVE_HEAD_JSON));
    {
        let template = row_mut(&mut registry, LIVE_HEAD_JSON);
        template["state"] = json!("historical_evidence_only");
        template["superseded_by"] = json!(PINNED_JSON);
        template["reason"] = json!("retired when #1609 pinned the exact candidate");
        template["successor_route"] = Value::Null;
    }
    row_mut(&mut registry, LIVE_HEAD_MD)["state"] = json!("historical_evidence_only");
    push_row(
        &mut registry,
        json!({
            "release": "0.11.0",
            "path": PINNED_JSON,
            "sha256": sha256_hex(&pinned_artifact()),
            "schema_generation": "ripr_swarm_live_head_release_authority/1.0",
            "authority_issue": 2379,
            "state": "pinned_exact_candidate",
            "supersedes": [LIVE_HEAD_JSON],
            "candidate": {
                "sha": PINNED_SHA,
                "tree": PINNED_TREE,
                "ref": format!("refs/tags/ripr-release-0.11.0-{PINNED_SHA}"),
            },
            "packets": {
                "selected_claim_packet_sha256": "3".repeat(64),
                "denominator_packet_sha256": "4".repeat(64),
            },
            "selection_template_sha256": template_sha,
        }),
    );
    registry
}

fn pinned_tree(registry: &Value) -> ArtifactTree {
    tree(registry, &[(PINNED_JSON, pinned_artifact())])
}

fn fired_rules(violations: &[String]) -> BTreeSet<String> {
    violations
        .iter()
        .filter_map(|violation| {
            violation
                .split_once(" :: ")
                .map(|(rule, _)| rule.to_string())
        })
        .collect()
}

/// Assert the fixture trips `rule` and only `rule`, returning the messages.
fn only_rule(tree: &ArtifactTree, rule: &str) -> Vec<String> {
    let outcome = evaluate_candidate_registry(tree, &controllers());
    assert_eq!(
        fired_rules(&outcome.violations),
        BTreeSet::from([rule.to_string()]),
        "violations: {:#?}",
        outcome.violations
    );
    assert_eq!(outcome.status(), "not_proven");
    let validated = outcome.validated();
    assert!(validated.is_err(), "{validated:?}");
    outcome.violations
}

fn clean(tree: &ArtifactTree) -> super::ValidatedRegistry {
    let outcome = evaluate_candidate_registry(tree, &controllers());
    assert_eq!(outcome.violations, Vec::<String>::new());
    assert_eq!(outcome.status(), "established");
    let validated = outcome.validated();
    assert!(validated.is_ok(), "{validated:?}");
    match validated {
        Ok(registry) => registry,
        Err(_) => super::ValidatedRegistry { rows: Vec::new() },
    }
}

#[test]
fn the_fixture_baseline_is_clean_and_parses_every_retained_artifact() {
    let registry = repository_registry();
    assert_eq!(
        registry["artifacts"].as_array().map(Vec::len),
        Some(5),
        "fixture must register all five retained artifacts"
    );
    let outcome = evaluate_candidate_registry(&tree(&registry, &[]), &controllers());
    assert_eq!(outcome.violations, Vec::<String>::new());
    assert_eq!(outcome.files.len(), 5);
    assert_eq!(outcome.releases.len(), 1);
    assert_eq!(
        outcome.releases[0].selection_rule.as_deref(),
        Some(LIVE_HEAD_JSON)
    );
    assert_eq!(outcome.releases[0].exact_candidate, None);
}

// Control 1.
#[test]
fn the_hard_cut_receipt_alone_is_never_current_authority() {
    let registry = clean(&tree(&repository_registry(), &[]));
    let bytes = bytes_of(HARD_CUT_JSON);
    for operation in [
        CandidateOperation::SelectionRule,
        CandidateOperation::ExactCandidate,
    ] {
        let resolved = resolve_candidate_authority(&registry, "0.11.0", &bytes, operation);
        assert!(
            resolved
                .as_ref()
                .is_err_and(|err| err.contains("historical_evidence_only")),
            "{operation:?}: {resolved:?}"
        );
    }
    let cited =
        resolve_candidate_authority(&registry, "0.11.0", &bytes, CandidateOperation::CiteHistory);
    assert_eq!(
        cited.map(|grant| (grant.registered_path, grant.state)),
        Ok((
            HARD_CUT_JSON.to_string(),
            LifecycleState::HistoricalEvidenceOnly
        ))
    );

    // Supplied without any registry, the receipt grants nothing.
    let mut alone = ArtifactTree::default();
    alone.files.insert(HARD_CUT_JSON.to_string(), bytes);
    let found = only_rule(&alone, RULE_REGISTRY_INPUT);
    assert!(found[0].contains("is missing"), "{found:#?}");
    let outcome = evaluate_candidate_registry(&alone, &controllers());
    assert_eq!(outcome.files.len(), 1);
    assert_eq!(outcome.files[0].state, None);
    assert!(outcome.files[0].permitted_operations.is_empty());
}

// Control 2.
#[test]
fn a_renamed_hard_cut_copy_still_classifies_as_historical_by_digest() {
    let copy = "docs/release-candidates/0.11.0-current-authority.json";
    let registry_value = repository_registry();
    let found = only_rule(
        &tree(&registry_value, &[(copy, bytes_of(HARD_CUT_JSON))]),
        RULE_REGISTRATION,
    );
    assert!(
        found[0].contains(copy) && found[0].contains("is not registered"),
        "{found:#?}"
    );

    let registry = clean(&tree(&registry_value, &[]));
    let grant = resolve_candidate_authority(
        &registry,
        "0.11.0",
        &bytes_of(HARD_CUT_JSON),
        CandidateOperation::CiteHistory,
    );
    assert_eq!(
        grant.map(|grant| (grant.registered_path, grant.state)),
        Ok((
            HARD_CUT_JSON.to_string(),
            LifecycleState::HistoricalEvidenceOnly
        )),
        "classification follows the digest, not the supplied path"
    );

    // Registering the copy under a second row is ambiguous, not a new authority.
    let mut duplicated = registry_value;
    let mut row = row_mut(&mut duplicated, HARD_CUT_JSON).clone();
    row["path"] = json!(copy);
    row["superseded_by"] = Value::Null;
    row["successor_route"] = json!("none");
    push_row(&mut duplicated, row);
    let found = only_rule(
        &tree(&duplicated, &[(copy, bytes_of(HARD_CUT_JSON))]),
        RULE_REGISTRATION,
    );
    assert!(
        found.iter().any(|v| v.contains("shares its sha256")),
        "{found:#?}"
    );
}

// Control 3.
#[test]
fn changed_bytes_under_a_registered_row_fail_the_digest_rule() {
    let mut changed = bytes_of(HARD_CUT_JSON);
    changed.extend_from_slice(b" ");
    let found = only_rule(
        &tree(&repository_registry(), &[(HARD_CUT_JSON, changed.clone())]),
        RULE_DIGEST,
    );
    assert!(
        found[0].contains(HARD_CUT_JSON) && found[0].contains("raw bytes hash to"),
        "{found:#?}"
    );

    let registry = clean(&tree(&repository_registry(), &[]));
    let resolved = resolve_candidate_authority(
        &registry,
        "0.11.0",
        &changed,
        CandidateOperation::CiteHistory,
    );
    assert!(
        resolved
            .as_ref()
            .is_err_and(|err| err.contains("match no registered")),
        "{resolved:?}"
    );
}

// Control 4.
#[test]
fn two_active_selection_templates_for_one_release_are_ambiguous() {
    let second = "docs/release-candidates/0.11.0-second-template.json";
    let bytes = serde_json::to_vec(&json!({
        "schema_version": "1.0",
        "kind": "ripr_swarm_live_head_release_authority",
        "status": "active_selection_template",
    }))
    .unwrap_or_default();
    let mut registry = repository_registry();
    push_row(
        &mut registry,
        json!({
            "release": "0.11.0",
            "path": second,
            "sha256": sha256_hex(&bytes),
            "schema_generation": "ripr_swarm_live_head_release_authority/1.0",
            "authority_issue": 2379,
            "state": "active_selection_template",
        }),
    );
    let found = only_rule(&tree(&registry, &[(second, bytes)]), RULE_CURRENTNESS);
    assert!(found[0].contains("has 2 current rows"), "{found:#?}");
}

// Control 5.
#[test]
fn a_template_cannot_satisfy_an_exact_candidate_prerequisite() {
    let registry = clean(&tree(&repository_registry(), &[]));
    let template = bytes_of(LIVE_HEAD_JSON);
    let exact = resolve_candidate_authority(
        &registry,
        "0.11.0",
        &template,
        CandidateOperation::ExactCandidate,
    );
    assert!(
        exact
            .as_ref()
            .is_err_and(|err| err.contains("active_selection_template")),
        "{exact:?}"
    );
    let selection = resolve_candidate_authority(
        &registry,
        "0.11.0",
        &template,
        CandidateOperation::SelectionRule,
    );
    assert_eq!(
        selection.map(|grant| grant.state),
        Ok(LifecycleState::ActiveSelectionTemplate)
    );

    // Re-registering the template bytes as pinned does not make them pinned.
    let mut value = repository_registry();
    {
        let row = row_mut(&mut value, LIVE_HEAD_JSON);
        row["state"] = json!("pinned_exact_candidate");
        row["candidate"] = json!({
            "sha": PINNED_SHA,
            "tree": PINNED_TREE,
            "ref": "refs/tags/ripr-release-0.11.0-x",
        });
        row["packets"] = json!({
            "selected_claim_packet_sha256": "3".repeat(64),
            "denominator_packet_sha256": "4".repeat(64),
        });
        row["selection_template_sha256"] = json!(sha256_hex(&bytes_of(HARD_CUT_JSON)));
        row["successor_route"] = Value::Null;
    }
    row_mut(&mut value, LIVE_HEAD_MD)["state"] = json!("pinned_exact_candidate");
    let found = only_rule(&tree(&value, &[]), RULE_STATE_IDENTITY);
    assert!(
        found
            .iter()
            .any(|v| v.contains("the artifact is still an active_selection_template")),
        "{found:#?}"
    );
}

/// Label, targeted edit to the pinned row, and expected message fragment.
type PinnedCase = (&'static str, fn(&mut Value), &'static str);

// Control 6.
#[test]
fn pinned_rows_require_exact_sha_tree_ref_and_packet_digests() {
    let cases: [PinnedCase; 7] = [
        (
            "sha",
            |row| row["candidate"]["sha"] = Value::Null,
            "candidate SHA",
        ),
        (
            "tree",
            |row| row["candidate"]["tree"] = Value::Null,
            "candidate tree",
        ),
        (
            "ref",
            |row| row["candidate"]["ref"] = json!("ripr-release-0.11.0"),
            "candidate ref",
        ),
        (
            "bare refs/ prefix",
            |row| row["candidate"]["ref"] = json!("refs/"),
            "candidate ref",
        ),
        (
            "selected-claim packet",
            |row| row["packets"]["selected_claim_packet_sha256"] = Value::Null,
            "selected-claim packet",
        ),
        (
            "denominator packet",
            |row| row["packets"] = Value::Null,
            "denominator packet",
        ),
        (
            "template",
            |row| row["selection_template_sha256"] = Value::Null,
            "selection template",
        ),
    ];
    for (label, edit, expected) in cases {
        let mut registry = retire_template(repository_registry());
        edit(row_mut(&mut registry, PINNED_JSON));
        let found = only_rule(&pinned_tree(&registry), RULE_STATE_IDENTITY);
        assert!(
            found.iter().any(|v| v.contains(expected)),
            "{label}: {found:#?}"
        );
    }
}

// Control 7.
#[test]
fn historical_rows_require_a_terminal_reason_and_a_successor() {
    let mut registry = repository_registry();
    row_mut(&mut registry, HARD_CUT_JSON)["reason"] = Value::Null;
    let found = only_rule(&tree(&registry, &[]), RULE_SUCCESSION);
    assert!(
        found[0].contains("without a terminal or invalidation reason"),
        "{found:#?}"
    );

    let mut registry = repository_registry();
    row_mut(&mut registry, FREEZE_JSON)["superseded_by"] = Value::Null;
    let found = only_rule(&tree(&registry, &[]), RULE_SUCCESSION);
    assert!(
        found
            .iter()
            .any(|v| v.contains("without superseded_by or a successor_route")),
        "{found:#?}"
    );

    let mut registry = repository_registry();
    row_mut(&mut registry, HARD_CUT_JSON)["state"] = json!("invalid");
    row_mut(&mut registry, HARD_CUT_MD)["state"] = json!("invalid");
    row_mut(&mut registry, HARD_CUT_JSON)["reason"] = json!("   ");
    let found = only_rule(&tree(&registry, &[]), RULE_SUCCESSION);
    assert!(
        found[0].contains("is invalid without a terminal or invalidation reason"),
        "{found:#?}"
    );
}

// Control 8.
#[test]
fn a_current_row_cannot_name_a_superseded_controller_or_stale_selection() {
    let mut registry = repository_registry();
    row_mut(&mut registry, LIVE_HEAD_JSON)["authority_issue"] = json!(2893);
    let found = only_rule(&tree(&registry, &[]), RULE_CURRENTNESS);
    assert!(
        found[0].contains("under authority #2893, but release 0.11.0 is controlled by #2379"),
        "{found:#?}"
    );

    let mut registry = retire_template(repository_registry());
    row_mut(&mut registry, PINNED_JSON)["selection_template_sha256"] =
        json!(sha256_hex(b"an older template copy"));
    let found = only_rule(&pinned_tree(&registry), RULE_CURRENTNESS);
    assert!(
        found[0].contains("the selection packet is stale or foreign"),
        "{found:#?}"
    );

    let mut registry = repository_registry();
    row_mut(&mut registry, LIVE_HEAD_JSON)["superseded_by"] = json!(FREEZE_JSON);
    let outcome = evaluate_candidate_registry(&tree(&registry, &[]), &controllers());
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| v.starts_with(RULE_CURRENTNESS)
                && v.contains("a superseded row cannot stay current")),
        "{:#?}",
        outcome.violations
    );
}

// Control 9.
#[test]
fn json_and_markdown_projection_disagreement_rejects() {
    let registry = repository_registry();
    let mut stale = tree(&registry, &[]);
    if let Some(readme) = stale.files.get_mut(PROJECTION_PATH) {
        *readme = String::from_utf8_lossy(readme)
            .replace("`historical_evidence_only`", "`active_selection_template`")
            .into_bytes();
    }
    let found = only_rule(&stale, RULE_PROJECTION);
    assert!(
        found[0].contains("disagrees with the projection"),
        "{found:#?}"
    );

    let mut missing = tree(&registry, &[]);
    missing.files.remove(PROJECTION_PATH);
    only_rule(&missing, RULE_PROJECTION);

    let mut stronger = repository_registry();
    row_mut(&mut stronger, HARD_CUT_MD)["state"] = json!("active_selection_template");
    let found = only_rule(&tree(&stronger, &[]), RULE_PROJECTION);
    assert!(
        found[0].contains("a projection cannot disagree with its authority"),
        "{found:#?}"
    );

    let validated = clean(&tree(&registry, &[]));
    let resolved = resolve_candidate_authority(
        &validated,
        "0.11.0",
        &bytes_of(LIVE_HEAD_MD),
        CandidateOperation::SelectionRule,
    );
    assert!(
        resolved
            .as_ref()
            .is_err_and(|err| err.contains("Markdown projection")),
        "{resolved:?}"
    );
}

// Control 10.
#[test]
fn normalized_output_is_byte_stable_across_roots_and_input_ordering() -> Result<(), String> {
    let registry = repository_registry();
    let mut reversed = registry.clone();
    if let Some(rows) = reversed["artifacts"].as_array_mut() {
        rows.reverse();
    }
    let first = evaluate_candidate_registry(&tree(&registry, &[]), &controllers());
    let second = evaluate_candidate_registry(&tree(&reversed, &[]), &controllers());
    assert_eq!(first.violations, Vec::<String>::new());
    assert_eq!(second.violations, Vec::<String>::new());
    assert_eq!(first.projection, second.projection);
    assert_eq!(
        registry_json(&first).to_string(),
        registry_json(&second).to_string()
    );

    let base = crate::tests::temp_dir("candidate-registry-roots");
    let fixture = tree(&registry, &[]);
    let mut outputs = Vec::new();
    for root in [base.join("a"), base.join("b").join("nested")] {
        for (path, bytes) in &fixture.files {
            let target = root.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            std::fs::write(&target, bytes).map_err(|err| err.to_string())?;
        }
        let read = read_artifact_tree(&root);
        assert_eq!(read, fixture);
        let outcome = evaluate_candidate_registry(&read, &controllers());
        outputs.push((registry_json(&outcome).to_string(), outcome.projection));
    }
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0], outputs[1]);
    Ok(())
}

/// The disk reader must descend into subdirectories: an unregistered file one
/// level down is as unregistered as one at the top.
#[test]
fn an_unregistered_file_in_a_subdirectory_is_read_and_rejected() -> Result<(), String> {
    let nested = "docs/release-candidates/archive/0.11.0-hard-cut.json";
    let fixture = tree(&repository_registry(), &[(nested, bytes_of(HARD_CUT_JSON))]);
    let root = crate::tests::temp_dir("candidate-registry-nested");
    for (path, bytes) in &fixture.files {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        std::fs::write(&target, bytes).map_err(|err| err.to_string())?;
    }
    let read = read_artifact_tree(&root);
    let _ = std::fs::remove_dir_all(&root);
    assert!(read.files.contains_key(nested), "{:?}", read.files.keys());
    assert_eq!(read, fixture);
    let found = only_rule(&read, RULE_REGISTRATION);
    assert!(
        found
            .iter()
            .any(|v| v.contains(nested) && v.contains("is not registered")),
        "{found:#?}"
    );
    Ok(())
}

// Control 11.
#[test]
fn missing_partial_or_unreadable_registry_input_is_not_proven() {
    let registry = repository_registry();

    let mut missing = tree(&registry, &[]);
    missing.files.remove(REGISTRY_PATH);
    only_rule(&missing, RULE_REGISTRY_INPUT);

    let mut truncated = tree(&registry, &[]);
    if let Some(bytes) = truncated.files.get_mut(REGISTRY_PATH) {
        bytes.truncate(bytes.len() / 2);
    }
    only_rule(&truncated, RULE_REGISTRY_INPUT);

    let mut unreadable = tree(&registry, &[]);
    unreadable.files.remove(REGISTRY_PATH);
    unreadable
        .unreadable
        .insert(REGISTRY_PATH.to_string(), "permission denied".to_string());
    only_rule(&unreadable, RULE_REGISTRY_INPUT);

    let mut empty = registry.clone();
    empty["artifacts"] = json!([]);
    only_rule(&tree(&empty, &[]), RULE_REGISTRY_INPUT);

    let mut unknown_state = registry;
    row_mut(&mut unknown_state, HARD_CUT_JSON)["state"] = json!("current");
    let outcome = evaluate_candidate_registry(&tree(&unknown_state, &[]), &controllers());
    assert_eq!(
        fired_rules(&outcome.violations),
        BTreeSet::from([RULE_REGISTRY_INPUT.to_string()])
    );
    assert_eq!(outcome.status(), "not_proven");
    let section = registry_json(&outcome);
    assert_eq!(section["status"], "not_proven");
    assert!(
        section["files"]
            .as_array()
            .is_some_and(|files| files.iter().all(|file| file["state"].is_null())),
        "{section}"
    );
}

// Control 12.
#[test]
fn retiring_the_template_to_a_pinned_candidate_keeps_historical_rows() {
    let before = repository_registry();
    let after = retire_template(before.clone());
    let registry = clean(&pinned_tree(&after));

    for path in [HARD_CUT_JSON, HARD_CUT_MD, FREEZE_JSON] {
        let mut left = before.clone();
        let mut right = after.clone();
        assert_eq!(
            row_mut(&mut left, path),
            row_mut(&mut right, path),
            "{path} row was rewritten"
        );
    }

    let template = resolve_candidate_authority(
        &registry,
        "0.11.0",
        &bytes_of(LIVE_HEAD_JSON),
        CandidateOperation::SelectionRule,
    );
    assert!(template.is_err(), "{template:?}");
    let pinned = resolve_candidate_authority(
        &registry,
        "0.11.0",
        &pinned_artifact(),
        CandidateOperation::ExactCandidate,
    );
    assert_eq!(
        pinned.map(|grant| (grant.state, grant.candidate_sha)),
        Ok((
            LifecycleState::PinnedExactCandidate,
            Some(PINNED_SHA.to_string())
        ))
    );
    let hard_cut = resolve_candidate_authority(
        &registry,
        "0.11.0",
        &bytes_of(HARD_CUT_JSON),
        CandidateOperation::ExactCandidate,
    );
    assert!(hard_cut.is_err(), "{hard_cut:?}");
}

/// Re-promoting a receipt through the registry alone must fail: a row
/// registered `active_selection_template` needs an artifact that declares
/// that status itself.
#[test]
fn a_template_row_requires_the_artifact_to_declare_template_status() {
    let original: Value = serde_json::from_slice(&bytes_of(LIVE_HEAD_JSON)).unwrap_or(Value::Null);
    assert_eq!(original["status"], "active_selection_template");
    let frozen = serde_json::from_slice::<Value>(&bytes_of(FREEZE_JSON))
        .ok()
        .and_then(|value| value["status"].as_str().map(str::to_string));
    assert_eq!(
        frozen.as_deref(),
        Some("selected_ref_created_qualification_pending")
    );
    for status in [frozen.map(Value::String), None] {
        let mut artifact = original.clone();
        match &status {
            Some(value) => artifact["status"] = value.clone(),
            None => {
                if let Some(object) = artifact.as_object_mut() {
                    object.remove("status");
                }
            }
        }
        let bytes = serde_json::to_vec_pretty(&artifact).unwrap_or_default();
        let mut registry = repository_registry();
        row_mut(&mut registry, LIVE_HEAD_JSON)["sha256"] = json!(sha256_hex(&bytes));
        let found = only_rule(
            &tree(&registry, &[(LIVE_HEAD_JSON, bytes)]),
            RULE_STATE_IDENTITY,
        );
        assert!(
            found.iter().any(|v| v
                .contains("registered active_selection_template but the artifact declares status")),
            "{status:?}: {found:#?}"
        );
    }
}

#[test]
fn a_template_row_may_not_carry_a_selection_template_digest() {
    let mut registry = repository_registry();
    row_mut(&mut registry, LIVE_HEAD_JSON)["selection_template_sha256"] = json!("5".repeat(64));
    let found = only_rule(&tree(&registry, &[]), RULE_STATE_IDENTITY);
    assert!(
        found
            .iter()
            .any(|v| v.contains(LIVE_HEAD_JSON) && v.contains("carries pinned candidate identity")),
        "{found:#?}"
    );
}

/// A JSON row that claims to be a projection would skip its own lifecycle
/// identity checks, so the claim itself is rejected.
#[test]
fn a_json_row_may_not_declare_itself_a_projection() {
    let mut registry = repository_registry();
    row_mut(&mut registry, FREEZE_JSON)["projection_of"] = json!(HARD_CUT_JSON);
    let outcome = evaluate_candidate_registry(&tree(&registry, &[]), &controllers());
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| v.starts_with(RULE_PROJECTION)
                && v.contains(FREEZE_JSON)
                && v.contains("only Markdown rows are projections")),
        "{:#?}",
        outcome.violations
    );
    assert_eq!(outcome.status(), "not_proven");
}

#[test]
fn registered_schema_generation_must_match_the_artifact() {
    let mut json_row = repository_registry();
    row_mut(&mut json_row, HARD_CUT_JSON)["schema_generation"] = json!("ripr_other_kind/9.9");
    let found = only_rule(&tree(&json_row, &[]), RULE_STATE_IDENTITY);
    assert!(
        found
            .iter()
            .any(|v| v.contains(HARD_CUT_JSON) && v.contains("registers schema_generation")),
        "{found:#?}"
    );

    let mut markdown_row = repository_registry();
    row_mut(&mut markdown_row, HARD_CUT_MD)["schema_generation"] = json!("ripr_other_kind/9.9");
    let found = only_rule(&tree(&markdown_row, &[]), RULE_STATE_IDENTITY);
    assert!(
        found
            .iter()
            .any(|v| v.contains(HARD_CUT_MD) && v.contains("Markdown rows must declare")),
        "{found:#?}"
    );
}

#[test]
fn a_pinned_row_must_name_the_artifact_selected_parent() {
    let mut artifact: Value = serde_json::from_slice(&pinned_artifact()).unwrap_or(Value::Null);
    artifact["selected_swarm_parent"] = json!("9".repeat(40));
    let bytes = serde_json::to_vec_pretty(&artifact).unwrap_or_default();
    let mut registry = retire_template(repository_registry());
    row_mut(&mut registry, PINNED_JSON)["sha256"] = json!(sha256_hex(&bytes));
    let found = only_rule(
        &tree(&registry, &[(PINNED_JSON, bytes)]),
        RULE_STATE_IDENTITY,
    );
    assert!(
        found
            .iter()
            .any(|v| v.contains("the artifact's selected_swarm_parent is")),
        "{found:#?}"
    );
}

#[test]
fn an_invalid_row_may_not_even_be_cited() {
    let mut registry = repository_registry();
    row_mut(&mut registry, FREEZE_JSON)["state"] = json!("invalid");
    let validated = clean(&tree(&registry, &[]));
    let cited = resolve_candidate_authority(
        &validated,
        "0.11.0",
        &bytes_of(FREEZE_JSON),
        CandidateOperation::CiteHistory,
    );
    assert!(cited.is_err(), "{cited:?}");
    let other_release = resolve_candidate_authority(
        &validated,
        "0.12.0",
        &bytes_of(LIVE_HEAD_JSON),
        CandidateOperation::SelectionRule,
    );
    assert!(other_release.is_err(), "{other_release:?}");
}

#[test]
fn a_registered_row_for_an_undeclared_release_is_rejected() {
    let mut registry = repository_registry();
    row_mut(&mut registry, FREEZE_JSON)["release"] = json!("0.10.9");
    let outcome = evaluate_candidate_registry(&tree(&registry, &[]), &controllers());
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| v.starts_with(RULE_REGISTRATION)
                && v.contains("which policy/release-targets.toml does not declare")),
        "{:#?}",
        outcome.violations
    );
}

#[test]
fn every_candidate_rule_owns_a_negative_fixture() {
    let mut changed = bytes_of(HARD_CUT_JSON);
    changed.push(b'\n');
    let mut missing = tree(&repository_registry(), &[]);
    missing.files.remove(REGISTRY_PATH);
    let mut state_identity = retire_template(repository_registry());
    row_mut(&mut state_identity, PINNED_JSON)["candidate"] = Value::Null;
    let mut currentness = repository_registry();
    row_mut(&mut currentness, LIVE_HEAD_JSON)["authority_issue"] = json!(1);
    let mut succession = repository_registry();
    row_mut(&mut succession, HARD_CUT_JSON)["reason"] = Value::Null;
    let mut projection = repository_registry();
    row_mut(&mut projection, HARD_CUT_MD)["state"] = json!("invalid");

    let fixtures = [
        (RULE_REGISTRY_INPUT, missing),
        (
            RULE_REGISTRATION,
            tree(
                &repository_registry(),
                &[("docs/release-candidates/extra.json", b"{}".to_vec())],
            ),
        ),
        (
            RULE_DIGEST,
            tree(&repository_registry(), &[(HARD_CUT_JSON, changed)]),
        ),
        (RULE_STATE_IDENTITY, pinned_tree(&state_identity)),
        (RULE_CURRENTNESS, tree(&currentness, &[])),
        (RULE_SUCCESSION, tree(&succession, &[])),
        (RULE_PROJECTION, tree(&projection, &[])),
    ];
    let mut covered = BTreeSet::new();
    for (rule, fixture) in &fixtures {
        only_rule(fixture, rule);
        covered.insert((*rule).to_string());
    }
    let expected = CANDIDATE_RULE_IDS
        .iter()
        .map(|rule| (*rule).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(covered, expected);
}

#[test]
fn the_repository_registry_classifies_every_retained_artifact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent();
    assert!(root.is_some(), "xtask has a parent workspace root");
    let Some(root) = root else { return };
    let manifest = std::fs::read_to_string(root.join(super::super::RELEASE_TARGETS_MANIFEST_PATH));
    assert!(manifest.is_ok(), "{manifest:?}");
    let manifest = manifest.unwrap_or_default();
    let targets = super::super::evaluate_release_targets(
        super::super::RELEASE_TARGETS_MANIFEST_PATH,
        &manifest,
    );
    let controllers = targets
        .releases
        .iter()
        .map(|release| ReleaseController {
            version: release.version.clone(),
            goal_issue: release.goal_issue,
        })
        .collect::<Vec<_>>();

    let tree = read_artifact_tree(root);
    let outcome = evaluate_candidate_registry(&tree, &controllers);
    assert_eq!(outcome.violations, Vec::<String>::new());
    assert_eq!(outcome.status(), "established");

    let states = outcome
        .files
        .iter()
        .map(|file| {
            (
                file.path.as_str(),
                file.state,
                file.permitted_operations.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        states,
        vec![
            (
                HARD_CUT_JSON,
                Some(LifecycleState::HistoricalEvidenceOnly),
                vec!["cite_history"]
            ),
            (
                HARD_CUT_MD,
                Some(LifecycleState::HistoricalEvidenceOnly),
                vec!["cite_history"]
            ),
            (
                LIVE_HEAD_JSON,
                Some(LifecycleState::ActiveSelectionTemplate),
                vec!["cite_history", "selection_rule"]
            ),
            (
                LIVE_HEAD_MD,
                Some(LifecycleState::ActiveSelectionTemplate),
                vec!["cite_history"]
            ),
            (
                FREEZE_JSON,
                Some(LifecycleState::HistoricalEvidenceOnly),
                vec!["cite_history"]
            ),
        ]
    );
    assert_eq!(outcome.releases.len(), 1);
    assert_eq!(
        outcome.releases[0].selection_rule.as_deref(),
        Some(LIVE_HEAD_JSON)
    );
    assert_eq!(outcome.releases[0].exact_candidate, None);
}
