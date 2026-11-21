//! Offline graph-integrity check for `policy/release-targets.toml` (#3013
//! Slice B).
//!
//! The manifest is the membership authority for release-candidate scope. This
//! checker is deterministic and network-free: it never reads GitHub, and it
//! never parses release-goal issue prose. Prose membership is written partly as
//! en-dash ranges, so a prose parser would silently miss the members inside a
//! range and then report a clean graph over issues it never saw. Comparing the
//! manifest against live milestones is Slice C's separate, explicitly
//! network-owned job.

use std::collections::{BTreeMap, BTreeSet};

use crate::{CiLedgerTable, FixKind, PolicyReportSpec, finish_policy_report, write_report};

pub(crate) const RELEASE_TARGETS_MANIFEST_PATH: &str = "policy/release-targets.toml";

/// Rule identifiers. Every violation names exactly one, so a fixture that
/// breaks one rule can be proved to trip that rule and no other.
const RULE_SCHEMA: &str = "schema";
const RULE_RELEASE_IDENTITY: &str = "release_identity";
const RULE_ROLE_UNIQUENESS: &str = "role_uniqueness";
const RULE_COMMITTED_DISJOINTNESS: &str = "committed_disjointness";
const RULE_NON_COMMITTED_EXCLUSION: &str = "non_committed_exclusion";
const RULE_PREREQUISITE_ORDERING: &str = "prerequisite_ordering";
const RULE_PARENT_ACCOUNTING: &str = "parent_accounting";
const RULE_REFERENTIAL_CLOSURE: &str = "referential_closure";

const RULE_IDS: &[&str] = &[
    RULE_SCHEMA,
    RULE_RELEASE_IDENTITY,
    RULE_ROLE_UNIQUENESS,
    RULE_COMMITTED_DISJOINTNESS,
    RULE_NON_COMMITTED_EXCLUSION,
    RULE_PREREQUISITE_ORDERING,
    RULE_PARENT_ACCOUNTING,
    RULE_REFERENTIAL_CLOSURE,
];

const REQUIRED_TOP_LEVEL_KEYS: &[&str] = &["schema_version", "non_claim", "control_issue"];

const RELEASE_KEYS: &[&str] = &[
    "version",
    "milestone",
    "goal_issue",
    "claim_blockers",
    "proof_blockers",
    "companions",
    "conditional_issues",
];
const PARENT_KEYS: &[&str] = &["issue", "leaves", "counted_in", "justification"];
const PREREQUISITE_KEYS: &[&str] = &["issue", "requires", "justification"];
const ROLLING_KEYS: &[&str] = &["issue", "justification"];

const KNOWN_TABLE_HEADERS: &[&str] = &["release", "parent", "prerequisite", "rolling"];

/// The four committed roles. `conditional_issues` is deliberately absent: a
/// conditional issue carries an intended destination without entering the
/// committed denominator.
const COMMITTED_LIST_KEYS: &[&str] = &["claim_blockers", "proof_blockers", "companions"];

#[derive(Clone, Debug)]
struct ReleaseRecord {
    line: usize,
    version: String,
    ordinal: Option<(u32, u32, u32)>,
    milestone: String,
    goal_issue: Option<u32>,
    roles: BTreeMap<String, Vec<u32>>,
}

impl ReleaseRecord {
    fn role(&self, key: &str) -> &[u32] {
        self.roles.get(key).map(Vec::as_slice).unwrap_or_default()
    }

    fn committed(&self) -> BTreeSet<u32> {
        let mut committed = BTreeSet::new();
        committed.extend(self.goal_issue);
        for key in COMMITTED_LIST_KEYS {
            committed.extend(self.role(key).iter().copied());
        }
        committed
    }
}

#[derive(Clone, Debug)]
struct ParentRecord {
    line: usize,
    issue: Option<u32>,
    leaves: Vec<u32>,
    counted_in: Option<String>,
}

#[derive(Clone, Debug)]
struct EdgeRecord {
    line: usize,
    issue: Option<u32>,
    requires: Option<u32>,
}

#[derive(Clone, Debug)]
struct RollingRecord {
    line: usize,
    issue: Option<u32>,
}

/// Outcome of evaluating one manifest document. `violations` is the ordered,
/// deterministic finding list; the remaining fields feed the JSON report.
#[derive(Clone, Debug)]
pub(crate) struct ReleaseTargetsOutcome {
    violations: Vec<String>,
    releases: Vec<ReleaseSummary>,
    parents_outside_committed_sets: usize,
    prerequisite_edges: usize,
    rolling_issues: usize,
}

#[derive(Clone, Debug)]
struct ReleaseSummary {
    version: String,
    milestone: String,
    goal_issue: Option<u32>,
    claim_blockers: usize,
    proof_blockers: usize,
    companions: usize,
    committed_total: usize,
    conditional_total: usize,
}

pub(crate) fn check_release_targets() -> Result<(), String> {
    let path = RELEASE_TARGETS_MANIFEST_PATH;
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            let violations = vec![format!("{path} [{RULE_SCHEMA}] cannot be read: {err}")];
            write_report(
                "release-targets.json",
                &release_targets_json(&ReleaseTargetsOutcome {
                    violations: violations.clone(),
                    releases: Vec::new(),
                    parents_outside_committed_sets: 0,
                    prerequisite_edges: 0,
                    rolling_issues: 0,
                }),
            )?;
            return finish_policy_report(report_spec(), &violations);
        }
    };

    let outcome = evaluate_release_targets(path, &text);
    write_report("release-targets.json", &release_targets_json(&outcome))?;
    finish_policy_report(report_spec(), &outcome.violations)
}

fn report_spec() -> PolicyReportSpec<'static> {
    PolicyReportSpec {
        report_file: "release-targets.md",
        check: "check-release-targets",
        why_it_matters: "Milestone membership is the committed candidate denominator. When the manifest, the release-goal graph, and milestone objects drift apart, progress counts stop meaning what they claim and conditional or umbrella work silently inflates a release promise. This check keeps the checked-in membership graph internally coherent offline; it does not read GitHub and does not qualify or publish any candidate.",
        fix_kind: FixKind::AuthorDecisionRequired,
        recommended_fixes: &[
            "Give every issue exactly one role in exactly one release; conditional and rolling work belongs outside every committed set.",
            "Record an umbrella parent as `[[parent]]` with `counted_in = \"none\"` unless it owns distinct final acceptance beyond its leaves, and then state that acceptance in `justification`.",
            "Declare every prerequisite endpoint in the manifest and keep a prerequisite in the same or an earlier release than the issue consuming it.",
            "Update policy/release-targets.toml rather than a release-goal issue body: the manifest is the parsed authority and the goal bodies are human-validated documentation.",
        ],
        rerun_command: "cargo xtask check-release-targets",
        exception_template: None,
    }
}

/// Evaluate one manifest document. Pure: no filesystem, no network, no clock.
fn evaluate_release_targets(path: &str, text: &str) -> ReleaseTargetsOutcome {
    let mut violations = Vec::new();
    let (document, mut parse_violations) = crate::parse_ci_ledger_document(path, text);
    for violation in parse_violations.drain(..) {
        violations.push(format!("{RULE_SCHEMA} :: {violation}"));
    }

    check_top_level(path, &document, &mut violations);
    for table in &document.tables {
        if !KNOWN_TABLE_HEADERS.contains(&table.header.as_str()) {
            violations.push(rule(
                RULE_SCHEMA,
                path,
                table.line,
                &format!("unknown table `[[{}]]`", table.header),
            ));
        }
    }

    let releases = parse_releases(path, &document, &mut violations);
    let parents = parse_parents(path, &document, &mut violations);
    let edges = parse_edges(path, &document, &mut violations);
    let rolling = parse_rolling(path, &document, &mut violations);

    check_release_identity(path, &releases, &mut violations);
    check_role_uniqueness(path, &releases, &mut violations);
    check_conditional_ownership(path, &releases, &mut violations);
    check_committed_disjointness(path, &releases, &mut violations);
    check_non_committed_exclusion(path, &releases, &rolling, &mut violations);
    check_parent_accounting(path, &releases, &parents, &mut violations);
    check_prerequisite_ordering(path, &releases, &edges, &mut violations);
    check_referential_closure(path, &releases, &parents, &edges, &rolling, &mut violations);

    ReleaseTargetsOutcome {
        violations,
        releases: releases.iter().map(release_summary).collect(),
        parents_outside_committed_sets: parents
            .iter()
            .filter(|parent| parent.counted_in.as_deref() == Some("none"))
            .count(),
        prerequisite_edges: edges.len(),
        rolling_issues: rolling.len(),
    }
}

fn release_summary(release: &ReleaseRecord) -> ReleaseSummary {
    ReleaseSummary {
        version: release.version.clone(),
        milestone: release.milestone.clone(),
        goal_issue: release.goal_issue,
        claim_blockers: release.role("claim_blockers").len(),
        proof_blockers: release.role("proof_blockers").len(),
        companions: release.role("companions").len(),
        committed_total: release.committed().len(),
        conditional_total: release.role("conditional_issues").len(),
    }
}

fn rule(id: &str, path: &str, line: usize, message: &str) -> String {
    format!("{id} :: {path}:{line} {message}")
}

fn check_top_level(path: &str, document: &crate::CiLedgerDocument, violations: &mut Vec<String>) {
    for key in REQUIRED_TOP_LEVEL_KEYS {
        if !document.top_level.contains_key(*key) {
            violations.push(rule(
                RULE_SCHEMA,
                path,
                1,
                &format!("missing required top-level key `{key}`"),
            ));
        }
    }
    for (key, value) in &document.top_level {
        if !REQUIRED_TOP_LEVEL_KEYS.contains(&key.as_str()) {
            violations.push(rule(
                RULE_SCHEMA,
                path,
                value.line,
                &format!("unknown top-level key `{key}`"),
            ));
        }
    }

    // Presence alone is not a schema. Without these, `schema_version =
    // "banana"`, a numeric `non_claim`, and a negative `control_issue` were all
    // accepted, so schema-version compatibility could not work and a later
    // standard TOML consumer could reject a document this gate blessed.
    if let Some(value) = document.top_level.get("schema_version") {
        match quoted_string(&value.raw) {
            Some(version) if version == SUPPORTED_SCHEMA_VERSION => {}
            Some(version) => violations.push(rule(
                RULE_SCHEMA,
                path,
                value.line,
                &format!(
                    "`schema_version` is `{version}`, but this checker supports only `{SUPPORTED_SCHEMA_VERSION}`"
                ),
            )),
            None => violations.push(rule(
                RULE_SCHEMA,
                path,
                value.line,
                &format!(
                    "`schema_version` must be a quoted string, got `{}`",
                    value.raw.trim()
                ),
            )),
        }
    }

    if let Some(value) = document.top_level.get("non_claim") {
        match quoted_string(&value.raw) {
            Some(text) if !text.trim().is_empty() => {}
            Some(_) => violations.push(rule(
                RULE_SCHEMA,
                path,
                value.line,
                "`non_claim` must state the boundary this manifest does not claim, not an empty string",
            )),
            None => violations.push(rule(
                RULE_SCHEMA,
                path,
                value.line,
                &format!(
                    "`non_claim` must be a quoted string, got `{}`",
                    value.raw.trim()
                ),
            )),
        }
    }

    if let Some(value) = document.top_level.get("control_issue")
        && let Err(message) = parse_issue_number(&value.raw)
    {
        violations.push(rule(
            RULE_SCHEMA,
            path,
            value.line,
            &format!("`control_issue`: {message}"),
        ));
    }
}

/// The one schema version this checker understands. A manifest declaring any
/// other version is rejected rather than parsed on the assumption that the
/// shape did not change.
const SUPPORTED_SCHEMA_VERSION: &str = "0.1";

/// Return the contents of a double-quoted scalar, or `None` when the raw value
/// is not a quoted string at all.
fn quoted_string(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
}

fn check_table_keys(
    path: &str,
    table: &CiLedgerTable,
    known: &[&str],
    violations: &mut Vec<String>,
) {
    for (key, value) in &table.values {
        if !known.contains(&key.as_str()) {
            violations.push(rule(
                RULE_SCHEMA,
                path,
                value.line,
                &format!("table `[[{}]]` has unknown key `{key}`", table.header),
            ));
        }
    }
}

fn parse_releases(
    path: &str,
    document: &crate::CiLedgerDocument,
    violations: &mut Vec<String>,
) -> Vec<ReleaseRecord> {
    let mut releases = Vec::new();
    for table in crate::ci_tables(document, "release") {
        check_table_keys(path, table, RELEASE_KEYS, violations);
        let version =
            required_string(path, table, "version", violations).unwrap_or_else(|| "?".to_string());
        let milestone = required_string(path, table, "milestone", violations)
            .unwrap_or_else(|| "?".to_string());
        let goal_issue = required_issue(path, table, "goal_issue", violations);
        let mut roles = BTreeMap::new();
        for key in [
            "claim_blockers",
            "proof_blockers",
            "companions",
            "conditional_issues",
        ] {
            roles.insert(
                key.to_string(),
                required_issue_array(path, table, key, violations),
            );
        }
        releases.push(ReleaseRecord {
            line: table.line,
            ordinal: parse_version_ordinal(&version),
            version,
            milestone,
            goal_issue,
            roles,
        });
    }
    releases
}

fn parse_parents(
    path: &str,
    document: &crate::CiLedgerDocument,
    violations: &mut Vec<String>,
) -> Vec<ParentRecord> {
    crate::ci_tables(document, "parent")
        .into_iter()
        .map(|table| {
            check_table_keys(path, table, PARENT_KEYS, violations);
            // A parent must state why it is or is not counted; the value is
            // for human review, so validate presence and drop it.
            required_string(path, table, "justification", violations);
            ParentRecord {
                line: table.line,
                issue: required_issue(path, table, "issue", violations),
                leaves: required_issue_array(path, table, "leaves", violations),
                counted_in: required_string(path, table, "counted_in", violations),
            }
        })
        .collect()
}

fn parse_edges(
    path: &str,
    document: &crate::CiLedgerDocument,
    violations: &mut Vec<String>,
) -> Vec<EdgeRecord> {
    crate::ci_tables(document, "prerequisite")
        .into_iter()
        .map(|table| {
            check_table_keys(path, table, PREREQUISITE_KEYS, violations);
            required_string(path, table, "justification", violations);
            EdgeRecord {
                line: table.line,
                issue: required_issue(path, table, "issue", violations),
                requires: required_issue(path, table, "requires", violations),
            }
        })
        .collect()
}

fn parse_rolling(
    path: &str,
    document: &crate::CiLedgerDocument,
    violations: &mut Vec<String>,
) -> Vec<RollingRecord> {
    crate::ci_tables(document, "rolling")
        .into_iter()
        .map(|table| {
            check_table_keys(path, table, ROLLING_KEYS, violations);
            required_string(path, table, "justification", violations);
            RollingRecord {
                line: table.line,
                issue: required_issue(path, table, "issue", violations),
            }
        })
        .collect()
}

fn required_string(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    violations: &mut Vec<String>,
) -> Option<String> {
    let mut local = Vec::new();
    let parsed = crate::ci_required_non_empty_table_string(path, table, key, &mut local);
    for violation in local {
        violations.push(format!("{RULE_SCHEMA} :: {violation}"));
    }
    parsed
}

/// Parse a bare positive issue number. Issue numbers are integers, not the
/// quoted strings the shared ledger helpers expect, so this owns its own
/// parsing rather than borrowing a string helper that would accept `"3005"`.
fn required_issue(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    violations: &mut Vec<String>,
) -> Option<u32> {
    let Some(value) = table.values.get(key) else {
        violations.push(rule(
            RULE_SCHEMA,
            path,
            table.line,
            &format!("table `[[{}]]` is missing `{key}`", table.header),
        ));
        return None;
    };
    match parse_issue_number(&value.raw) {
        Ok(issue) => Some(issue),
        Err(err) => {
            violations.push(rule(
                RULE_SCHEMA,
                path,
                value.line,
                &format!("table `[[{}]]` field `{key}`: {err}", table.header),
            ));
            None
        }
    }
}

fn required_issue_array(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    violations: &mut Vec<String>,
) -> Vec<u32> {
    let Some(value) = table.values.get(key) else {
        violations.push(rule(
            RULE_SCHEMA,
            path,
            table.line,
            &format!("table `[[{}]]` is missing `{key}`", table.header),
        ));
        return Vec::new();
    };
    let raw = value.raw.trim();
    if !raw.starts_with('[') || !raw.ends_with(']') {
        violations.push(rule(
            RULE_SCHEMA,
            path,
            value.line,
            &format!(
                "table `[[{}]]` field `{key}` must be an inline array of issue numbers",
                table.header
            ),
        ));
        return Vec::new();
    }
    let mut issues = Vec::new();
    for item in raw[1..raw.len() - 1].split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        match parse_issue_number(item) {
            Ok(issue) => issues.push(issue),
            Err(err) => violations.push(rule(
                RULE_SCHEMA,
                path,
                value.line,
                &format!("table `[[{}]]` field `{key}`: {err}", table.header),
            )),
        }
    }
    issues
}

fn parse_issue_number(raw: &str) -> Result<u32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("expected a bare positive issue number".to_string());
    }
    if !trimmed.chars().all(|character| character.is_ascii_digit()) {
        return Err(format!(
            "expected a bare positive issue number, got `{trimmed}`"
        ));
    }
    let issue: u32 = trimmed
        .parse()
        .map_err(|err| format!("issue number `{trimmed}` is out of range: {err}"))?;
    if issue == 0 {
        return Err("issue number `0` is not a real issue".to_string());
    }
    Ok(issue)
}

fn parse_version_ordinal(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn check_release_identity(path: &str, releases: &[ReleaseRecord], violations: &mut Vec<String>) {
    // A manifest with no releases makes every later rule vacuously true, so the
    // command would exit 0 after the entire candidate denominator had been
    // deleted. Zero subjects is not a passing check.
    if releases.is_empty() {
        violations.push(rule(
            RULE_RELEASE_IDENTITY,
            path,
            1,
            "the manifest declares no `[[release]]` record, so every membership rule \
             would pass over an empty denominator; a release manifest must name at \
             least one release",
        ));
    }

    let mut seen_versions: BTreeSet<&str> = BTreeSet::new();
    let mut seen_milestones: BTreeSet<&str> = BTreeSet::new();
    let mut previous: Option<(u32, u32, u32)> = None;
    for release in releases {
        let expected = format!("{} candidate", release.version);
        if release.milestone != expected {
            violations.push(rule(
                RULE_RELEASE_IDENTITY,
                path,
                release.line,
                &format!(
                    "release `{}` declares milestone `{}` but the milestone name must be `{expected}`",
                    release.version, release.milestone
                ),
            ));
        }
        if !seen_versions.insert(release.version.as_str()) {
            violations.push(rule(
                RULE_RELEASE_IDENTITY,
                path,
                release.line,
                &format!("release version `{}` is declared twice", release.version),
            ));
        }
        if !seen_milestones.insert(release.milestone.as_str()) {
            violations.push(rule(
                RULE_RELEASE_IDENTITY,
                path,
                release.line,
                &format!("milestone `{}` is declared twice", release.milestone),
            ));
        }
        match release.ordinal {
            None => violations.push(rule(
                RULE_RELEASE_IDENTITY,
                path,
                release.line,
                &format!(
                    "release version `{}` is not a `major.minor.patch` version",
                    release.version
                ),
            )),
            Some(ordinal) => {
                if let Some(previous) = previous
                    && ordinal <= previous
                {
                    violations.push(rule(
                        RULE_RELEASE_IDENTITY,
                        path,
                        release.line,
                        &format!(
                            "release `{}` is declared after a later or equal version; releases must be declared in ascending version order because that order is the prerequisite ordering",
                            release.version
                        ),
                    ));
                }
                previous = Some(ordinal);
            }
        }
    }
}

fn check_role_uniqueness(path: &str, releases: &[ReleaseRecord], violations: &mut Vec<String>) {
    for release in releases {
        let mut owner: BTreeMap<u32, &str> = BTreeMap::new();
        if let Some(goal) = release.goal_issue {
            owner.insert(goal, "goal_issue");
        }
        for key in [
            "claim_blockers",
            "proof_blockers",
            "companions",
            "conditional_issues",
        ] {
            for issue in release.role(key) {
                if let Some(existing) = owner.insert(*issue, key) {
                    violations.push(rule(
                        RULE_ROLE_UNIQUENESS,
                        path,
                        release.line,
                        &format!(
                            "issue #{issue} appears in release `{}` under both `{existing}` and `{key}`; every issue takes exactly one role",
                            release.version
                        ),
                    ));
                }
            }
        }
    }
}

/// `check_role_uniqueness` scopes ownership to a single release, and
/// `check_committed_disjointness` covers only committed sets, so a conditional
/// issue could be declared under two releases with no rule firing. The report
/// states the contract as one role in one release; without this the manifest
/// could record two conflicting intended destinations while passing.
fn check_conditional_ownership(
    path: &str,
    releases: &[ReleaseRecord],
    violations: &mut Vec<String>,
) {
    let mut owner: BTreeMap<u32, String> = BTreeMap::new();
    for release in releases {
        for issue in release.role("conditional_issues") {
            if let Some(existing) = owner.insert(*issue, release.version.clone()) {
                violations.push(rule(
                    RULE_ROLE_UNIQUENESS,
                    path,
                    release.line,
                    &format!(
                        "issue #{issue} is conditional under both `{existing}` and `{}`; \
                         a conditional issue names one intended destination",
                        release.version
                    ),
                ));
            }
        }
    }
}

fn check_committed_disjointness(
    path: &str,
    releases: &[ReleaseRecord],
    violations: &mut Vec<String>,
) {
    let mut owner: BTreeMap<u32, String> = BTreeMap::new();
    for release in releases {
        for issue in release.committed() {
            if let Some(existing) = owner.insert(issue, release.version.clone()) {
                violations.push(rule(
                    RULE_COMMITTED_DISJOINTNESS,
                    path,
                    release.line,
                    &format!(
                        "issue #{issue} is committed to both `{existing}` and `{}`; one issue belongs to at most one release milestone",
                        release.version
                    ),
                ));
            }
        }
    }
}

fn check_non_committed_exclusion(
    path: &str,
    releases: &[ReleaseRecord],
    rolling: &[RollingRecord],
    violations: &mut Vec<String>,
) {
    let mut committed: BTreeMap<u32, String> = BTreeMap::new();
    for release in releases {
        for issue in release.committed() {
            committed
                .entry(issue)
                .or_insert_with(|| release.version.clone());
        }
    }
    for release in releases {
        for issue in release.role("conditional_issues") {
            if let Some(owner) = committed.get(issue) {
                violations.push(rule(
                    RULE_NON_COMMITTED_EXCLUSION,
                    path,
                    release.line,
                    &format!(
                        "issue #{issue} is conditional under `{}` but is also committed to `{owner}`; conditional work stays outside every committed set until an admission comment cites the evidence that made it blocking",
                        release.version
                    ),
                ));
            }
        }
    }
    // A repeated `[[rolling]] issue = N` passed every rule while incrementing
    // `rolling_issues` twice, so the reported denominator overstated the rolling
    // set. Parents and release versions already reject duplicates; rolling had
    // no equivalent.
    let mut seen_rolling: BTreeMap<u32, usize> = BTreeMap::new();
    for record in rolling {
        let Some(issue) = record.issue else { continue };
        if let Some(first) = seen_rolling.insert(issue, record.line) {
            violations.push(rule(
                RULE_NON_COMMITTED_EXCLUSION,
                path,
                record.line,
                &format!(
                    "issue #{issue} is declared rolling more than once (first at line {first}); a repeated record inflates the reported rolling denominator"
                ),
            ));
        }
        if let Some(owner) = committed.get(&issue) {
            violations.push(rule(
                RULE_NON_COMMITTED_EXCLUSION,
                path,
                record.line,
                &format!(
                    "issue #{issue} is rolling but is also committed to `{owner}`; rolling work stays outside every release milestone"
                ),
            ));
        }
    }
}

fn check_parent_accounting(
    path: &str,
    releases: &[ReleaseRecord],
    parents: &[ParentRecord],
    violations: &mut Vec<String>,
) {
    let versions: BTreeSet<&str> = releases
        .iter()
        .map(|release| release.version.as_str())
        .collect();
    let mut committed: BTreeMap<u32, String> = BTreeMap::new();
    for release in releases {
        for issue in release.committed() {
            committed
                .entry(issue)
                .or_insert_with(|| release.version.clone());
        }
    }

    let mut seen: BTreeSet<u32> = BTreeSet::new();
    for parent in parents {
        let Some(issue) = parent.issue else { continue };
        if !seen.insert(issue) {
            violations.push(rule(
                RULE_PARENT_ACCOUNTING,
                path,
                parent.line,
                &format!("parent #{issue} is declared twice"),
            ));
        }
        if parent.leaves.is_empty() {
            violations.push(rule(
                RULE_PARENT_ACCOUNTING,
                path,
                parent.line,
                &format!(
                    "parent #{issue} names no leaves; an umbrella with no recorded leaf cannot be checked for double counting"
                ),
            ));
        }
        if parent.leaves.contains(&issue) {
            violations.push(rule(
                RULE_PARENT_ACCOUNTING,
                path,
                parent.line,
                &format!("parent #{issue} lists itself as its own leaf"),
            ));
        }
        let Some(counted_in) = parent.counted_in.as_deref() else {
            continue;
        };
        if counted_in == "none" {
            if let Some(owner) = committed.get(&issue) {
                violations.push(rule(
                    RULE_PARENT_ACCOUNTING,
                    path,
                    parent.line,
                    &format!(
                        "parent #{issue} declares `counted_in = \"none\"` but is committed to `{owner}`; a parent outside milestone progress must not appear in a committed set"
                    ),
                ));
            }
            continue;
        }
        if !versions.contains(counted_in) {
            violations.push(rule(
                RULE_PARENT_ACCOUNTING,
                path,
                parent.line,
                &format!(
                    "parent #{issue} declares `counted_in = \"{counted_in}\"`, which is not a declared release or `\"none\"`"
                ),
            ));
            continue;
        }
        match committed.get(&issue) {
            None => violations.push(rule(
                RULE_PARENT_ACCOUNTING,
                path,
                parent.line,
                &format!(
                    "parent #{issue} declares `counted_in = \"{counted_in}\"` but is not committed to that release"
                ),
            )),
            Some(owner) if owner != counted_in => violations.push(rule(
                RULE_PARENT_ACCOUNTING,
                path,
                parent.line,
                &format!(
                    "parent #{issue} declares `counted_in = \"{counted_in}\"` but is committed to `{owner}`"
                ),
            )),
            Some(_) => {}
        }
        for leaf in &parent.leaves {
            if committed.get(leaf).map(String::as_str) == Some(counted_in) {
                violations.push(rule(
                    RULE_PARENT_ACCOUNTING,
                    path,
                    parent.line,
                    &format!(
                        "parent #{issue} and its leaf #{leaf} are both committed to `{counted_in}`; that double-counts one capability in the same denominator"
                    ),
                ));
            }
        }
    }
}

fn check_prerequisite_ordering(
    path: &str,
    releases: &[ReleaseRecord],
    edges: &[EdgeRecord],
    violations: &mut Vec<String>,
) {
    let mut placement: BTreeMap<u32, (&str, (u32, u32, u32))> = BTreeMap::new();
    for release in releases {
        let Some(ordinal) = release.ordinal else {
            continue;
        };
        let mut record = |issue: u32| {
            placement
                .entry(issue)
                .or_insert((release.version.as_str(), ordinal));
        };
        if let Some(goal) = release.goal_issue {
            record(goal);
        }
        for key in [
            "claim_blockers",
            "proof_blockers",
            "companions",
            "conditional_issues",
        ] {
            for issue in release.role(key) {
                record(*issue);
            }
        }
    }

    for edge in edges {
        let (Some(issue), Some(requires)) = (edge.issue, edge.requires) else {
            continue;
        };
        if issue == requires {
            violations.push(rule(
                RULE_PREREQUISITE_ORDERING,
                path,
                edge.line,
                &format!("issue #{issue} is declared as its own prerequisite"),
            ));
            continue;
        }
        let (Some(child), Some(parent)) = (placement.get(&issue), placement.get(&requires)) else {
            continue;
        };
        if parent.1 > child.1 {
            violations.push(rule(
                RULE_PREREQUISITE_ORDERING,
                path,
                edge.line,
                &format!(
                    "issue #{issue} targets `{}` but its prerequisite #{requires} targets the later `{}`; a child may not target an earlier candidate than an unresolved prerequisite",
                    child.0, parent.0
                ),
            ));
        }
    }

    // Ordinal comparison cannot see a cycle inside one release: issues sharing
    // a release share an ordinal, so `parent > child` is false for every
    // intra-release edge and `A requires B` beside `B requires A` passes. The
    // manifest already declares intra-release chains, so this is reachable.
    let mut successors: BTreeMap<u32, Vec<(u32, usize)>> = BTreeMap::new();
    for edge in edges {
        if let (Some(issue), Some(requires)) = (edge.issue, edge.requires)
            && issue != requires
        {
            successors
                .entry(issue)
                .or_default()
                .push((requires, edge.line));
        }
    }
    for start in successors.keys().copied() {
        // Depth-first from each declared consumer. A path returning to its own
        // start is a cycle; reporting from the smallest start keeps the message
        // deterministic.
        let mut stack = vec![(start, Vec::new())];
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        while let Some((node, path_so_far)) = stack.pop() {
            for (next, line) in successors.get(&node).map(Vec::as_slice).unwrap_or_default() {
                let mut chain = path_so_far.clone();
                chain.push(*next);
                if *next == start {
                    if start <= *chain.iter().min().unwrap_or(&start) {
                        let rendered = std::iter::once(start)
                            .chain(chain.iter().copied())
                            .map(|issue| format!("#{issue}"))
                            .collect::<Vec<_>>()
                            .join(" -> ");
                        violations.push(rule(
                            RULE_PREREQUISITE_ORDERING,
                            path,
                            *line,
                            &format!(
                                "prerequisite cycle {rendered}; a cycle inside one release is invisible to ordinal comparison because its members share an ordinal"
                            ),
                        ));
                    }
                } else if seen.insert(*next) {
                    stack.push((*next, chain));
                }
            }
        }
    }
}

fn check_referential_closure(
    path: &str,
    releases: &[ReleaseRecord],
    parents: &[ParentRecord],
    edges: &[EdgeRecord],
    rolling: &[RollingRecord],
    violations: &mut Vec<String>,
) {
    let mut declared: BTreeSet<u32> = BTreeSet::new();
    for release in releases {
        declared.extend(release.goal_issue);
        for key in [
            "claim_blockers",
            "proof_blockers",
            "companions",
            "conditional_issues",
        ] {
            declared.extend(release.role(key).iter().copied());
        }
    }
    declared.extend(parents.iter().filter_map(|parent| parent.issue));
    declared.extend(rolling.iter().filter_map(|record| record.issue));

    for edge in edges {
        for (label, issue) in [("issue", edge.issue), ("requires", edge.requires)] {
            let Some(issue) = issue else { continue };
            if !declared.contains(&issue) {
                violations.push(rule(
                    RULE_REFERENTIAL_CLOSURE,
                    path,
                    edge.line,
                    &format!(
                        "prerequisite `{label} = {issue}` names issue #{issue}, which no release role, parent, or rolling record declares; an undeclared endpoint is a member the graph never saw"
                    ),
                ));
            }
        }
    }
}

/// Deterministic JSON report. Field order is fixed and every collection is
/// emitted in manifest declaration order or sorted order.
fn release_targets_json(outcome: &ReleaseTargetsOutcome) -> String {
    let releases = outcome
        .releases
        .iter()
        .map(|release| {
            serde_json::json!({
                "version": release.version,
                "milestone": release.milestone,
                "goal_issue": release.goal_issue,
                "claim_blockers": release.claim_blockers,
                "proof_blockers": release.proof_blockers,
                "companions": release.companions,
                "committed_total": release.committed_total,
                "conditional_total": release.conditional_total,
            })
        })
        .collect::<Vec<_>>();

    let value = serde_json::json!({
        "check": "check-release-targets",
        "manifest": RELEASE_TARGETS_MANIFEST_PATH,
        "status": if outcome.violations.is_empty() { "pass" } else { "fail" },
        "network_used": false,
        "rules": RULE_IDS,
        "releases": releases,
        "parents_outside_committed_sets": outcome.parents_outside_committed_sets,
        "prerequisite_edges": outcome.prerequisite_edges,
        "rolling_issues": outcome.rolling_issues,
        "violations": outcome.violations,
        "non_claim": "Offline manifest integrity only. This report does not read GitHub, does not establish milestone parity, and does not qualify or publish any release candidate.",
    });
    let mut body = serde_json::to_string_pretty(&value)
        .unwrap_or_else(|_| "{\"status\":\"fail\"}".to_string());
    body.push('\n');
    body
}

#[cfg(test)]
mod tests;
