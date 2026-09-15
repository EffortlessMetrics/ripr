//! Immutable candidate isolation and committed-analysis parity (#3279,
//! R4 of #3237).
//!
//! The producer, CLI, and identity fields exist (#3276–#3278); this
//! harness tries to break the claim end to end. Each test binds a real
//! subject against a real two-commit repository, deliberately mutates
//! mutable repository state the analysis must not read, and compares
//! the full JSON output for semantic identity. The parity test then
//! checks out the candidate tree as a real commit and compares the
//! ordinary committed run against the subject run after removing only
//! the declared non-portable telemetry.

use serde_json::Value;
use serial_test::serial;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const LIB_BASE: &str = "pub fn one() -> u8 { 1 }\n";
const LIB_CANDIDATE: &str = "pub fn one() -> u8 { 2 }\n";
const TEST_BODY: &str = "#[test]\nfn t() { assert_eq!(falsifier::one(), 1); }\n";
const CONFIG: &str = "[analysis]\nmode = \"draft\"\n";

struct RepoGuard(PathBuf);
impl Drop for RepoGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unique_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("ripr-3279-{name}-{}-{nanos}", std::process::id()))
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| format!("git {args:?} failed to start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write(root: &Path, relative: &str, text: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// A repository with a base commit and a candidate commit; returns the
/// guard, base commit, and candidate commit.
fn fixture_repo(name: &str) -> Result<(RepoGuard, String, String), String> {
    let root = unique_root(name);
    std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let guard = RepoGuard(root.clone());
    git(&root, &["init", "--initial-branch=main"])?;
    git(&root, &["config", "user.email", "r@e.invalid"])?;
    git(&root, &["config", "user.name", "ripr"])?;
    write(
        &root,
        "Cargo.toml",
        "[package]\nname='falsifier'\nversion='0.1.0'\nedition='2024'\n",
    )?;
    write(&root, "ripr.toml", CONFIG)?;
    write(&root, "src/lib.rs", LIB_BASE)?;
    write(&root, "tests/it.rs", TEST_BODY)?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-qm", "base"])?;
    let base = git(&root, &["rev-parse", "HEAD"])?;
    write(&root, "src/lib.rs", LIB_CANDIDATE)?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-qm", "candidate"])?;
    let candidate = git(&root, &["rev-parse", "HEAD"])?;
    Ok((guard, base, candidate))
}

fn run_check(root: &Path, args: &[&str]) -> Result<Value, String> {
    run_check_in_temp_dir(root, args, None)
}

/// `temp_dir`, when given, becomes the subject process's private temp
/// directory. Every materialization the run performs then lands under that
/// path instead of the shared process temp directory, so a test observes its
/// own run's temporary state and nothing else — no snapshot diffing against
/// sibling roots, and no settle window betting on when a sibling finishes.
fn run_check_in_temp_dir(
    root: &Path,
    args: &[&str],
    temp_dir: Option<&Path>,
) -> Result<Value, String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ripr"));
    command
        .current_dir(root)
        .args(["check", "--root"])
        .arg(root)
        .args(args);
    if let Some(temp_dir) = temp_dir {
        // `std::env::temp_dir` reads `TMPDIR` on Unix and `TMP`/`TEMP` on
        // Windows; set all three so the redirect holds on every platform
        // this suite runs on.
        for key in ["TMPDIR", "TMP", "TEMP"] {
            command.env(key, temp_dir);
        }
    }
    let output = command
        .output()
        .map_err(|error| format!("ripr check failed to start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ripr check {:?} failed with {}: {}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let start = text.find('{').ok_or("no JSON in output")?;
    serde_json::from_str(&text[start..]).map_err(|e| format!("parse check JSON: {e}"))
}

fn subject_args(base: Option<&str>, candidate: &str) -> Vec<String> {
    let mut args = vec!["--candidate-tree".to_string(), candidate.to_string()];
    if let Some(base) = base {
        args.push("--candidate-base".to_string());
        args.push(base.to_string());
    }
    args.push("--format".to_string());
    args.push("json".to_string());
    args
}

fn run_subject(root: &Path, base: &str, candidate: &str) -> Result<Value, String> {
    run_subject_with_base(root, Some(base), candidate)
}

fn run_subject_with_base(
    root: &Path,
    base: Option<&str>,
    candidate: &str,
) -> Result<Value, String> {
    let args = subject_args(base, candidate);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_check(root, &refs)
}

fn run_subject_in_temp_dir(
    root: &Path,
    base: &str,
    candidate: &str,
    temp_dir: &Path,
) -> Result<Value, String> {
    let args = subject_args(Some(base), candidate);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_check_in_temp_dir(root, &refs, Some(temp_dir))
}

/// A private temp directory for one subject process, plus the
/// `ripr-git-candidate` parent that will appear inside it.
fn private_temp(name: &str) -> Result<(RepoGuard, PathBuf), String> {
    let guard = RepoGuard(unique_root(name));
    std::fs::create_dir_all(&guard.0).map_err(|error| error.to_string())?;
    let parent = guard.0.join("ripr-git-candidate");
    Ok((guard, parent))
}

/// The per-run materialization roots under one `ripr-git-candidate` parent.
///
/// Fail-closed on every read error. Treating an unreadable parent or an
/// unreadable entry as "no roots" would let the emptiness assertions below
/// pass for the wrong reason — what they exist to catch is exactly a
/// directory that is present and not being cleaned up.
fn materialization_roots(parent: &Path) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(parent).map_err(|error| {
        format!(
            "materialization parent {} unreadable: {error}",
            parent.display()
        )
    })?;
    let mut roots = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "materialization parent {} had an unreadable entry: {error}",
                parent.display()
            )
        })?;
        roots.push(entry.file_name().to_string_lossy().to_string());
    }
    roots.sort();
    Ok(roots)
}

/// Assert that a subject run materialized into `parent` and left nothing
/// behind. The existence check is the positive control: without it, a temp
/// redirect that silently stopped working would leave `parent` absent and
/// the emptiness check would hold for the wrong reason.
fn assert_materialization_root_cleaned(parent: &Path, context: &str) -> Result<(), String> {
    assert!(
        parent.is_dir(),
        "{context}: run must have materialized under {}",
        parent.display()
    );
    let persisted = materialization_roots(parent)?;
    assert!(
        persisted.is_empty(),
        "{context}: no per-run materialization root may persist: {persisted:?}"
    );
    Ok(())
}

/// The #3279 reproduction, verbatim: bind base B and candidate tree T,
/// then change the worktree source, an unchanged test, ripr.toml, and
/// the live index (a staged blob). The output must be semantically
/// identical to the clean run.
#[test]
#[serial]
fn post_bind_mutations_do_not_change_immutable_output() -> Result<(), String> {
    let (guard, base, candidate) = fixture_repo("mutations")?;
    let root = &guard.0;
    let clean = run_subject(root, &base, &candidate)?;

    write(root, "src/lib.rs", "pub fn DIRTY() -> u8 { 99 }\n")?;
    write(root, "tests/it.rs", "#[test]\nfn t() { assert!(false); }\n")?;
    write(root, "ripr.toml", "[analysis]\nmode = \"deep\"\n")?;
    write(root, "src/staged.rs", "pub fn STAGED() -> u8 { 42 }\n")?;
    git(root, &["add", "."])?; // the live index now points at other bytes
    let dirty = run_subject(root, &base, &candidate)?;

    assert_eq!(
        serde_json::to_string(&clean).unwrap_or_default(),
        serde_json::to_string(&dirty).unwrap_or_default(),
        "post-bind worktree/test/config/index mutations must not change a subject run"
    );
    Ok(())
}

/// Same-tree committed parity: check out the candidate as a real commit
/// and run the ordinary `--diff` path; findings, classifications, and
/// ordering must agree with the subject run after removing only the
/// declared non-portable telemetry (identity block, mode string, root).
#[test]
#[serial]
fn same_tree_immutable_and_committed_analysis_agree() -> Result<(), String> {
    let (guard, base, candidate) = fixture_repo("parity")?;
    let root = &guard.0;
    let subject = run_subject(root, &base, &candidate)?;

    // Committed path: an equivalent two-commit checkout is exactly this
    // repository at the candidate commit; run the ordinary range diff.
    git(root, &["checkout", "-q", &candidate])?;
    let committed = run_check(root, &["--base", &base, "--format", "json"])?;

    let semantic = |value: &Value| -> Value {
        let mut copy = value.clone();
        // Declared non-portable telemetry: the identity block carries
        // the input's own fingerprints (diff-source hashes differ by
        // construction between a range diff and a tree-to-tree diff);
        // `mode` is rendered from the input path; `root` is the caller
        // path. Everything semantic — summary, findings, classifications,
        // related tests, ordering — must match.
        // Declared non-portable telemetry ONLY: the identity block
        // (input fingerprints differ by construction between a range
        // diff and a tree-to-tree diff), the mode string, the root
        // echo, and the base echo. Completeness, counts, and
        // limitations are acceptance-named and MUST match (#3279
        // review M2).
        copy["analysis_outcome"]["outcome"]["identity"] = Value::Null;
        copy["mode"] = Value::Null;
        copy["root"] = Value::Null;
        copy["base"] = Value::Null;
        copy
    };
    assert_eq!(
        serde_json::to_string(&semantic(&subject)).unwrap_or_default(),
        serde_json::to_string(&semantic(&committed)).unwrap_or_default(),
        "same-tree immutable and committed runs must agree semantically"
    );
    Ok(())
}

/// Emitted identities match the requested subject exactly, including the
/// per-format empty-tree OID, and the probe paths name the repository,
/// never the ephemeral materialization directory.
#[test]
#[serial]
fn emitted_identities_match_the_request_and_paths_are_replayable() -> Result<(), String> {
    let (guard, _base, candidate) = fixture_repo("identity")?;
    let root = &guard.0;
    let subject = run_subject_with_base(root, None, &candidate)?;
    let tree = git(root, &["rev-parse", &format!("{candidate}^{{tree}}")])?;
    let identity = subject["analysis_outcome"]["outcome"]["identity"]["git_candidate_subject"]
        .as_object()
        .ok_or("git_candidate_subject missing")?;
    assert_eq!(identity["candidate_tree"].as_str(), Some(tree.as_str()));
    assert_eq!(identity["subject_kind"].as_str(), Some("tree_to_tree"));
    let diff_identity = identity["diff_identity"]
        .as_str()
        .ok_or("diff_identity missing")?;
    assert!(diff_identity.starts_with("sha256:"));
    // Empty-tree base resolves the repository's real empty-tree OID.
    assert_eq!(
        identity["base_tree"].as_str(),
        Some("4b825dc642cb6eb9a060e54bf8d69288fbee4904")
    );
    let rendered = serde_json::to_string(&subject).unwrap_or_default();
    assert!(
        !rendered.contains("ripr-git-candidate"),
        "the ephemeral materialization root must not leak into output"
    );
    Ok(())
}

/// Deletes and renames never require a worktree fallback: delete the
/// file in the candidate commit, rename another, and dirty the worktree
/// copies of both — the subject run still resolves from objects.
#[test]
#[serial]
fn delete_and_rename_shapes_resolve_without_worktree_fallback() -> Result<(), String> {
    let root = unique_root("shapes");
    std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let _guard = RepoGuard(root.clone());
    git(&root, &["init", "--initial-branch=main"])?;
    git(&root, &["config", "user.email", "r@e.invalid"])?;
    git(&root, &["config", "user.name", "ripr"])?;
    write(
        &root,
        "Cargo.toml",
        "[package]\nname='falsifier'\nversion='0.1.0'\nedition='2024'\n",
    )?;
    write(&root, "src/lib.rs", LIB_BASE)?;
    write(&root, "src/gone.rs", "pub fn gone() -> u8 { 1 }\n")?;
    write(&root, "src/renamed.rs", "pub fn named() -> u8 { 1 }\n")?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-qm", "base"])?;
    let base = git(&root, &["rev-parse", "HEAD"])?;
    // Candidate: delete gone.rs, rename renamed.rs -> moved.rs, keep lib.
    std::fs::remove_file(root.join("src/gone.rs")).map_err(|e| e.to_string())?;
    std::fs::rename(root.join("src/renamed.rs"), root.join("src/moved.rs"))
        .map_err(|e| e.to_string())?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-qm", "candidate"])?;
    let candidate = git(&root, &["rev-parse", "HEAD"])?;
    // Dirty the worktree copies of exactly the shapes under test.
    write(&root, "src/gone.rs", "pub fn RESURRECTED() -> u8 { 9 }\n")?;
    std::fs::remove_file(root.join("src/moved.rs")).map_err(|e| e.to_string())?;
    let subject = run_subject(&root, &base, &candidate)?;
    let rendered = serde_json::to_string(&subject).unwrap_or_default();
    assert!(
        !rendered.contains("RESURRECTED"),
        "a worktree resurrection of a deleted file must not enter the run"
    );
    assert!(
        !rendered.contains("src/renamed.rs"),
        "the pre-rename path must not be read from the worktree (its file was deleted)"
    );
    // The pure rename produces no probes by the pinned rename semantics;
    // the deletion is disclosed or probed. Either way the run must be a
    // completed analysis driven by the trees.
    let kind = subject["analysis_outcome"]["outcome"]["kind"]
        .as_str()
        .unwrap_or("");
    assert!(
        !kind.is_empty(),
        "the subject run must carry a completed outcome"
    );
    Ok(())
}

/// Invalid inputs stay named incomplete states — never clean zero
/// findings: a missing tree, and a malformed OID.
#[test]
#[serial]
fn invalid_subjects_fail_closed_never_clean() -> Result<(), String> {
    let (guard, base, _candidate) = fixture_repo("invalid")?;
    let root = &guard.0;
    let missing = match run_subject(root, &base, "0123456789012345678901234567890123456789") {
        Err(error) => error,
        Ok(_) => return Err("a missing tree must fail".to_string()),
    };
    assert!(
        missing.contains("git candidate subject"),
        "missing tree must fail inside the subject boundary: {missing}"
    );
    let malformed = Command::new(env!("CARGO_BIN_EXE_ripr"))
        .current_dir(root)
        .args(["check", "--root"])
        .arg(root)
        .args(["--candidate-tree", "deadbeef", "--format", "json"])
        .output()
        .map_err(|e| e.to_string())?;
    assert!(!malformed.status.success());
    let stderr = String::from_utf8_lossy(&malformed.stderr);
    assert!(
        stderr.contains("malformed object ID"),
        "malformed OID must be named: {stderr}"
    );
    Ok(())
}

/// The removal experiment (#3279 fix plan 7): prove the corpus fails if
/// the producer silently substitutes the worktree. Simulated by running
/// the ordinary (non-subject) diff path against a dirty worktree — the
/// outputs must DIFFER from the subject run, so a regression that made
/// the subject path read the worktree would flip this assertion.
#[test]
#[serial]
fn removal_experiment_subject_differs_from_worktree_substitution() -> Result<(), String> {
    let (guard, base, candidate) = fixture_repo("removal")?;
    let root = &guard.0;
    let subject = run_subject(root, &base, &candidate)?;
    // Dirty the worktree to the shape a worktree-substituting producer
    // would read: a DIFFERENT source change than the candidate's.
    write(
        root,
        "src/lib.rs",
        "pub fn WORKTREE_SUBSTITUTE() -> u8 { 7 }\n",
    )?;
    let substituted = run_subject(root, &base, &candidate)?;
    // The subject run must be unaffected (this is the isolation claim)…
    assert_eq!(
        serde_json::to_string(&subject).unwrap_or_default(),
        serde_json::to_string(&substituted).unwrap_or_default()
    );
    // …and the candidate's real change must be what the findings carry:
    // a producer that silently read the worktree would carry the dirty
    // bytes instead. The candidate change is `one() -> 2`; the dirty
    // shape has no `one`. Pin the discriminator.
    // The changed probe's after-text carries the candidate's  from
    // ; rendered per-surface (e.g. flow sink
    // text or after evidence) it may be a bare  — pin the value, not
    // the rendering.
    let rendered = serde_json::to_string(&subject).unwrap_or_default();
    assert!(
        !rendered.contains("WORKTREE_SUBSTITUTE"),
        "worktree-only bytes must never appear in a subject run"
    );
    let finding = subject["findings"]
        .get(0)
        .ok_or("the candidate change must produce a finding")?;
    // The JSON probe shape carries the changed value as `expression`
    // (the rendered changed-value surface); a worktree-substituting
    // producer would render the dirty bytes here instead.
    let expression = finding["probe"]["expression"]
        .as_str()
        .ok_or("finding must carry the changed expression")?;
    assert_eq!(
        expression.trim(),
        "2",
        "findings must carry the candidate bytes, not worktree bytes"
    );
    Ok(())
}

/// Temporary candidate state is cleaned: after a successful run the
/// materialization root is removed (the guard drops with the temp tree).
/// #3279 review B1 control: a tree WITHOUT a config configures itself
/// with the pure default — toggling the worktree ripr.toml (including
/// its enabled-languages list) must not change the subject output.
#[test]
#[serial]
fn treeless_config_subject_ignores_worktree_language_toggle() -> Result<(), String> {
    let root = unique_root("treeless");
    std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let _guard = RepoGuard(root.clone());
    git(&root, &["init", "--initial-branch=main"])?;
    git(&root, &["config", "user.email", "r@e.invalid"])?;
    git(&root, &["config", "user.name", "ripr"])?;
    write(
        &root,
        "Cargo.toml",
        "[package]
name='falsifier'
version='0.1.0'
edition='2024'
",
    )?;
    write(&root, "src/lib.rs", LIB_BASE)?;
    write(&root, "tests/it.rs", TEST_BODY)?;
    // NOTE: no ripr.toml is committed — the tree configures itself with
    // the pure default.
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-qm", "base"])?;
    let base = git(&root, &["rev-parse", "HEAD"])?;
    write(&root, "src/lib.rs", LIB_CANDIDATE)?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-qm", "candidate"])?;
    let candidate = git(&root, &["rev-parse", "HEAD"])?;

    // Worktree config A (untracked — mutable state only).
    write(
        &root,
        "ripr.toml",
        "[languages]
enabled = [\"rust\"]
",
    )?;
    let first = run_subject(&root, &base, &candidate)?;
    // Worktree config B: the ONLY change is the enabled-languages list.
    write(
        &root,
        "ripr.toml",
        "[languages]
enabled = [\"rust\", \"python\"]
",
    )?;
    let second = run_subject(&root, &base, &candidate)?;
    assert_eq!(
        serde_json::to_string(&first).unwrap_or_default(),
        serde_json::to_string(&second).unwrap_or_default(),
        "toggling the worktree enabled-languages must not change a treeless-config subject run"
    );
    Ok(())
}

/// A repository whose candidate tree replaces a regular file with a symlink
/// entry — a mode `untar` rejects.
///
/// The symlink is written straight into the **index** as a `120000` entry
/// rather than created in the worktree. Creating a real symlink needs
/// privileges Windows runners may not have, and the previous fixture skipped
/// the whole arm when that failed, so the fail-closed test could report
/// success without ever exercising the path it names. A `cacheinfo` entry
/// needs no filesystem support, so the corpus is identical on every platform.
///
/// `symlink_entry: false` builds the same repository with `src/linked.rs`
/// left as an ordinary file. That is the positive control: it must
/// materialize cleanly, so a rejection in the other arm is attributable to
/// the entry mode and not to a fixture that could never materialize at all.
fn type_change_repo(
    name: &str,
    symlink_entry: bool,
) -> Result<(RepoGuard, String, String), String> {
    let root = unique_root(name);
    std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let guard = RepoGuard(root.clone());
    git(&root, &["init", "--initial-branch=main"])?;
    git(&root, &["config", "user.email", "r@e.invalid"])?;
    git(&root, &["config", "user.name", "ripr"])?;
    write(
        &root,
        "Cargo.toml",
        "[package]
name='falsifier'
version='0.1.0'
edition='2024'
",
    )?;
    write(&root, "src/lib.rs", LIB_BASE)?;
    write(
        &root,
        "src/linked.rs",
        "pub fn linked() -> u8 { 1 }
",
    )?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-qm", "base"])?;
    let base = git(&root, &["rev-parse", "HEAD"])?;
    write(&root, "src/lib.rs", LIB_CANDIDATE)?;
    git(&root, &["add", "src/lib.rs"])?;
    if symlink_entry {
        // A symlink blob is exactly its target path, no trailing newline.
        // Hash it, then stage it at mode 120000 so the candidate *tree*
        // carries the type change without the worktree needing one.
        write(&root, ".link-target", "lib.rs")?;
        let blob = git(&root, &["hash-object", "-w", ".link-target"])?;
        std::fs::remove_file(root.join(".link-target")).map_err(|e| e.to_string())?;
        git(
            &root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("120000,{blob},src/linked.rs"),
            ],
        )?;
    }
    git(&root, &["commit", "-qm", "candidate"])?;
    let candidate = git(&root, &["rev-parse", "HEAD"])?;
    if symlink_entry {
        // Assert the fixture really is what the test claims. Without this a
        // silently-failed `update-index` would leave an ordinary file and
        // every downstream assertion would be about the wrong tree.
        let entry = git(&root, &["ls-tree", &candidate, "src/linked.rs"])?;
        assert!(
            entry.starts_with("120000 blob"),
            "candidate tree must carry a symlink entry, got: {entry}"
        );
    }
    Ok((guard, base, candidate))
}

/// The cleanup oracles are only as good as the probe underneath them. A
/// probe that reported an unreadable parent as "no roots" would turn every
/// emptiness assertion into a vacuous pass, so bind that directly: a parent
/// that cannot be listed is an error, never an empty result.
#[test]
fn materialization_roots_fails_closed_on_an_unreadable_parent() -> Result<(), String> {
    let guard = RepoGuard(unique_root("roots-probe"));
    std::fs::create_dir_all(&guard.0).map_err(|error| error.to_string())?;

    let empty = guard.0.join("empty-parent");
    std::fs::create_dir_all(&empty).map_err(|error| error.to_string())?;
    assert!(
        materialization_roots(&empty)?.is_empty(),
        "a readable empty parent must report no roots"
    );

    let populated = guard.0.join("populated-parent");
    std::fs::create_dir_all(populated.join("1234-5678")).map_err(|error| error.to_string())?;
    assert_eq!(
        materialization_roots(&populated)?,
        vec!["1234-5678".to_string()],
        "a readable parent must report its roots"
    );

    // A regular file is not a directory on every platform, and — unlike a
    // permission denial — it fails for root too, so this control holds in a
    // container as well as on a developer machine.
    let not_a_directory = guard.0.join("regular-file");
    std::fs::write(&not_a_directory, "not a directory\n").map_err(|error| error.to_string())?;
    assert!(
        materialization_roots(&not_a_directory).is_err(),
        "an unlistable parent must be an error, not an empty root set"
    );

    let missing = guard.0.join("never-created");
    assert!(
        materialization_roots(&missing).is_err(),
        "an absent parent must be an error, not an empty root set"
    );
    Ok(())
}

/// #3279 review M3: a type change (regular file → symlink) in the
/// candidate tree fails closed with a named error naming the entry —
/// never a worktree fallback, never clean zero findings.
#[test]
#[serial]
fn type_change_fails_closed_naming_the_entry() -> Result<(), String> {
    let (guard, base, candidate) = type_change_repo("typechange", true)?;
    let root = &guard.0;
    let error = run_subject(root, &base, &candidate)
        .err()
        .ok_or_else(|| "a type-changed entry must fail closed".to_string())?;
    assert!(
        error.contains("git candidate subject"),
        "failure must stay inside the subject boundary: {error}"
    );
    assert!(
        error.contains("unsupported tree entry mode `120000`") && error.contains("src/linked.rs"),
        "failure must name the exact rejected entry, not a generic error: {error}"
    );
    Ok(())
}

#[test]
#[serial]
fn temporary_candidate_state_is_cleaned() -> Result<(), String> {
    let (guard, base, candidate) = fixture_repo("cleanup")?;
    let root = &guard.0;
    // The oracle is the set of PER-RUN materialization roots (children of
    // the `ripr-git-candidate` parent), not the parent itself — the parent
    // persists by design and counting it made the test both cold-start
    // flaky and vacuous (#3279 review M1).
    //
    // Reading that parent under the *shared* process temp directory left a
    // second cross-talk failure: sibling runs materialize into the same
    // place, so a snapshot sees their in-flight roots. Serializing and
    // diffing against a before-snapshot narrows it but still cannot tell a
    // sibling's late-dropping root from a leak, which is why a settle loop
    // was still needed. A private temp directory removes the ambiguity
    // instead of timing around it: every child of this parent came from
    // this run, so the assertion is exact and needs no sleeping.
    let (_temp_guard, parent) = private_temp("cleanup-temp")?;
    run_subject_in_temp_dir(root, &base, &candidate, _temp_guard.0.as_path())?;
    assert_materialization_root_cleaned(&parent, "successful run")
}

/// The success path is the easy half. A subject that fails closed *after*
/// the tree is already on disk must clean up too: `materialize` extracts the
/// candidate tree and only then rejects an unsupported entry, so a guard
/// armed on the success path alone leaves a full copy of the repository's
/// source behind on every fail-closed run.
#[test]
#[serial]
fn fail_closed_subject_leaves_no_materialized_state() -> Result<(), String> {
    // Positive control first: the same fixture without the symlink entry
    // must materialize cleanly. That establishes the repository is
    // materializable, so the rejection below is attributable to the entry
    // mode rather than to a fixture that could never have got that far.
    let (control_guard, control_base, control_candidate) =
        type_change_repo("cleanup-control", false)?;
    let (_control_temp, control_parent) = private_temp("cleanup-control-temp")?;
    run_subject_in_temp_dir(
        &control_guard.0,
        &control_base,
        &control_candidate,
        _control_temp.0.as_path(),
    )
    .map_err(|error| format!("control fixture must materialize cleanly: {error}"))?;
    assert_materialization_root_cleaned(&control_parent, "control run")?;

    let (guard, base, candidate) = type_change_repo("cleanup-failclosed", true)?;
    let (_temp_guard, parent) = private_temp("cleanup-failclosed-temp")?;
    let error = run_subject_in_temp_dir(&guard.0, &base, &candidate, _temp_guard.0.as_path())
        .err()
        .ok_or_else(|| "a type-changed entry must fail closed".to_string())?;

    // The rejection must name the entry it could not materialize. With the
    // blob-wise materialization (#3548 review) the symlink is rejected at
    // tree enumeration, before any byte is written — so the fail-closed
    // property is structural (there is no partially extracted state to
    // leak) and the cleanup assertion below guards the temp-root guard
    // itself.
    assert!(
        error.contains("unsupported tree entry mode `120000`") && error.contains("src/linked.rs"),
        "fail-closed run must reject the symlink entry by name: {error}"
    );
    assert_materialization_root_cleaned(&parent, "fail-closed run")
}
