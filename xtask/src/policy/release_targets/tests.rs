//! Discriminating proof for `check-release-targets` (#3013 Slice B).
//!
//! Every offline rule owns at least one fixture that violates exactly that
//! rule, and each fixture asserts the checker reports that rule and no other.
//! `every_rule_owns_a_negative_fixture` fails if a rule is ever added without
//! a fixture that can trip it, so a rule that cannot fail cannot ship.

use std::collections::BTreeSet;
use std::path::Path;

use super::{
    RELEASE_TARGETS_MANIFEST_PATH, RULE_COMMITTED_DISJOINTNESS, RULE_IDS,
    RULE_NON_COMMITTED_EXCLUSION, RULE_PARENT_ACCOUNTING, RULE_PREREQUISITE_ORDERING,
    RULE_REFERENTIAL_CLOSURE, RULE_RELEASE_IDENTITY, RULE_ROLE_UNIQUENESS, RULE_SCHEMA,
    evaluate_release_targets, release_targets_json,
};

const FIXTURE_PATH: &str = "fixture/release-targets.toml";

/// A structurally complete manifest that satisfies every rule. Each negative
/// fixture is this document with one targeted edit, so a reported violation is
/// attributable to that edit and nothing else.
fn valid_manifest() -> String {
    "\
schema_version = \"0.1\"
non_claim = \"no publication claim\"
control_issue = 9000

[[release]]
version = \"0.12.0\"
milestone = \"0.12.0 candidate\"
goal_issue = 100
claim_blockers = [101]
proof_blockers = [102]
companions = [103]
conditional_issues = [104]

[[release]]
version = \"0.13.0\"
milestone = \"0.13.0 candidate\"
goal_issue = 200
claim_blockers = [201]
proof_blockers = []
companions = []
conditional_issues = [204]

[[parent]]
issue = 300
leaves = [101, 102]
counted_in = \"none\"
justification = \"umbrella outside milestone progress\"

[[prerequisite]]
issue = 201
requires = 101
justification = \"consumes the earlier candidate leaf\"

[[rolling]]
issue = 400
justification = \"rolling control work\"
"
    .to_string()
}

fn violations(text: &str) -> Vec<String> {
    evaluate_release_targets(FIXTURE_PATH, text).violations
}

/// Assert the fixture trips `rule` and only `rule`. Returns the messages so a
/// test can also pin the exact wording it proved.
fn only_rule(text: &str, rule: &str) -> Vec<String> {
    let found = violations(text);
    assert!(
        !found.is_empty(),
        "fixture reported no violation; it does not discriminate rule `{rule}`"
    );
    let fired = fired_rules(&found);
    assert_eq!(
        fired,
        BTreeSet::from([rule.to_string()]),
        "fixture tripped {fired:?} instead of only `{rule}`; violations: {found:#?}"
    );
    found
}

fn fired_rules(found: &[String]) -> BTreeSet<String> {
    let mut rules = BTreeSet::new();
    for violation in found {
        let named = violation.split_once(" :: ");
        assert!(
            named.is_some(),
            "violation `{violation}` names no rule, so no fixture can be attributed to a rule"
        );
        if let Some((rule, _)) = named {
            rules.insert(rule.to_string());
        }
    }
    rules
}

/// Every negative fixture, paired with the rule it must trip. Each named test
/// below pins the exact message for one of these; `every_rule_owns_a_negative_
/// fixture` reads the same table, so a rule added without a fixture that can
/// trip it fails the suite.
fn negative_fixtures() -> Vec<(&'static str, String)> {
    vec![
        (RULE_SCHEMA, missing_top_level_key()),
        (RULE_SCHEMA, quoted_goal_issue()),
        (RULE_SCHEMA, quoted_issue_in_role_array()),
        (RULE_SCHEMA, unknown_top_level_key()),
        (RULE_RELEASE_IDENTITY, mismatched_milestone_name()),
        (RULE_RELEASE_IDENTITY, descending_release_order()),
        (RULE_ROLE_UNIQUENESS, one_issue_in_two_roles()),
        (RULE_COMMITTED_DISJOINTNESS, one_issue_in_two_releases()),
        (RULE_NON_COMMITTED_EXCLUSION, committed_conditional_issue()),
        (RULE_NON_COMMITTED_EXCLUSION, committed_rolling_issue()),
        (
            RULE_PREREQUISITE_ORDERING,
            prerequisite_in_a_later_release(),
        ),
        (RULE_PREREQUISITE_ORDERING, self_prerequisite()),
        (RULE_PARENT_ACCOUNTING, parent_counted_where_it_is_absent()),
        (RULE_PARENT_ACCOUNTING, parent_without_leaves()),
        (RULE_PARENT_ACCOUNTING, parent_as_its_own_leaf()),
        (RULE_PARENT_ACCOUNTING, committed_parent_claiming_none()),
        (RULE_PARENT_ACCOUNTING, parent_and_leaf_in_one_denominator()),
        (RULE_REFERENTIAL_CLOSURE, undeclared_prerequisite_endpoint()),
    ]
}

fn missing_top_level_key() -> String {
    valid_manifest().replace("non_claim = \"no publication claim\"\n", "")
}

fn quoted_goal_issue() -> String {
    valid_manifest().replace("goal_issue = 100", "goal_issue = \"100\"")
}

/// Quotes `102`, which only the parent's exempt `leaves` list references, so
/// the dropped issue cannot also trip referential closure.
fn quoted_issue_in_role_array() -> String {
    valid_manifest().replace("proof_blockers = [102]", "proof_blockers = [\"102\"]")
}

fn unknown_top_level_key() -> String {
    valid_manifest().replace(
        "control_issue = 9000",
        "control_issue = 9000\nowner = \"someone\"",
    )
}

fn mismatched_milestone_name() -> String {
    valid_manifest().replace(
        "milestone = \"0.13.0 candidate\"",
        "milestone = \"0.13 candidate\"",
    )
}

/// Renumbers the first release above the second. The prerequisite table is
/// dropped with it: leaving it in place would move `#101` into the later
/// release too, so ordering would fire for the same edit and the fixture would
/// stop discriminating declaration order on its own.
fn descending_release_order() -> String {
    valid_manifest()
        .replace("version = \"0.12.0\"", "version = \"0.14.0\"")
        .replace(
            "milestone = \"0.12.0 candidate\"",
            "milestone = \"0.14.0 candidate\"",
        )
        .replace(
            "[[prerequisite]]\nissue = 201\nrequires = 101\njustification = \"consumes the earlier candidate leaf\"\n",
            "",
        )
}

fn one_issue_in_two_roles() -> String {
    valid_manifest().replace("companions = [103]", "companions = [101, 103]")
}

fn one_issue_in_two_releases() -> String {
    valid_manifest().replace("claim_blockers = [201]", "claim_blockers = [101, 201]")
}

fn committed_conditional_issue() -> String {
    valid_manifest().replace("companions = []", "companions = [104]")
}

fn committed_rolling_issue() -> String {
    valid_manifest().replace("companions = []", "companions = [400]")
}

fn prerequisite_in_a_later_release() -> String {
    valid_manifest().replace("issue = 201\nrequires = 101", "issue = 101\nrequires = 201")
}

fn self_prerequisite() -> String {
    valid_manifest().replace("issue = 201\nrequires = 101", "issue = 201\nrequires = 201")
}

fn parent_counted_where_it_is_absent() -> String {
    valid_manifest().replace("counted_in = \"none\"", "counted_in = \"0.12.0\"")
}

fn parent_without_leaves() -> String {
    valid_manifest().replace("leaves = [101, 102]", "leaves = []")
}

fn parent_as_its_own_leaf() -> String {
    valid_manifest().replace("leaves = [101, 102]", "leaves = [300, 101]")
}

fn committed_parent_claiming_none() -> String {
    valid_manifest().replace("issue = 300\nleaves", "issue = 103\nleaves")
}

fn parent_and_leaf_in_one_denominator() -> String {
    valid_manifest().replace(
        "issue = 300\nleaves = [101, 102]\ncounted_in = \"none\"",
        "issue = 103\nleaves = [101, 102]\ncounted_in = \"0.12.0\"",
    )
}

fn undeclared_prerequisite_endpoint() -> String {
    valid_manifest().replace("requires = 101", "requires = 999")
}

#[test]
fn baseline_fixture_is_clean() {
    assert_eq!(violations(&valid_manifest()), Vec::<String>::new());
}

#[test]
fn missing_required_top_level_key_is_a_schema_violation() {
    let found = only_rule(&missing_top_level_key(), RULE_SCHEMA);
    assert_eq!(
        found,
        vec![format!(
            "{RULE_SCHEMA} :: {FIXTURE_PATH}:1 missing required top-level key `non_claim`"
        )]
    );
}

#[test]
fn quoted_issue_number_is_a_schema_violation() {
    let found = only_rule(&quoted_goal_issue(), RULE_SCHEMA);
    assert!(
        found[0].contains("expected a bare positive issue number, got `\"100\"`"),
        "{found:#?}"
    );
}

#[test]
fn quoted_issue_inside_a_role_array_is_a_schema_violation() {
    let found = only_rule(&quoted_issue_in_role_array(), RULE_SCHEMA);
    assert!(found[0].contains("field `proof_blockers`"), "{found:#?}");
}

#[test]
fn unknown_top_level_key_is_a_schema_violation() {
    let found = only_rule(&unknown_top_level_key(), RULE_SCHEMA);
    assert!(
        found[0].contains("unknown top-level key `owner`"),
        "{found:#?}"
    );
}

#[test]
fn milestone_name_must_match_the_release_version() {
    let found = only_rule(&mismatched_milestone_name(), RULE_RELEASE_IDENTITY);
    assert!(
        found[0].contains("the milestone name must be `0.13.0 candidate`"),
        "{found:#?}"
    );
}

#[test]
fn releases_declared_out_of_ascending_order_are_a_release_identity_violation() {
    let found = only_rule(&descending_release_order(), RULE_RELEASE_IDENTITY);
    assert!(
        found[0].contains("must be declared in ascending version order"),
        "{found:#?}"
    );
}

#[test]
fn one_issue_in_two_roles_of_one_release_is_a_role_uniqueness_violation() {
    let found = only_rule(&one_issue_in_two_roles(), RULE_ROLE_UNIQUENESS);
    assert!(
        found[0].contains(
            "issue #101 appears in release `0.12.0` under both `claim_blockers` and `companions`"
        ),
        "{found:#?}"
    );
}

#[test]
fn one_issue_committed_to_two_releases_is_a_disjointness_violation() {
    let found = only_rule(&one_issue_in_two_releases(), RULE_COMMITTED_DISJOINTNESS);
    assert!(
        found[0].contains("issue #101 is committed to both `0.12.0` and `0.13.0`"),
        "{found:#?}"
    );
}

#[test]
fn a_conditional_issue_inside_a_committed_set_is_an_exclusion_violation() {
    let found = only_rule(&committed_conditional_issue(), RULE_NON_COMMITTED_EXCLUSION);
    assert!(
        found[0].contains("issue #104 is conditional under `0.12.0` but is also committed to"),
        "{found:#?}"
    );
}

#[test]
fn a_rolling_issue_inside_a_committed_set_is_an_exclusion_violation() {
    let found = only_rule(&committed_rolling_issue(), RULE_NON_COMMITTED_EXCLUSION);
    assert!(
        found[0].contains("issue #400 is rolling but is also committed to `0.13.0`"),
        "{found:#?}"
    );
}

#[test]
fn a_prerequisite_in_a_later_release_is_an_ordering_violation() {
    let found = only_rule(
        &prerequisite_in_a_later_release(),
        RULE_PREREQUISITE_ORDERING,
    );
    assert!(
        found[0].contains(
            "issue #101 targets `0.12.0` but its prerequisite #201 targets the later `0.13.0`"
        ),
        "{found:#?}"
    );
}

#[test]
fn a_self_prerequisite_is_an_ordering_violation() {
    let found = only_rule(&self_prerequisite(), RULE_PREREQUISITE_ORDERING);
    assert!(
        found[0].contains("issue #201 is declared as its own prerequisite"),
        "{found:#?}"
    );
}

#[test]
fn a_parent_counted_in_a_release_it_does_not_belong_to_is_an_accounting_violation() {
    let found = only_rule(&parent_counted_where_it_is_absent(), RULE_PARENT_ACCOUNTING);
    assert!(
        found[0].contains(
            "parent #300 declares `counted_in = \"0.12.0\"` but is not committed to that release"
        ),
        "{found:#?}"
    );
}

#[test]
fn a_parent_with_no_leaves_is_an_accounting_violation() {
    let found = only_rule(&parent_without_leaves(), RULE_PARENT_ACCOUNTING);
    assert!(
        found[0].contains("parent #300 names no leaves"),
        "{found:#?}"
    );
}

#[test]
fn a_parent_listing_itself_as_a_leaf_is_an_accounting_violation() {
    let found = only_rule(&parent_as_its_own_leaf(), RULE_PARENT_ACCOUNTING);
    assert!(
        found[0].contains("parent #300 lists itself as its own leaf"),
        "{found:#?}"
    );
}

#[test]
fn a_parent_claiming_to_be_outside_progress_while_committed_is_an_accounting_violation() {
    let found = only_rule(&committed_parent_claiming_none(), RULE_PARENT_ACCOUNTING);
    assert!(
        found[0]
            .contains("parent #103 declares `counted_in = \"none\"` but is committed to `0.12.0`"),
        "{found:#?}"
    );
}

#[test]
fn a_parent_and_its_leaf_in_one_denominator_is_a_double_count_violation() {
    let found = only_rule(
        &parent_and_leaf_in_one_denominator(),
        RULE_PARENT_ACCOUNTING,
    );
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(
        found[0].contains(
            "parent #103 and its leaf #101 are both committed to `0.12.0`; that double-counts one capability in the same denominator"
        ),
        "{found:#?}"
    );
    assert!(
        found[1].contains("its leaf #102 are both committed"),
        "{found:#?}"
    );
}

#[test]
fn an_undeclared_prerequisite_endpoint_is_a_closure_violation() {
    let found = only_rule(
        &undeclared_prerequisite_endpoint(),
        RULE_REFERENTIAL_CLOSURE,
    );
    assert!(
        found[0].contains(
            "names issue #999, which no release role, parent, or rolling record declares"
        ),
        "{found:#?}"
    );
}

#[test]
fn every_rule_owns_a_negative_fixture() {
    let mut covered = BTreeSet::new();
    for (rule, text) in negative_fixtures() {
        only_rule(&text, rule);
        covered.insert(rule.to_string());
    }
    let expected = RULE_IDS
        .iter()
        .map(|rule| (*rule).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        covered, expected,
        "a rule without a discriminating negative fixture cannot be claimed to enforce anything"
    );
}

#[test]
fn the_repository_manifest_passes_every_rule() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent();
    assert!(root.is_some(), "xtask has a parent workspace root");
    let Some(root) = root else { return };
    let path = root.join(RELEASE_TARGETS_MANIFEST_PATH);
    let read = std::fs::read_to_string(&path);
    assert!(read.is_ok(), "failed to read {}: {read:?}", path.display());
    let Ok(text) = read else { return };
    let outcome = evaluate_release_targets(RELEASE_TARGETS_MANIFEST_PATH, &text);
    assert_eq!(outcome.violations, Vec::<String>::new());
    assert_eq!(outcome.releases.len(), 3);
    assert_eq!(
        outcome
            .releases
            .iter()
            .map(|release| release.version.as_str())
            .collect::<Vec<_>>(),
        vec!["0.11.1", "0.12.0", "0.13.0"]
    );
    // Fixed at the live denominators verified on 2026-08-08. If a membership
    // edit lands, this number must move deliberately.
    assert_eq!(
        outcome
            .releases
            .iter()
            .map(|release| release.committed_total)
            .collect::<Vec<_>>(),
        vec![3, 19, 16]
    );
    // Exact, not `>=`: a loose bound lets a record appear silently, which is
    // the drift this manifest exists to surface. Nine parents declare
    // `counted_in = "none"`. That is *not* the same as the six live-milestone
    // divergences named in the manifest header -- #2664, #2665, and #2671 are
    // umbrella parents that carry no milestone at all, so they are recorded
    // correctly and diverge from nothing. This checker is network-free and
    // cannot observe milestone membership; it counts what the manifest
    // declares, which is why the field says `committed_sets` rather than
    // `milestone`.
    assert_eq!(outcome.parents_outside_committed_sets, 9);
}

#[test]
fn json_report_is_deterministic_and_declares_no_network_use() {
    let text = valid_manifest();
    let first = release_targets_json(&evaluate_release_targets(FIXTURE_PATH, &text));
    let second = release_targets_json(&evaluate_release_targets(FIXTURE_PATH, &text));
    assert_eq!(first, second);
    assert!(first.contains("\"network_used\": false"), "{first}");
    assert!(first.contains("\"status\": \"pass\""), "{first}");
    assert!(first.ends_with('\n'));
}

#[test]
fn json_report_records_the_failing_rule() {
    let text = undeclared_prerequisite_endpoint();
    let json = release_targets_json(&evaluate_release_targets(FIXTURE_PATH, &text));
    assert!(json.contains("\"status\": \"fail\""), "{json}");
    assert!(json.contains(RULE_REFERENTIAL_CLOSURE), "{json}");
}

/// A manifest with valid metadata and no `[[release]]` record made every
/// membership rule vacuously true, so the command exited 0 after the entire
/// candidate denominator had been deleted. Zero subjects is not a pass.
#[test]
fn an_empty_release_set_is_a_violation() {
    let text = "\
schema_version = \"0.1\"
non_claim = \"no publication claim\"
control_issue = 9000
";
    let found = only_rule(text, "release_identity");
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("declares no `[[release]]` record")),
        "the violation must name the empty denominator: {found:#?}"
    );
}

/// `role_uniqueness` scoped ownership to one release and
/// `committed_disjointness` covered only committed sets, so the same issue
/// could be conditional under two releases with no rule firing — two
/// conflicting intended destinations in a manifest claiming one role in one
/// release.
#[test]
fn a_conditional_issue_may_not_name_two_destinations() {
    let text = valid_manifest().replace(
        "conditional_issues = [204]",
        "conditional_issues = [204, 104]",
    );
    let found = only_rule(&text, "role_uniqueness");
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("#104 is conditional under both")),
        "the violation must name the issue and both releases: {found:#?}"
    );
}

/// Parents and release versions already rejected duplicates; `[[rolling]]` did
/// not. A repeated record passed every rule while counting twice, so the
/// reported rolling denominator overstated the set.
#[test]
fn a_repeated_rolling_record_is_a_violation() {
    let text = format!(
        "{}\n[[rolling]]\nissue = 400\njustification = \"duplicate of the earlier record\"\n",
        valid_manifest()
    );
    let found = only_rule(&text, "non_committed_exclusion");
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("#400 is declared rolling more than once")),
        "the violation must name the repeated issue: {found:#?}"
    );
}

/// Presence alone is not a schema. Before these checks the manifest metadata
/// accepted `schema_version = "banana"`, a numeric `non_claim`, and a negative
/// `control_issue`, so schema-version compatibility could not work and a later
/// standard TOML consumer could reject a document this gate had blessed.
#[test]
fn top_level_metadata_is_type_and_value_checked() {
    let unsupported =
        valid_manifest().replace("schema_version = \"0.1\"", "schema_version = \"banana\"");
    let found = only_rule(&unsupported, "schema");
    assert!(
        found.iter().any(|violation| violation
            .contains("`schema_version` is `banana`, but this checker supports only `0.1`")),
        "an unsupported schema version must be named: {found:#?}"
    );

    let unquoted = valid_manifest().replace("schema_version = \"0.1\"", "schema_version = 0.1");
    let found = only_rule(&unquoted, "schema");
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("`schema_version` must be a quoted string")),
        "a non-string schema version must be rejected: {found:#?}"
    );

    let numeric_non_claim =
        valid_manifest().replace("non_claim = \"no publication claim\"", "non_claim = 7");
    let found = only_rule(&numeric_non_claim, "schema");
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("`non_claim` must be a quoted string")),
        "a numeric non_claim must be rejected: {found:#?}"
    );

    let empty_non_claim =
        valid_manifest().replace("non_claim = \"no publication claim\"", "non_claim = \"\"");
    let found = only_rule(&empty_non_claim, "schema");
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("must state the boundary")),
        "an empty non_claim must be rejected: {found:#?}"
    );

    let negative_control = valid_manifest().replace("control_issue = 9000", "control_issue = -1");
    let found = only_rule(&negative_control, "schema");
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("`control_issue`:")),
        "a negative control_issue must be rejected: {found:#?}"
    );
}

/// Ordinal comparison cannot see a cycle inside one release: its members share
/// an ordinal, so `parent > child` is false for every intra-release edge and
/// `A requires B` beside `B requires A` passed every rule.
#[test]
fn an_intra_release_prerequisite_cycle_is_a_violation() {
    let text = format!(
        "{}\n[[prerequisite]]\nissue = 101\nrequires = 103\njustification = \"cycle back to the companion\"\n\n[[prerequisite]]\nissue = 103\nrequires = 101\njustification = \"and back again\"\n",
        valid_manifest()
    );
    let found = only_rule(&text, "prerequisite_ordering");
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("prerequisite cycle")),
        "the violation must name the cycle: {found:#?}"
    );
}
