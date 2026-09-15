use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{RustJudgedPanelItem, RustJudgedPanelManifest};
use crate::run::capture_process_output_isolated;

const SUBJECTS_PATH: &str = "metrics/rust-judged-behavior-panel/subjects.json";
const FIXED_GIT_DATE: &str = "2001-01-01T00:00:00Z";
const GIT_ENV_TO_REMOVE: [&str; 20] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_INDEX_VERSION",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_SHALLOW_FILE",
    "GIT_QUARANTINE_PATH",
    "GIT_REPLACE_REF_BASE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_GRAFT_FILE",
    "GIT_TEMPLATE_DIR",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_EXEC_PATH",
];
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectAuthority {
    schema_version: String,
    kind: String,
    cases: Vec<SubjectCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectCase {
    case_id: String,
    subject_id: String,
    repository: String,
    subject_root: String,
    expected_direction: String,
    anchor_file: String,
    anchor_line: u64,
    owner: String,
    behavior_family: String,
    changed_behavior: String,
    expected_classification: String,
    expected_actionability: String,
    expected_static_limit_kind: Option<String>,
    expected_missing: Vec<String>,
    expected_recommendation: String,
    relation_basis: String,
    oracle_family: String,
    propagation_witness: String,
    cargo_toml: SubjectFile,
    cargo_lock: SubjectFile,
    config: SubjectFile,
    source_before: SubjectFile,
    source_after: SubjectFile,
    tests: Vec<SubjectFile>,
    diff: SubjectFile,
    expected_base: String,
    expected_head: String,
    expected_tree: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectFile {
    source_path: String,
    repository_path: String,
    sha256: String,
}

#[derive(Debug, Eq, PartialEq)]
struct MaterializedIdentity {
    base: String,
    head: String,
    tree: String,
}

#[derive(Clone, Debug)]
pub(super) struct ReplaySubjectFile {
    pub(super) source_path: String,
    pub(super) repository_path: String,
    pub(super) sha256: String,
}

#[derive(Debug)]
pub(super) struct ReplaySubject {
    pub(super) case_id: String,
    pub(super) subject_id: String,
    pub(super) expected_direction: String,
    pub(super) root: PathBuf,
    pub(super) base: String,
    pub(super) head: String,
    pub(super) tree: String,
    pub(super) cargo_toml: ReplaySubjectFile,
    pub(super) cargo_lock: ReplaySubjectFile,
    pub(super) config: ReplaySubjectFile,
    pub(super) source_before: ReplaySubjectFile,
    pub(super) source_after: ReplaySubjectFile,
    pub(super) tests: Vec<ReplaySubjectFile>,
    pub(super) diff: ReplaySubjectFile,
}

#[derive(Clone, Debug)]
pub(super) struct PacketSubject {
    pub(super) case_id: String,
    pub(super) subject_id: String,
    pub(super) repository: String,
    pub(super) expected_direction: String,
    pub(super) anchor_file: String,
    pub(super) anchor_line: u64,
    pub(super) owner: String,
    pub(super) behavior_family: String,
    pub(super) changed_behavior: String,
    pub(super) expected_classification: String,
    pub(super) expected_actionability: String,
    pub(super) expected_static_limit_kind: Option<String>,
    pub(super) expected_missing: Vec<String>,
    pub(super) expected_recommendation: String,
    pub(super) cargo_toml: ReplaySubjectFile,
    pub(super) cargo_lock: ReplaySubjectFile,
    pub(super) config: ReplaySubjectFile,
    pub(super) source_before: ReplaySubjectFile,
    pub(super) source_after: ReplaySubjectFile,
    pub(super) tests: Vec<ReplaySubjectFile>,
    pub(super) diff: ReplaySubjectFile,
    pub(super) expected_base: String,
    pub(super) expected_head: String,
    pub(super) expected_tree: String,
}

pub(super) struct RepositoryState {
    pub(super) head: String,
    pub(super) tree: String,
    pub(super) dirty: bool,
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn validate_at(root: &Path, manifest: &RustJudgedPanelManifest) -> Result<(), String> {
    let authority = load_at(root)?;
    validate_authority(root, manifest, &authority)?;
    let scratch = scratch(root, "canonical")?;
    for case in &authority.cases {
        materialize_subject(root, &scratch.0, case, &[])?;
    }
    Ok(())
}

pub(super) fn materialize_for_replay(
    root: &Path,
    scratch_root: &Path,
    manifest: &RustJudgedPanelManifest,
) -> Result<Vec<ReplaySubject>, String> {
    let authority = load_at(root)?;
    validate_authority(root, manifest, &authority)?;
    authority
        .cases
        .iter()
        .map(|case| {
            let identity = materialize_subject(root, scratch_root, case, &[])?;
            Ok(ReplaySubject {
                case_id: case.case_id.clone(),
                subject_id: case.subject_id.clone(),
                expected_direction: case.expected_direction.clone(),
                root: scratch_root.join(&case.case_id),
                base: identity.base,
                head: identity.head,
                tree: identity.tree,
                cargo_toml: replay_file(&case.cargo_toml),
                cargo_lock: replay_file(&case.cargo_lock),
                config: replay_file(&case.config),
                source_before: replay_file(&case.source_before),
                source_after: replay_file(&case.source_after),
                tests: case.tests.iter().map(replay_file).collect(),
                diff: replay_file(&case.diff),
            })
        })
        .collect()
}

pub(super) fn load_for_packet(
    root: &Path,
    manifest: &RustJudgedPanelManifest,
) -> Result<Vec<PacketSubject>, String> {
    let authority = load_at(root)?;
    validate_authority(root, manifest, &authority)?;
    Ok(authority
        .cases
        .iter()
        .map(|case| PacketSubject {
            case_id: case.case_id.clone(),
            subject_id: case.subject_id.clone(),
            repository: case.repository.clone(),
            expected_direction: case.expected_direction.clone(),
            anchor_file: case.anchor_file.clone(),
            anchor_line: case.anchor_line,
            owner: case.owner.clone(),
            behavior_family: case.behavior_family.clone(),
            changed_behavior: case.changed_behavior.clone(),
            expected_classification: case.expected_classification.clone(),
            expected_actionability: case.expected_actionability.clone(),
            expected_static_limit_kind: case.expected_static_limit_kind.clone(),
            expected_missing: case.expected_missing.clone(),
            expected_recommendation: case.expected_recommendation.clone(),
            cargo_toml: replay_file(&case.cargo_toml),
            cargo_lock: replay_file(&case.cargo_lock),
            config: replay_file(&case.config),
            source_before: replay_file(&case.source_before),
            source_after: replay_file(&case.source_after),
            tests: case.tests.iter().map(replay_file).collect(),
            diff: replay_file(&case.diff),
            expected_base: case.expected_base.clone(),
            expected_head: case.expected_head.clone(),
            expected_tree: case.expected_tree.clone(),
        })
        .collect())
}

pub(super) fn repository_state(root: &Path) -> Result<RepositoryState, String> {
    let head = git(root, &["rev-parse", "HEAD"], &[])?;
    let tree = git(root, &["rev-parse", "HEAD^{tree}"], &[])?;
    let status = git(
        root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
        &[],
    )?;
    Ok(RepositoryState {
        head,
        tree,
        dirty: !status.is_empty(),
    })
}

pub(super) fn executed_diff_identity(root: &Path, base: &str) -> Result<String, String> {
    let range = format!("{base}...HEAD");
    let args = vec![
        "-C".to_string(),
        root.display().to_string(),
        "diff".to_string(),
        "--no-ext-diff".to_string(),
        "--submodule=short".to_string(),
        "--unified=0".to_string(),
        range,
    ];
    let bytes = crate::run::capture_process_output("git", &args, &[])
        .map_err(|error| format!("derive executed diff identity: {}", error.message))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
pub(super) fn materialize_diff_fixture(
    root: &Path,
) -> Result<(String, String, String, String), String> {
    fs::create_dir_all(root).map_err(|error| format!("create diff identity fixture: {error}"))?;
    git(root, &["init", "--quiet", "--object-format=sha1"], &[])?;
    git(root, &["symbolic-ref", "HEAD", "refs/heads/main"], &[])?;
    fs::write(root.join("input.txt"), b"before\n")
        .map_err(|error| format!("write diff identity fixture base: {error}"))?;
    git(root, &["add", "--", "input.txt"], &[])?;
    let base_tree = git(root, &["write-tree"], &[])?;
    let base = git(
        root,
        &["commit-tree", &base_tree, "-m", "diff identity base"],
        &[],
    )?;
    fs::write(root.join("input.txt"), b"after\n")
        .map_err(|error| format!("write diff identity fixture head: {error}"))?;
    git(root, &["add", "--", "input.txt"], &[])?;
    let tree = git(root, &["write-tree"], &[])?;
    let head = git(
        root,
        &[
            "commit-tree",
            &tree,
            "-p",
            &base,
            "-m",
            "diff identity head",
        ],
        &[],
    )?;
    git(root, &["update-ref", "refs/heads/main", &head], &[])?;
    let identity = executed_diff_identity(root, &base)?;
    Ok((base, head, tree, identity))
}

fn replay_file(file: &SubjectFile) -> ReplaySubjectFile {
    ReplaySubjectFile {
        source_path: file.source_path.clone(),
        repository_path: file.repository_path.clone(),
        sha256: file.sha256.clone(),
    }
}

fn load_at(root: &Path) -> Result<SubjectAuthority, String> {
    let path = root.join(SUBJECTS_PATH);
    let body = fs::read_to_string(&path)
        .map_err(|error| format!("read subject authority `{}`: {error}", path.display()))?;
    let value = super::parse_json_without_duplicate_keys(&body)
        .map_err(|error| format!("parse subject authority `{}`: {error}", path.display()))?;
    serde_json::from_value(value)
        .map_err(|error| format!("parse subject authority `{}`: {error}", path.display()))
}

fn validate_authority(
    root: &Path,
    manifest: &RustJudgedPanelManifest,
    authority: &SubjectAuthority,
) -> Result<(), String> {
    let mut violations = Vec::new();
    require(
        &mut violations,
        authority.schema_version == "0.1",
        "subjects.schema_version must be `0.1`",
    );
    require(
        &mut violations,
        authority.kind == "rust_judged_panel_subject_authority",
        "subjects.kind is invalid",
    );
    require(
        &mut violations,
        authority.cases.len() == manifest.items.len(),
        "subjects must own every selected case exactly once",
    );

    let mut seen = BTreeSet::new();
    for case in &authority.cases {
        if !seen.insert(case.case_id.as_str()) {
            violations.push(format!("subjects duplicate case `{}`", case.case_id));
        }
        match manifest.items.iter().find(|item| item.id == case.case_id) {
            Some(item) => validate_manifest_join(case, item, &mut violations),
            None => violations.push(format!(
                "subjects case `{}` is not selected by the manifest",
                case.case_id
            )),
        }
        validate_case_binding(case, &mut violations);
        for file in subject_files(case) {
            if let Err(error) = validate_subject_file(root, case, file) {
                violations.push(error);
            }
        }
        for (label, value) in [
            ("expected_base", &case.expected_base),
            ("expected_head", &case.expected_head),
            ("expected_tree", &case.expected_tree),
        ] {
            require(
                &mut violations,
                is_git_oid(value),
                &format!(
                    "subjects `{}` {label} must be a 40-hex Git object id",
                    case.case_id
                ),
            );
        }
    }

    violations.sort();
    violations.dedup();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Rust judged panel subjects have {} violation(s):\n- {}",
            violations.len(),
            violations.join("\n- ")
        ))
    }
}

fn validate_case_binding(case: &SubjectCase, violations: &mut Vec<String>) {
    let direction = case.expected_direction.replace('_', "-");
    let expected_case_id = format!("rust-{}-{direction}", case.subject_id);
    check_equal(
        violations,
        &format!("subjects `{}` derived case identity", case.case_id),
        &case.case_id,
        &expected_case_id,
    );
    let expected_root = format!(
        "metrics/rust-judged-behavior-panel/subjects/{}/",
        case.subject_id
    );
    check_equal(
        violations,
        &format!("subjects `{}` derived subject root", case.case_id),
        &case.subject_root,
        &expected_root,
    );
}

fn validate_manifest_join(
    case: &SubjectCase,
    item: &RustJudgedPanelItem,
    violations: &mut Vec<String>,
) {
    let label = format!("subjects `{}`", case.case_id);
    for (field, actual, expected) in [
        (
            "repository",
            case.repository.as_str(),
            item.repository.as_str(),
        ),
        (
            "diff path",
            case.diff.source_path.as_str(),
            item.diff_path.as_str(),
        ),
        (
            "expected direction",
            case.expected_direction.as_str(),
            item.expected_direction.as_str(),
        ),
        (
            "anchor file",
            case.anchor_file.as_str(),
            item.anchor.file.as_str(),
        ),
        ("owner", case.owner.as_str(), item.anchor.owner.as_str()),
        (
            "behavior family",
            case.behavior_family.as_str(),
            item.behavior_family.as_str(),
        ),
        (
            "changed behavior",
            case.changed_behavior.as_str(),
            item.anchor.changed_behavior.as_str(),
        ),
        (
            "expected classification",
            case.expected_classification.as_str(),
            item.expected_classification.as_str(),
        ),
        (
            "expected actionability",
            case.expected_actionability.as_str(),
            item.expected_actionability.as_str(),
        ),
        (
            "relation basis",
            case.relation_basis.as_str(),
            item.selection_dimensions.relation_basis.as_str(),
        ),
        (
            "oracle family",
            case.oracle_family.as_str(),
            item.selection_dimensions.oracle_family.as_str(),
        ),
        (
            "propagation witness",
            case.propagation_witness.as_str(),
            item.selection_dimensions.propagation_witness.as_str(),
        ),
    ] {
        check_equal(violations, &format!("{label} {field}"), actual, expected);
    }
    require(
        violations,
        case.anchor_line == item.anchor.line,
        &format!("{label} anchor line does not match the manifest"),
    );
    require(
        violations,
        case.expected_static_limit_kind.as_ref() == item.expected_static_limit_kind.value(),
        &format!("{label} expected static limit does not match the manifest"),
    );
}

fn subject_files(case: &SubjectCase) -> Vec<&SubjectFile> {
    let mut files = vec![
        &case.cargo_toml,
        &case.cargo_lock,
        &case.config,
        &case.source_before,
        &case.source_after,
        &case.diff,
    ];
    files.extend(case.tests.iter());
    files
}

fn validate_subject_file(
    root: &Path,
    case: &SubjectCase,
    file: &SubjectFile,
) -> Result<(), String> {
    safe_relative(&file.source_path)?;
    safe_relative(&file.repository_path)?;
    if !file.source_path.starts_with(&case.subject_root)
        && file.source_path != case.diff.source_path
    {
        return Err(format!(
            "subjects `{}` file `{}` escapes subject root `{}`",
            case.case_id, file.source_path, case.subject_root
        ));
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize repository root `{}`: {error}", root.display()))?;
    let source_path = root.join(&file.source_path);
    let canonical_source = fs::canonicalize(&source_path).map_err(|error| {
        format!(
            "canonicalize subject source `{}`: {error}",
            source_path.display()
        )
    })?;
    if !canonical_source.starts_with(&canonical_root) {
        return Err(format!(
            "subjects `{}` file `{}` escapes the canonical repository root",
            case.case_id, file.source_path
        ));
    }
    if file.source_path != case.diff.source_path {
        let canonical_subject_root =
            fs::canonicalize(root.join(&case.subject_root)).map_err(|error| {
                format!("canonicalize subject root `{}`: {error}", case.subject_root)
            })?;
        if !canonical_source.starts_with(&canonical_subject_root) {
            return Err(format!(
                "subjects `{}` file `{}` escapes the canonical subject root `{}`",
                case.case_id, file.source_path, case.subject_root
            ));
        }
    }
    check_digest(&canonical_source, &file.sha256, &case.case_id)
}

fn materialize_subject(
    root: &Path,
    scratch_root: &Path,
    case: &SubjectCase,
    inherited_git_env: &[(&str, &str)],
) -> Result<MaterializedIdentity, String> {
    let repo = scratch_root.join(&case.case_id);
    fs::create_dir_all(&repo)
        .map_err(|error| format!("create subject repository `{}`: {error}", repo.display()))?;
    for file in [&case.cargo_toml, &case.cargo_lock, &case.config]
        .into_iter()
        .chain(case.tests.iter())
        .chain(std::iter::once(&case.source_before))
    {
        copy_subject_file(root, &repo, file)?;
    }

    git(
        &repo,
        &["init", "--quiet", "--object-format=sha1"],
        inherited_git_env,
    )?;
    git(
        &repo,
        &["symbolic-ref", "HEAD", "refs/heads/main"],
        inherited_git_env,
    )?;
    for file in [&case.cargo_toml, &case.cargo_lock, &case.config]
        .into_iter()
        .chain(case.tests.iter())
        .chain(std::iter::once(&case.source_before))
    {
        index_file(&repo, file, inherited_git_env)?;
    }
    let base_tree = git(&repo, &["write-tree"], inherited_git_env)?;
    let base = git(
        &repo,
        &["commit-tree", &base_tree, "-m", "rust judged panel base"],
        inherited_git_env,
    )?;

    copy_subject_file(root, &repo, &case.diff)?;
    git(
        &repo,
        &[
            "apply",
            "--whitespace=nowarn",
            "--",
            &case.diff.repository_path,
        ],
        inherited_git_env,
    )?;
    fs::remove_file(repo.join(&case.diff.repository_path)).map_err(|error| {
        format!(
            "remove staged diff `{}`: {error}",
            case.diff.repository_path
        )
    })?;
    check_digest(
        &repo.join(&case.source_after.repository_path),
        &case.source_after.sha256,
        &case.case_id,
    )?;
    index_file(&repo, &case.source_after, inherited_git_env)?;
    let tree = git(&repo, &["write-tree"], inherited_git_env)?;
    let head = git(
        &repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            &base,
            "-m",
            "rust judged panel head",
        ],
        inherited_git_env,
    )?;
    git(
        &repo,
        &["update-ref", "refs/heads/main", &head],
        inherited_git_env,
    )?;

    let identity = MaterializedIdentity { base, head, tree };
    let expected = MaterializedIdentity {
        base: case.expected_base.clone(),
        head: case.expected_head.clone(),
        tree: case.expected_tree.clone(),
    };
    if identity != expected {
        return Err(format!(
            "subject `{}` deterministic identity mismatch: expected {expected:?}; actual {identity:?}",
            case.case_id
        ));
    }
    Ok(identity)
}

fn copy_subject_file(root: &Path, repo: &Path, file: &SubjectFile) -> Result<(), String> {
    let destination = repo.join(&file.repository_path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create `{}`: {error}", parent.display()))?;
    }
    fs::copy(root.join(&file.source_path), &destination).map_err(|error| {
        format!(
            "copy `{}` to `{}`: {error}",
            file.source_path,
            destination.display()
        )
    })?;
    Ok(())
}

fn index_file(
    repo: &Path,
    file: &SubjectFile,
    inherited_git_env: &[(&str, &str)],
) -> Result<(), String> {
    let blob = git(
        repo,
        &["hash-object", "-w", "--", &file.repository_path],
        inherited_git_env,
    )?;
    let cache = format!("100644,{blob},{}", file.repository_path);
    git(
        repo,
        &["update-index", "--add", "--cacheinfo", &cache],
        inherited_git_env,
    )
    .map(|_| ())
}

fn git(repo: &Path, args: &[&str], inherited_env: &[(&str, &str)]) -> Result<String, String> {
    let mut owned = vec!["-C".to_string(), repo.display().to_string()];
    owned.extend(args.iter().map(|arg| (*arg).to_string()));
    let null_config = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let envs = [
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_SYSTEM", null_config),
        ("GIT_CONFIG_GLOBAL", null_config),
        ("GIT_CONFIG_COUNT", "0"),
        ("GIT_ATTR_NOSYSTEM", "1"),
        ("GIT_DEFAULT_HASH", "sha1"),
        ("GIT_AUTHOR_NAME", "RIPR Judged Panel"),
        ("GIT_AUTHOR_EMAIL", "panel@example.invalid"),
        ("GIT_AUTHOR_DATE", FIXED_GIT_DATE),
        ("GIT_COMMITTER_NAME", "RIPR Judged Panel"),
        ("GIT_COMMITTER_EMAIL", "panel@example.invalid"),
        ("GIT_COMMITTER_DATE", FIXED_GIT_DATE),
    ];
    let bytes =
        capture_process_output_isolated("git", &owned, inherited_env, &GIT_ENV_TO_REMOVE, &envs)
            .map_err(|error| error.message)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

fn check_digest(path: &Path, expected: &str, case_id: &str) -> Result<(), String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("read digest input `{}`: {error}", path.display()))?;
    let actual = format!("sha256:{:x}", Sha256::digest(&bytes));
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "subjects `{case_id}` digest mismatch for `{}`: expected {expected}, got {actual}",
            path.display()
        ))
    }
}

fn scratch(root: &Path, label: &str) -> Result<Scratch, String> {
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = root
        .join("target/ripr/rust-judged-panel-subjects")
        .join(format!("{label}-{}-{sequence}", std::process::id()));
    if path.exists() {
        return Err(format!(
            "subject scratch already exists: `{}`",
            path.display()
        ));
    }
    fs::create_dir_all(&path)
        .map_err(|error| format!("create subject scratch `{}`: {error}", path.display()))?;
    Ok(Scratch(path))
}

fn safe_relative(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "path `{value}` must be normalized repository-relative text"
        ));
    }
    Ok(())
}

fn require(violations: &mut Vec<String>, condition: bool, message: &str) {
    if !condition {
        violations.push(message.to_string());
    }
}

fn check_equal(violations: &mut Vec<String>, label: &str, actual: &str, expected: &str) {
    if actual != expected {
        violations.push(format!("{label}: expected `{expected}`, got `{actual}`"));
    }
}

fn is_git_oid(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;
    use sha2::Digest;

    use super::{load_at, materialize_subject, scratch, validate_at};

    fn repository_root() -> Result<PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "xtask manifest must have a repository parent".to_string())
    }

    fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
        fs::create_dir_all(destination).map_err(|error| error.to_string())?;
        for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let target = destination.join(entry.file_name());
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                copy_tree(&entry.path(), &target)?;
            } else {
                fs::copy(entry.path(), target).map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    #[test]
    fn canonical_subjects_materialize_with_stable_identities_after_relocation() -> Result<(), String>
    {
        let root = repository_root()?;
        let authority = load_at(&root)?;
        let first = scratch(&root, "relocation-a")?;
        let second = scratch(&root, "relocation-b")?;
        for case in &authority.cases {
            let left = materialize_subject(&root, &first.0, case, &[])?;
            let right = materialize_subject(&root, &second.0, case, &[])?;
            if left != right {
                return Err(format!(
                    "subject `{}` changed after relocation",
                    case.case_id
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn hostile_git_environment_cannot_change_subject_identity() -> Result<(), String> {
        let root = repository_root()?;
        let authority = load_at(&root)?;
        let scratch = scratch(&root, "hostile-git")?;
        let hostile = scratch.0.join("hostile.gitconfig");
        fs::write(
            &hostile,
            "[init]\n\tdefaultObjectFormat = sha256\n[core]\n\tautocrlf = true\n",
        )
        .map_err(|error| error.to_string())?;
        let hostile_text = hostile.display().to_string();
        let external = scratch.0.join("external").display().to_string();
        let env = [
            ("GIT_DIR", external.as_str()),
            ("GIT_WORK_TREE", external.as_str()),
            ("GIT_INDEX_FILE", external.as_str()),
            ("GIT_OBJECT_DIRECTORY", external.as_str()),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", external.as_str()),
            ("GIT_COMMON_DIR", external.as_str()),
            ("GIT_TEMPLATE_DIR", external.as_str()),
            ("GIT_CONFIG_PARAMETERS", "malformed"),
            ("GIT_CONFIG_GLOBAL", hostile_text.as_str()),
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "init.defaultObjectFormat"),
            ("GIT_CONFIG_VALUE_0", "sha256"),
        ];
        for case in &authority.cases {
            materialize_subject(&root, &scratch.0, case, &env)?;
        }
        Ok(())
    }

    #[test]
    fn changed_governed_byte_is_rejected_before_materialization() -> Result<(), String> {
        let canonical = repository_root()?;
        let scratch = scratch(&canonical, "changed-byte")?;
        let root = scratch.0.join("repo");
        copy_tree(
            &canonical.join("metrics/rust-judged-behavior-panel"),
            &root.join("metrics/rust-judged-behavior-panel"),
        )?;
        let changed = root.join(
            "metrics/rust-judged-behavior-panel/subjects/boundary-missing-equality/source.before.rs",
        );
        fs::write(&changed, "pub fn changed() {}\n").map_err(|error| error.to_string())?;
        let manifest =
            super::super::load_and_validate_at(&root, Path::new(super::super::MANIFEST_PATH))?;
        let error = validate_at(&root, &manifest)
            .err()
            .ok_or_else(|| "changed governed byte was accepted".to_string())?;
        if error.contains("digest mismatch") {
            Ok(())
        } else {
            Err(format!("unexpected changed-byte error: {error}"))
        }
    }

    #[test]
    fn resealed_changed_byte_is_rejected_by_git_identity() -> Result<(), String> {
        let canonical = repository_root()?;
        let scratch = scratch(&canonical, "resealed-byte")?;
        let root = scratch.0.join("repo");
        copy_tree(
            &canonical.join("metrics/rust-judged-behavior-panel"),
            &root.join("metrics/rust-judged-behavior-panel"),
        )?;
        let changed = root.join(
            "metrics/rust-judged-behavior-panel/subjects/boundary-missing-equality/source.before.rs",
        );
        let changed_bytes = b"pub fn discounted_total(_: i32, _: i32) -> i32 { 0 }\n";
        fs::write(&changed, changed_bytes).map_err(|error| error.to_string())?;
        let path = root.join("metrics/rust-judged-behavior-panel/subjects.json");
        let mut value: Value =
            serde_json::from_str(&fs::read_to_string(&path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        value["cases"][0]["source_before"]["sha256"] =
            serde_json::json!(format!("sha256:{:x}", sha2::Sha256::digest(changed_bytes)));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let manifest =
            super::super::load_and_validate_at(&root, Path::new(super::super::MANIFEST_PATH))?;
        let error = validate_at(&root, &manifest)
            .err()
            .ok_or_else(|| "resealed governed byte was accepted".to_string())?;
        if error.contains("failed") || error.contains("identity mismatch") {
            Ok(())
        } else {
            Err(format!("unexpected resealed-byte error: {error}"))
        }
    }

    #[test]
    fn complete_wrong_subject_substitution_fails_independent_binding() -> Result<(), String> {
        let canonical = repository_root()?;
        let scratch = scratch(&canonical, "swapped-subject")?;
        let root = scratch.0.join("repo");
        copy_tree(
            &canonical.join("metrics/rust-judged-behavior-panel"),
            &root.join("metrics/rust-judged-behavior-panel"),
        )?;
        let path = root.join("metrics/rust-judged-behavior-panel/subjects.json");
        let mut value: Value =
            serde_json::from_str(&fs::read_to_string(&path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        let cases = value
            .get_mut("cases")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "subjects cases missing".to_string())?;
        let quiet = cases
            .get(1)
            .and_then(Value::as_object)
            .cloned()
            .ok_or_else(|| "quiet subject missing".to_string())?;
        let gap = cases
            .get_mut(0)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "gap case missing".to_string())?;
        for field in [
            "subject_id",
            "subject_root",
            "cargo_toml",
            "cargo_lock",
            "config",
            "source_before",
            "source_after",
            "tests",
            "expected_base",
            "expected_head",
            "expected_tree",
        ] {
            let replacement = quiet
                .get(field)
                .cloned()
                .ok_or_else(|| format!("quiet subject missing `{field}`"))?;
            gap.insert(field.to_string(), replacement);
        }
        fs::write(
            &path,
            serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let manifest =
            super::super::load_and_validate_at(&root, Path::new(super::super::MANIFEST_PATH))?;
        let error = validate_at(&root, &manifest)
            .err()
            .ok_or_else(|| "complete wrong-subject substitution was accepted".to_string())?;
        if error.contains("derived case identity") {
            Ok(())
        } else {
            Err(format!("unexpected wrong-subject error: {error}"))
        }
    }

    #[test]
    fn canonical_check_reaches_subject_authority() -> Result<(), String> {
        let canonical = repository_root()?;
        let scratch = scratch(&canonical, "route")?;
        let root = scratch.0.join("repo");
        copy_tree(
            &canonical.join("metrics/rust-judged-behavior-panel"),
            &root.join("metrics/rust-judged-behavior-panel"),
        )?;
        fs::remove_file(root.join(super::SUBJECTS_PATH)).map_err(|error| error.to_string())?;
        let error = super::super::check_at(&root)
            .err()
            .ok_or_else(|| "canonical check bypassed missing subjects".to_string())?;
        if error.contains("read subject authority") {
            Ok(())
        } else {
            Err(format!("unexpected route error: {error}"))
        }
    }
}
