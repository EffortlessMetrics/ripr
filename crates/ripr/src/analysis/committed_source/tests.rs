use super::*;

const DEADLINE: Option<Duration> = Some(Duration::from_mins(1));

struct RepoGuard(PathBuf);

impl Drop for RepoGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture_root(name: &str) -> Result<RepoGuard, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "ripr-committed-source-{name}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    let guard = RepoGuard(root);
    git(&guard.0, &["init", "--initial-branch=main"])?;
    git(&guard.0, &["config", "user.email", "ripr@example.invalid"])?;
    git(&guard.0, &["config", "user.name", "ripr test"])?;
    git(&guard.0, &["config", "commit.gpgsign", "false"])?;
    Ok(guard)
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = crate::git::run_git_output_with_deadline(root, args, DEADLINE)?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write(root: &Path, relative: &str, text: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, text).map_err(|error| error.to_string())
}

fn commit_all(root: &Path, message: &str) -> Result<String, String> {
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", message])?;
    git(root, &["rev-parse", "HEAD"])
}

#[test]
fn clean_tree_needs_no_overlay() -> Result<(), String> {
    let repo = fixture_root("clean")?;
    write(&repo.0, "src/lib.rs", "pub fn one() -> u8 { 1 }\n")?;
    commit_all(&repo.0, "base")?;
    // An untracked file no adapter reads cannot change the analysis.
    write(&repo.0, "notes.txt", "scratch\n")?;
    let overlay = probe(&repo.0, DEADLINE)?;
    assert!(overlay.is_none(), "clean tree: {overlay:?}");
    Ok(())
}

/// An untracked source or test file has no `HEAD` content, so the committed
/// view leaves it out instead of reading it as evidence. Untracked files no
/// adapter reads stay out of the dirty set, and a tracked README edit stays
/// out of the note's source subset.
#[test]
fn untracked_source_reads_absent_and_only_source_paths_raise_the_note() -> Result<(), String> {
    let repo = fixture_root("untracked")?;
    write(&repo.0, "src/lib.rs", "pub fn one() -> u8 { 1 }\n")?;
    write(&repo.0, "README.md", "committed\n")?;
    commit_all(&repo.0, "base")?;
    write(&repo.0, "tests/new.rs", "#[test]\nfn t() {}\n")?;
    write(&repo.0, "notes.txt", "scratch\n")?;
    write(&repo.0, "README.md", "edited\n")?;

    let Some(overlay) = probe(&repo.0, DEADLINE)? else {
        return Err("an untracked test file must produce an overlay".to_string());
    };
    assert_eq!(
        overlay.dirty_paths().collect::<Vec<_>>(),
        vec!["tests/new.rs"],
        "only adapter-routed paths: the README edit and notes.txt are not read through the overlay"
    );
    assert_eq!(
        overlay.dirty_source_paths(),
        vec!["tests/new.rs".to_string()],
        "a README edit does not raise the uncommitted-edits note"
    );
    assert!(
        overlay.committed_paths_missing_on_disk().is_empty(),
        "an untracked file has no HEAD content to miss"
    );
    with_overlay(Some(Arc::new(overlay)), || {
        assert_eq!(
            lookup(&repo.0, Path::new("tests/new.rs")),
            CommittedSourceRead::AbsentAtHead,
            "an untracked test is not committed evidence"
        );
    });
    Ok(())
}

#[test]
fn dirty_paths_read_head_bytes_and_staged_additions_read_absent() -> Result<(), String> {
    let repo = fixture_root("dirty")?;
    write(&repo.0, "src/lib.rs", "pub fn committed() -> u8 { 1 }\n")?;
    write(&repo.0, "src/moved.rs", "pub fn moved() -> u8 { 2 }\n")?;
    write(&repo.0, "src/clean.rs", "pub fn clean() -> u8 { 3 }\n")?;
    commit_all(&repo.0, "base")?;
    write(&repo.0, "src/lib.rs", "pub fn dirty() -> u8 { 9 }\n")?;
    write(&repo.0, "src/staged_new.rs", "pub fn staged() {}\n")?;
    git(&repo.0, &["add", "src/staged_new.rs"])?;
    git(&repo.0, &["mv", "src/moved.rs", "src/renamed.rs"])?;

    let Some(overlay) = probe(&repo.0, DEADLINE)? else {
        return Err("a dirty tracked tree must produce an overlay".to_string());
    };
    assert_eq!(
        overlay.dirty_paths().collect::<Vec<_>>(),
        vec![
            "src/lib.rs",
            "src/moved.rs",
            "src/renamed.rs",
            "src/staged_new.rs"
        ],
        "both sides of a staged rename and the staged addition are dirty"
    );
    assert_eq!(
        overlay.committed_paths_missing_on_disk(),
        vec!["src/moved.rs".to_string()],
        "the rename source exists at HEAD but not on disk"
    );
    let overlay = Some(Arc::new(overlay));
    with_overlay(overlay, || -> Result<(), String> {
        assert_eq!(
            read_source_bytes(&repo.0, Path::new("src/lib.rs")).map_err(|e| e.to_string())?,
            Some(b"pub fn committed() -> u8 { 1 }\n".to_vec()),
            "a dirty path reads its HEAD blob, not the working tree"
        );
        assert_eq!(
            read_source_bytes(&repo.0, Path::new("./src/lib.rs")).map_err(|e| e.to_string())?,
            Some(b"pub fn committed() -> u8 { 1 }\n".to_vec()),
            "path spelling does not bypass the overlay"
        );
        assert_eq!(
            read_source_bytes(&repo.0, Path::new("src/staged_new.rs")).map_err(|e| e.to_string())?,
            None,
            "a staged addition has no committed content"
        );
        assert_eq!(
            read_source_bytes(&repo.0, Path::new("src/clean.rs")).map_err(|e| e.to_string())?,
            Some(b"pub fn clean() -> u8 { 3 }\n".to_vec()),
            "a clean path reads the working tree"
        );
        assert_eq!(
            lookup(&repo.0.join("src"), Path::new("lib.rs")),
            CommittedSourceRead::Worktree,
            "a different root never matches overlay keys"
        );
        Ok(())
    })?;
    assert_eq!(
        read_source_bytes(&repo.0, Path::new("src/lib.rs")).map_err(|e| e.to_string())?,
        Some(b"pub fn dirty() -> u8 { 9 }\n".to_vec()),
        "outside the scope the overlay is uninstalled"
    );
    Ok(())
}

#[test]
fn nested_root_keys_are_relative_to_the_analyzed_root() -> Result<(), String> {
    let repo = fixture_root("nested")?;
    write(
        &repo.0,
        "crate_a/src/lib.rs",
        "pub fn nested() -> u8 { 1 }\n",
    )?;
    write(&repo.0, "outside.rs", "pub fn outside() -> u8 { 1 }\n")?;
    commit_all(&repo.0, "base")?;
    write(
        &repo.0,
        "crate_a/src/lib.rs",
        "pub fn nested() -> u8 { 2 }\n",
    )?;
    write(&repo.0, "outside.rs", "pub fn outside() -> u8 { 2 }\n")?;
    let root = repo.0.join("crate_a");
    let Some(overlay) = probe(&root, DEADLINE)? else {
        return Err("the nested dirty file must produce an overlay".to_string());
    };
    assert_eq!(
        overlay.dirty_paths().collect::<Vec<_>>(),
        vec!["src/lib.rs"],
        "keys drop the repository prefix and exclude paths outside the root"
    );
    with_overlay(Some(Arc::new(overlay)), || {
        assert_eq!(
            lookup(&root, Path::new("src/lib.rs")),
            CommittedSourceRead::Committed(b"pub fn nested() -> u8 { 1 }\n".to_vec())
        );
    });
    Ok(())
}

#[test]
fn porcelain_parser_pairs_rename_sources_and_rejects_malformed_records() -> Result<(), String> {
    let parsed = parse_porcelain_z(b" M a.rs\0R  new.rs\0old.rs\0A  added.rs\0")?;
    assert_eq!(parsed.dirty, vec!["a.rs", "new.rs", "old.rs", "added.rs"]);
    // Review of #4442: only routed paths are kept, so an unrelated edited
    // binary, README or non-UTF-8 name costs no blob load and cannot fail
    // the probe; ignored routed files and ignored directories are kept.
    let parsed = parse_porcelain_z(
        b" M \xffdata.bin\0 M README.md\0R  src/lib.rs\0notes.txt\0!! build/\0!! tests/local.rs\0!! local.log\0",
    )?;
    assert_eq!(parsed.dirty, vec!["src/lib.rs"]);
    assert_eq!(parsed.ignored_files, vec!["tests/local.rs"]);
    assert_eq!(parsed.ignored_directories, vec!["build"]);
    assert!(
        parse_porcelain_z(b"M\0").is_err(),
        "a truncated record fails closed"
    );
    assert!(
        parse_porcelain_z(b" M \xff.rs\0").is_err(),
        "a non-UTF-8 path fails closed"
    );
    Ok(())
}

#[test]
fn missing_on_disk_disclosure_names_bounded_paths() {
    // The disclosure lives with the pipeline wiring; keep the key contract
    // here: only committed (Some) entries missing on disk are named.
    let overlay = CommittedSourceOverlay {
        root: std::env::temp_dir().join("ripr-committed-source-missing-none"),
        canonical_root: None,
        ignored: IgnoredPaths::default(),
        entries: BTreeMap::from([
            ("gone.rs".to_string(), Some(b"x".to_vec())),
            ("never.rs".to_string(), None),
        ]),
    };
    assert_eq!(
        overlay.committed_paths_missing_on_disk(),
        vec!["gone.rs".to_string()]
    );
}

/// The committed-range diff binds line numbers to HEAD content. A dirty,
/// line-shifted working copy of the changed file must not move the probe
/// onto the uncommitted bytes.
#[test]
fn committed_range_analysis_binds_to_committed_content_of_a_dirty_file() -> Result<(), String> {
    let repo = fixture_root("pipeline")?;
    write(
        &repo.0,
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write(
        &repo.0,
        "src/lib.rs",
        "pub fn base_value() -> u8 {\n    0\n}\n",
    )?;
    let base = commit_all(&repo.0, "base")?;
    let committed = "pub fn base_value() -> u8 {\n    0\n}\n\npub fn committed_threshold(value: i32) -> bool {\n    value >= 10\n}\n";
    write(&repo.0, "src/lib.rs", committed)?;
    commit_all(&repo.0, "committed change")?;
    // Fixture construction: the committed diff adds `value >= 10` at line 6.
    let committed_line = committed
        .lines()
        .position(|line| line.contains("value >= 10"))
        .map(|index| index + 1);
    assert_eq!(
        committed_line,
        Some(6),
        "fixture must place the predicate on line 6"
    );
    // Dirty the file: shifted lines put a different predicate on line 6.
    write(
        &repo.0,
        "src/lib.rs",
        "pub fn dirty_one() -> u8 {\n    1\n}\n\npub fn dirty_gate(flag: i32) -> bool {\n    flag < 3\n}\n\npub fn base_value() -> u8 {\n    0\n}\n\npub fn committed_threshold(value: i32) -> bool {\n    value >= 10\n}\n",
    )?;
    assert!(
        !git(&repo.0, &["status", "--porcelain", "--untracked-files=no"])?.is_empty(),
        "fixture must leave src/lib.rs dirty"
    );

    let options = crate::analysis::AnalysisOptions {
        root: repo.0.clone(),
        base: Some(base),
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
        resolve_tsconfig_paths: false,
        perl_facts_path: None,
        git_timeout: DEADLINE,
        git_candidate: None,
        production_like_targets: Default::default(),
        test_harnesses: Vec::new(),
        resolved_subject_identity: None,
        open_rust_index_paths: Default::default(),
    };
    let result = crate::analysis::run_analysis_with_oracle_policy(
        &options,
        &crate::config::OraclePolicy::default(),
        &[crate::analysis::language::LanguageId::Rust],
    )?;
    let observed = result
        .findings
        .iter()
        .map(|finding| {
            format!(
                "{}:{} owner={:?} expr={}",
                finding.probe.location.file.display(),
                finding.probe.location.line,
                finding.probe.owner,
                finding.probe.expression
            )
        })
        .collect::<Vec<_>>();
    assert!(
        result.findings.iter().any(|finding| {
            finding.probe.location.line == 6
                && finding.probe.expression.contains("value >= 10")
                && finding
                    .probe
                    .owner
                    .as_ref()
                    .is_some_and(|owner| format!("{owner:?}").contains("committed_threshold"))
        }),
        "the committed predicate must bind at its committed line and owner: {observed:?}"
    );
    assert!(
        result.findings.iter().all(|finding| {
            !finding.probe.expression.contains("flag")
                && finding
                    .probe
                    .owner
                    .as_ref()
                    .is_none_or(|owner| !format!("{owner:?}").contains("dirty"))
        }),
        "no finding may bind to uncommitted bytes: {observed:?}"
    );
    Ok(())
}

/// Review of #4442: the workspace authority re-reads a file to confirm the
/// indexed facts are current. In committed-history mode the index holds `HEAD`
/// bytes for a dirty path, so that re-read must go through the overlay, and
/// must resolve it from the authority's canonical root; a direct working-tree
/// read rejects every dirty file's test-target evidence.
#[test]
fn workspace_authority_confirms_committed_bytes_of_a_dirty_file() -> Result<(), String> {
    use crate::analysis::facts::{FileFacts, WorkspaceRootAuthority};
    let repo = fixture_root("authority")?;
    let committed_lib = "pub fn gate(v: i32) -> bool { v >= 10 }\n";
    let committed_test = "#[test]\nfn gate_boundary() { assert!(crate::gate(10)); }\n";
    write(
        &repo.0,
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write(
        &repo.0,
        "src/lib.rs",
        "pub fn gate(v: i32) -> bool { v > 99 }\n",
    )?;
    write(&repo.0, "tests/gate.rs", "#[test]\nfn edited() {}\n")?;
    let files = [
        ("src/lib.rs", committed_lib),
        ("tests/gate.rs", committed_test),
    ]
    .into_iter()
    .map(|(path, source)| {
        (
            PathBuf::from(path),
            FileFacts {
                path: PathBuf::from(path),
                source: source.to_string(),
                ..FileFacts::default()
            },
        )
    })
    .collect();
    let validates = || {
        WorkspaceRootAuthority::from_index(&repo.0, &files).validates_target(
            Path::new("tests/gate.rs"),
            Path::new("src/lib.rs"),
            committed_test,
        )
    };
    assert!(
        !validates(),
        "fixture: without an overlay the edited working tree must not match"
    );
    // A non-canonical spelling of the root, as the CLI may pass it.
    let spelled_root = repo.0.join("src").join("..");
    let overlay = CommittedSourceOverlay::from_entries(
        &spelled_root,
        [
            ("src/lib.rs", Some(committed_lib.as_bytes())),
            ("tests/gate.rs", Some(committed_test.as_bytes())),
        ],
    );
    assert!(
        with_overlay(Some(Arc::new(overlay)), validates),
        "the authority must confirm committed bytes through the overlay"
    );
    Ok(())
}

/// Review of #4442: Git status omits ignored files by default, but discovery
/// walks the disk without `.gitignore`, so an ignored local test was read as
/// committed evidence. Ignored routed files and files under an ignored
/// directory read absent; a force-added tracked file there keeps its
/// working-tree read; and none of them raises the uncommitted-edits note.
/// Also runs the probe with no deadline (`--git-timeout 0`).
#[test]
fn ignored_sources_read_absent_unless_tracked() -> Result<(), String> {
    let repo = fixture_root("ignored")?;
    write(&repo.0, ".gitignore", "tests/local.rs\nscratch/\nbuild/\n")?;
    write(&repo.0, "src/lib.rs", "pub fn one() -> u8 { 1 }\n")?;
    write(&repo.0, "build2/lib.rs", "pub fn two() -> u8 { 2 }\n")?;
    write(&repo.0, "scratch/kept.rs", "pub fn kept() {}\n")?;
    git(&repo.0, &["add", "-f", "scratch/kept.rs"])?;
    commit_all(&repo.0, "base")?;
    write(&repo.0, "tests/local.rs", "#[test]\nfn local() {}\n")?;
    write(&repo.0, "scratch/probe.rs", "#[test]\nfn probe() {}\n")?;
    // `build/` holds no tracked file, so status reports it as one `build/`
    // record and the directory-prefix branch decides its files.
    write(&repo.0, "build/gen/out.rs", "#[test]\nfn generated() {}\n")?;
    let overlay = probe(&repo.0, None)?.ok_or("ignored sources need an overlay")?;
    assert_eq!(
        overlay.lookup(&repo.0, Path::new("tests/local.rs")),
        CommittedSourceRead::AbsentAtHead
    );
    assert_eq!(
        overlay.lookup(&repo.0, Path::new("scratch/probe.rs")),
        CommittedSourceRead::AbsentAtHead
    );
    assert_eq!(
        overlay.lookup(&repo.0, Path::new("scratch/kept.rs")),
        CommittedSourceRead::Worktree
    );
    assert_eq!(
        overlay.lookup(&repo.0, Path::new("build/gen/out.rs")),
        CommittedSourceRead::AbsentAtHead
    );
    // A sibling that only shares the directory's prefix is not inside it.
    assert_eq!(
        overlay.lookup(&repo.0, Path::new("build2/lib.rs")),
        CommittedSourceRead::Worktree
    );
    assert_eq!(
        overlay.lookup(&repo.0, Path::new("src/lib.rs")),
        CommittedSourceRead::Worktree
    );
    assert!(overlay.dirty_source_paths().is_empty());
    Ok(())
}
