//! Candidate-artifact lifecycle authority for `docs/release-candidates/`
//! (#3842).
//!
//! `policy/release-targets.toml` owns release membership and names each
//! release's controller issue. This module owns the other half of the same
//! release graph: which retained release-candidate artifact is current,
//! pinned, historical, or invalid. It deliberately reuses the manifest's
//! releases and controllers instead of declaring a second release graph.
//!
//! Consumer law, enforced by [`resolve_candidate_authority`]:
//!
//! ```text
//! current authority = a registered row
//!                   + a matching raw-byte SHA-256
//!                   + a lifecycle state permitted for the requested operation
//!                   + every state-specific identity (checked before a
//!                     `ValidatedRegistry` can exist)
//! ```
//!
//! Classification is by raw-byte digest, never by path, filename, version
//! string, receipt wording, or presence in the directory. A renamed copy of a
//! historical receipt still resolves as historical; changed bytes resolve as
//! unregistered. A missing, partial, unreadable, or contradictory registry is
//! `not_proven` and grants nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) const ARTIFACT_DIR: &str = "docs/release-candidates";
pub(crate) const REGISTRY_PATH: &str = "docs/release-candidates/index.json";
pub(crate) const PROJECTION_PATH: &str = "docs/release-candidates/README.md";
/// Report copy of the expected Markdown projection, so an author can replace
/// a stale `README.md` with the exact derived bytes.
pub(crate) const PROJECTION_REPORT_FILE: &str = "release-candidate-registry.md";

const REGISTRY_KIND: &str = "ripr_release_candidate_authority_registry";
const REGISTRY_SCHEMA_VERSION: &str = "1.0";
const MARKDOWN_GENERATION: &str = "markdown_projection";

/// Rule identifiers. Every violation names exactly one so a fixture can be
/// attributed to the guard it trips.
pub(crate) const RULE_REGISTRY_INPUT: &str = "candidate_registry_input";
pub(crate) const RULE_REGISTRATION: &str = "candidate_registration";
pub(crate) const RULE_DIGEST: &str = "candidate_digest";
pub(crate) const RULE_STATE_IDENTITY: &str = "candidate_state_identity";
pub(crate) const RULE_CURRENTNESS: &str = "candidate_currentness";
pub(crate) const RULE_SUCCESSION: &str = "candidate_succession";
pub(crate) const RULE_PROJECTION: &str = "candidate_projection";

pub(crate) const CANDIDATE_RULE_IDS: &[&str] = &[
    RULE_REGISTRY_INPUT,
    RULE_REGISTRATION,
    RULE_DIGEST,
    RULE_STATE_IDENTITY,
    RULE_CURRENTNESS,
    RULE_SUCCESSION,
    RULE_PROJECTION,
];

/// Closed lifecycle vocabulary. Unknown strings fail deserialization and are
/// therefore `not_proven`, never silently mapped to a nearby state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LifecycleState {
    ActiveSelectionTemplate,
    PinnedExactCandidate,
    HistoricalEvidenceOnly,
    Invalid,
}

impl LifecycleState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ActiveSelectionTemplate => "active_selection_template",
            Self::PinnedExactCandidate => "pinned_exact_candidate",
            Self::HistoricalEvidenceOnly => "historical_evidence_only",
            Self::Invalid => "invalid",
        }
    }

    fn is_current(self) -> bool {
        matches!(
            self,
            Self::ActiveSelectionTemplate | Self::PinnedExactCandidate
        )
    }
}

/// The operation a consumer is trying to satisfy. Selection, cut,
/// qualification, source-sync, and publication prerequisites map onto the
/// two authority operations; history may only be cited.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateOperation {
    /// Cite as audit history. Satisfies no release prerequisite.
    CiteHistory,
    /// Use as the live selection rule before an exact candidate exists.
    SelectionRule,
    /// Use as the exact immutable candidate for qualification, source
    /// preflight/sync, or publication.
    ExactCandidate,
}

impl CandidateOperation {
    const ALL: [Self; 3] = [Self::CiteHistory, Self::SelectionRule, Self::ExactCandidate];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::CiteHistory => "cite_history",
            Self::SelectionRule => "selection_rule",
            Self::ExactCandidate => "exact_candidate",
        }
    }

    fn permits(self, state: LifecycleState) -> bool {
        match self {
            Self::CiteHistory => state != LifecycleState::Invalid,
            Self::SelectionRule => state == LifecycleState::ActiveSelectionTemplate,
            Self::ExactCandidate => state == LifecycleState::PinnedExactCandidate,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistryDocument {
    schema_version: String,
    kind: String,
    control_issue: u64,
    non_claim: String,
    artifacts: Vec<ArtifactRow>,
}

/// One registered artifact. Optional identity fields are `Option` so a
/// missing pinned identity is reported by the state rule that requires it
/// rather than collapsing into an opaque parse failure.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArtifactRow {
    pub(crate) release: String,
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) schema_generation: String,
    pub(crate) authority_issue: u64,
    pub(crate) state: LifecycleState,
    #[serde(default)]
    pub(crate) projection_of: Option<String>,
    #[serde(default)]
    pub(crate) supersedes: Vec<String>,
    #[serde(default)]
    pub(crate) superseded_by: Option<String>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
    #[serde(default)]
    pub(crate) successor_route: Option<String>,
    #[serde(default)]
    pub(crate) candidate: Option<CandidateIdentity>,
    #[serde(default)]
    pub(crate) packets: Option<PacketDigests>,
    #[serde(default)]
    pub(crate) selection_template_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateIdentity {
    #[serde(default)]
    pub(crate) sha: Option<String>,
    #[serde(default)]
    pub(crate) tree: Option<String>,
    #[serde(default, rename = "ref")]
    pub(crate) git_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PacketDigests {
    #[serde(default)]
    pub(crate) selected_claim_packet_sha256: Option<String>,
    #[serde(default)]
    pub(crate) denominator_packet_sha256: Option<String>,
}

/// Every file under [`ARTIFACT_DIR`], keyed by its normalized
/// repository-relative path, plus any file that could not be read.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ArtifactTree {
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    pub(crate) unreadable: BTreeMap<String, String>,
}

/// Read the artifact directory beneath `root`. Paths are normalized to
/// forward-slash repository-relative form, so equivalent checkout roots yield
/// identical trees.
pub(crate) fn read_artifact_tree(root: &Path) -> ArtifactTree {
    let mut tree = ArtifactTree::default();
    collect_tree(&root.join(ARTIFACT_DIR), ARTIFACT_DIR, &mut tree);
    tree
}

fn collect_tree(directory: &Path, relative: &str, tree: &mut ArtifactTree) {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                tree.unreadable.insert(
                    relative.to_string(),
                    format!("cannot list directory: {err}"),
                );
            }
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                tree.unreadable
                    .insert(relative.to_string(), format!("cannot list entry: {err}"));
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = format!("{relative}/{name}");
        let path = entry.path();
        if path.is_dir() {
            collect_tree(&path, &child, tree);
            continue;
        }
        match std::fs::read(&path) {
            Ok(bytes) => {
                tree.files.insert(child, bytes);
            }
            Err(err) => {
                tree.unreadable.insert(child, format!("cannot read: {err}"));
            }
        }
    }
}

/// Release controller as declared by `policy/release-targets.toml`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseController {
    pub(crate) version: String,
    pub(crate) goal_issue: Option<u32>,
}

/// Per-file classification after evaluation. `state` is `None` for an
/// unregistered or unclassifiable file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileClassification {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) registered_path: Option<String>,
    pub(crate) state: Option<LifecycleState>,
    pub(crate) permitted_operations: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseAuthority {
    pub(crate) release: String,
    pub(crate) selection_rule: Option<String>,
    pub(crate) exact_candidate: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct CandidateRegistryOutcome {
    pub(crate) violations: Vec<String>,
    rows: Vec<ArtifactRow>,
    pub(crate) files: Vec<FileClassification>,
    pub(crate) releases: Vec<ReleaseAuthority>,
    /// Markdown projection derived from the typed rows. Empty when the
    /// registry could not be parsed.
    pub(crate) projection: String,
}

impl CandidateRegistryOutcome {
    pub(crate) fn status(&self) -> &'static str {
        if self.violations.is_empty() {
            "established"
        } else {
            "not_proven"
        }
    }

    /// The only way to obtain a registry a consumer may resolve against.
    pub(crate) fn validated(&self) -> Result<ValidatedRegistry, String> {
        if !self.violations.is_empty() {
            return Err(format!(
                "candidate registry is not_proven ({} violation(s)); no artifact is current authority",
                self.violations.len()
            ));
        }
        if self.rows.is_empty() {
            return Err("candidate registry is not_proven: it registers no artifact".to_string());
        }
        Ok(ValidatedRegistry {
            rows: self.rows.clone(),
        })
    }
}

/// A registry that passed every rule. Its rows carry every state-specific
/// identity, so resolution only has to match digest, release, and state.
#[derive(Clone, Debug)]
pub(crate) struct ValidatedRegistry {
    rows: Vec<ArtifactRow>,
}

/// A granted resolution: the registered row the supplied bytes match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CandidateGrant {
    pub(crate) registered_path: String,
    pub(crate) state: LifecycleState,
    pub(crate) candidate_sha: Option<String>,
}

/// Resolve supplied artifact bytes for one operation. The supplied path is
/// never consulted: filenames, version strings, and receipt wording confer no
/// authority.
pub(crate) fn resolve_candidate_authority(
    registry: &ValidatedRegistry,
    release: &str,
    bytes: &[u8],
    operation: CandidateOperation,
) -> Result<CandidateGrant, String> {
    let digest = sha256_hex(bytes);
    let Some(row) = registry.rows.iter().find(|row| row.sha256 == digest) else {
        return Err(format!(
            "bytes with sha256 {digest} match no registered release-candidate artifact; unregistered or changed bytes are non-authoritative"
        ));
    };
    if row.release != release {
        return Err(format!(
            "bytes match `{}` for release {}, not the requested release {release}",
            row.path, row.release
        ));
    }
    if row.projection_of.is_some() && operation != CandidateOperation::CiteHistory {
        return Err(format!(
            "`{}` is a Markdown projection; a projection may be cited but cannot satisfy `{}`",
            row.path,
            operation.as_str()
        ));
    }
    if !operation.permits(row.state) {
        return Err(format!(
            "`{}` is `{}`; that lifecycle state cannot satisfy `{}`",
            row.path,
            row.state.as_str(),
            operation.as_str()
        ));
    }
    Ok(CandidateGrant {
        registered_path: row.path.clone(),
        state: row.state,
        candidate_sha: row.candidate.as_ref().and_then(|c| c.sha.clone()),
    })
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn non_empty(value: Option<&str>) -> bool {
    value.is_some_and(|text| !text.trim().is_empty())
}

fn rule(id: &str, subject: &str, message: &str) -> String {
    format!("{id} :: {subject} {message}")
}

fn is_control_file(path: &str) -> bool {
    path == REGISTRY_PATH || path == PROJECTION_PATH
}

/// Evaluate the registry against the artifact tree and the manifest's release
/// controllers. Pure: no filesystem, no network, no clock.
pub(crate) fn evaluate_candidate_registry(
    tree: &ArtifactTree,
    controllers: &[ReleaseController],
) -> CandidateRegistryOutcome {
    let mut violations = Vec::new();
    for (path, err) in &tree.unreadable {
        violations.push(rule(
            RULE_REGISTRY_INPUT,
            path,
            &format!("is unreadable: {err}"),
        ));
    }

    let rows = match parse_registry(tree, &mut violations) {
        Some(rows) => rows,
        None => {
            violations.sort();
            return CandidateRegistryOutcome {
                files: classify_unresolved(tree),
                violations,
                rows: Vec::new(),
                releases: Vec::new(),
                projection: String::new(),
            };
        }
    };

    let by_path = check_registration(tree, &rows, controllers, &mut violations);
    for row in &rows {
        check_row_identity(tree, row, &mut violations);
    }
    check_succession(&rows, &by_path, &mut violations);
    check_currentness(&rows, controllers, &mut violations);
    check_projection_rows(&rows, &by_path, &mut violations);

    let projection = render_projection(&rows);
    match tree.files.get(PROJECTION_PATH) {
        None => violations.push(rule(
            RULE_PROJECTION,
            PROJECTION_PATH,
            &format!(
                "is missing; it must be the Markdown projection derived from {REGISTRY_PATH}"
            ),
        )),
        Some(bytes) if bytes.as_slice() != projection.as_bytes() => violations.push(rule(
            RULE_PROJECTION,
            PROJECTION_PATH,
            &format!(
                "disagrees with the projection derived from {REGISTRY_PATH}; replace it with target/ripr/reports/{PROJECTION_REPORT_FILE}"
            ),
        )),
        Some(_) => {}
    }

    violations.sort();
    violations.dedup();

    let mut outcome = CandidateRegistryOutcome {
        violations,
        rows,
        files: Vec::new(),
        releases: Vec::new(),
        projection,
    };
    match outcome.validated() {
        Ok(registry) => {
            outcome.files = classify_files(tree, &registry);
            outcome.releases = release_authorities(&registry);
        }
        Err(_) => outcome.files = classify_unresolved(tree),
    }
    outcome
}

fn parse_registry(tree: &ArtifactTree, violations: &mut Vec<String>) -> Option<Vec<ArtifactRow>> {
    let Some(bytes) = tree.files.get(REGISTRY_PATH) else {
        if !tree.unreadable.contains_key(REGISTRY_PATH) {
            violations.push(rule(
                RULE_REGISTRY_INPUT,
                REGISTRY_PATH,
                "is missing; without the registry no release-candidate artifact is current authority",
            ));
        }
        return None;
    };
    let document: RegistryDocument = match serde_json::from_slice(bytes) {
        Ok(document) => document,
        Err(err) => {
            violations.push(rule(
                RULE_REGISTRY_INPUT,
                REGISTRY_PATH,
                &format!("is not a well-formed registry: {err}"),
            ));
            return None;
        }
    };
    let mut ok = true;
    if document.schema_version != REGISTRY_SCHEMA_VERSION {
        violations.push(rule(
            RULE_REGISTRY_INPUT,
            REGISTRY_PATH,
            &format!(
                "declares schema_version `{}`; expected `{REGISTRY_SCHEMA_VERSION}`",
                document.schema_version
            ),
        ));
        ok = false;
    }
    if document.kind != REGISTRY_KIND {
        violations.push(rule(
            RULE_REGISTRY_INPUT,
            REGISTRY_PATH,
            &format!(
                "declares kind `{}`; expected `{REGISTRY_KIND}`",
                document.kind
            ),
        ));
        ok = false;
    }
    if document.non_claim.trim().is_empty() || document.control_issue == 0 {
        violations.push(rule(
            RULE_REGISTRY_INPUT,
            REGISTRY_PATH,
            "must carry a non-empty non_claim and a positive control_issue",
        ));
        ok = false;
    }
    if document.artifacts.is_empty() {
        violations.push(rule(
            RULE_REGISTRY_INPUT,
            REGISTRY_PATH,
            "registers no artifact; zero subjects is not a clean registry",
        ));
        ok = false;
    }
    if !ok {
        return None;
    }
    let mut rows = document.artifacts;
    rows.sort_by(|left, right| left.path.cmp(&right.path));
    Some(rows)
}

fn check_registration<'a>(
    tree: &ArtifactTree,
    rows: &'a [ArtifactRow],
    controllers: &[ReleaseController],
    violations: &mut Vec<String>,
) -> BTreeMap<&'a str, &'a ArtifactRow> {
    let prefix = format!("{ARTIFACT_DIR}/");
    let declared = controllers
        .iter()
        .map(|controller| controller.version.as_str())
        .collect::<BTreeSet<_>>();
    let mut by_path = BTreeMap::new();
    let mut digests: BTreeMap<&str, &str> = BTreeMap::new();

    for row in rows {
        let path = row.path.as_str();
        if !path.starts_with(&prefix)
            || path.contains('\\')
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || is_control_file(path)
        {
            violations.push(rule(
                RULE_REGISTRATION,
                path,
                &format!(
                    "is not a canonical artifact path under {ARTIFACT_DIR}/ (control files are not artifacts)"
                ),
            ));
        }
        if by_path.insert(path, row).is_some() {
            violations.push(rule(
                RULE_REGISTRATION,
                path,
                "is registered more than once",
            ));
        }
        if !declared.contains(row.release.as_str()) {
            violations.push(rule(
                RULE_REGISTRATION,
                path,
                &format!(
                    "names release `{}`, which policy/release-targets.toml does not declare",
                    row.release
                ),
            ));
        }
        if !is_hex(&row.sha256, 64) {
            violations.push(rule(
                RULE_DIGEST,
                path,
                "has a sha256 that is not 64 lowercase hex characters",
            ));
        } else if let Some(other) = digests.insert(row.sha256.as_str(), path) {
            violations.push(rule(
                RULE_REGISTRATION,
                path,
                &format!(
                    "shares its sha256 with `{other}`; one byte sequence may classify only once"
                ),
            ));
        }
        match tree.files.get(path) {
            None if !tree.unreadable.contains_key(path) => violations.push(rule(
                RULE_REGISTRATION,
                path,
                "is registered but absent from the artifact directory",
            )),
            None => {}
            Some(bytes) => {
                let actual = sha256_hex(bytes);
                if is_hex(&row.sha256, 64) && actual != row.sha256 {
                    violations.push(rule(
                        RULE_DIGEST,
                        path,
                        &format!(
                            "raw bytes hash to {actual}, not the registered {}",
                            row.sha256
                        ),
                    ));
                }
            }
        }
    }

    for path in tree.files.keys() {
        if !is_control_file(path) && !by_path.contains_key(path.as_str()) {
            violations.push(rule(
                RULE_REGISTRATION,
                path,
                "is not registered; an unregistered artifact is non-authoritative and may not be retained silently",
            ));
        }
    }
    by_path
}

/// Schema generation, format, and state-specific identities for one row.
fn check_row_identity(tree: &ArtifactTree, row: &ArtifactRow, violations: &mut Vec<String>) {
    let path = row.path.as_str();
    let json = if path.ends_with(".json") {
        if row.projection_of.is_some() {
            violations.push(rule(
                RULE_PROJECTION,
                path,
                "is JSON but declares projection_of; only Markdown rows are projections",
            ));
        }
        match tree
            .files
            .get(path)
            .map(|bytes| serde_json::from_slice::<Value>(bytes))
        {
            Some(Ok(value)) => {
                let generation = json_generation(&value);
                if generation.as_deref() != Some(row.schema_generation.as_str()) {
                    violations.push(rule(
                        RULE_STATE_IDENTITY,
                        path,
                        &format!(
                            "registers schema_generation `{}` but the artifact declares `{}`",
                            row.schema_generation,
                            generation.unwrap_or_else(|| "<none>".to_string())
                        ),
                    ));
                }
                Some(value)
            }
            Some(Err(err)) => {
                violations.push(rule(
                    RULE_STATE_IDENTITY,
                    path,
                    &format!("is not parseable JSON: {err}"),
                ));
                None
            }
            None => None,
        }
    } else if path.ends_with(".md") {
        if row.projection_of.is_none() {
            violations.push(rule(
                RULE_PROJECTION,
                path,
                "is Markdown without projection_of; human Markdown cannot own lifecycle state",
            ));
        }
        if row.schema_generation != MARKDOWN_GENERATION {
            violations.push(rule(
                RULE_STATE_IDENTITY,
                path,
                &format!("Markdown rows must declare schema_generation `{MARKDOWN_GENERATION}`"),
            ));
        }
        None
    } else {
        violations.push(rule(
            RULE_REGISTRATION,
            path,
            "has an unsupported artifact format; register JSON authority or a Markdown projection",
        ));
        None
    };

    if row.projection_of.is_some() {
        // Projection rows own no lifecycle; `check_projection_rows` binds them.
        return;
    }
    let declared_status = json
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str);

    match row.state {
        LifecycleState::ActiveSelectionTemplate => {
            if row.candidate.is_some()
                || row.packets.is_some()
                || row.selection_template_sha256.is_some()
            {
                violations.push(rule(
                    RULE_STATE_IDENTITY,
                    path,
                    "is an active_selection_template but carries pinned candidate identity; a template selects no exact candidate",
                ));
            }
            if json.is_some() && declared_status != Some("active_selection_template") {
                violations.push(rule(
                    RULE_STATE_IDENTITY,
                    path,
                    &format!(
                        "is registered active_selection_template but the artifact declares status `{}`",
                        declared_status.unwrap_or("<none>")
                    ),
                ));
            }
        }
        LifecycleState::PinnedExactCandidate => {
            check_pinned_identity(row, json.as_ref(), violations)
        }
        LifecycleState::HistoricalEvidenceOnly | LifecycleState::Invalid => {}
    }
}

fn check_pinned_identity(row: &ArtifactRow, json: Option<&Value>, violations: &mut Vec<String>) {
    let path = row.path.as_str();
    let candidate = row.candidate.as_ref();
    let sha = candidate.and_then(|c| c.sha.as_deref());
    if !sha.is_some_and(|value| is_hex(value, 40)) {
        violations.push(rule(
            RULE_STATE_IDENTITY,
            path,
            "is pinned_exact_candidate without an exact 40-hex candidate SHA",
        ));
    }
    if !candidate
        .and_then(|c| c.tree.as_deref())
        .is_some_and(|value| is_hex(value, 40))
    {
        violations.push(rule(
            RULE_STATE_IDENTITY,
            path,
            "is pinned_exact_candidate without an exact 40-hex candidate tree",
        ));
    }
    if !candidate
        .and_then(|c| c.git_ref.as_deref())
        .is_some_and(|value| value.starts_with("refs/") && value.len() > "refs/".len())
    {
        violations.push(rule(
            RULE_STATE_IDENTITY,
            path,
            "is pinned_exact_candidate without a fully qualified immutable candidate ref",
        ));
    }
    let packets = row.packets.as_ref();
    for (label, value) in [
        (
            "selected-claim packet",
            packets.and_then(|p| p.selected_claim_packet_sha256.as_deref()),
        ),
        (
            "denominator packet",
            packets.and_then(|p| p.denominator_packet_sha256.as_deref()),
        ),
        (
            "selection template",
            row.selection_template_sha256.as_deref(),
        ),
    ] {
        if !value.is_some_and(|digest| is_hex(digest, 64)) {
            violations.push(rule(
                RULE_STATE_IDENTITY,
                path,
                &format!("is pinned_exact_candidate without a 64-hex {label} sha256"),
            ));
        }
    }
    if let Some(value) = json {
        let status = value.get("status").and_then(Value::as_str);
        if status == Some("active_selection_template") {
            violations.push(rule(
                RULE_STATE_IDENTITY,
                path,
                "is registered pinned_exact_candidate but the artifact is still an active_selection_template",
            ));
        }
        let parent = value.get("selected_swarm_parent").and_then(Value::as_str);
        if sha.is_some() && parent != sha {
            violations.push(rule(
                RULE_STATE_IDENTITY,
                path,
                &format!(
                    "registers candidate SHA `{}` but the artifact's selected_swarm_parent is `{}`",
                    sha.unwrap_or_default(),
                    parent.unwrap_or("<none>")
                ),
            ));
        }
    }
}

fn json_generation(value: &Value) -> Option<String> {
    let kind = value.get("kind").and_then(Value::as_str)?;
    let version = match value.get("schema_version")? {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    Some(format!("{kind}/{version}"))
}

fn check_succession(
    rows: &[ArtifactRow],
    by_path: &BTreeMap<&str, &ArtifactRow>,
    violations: &mut Vec<String>,
) {
    for row in rows.iter().filter(|row| row.projection_of.is_none()) {
        let path = row.path.as_str();
        if !row.state.is_current() {
            if !non_empty(row.reason.as_deref()) {
                violations.push(rule(
                    RULE_SUCCESSION,
                    path,
                    &format!(
                        "is {} without a terminal or invalidation reason",
                        row.state.as_str()
                    ),
                ));
            }
            if row.superseded_by.is_none() && !non_empty(row.successor_route.as_deref()) {
                violations.push(rule(
                    RULE_SUCCESSION,
                    path,
                    &format!(
                        "is {} without superseded_by or a successor_route",
                        row.state.as_str()
                    ),
                ));
            }
        }
        if let Some(successor) = row.superseded_by.as_deref() {
            match by_path.get(successor) {
                None => violations.push(rule(
                    RULE_SUCCESSION,
                    path,
                    &format!("names superseded_by `{successor}`, which is not a registered row"),
                )),
                Some(next) => {
                    if next.path == row.path
                        || next.release != row.release
                        || next.projection_of.is_some()
                        || next.state == LifecycleState::Invalid
                    {
                        violations.push(rule(
                            RULE_SUCCESSION,
                            path,
                            &format!(
                                "names superseded_by `{successor}`, which is not a distinct, valid, same-release authority row"
                            ),
                        ));
                    } else if !next.supersedes.iter().any(|item| item == path) {
                        violations.push(rule(
                            RULE_SUCCESSION,
                            path,
                            &format!("names superseded_by `{successor}`, which does not list it in supersedes"),
                        ));
                    }
                }
            }
        }
        for predecessor in &row.supersedes {
            let back = by_path
                .get(predecessor.as_str())
                .and_then(|prev| prev.superseded_by.as_deref());
            if back != Some(path) {
                violations.push(rule(
                    RULE_SUCCESSION,
                    path,
                    &format!(
                        "lists `{predecessor}` in supersedes, but that row does not name it as superseded_by"
                    ),
                ));
            }
        }
    }
}

fn check_currentness(
    rows: &[ArtifactRow],
    controllers: &[ReleaseController],
    violations: &mut Vec<String>,
) {
    let mut current_by_release: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let digests = rows
        .iter()
        .map(|row| (row.path.as_str(), row.sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    for row in rows
        .iter()
        .filter(|row| row.projection_of.is_none() && row.state.is_current())
    {
        let path = row.path.as_str();
        current_by_release
            .entry(row.release.as_str())
            .or_default()
            .push(path);
        if let Some(successor) = row.superseded_by.as_deref() {
            violations.push(rule(
                RULE_CURRENTNESS,
                path,
                &format!(
                    "is {} but names superseded_by `{successor}`; a superseded row cannot stay current",
                    row.state.as_str()
                ),
            ));
        }
        let controller = controllers
            .iter()
            .find(|controller| controller.version == row.release)
            .and_then(|controller| controller.goal_issue);
        if let Some(controller) = controller
            && u64::from(controller) != row.authority_issue
        {
            violations.push(rule(
                RULE_CURRENTNESS,
                path,
                &format!(
                    "is {} under authority #{}, but release {} is controlled by #{controller}",
                    row.state.as_str(),
                    row.authority_issue,
                    row.release
                ),
            ));
        }
        if row.state == LifecycleState::PinnedExactCandidate
            && let Some(template) = row.selection_template_sha256.as_deref()
            && !row
                .supersedes
                .iter()
                .any(|prev| digests.get(prev.as_str()) == Some(&template))
        {
            violations.push(rule(
                RULE_CURRENTNESS,
                path,
                &format!(
                    "binds selection template sha256 {template}, which matches no row it supersedes; the selection packet is stale or foreign"
                ),
            ));
        }
    }
    for (release, paths) in current_by_release {
        if paths.len() > 1 {
            violations.push(rule(
                RULE_CURRENTNESS,
                release,
                &format!(
                    "has {} current rows ({}); at most one active template or pinned candidate may be current",
                    paths.len(),
                    paths.join(", ")
                ),
            ));
        }
    }
}

fn check_projection_rows(
    rows: &[ArtifactRow],
    by_path: &BTreeMap<&str, &ArtifactRow>,
    violations: &mut Vec<String>,
) {
    for row in rows {
        let Some(source) = row.projection_of.as_deref() else {
            continue;
        };
        let path = row.path.as_str();
        match by_path.get(source) {
            Some(origin) if origin.projection_of.is_none() && origin.release == row.release => {
                if origin.state != row.state {
                    violations.push(rule(
                        RULE_PROJECTION,
                        path,
                        &format!(
                            "is registered `{}` but its source `{source}` is `{}`; a projection cannot disagree with its authority",
                            row.state.as_str(),
                            origin.state.as_str()
                        ),
                    ));
                }
            }
            _ => violations.push(rule(
                RULE_PROJECTION,
                path,
                &format!("names projection_of `{source}`, which is not a registered same-release JSON row"),
            )),
        }
        if !row.supersedes.is_empty()
            || row.superseded_by.is_some()
            || row.reason.is_some()
            || row.successor_route.is_some()
            || row.candidate.is_some()
            || row.packets.is_some()
            || row.selection_template_sha256.is_some()
        {
            violations.push(rule(
                RULE_PROJECTION,
                path,
                "is a projection but carries lifecycle fields; only its source row owns succession and identity",
            ));
        }
    }
}

fn classify_files(tree: &ArtifactTree, registry: &ValidatedRegistry) -> Vec<FileClassification> {
    tree.files
        .iter()
        .filter(|(path, _)| !is_control_file(path))
        .map(|(path, bytes)| {
            let row = registry
                .rows
                .iter()
                .find(|row| row.sha256 == sha256_hex(bytes));
            let permitted_operations = row
                .map(|row| {
                    CandidateOperation::ALL
                        .into_iter()
                        .filter(|operation| {
                            resolve_candidate_authority(registry, &row.release, bytes, *operation)
                                .is_ok()
                        })
                        .map(CandidateOperation::as_str)
                        .collect()
                })
                .unwrap_or_default();
            FileClassification {
                path: path.clone(),
                sha256: sha256_hex(bytes),
                registered_path: row.map(|row| row.path.clone()),
                state: row.map(|row| row.state),
                permitted_operations,
            }
        })
        .collect()
}

fn classify_unresolved(tree: &ArtifactTree) -> Vec<FileClassification> {
    tree.files
        .iter()
        .filter(|(path, _)| !is_control_file(path))
        .map(|(path, bytes)| FileClassification {
            path: path.clone(),
            sha256: sha256_hex(bytes),
            registered_path: None,
            state: None,
            permitted_operations: Vec::new(),
        })
        .collect()
}

fn release_authorities(registry: &ValidatedRegistry) -> Vec<ReleaseAuthority> {
    let releases = registry
        .rows
        .iter()
        .map(|row| row.release.as_str())
        .collect::<BTreeSet<_>>();
    releases
        .into_iter()
        .map(|release| {
            let pick = |state: LifecycleState| {
                registry
                    .rows
                    .iter()
                    .find(|row| {
                        row.release == release && row.projection_of.is_none() && row.state == state
                    })
                    .map(|row| row.path.clone())
            };
            ReleaseAuthority {
                release: release.to_string(),
                selection_rule: pick(LifecycleState::ActiveSelectionTemplate),
                exact_candidate: pick(LifecycleState::PinnedExactCandidate),
            }
        })
        .collect()
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn link(path: &str) -> String {
    let name = file_name(path);
    format!("[`{name}`]({name})")
}

/// Deterministic Markdown projection of the typed rows. It restates the
/// registry and adds no state of its own.
pub(crate) fn render_projection(rows: &[ArtifactRow]) -> String {
    let mut sorted = rows.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        (left.release.as_str(), left.path.as_str())
            .cmp(&(right.release.as_str(), right.path.as_str()))
    });
    let mut out = String::new();
    out.push_str("# Release-candidate artifact registry\n\n");
    out.push_str(
        "<!-- Generated from index.json by `cargo xtask check-release-targets`. Do not edit by hand. -->\n\n",
    );
    out.push_str(
        "[`index.json`](index.json) is the only lifecycle authority for the artifacts in this directory. This page is a projection of it and cannot strengthen any state.\n\n",
    );
    out.push_str(
        "An artifact is current authority only with a registered row, a matching raw-byte SHA-256, a lifecycle state permitted for the requested operation, and every state-specific identity. A filename, version string, \"hard cut\" wording, issue closure, or presence in this directory confers no authority. `historical_evidence_only` artifacts may be cited as history but satisfy no selection, cut, qualification, source-sync, or publication prerequisite. Unregistered files fail `cargo xtask check-release-targets`.\n",
    );
    let mut current_release: Option<&str> = None;
    for row in &sorted {
        if current_release != Some(row.release.as_str()) {
            current_release = Some(row.release.as_str());
            out.push_str(&format!("\n## {}\n\n", row.release));
            out.push_str("| Artifact | State | Authority | Successor | SHA-256 |\n");
            out.push_str("|---|---|---|---|---|\n");
        }
        let successor = match (row.projection_of.as_deref(), row.superseded_by.as_deref()) {
            (Some(source), _) => format!("projection of {}", link(source)),
            (None, Some(next)) => link(next),
            (None, None) => "-".to_string(),
        };
        out.push_str(&format!(
            "| {} | `{}` | #{} | {} | `{}` |\n",
            link(&row.path),
            row.state.as_str(),
            row.authority_issue,
            successor,
            row.sha256
        ));
    }
    let notes = sorted
        .iter()
        .filter(|row| row.reason.is_some() || row.successor_route.is_some())
        .collect::<Vec<_>>();
    if !notes.is_empty() {
        out.push_str("\n## Lifecycle reasons\n\n");
        for row in notes {
            out.push_str(&format!("- `{}`:", file_name(&row.path)));
            if let Some(reason) = row.reason.as_deref() {
                out.push_str(&format!(" {reason}"));
            }
            if let Some(route) = row.successor_route.as_deref() {
                out.push_str(&format!(" Successor route: {route}"));
            }
            out.push('\n');
        }
    }
    out
}

/// Deterministic JSON section for the release-targets report.
pub(crate) fn registry_json(outcome: &CandidateRegistryOutcome) -> Value {
    let files = outcome
        .files
        .iter()
        .map(|file| {
            serde_json::json!({
                "path": file.path,
                "sha256": file.sha256,
                "registered_path": file.registered_path,
                "state": file.state.map(LifecycleState::as_str),
                "permitted_operations": file.permitted_operations,
            })
        })
        .collect::<Vec<_>>();
    let releases = outcome
        .releases
        .iter()
        .map(|release| {
            serde_json::json!({
                "release": release.release,
                "selection_rule": release.selection_rule,
                "exact_candidate": release.exact_candidate,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "registry": REGISTRY_PATH,
        "projection": PROJECTION_PATH,
        "status": outcome.status(),
        "rules": CANDIDATE_RULE_IDS,
        "files": files,
        "current_authority": releases,
        "non_claim": "Lifecycle classification of retained release-candidate artifacts only. It does not select, construct, qualify, sync, tag, or publish a candidate.",
    })
}

#[cfg(test)]
mod tests;
