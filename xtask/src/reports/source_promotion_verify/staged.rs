//! Staged return verification. This route is distinct from direct-J construction.
use super::*;
use serde::Deserialize;
use std::io::Read;
use std::time::Duration;

const LIMIT: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema: String,
    preflight_sha256: String,
    source_parent: String,
    swarm_base: String,
    candidate: String,
    candidate_tree: String,
    rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    path: String,
    source_entry: Option<String>,
    candidate_entry: Option<String>,
    disposition: String,
    rationale: String,
    evidence: String,
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("inspect staged ledger: {e}"))?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > LIMIT as u64 {
        return Err("staged ledger must be a bounded regular file".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > LIMIT {
        return Err("staged ledger exceeds byte limit".into());
    }
    Ok(bytes)
}

pub(super) fn command(args: &[String]) -> Result<(), String> {
    let mut values = BTreeMap::new();
    let mut it = args[1..].iter();
    while let Some(key) = it.next() {
        if !matches!(
            key.as_str(),
            "--preflight" | "--survivors" | "--source-main" | "--swarm-base" | "--preview"
        ) {
            return Err(format!("unknown staged verification option {key}"));
        }
        let value = it
            .next()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| format!("missing value for {key}"))?;
        if values.insert(key.as_str(), value.as_str()).is_some() {
            return Err(format!("duplicate option {key}"));
        }
    }
    let required = |key| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| format!("missing {key}"))
    };
    let source = required("--source-main")?;
    let base = required("--swarm-base")?;
    let preview = required("--preview")?;
    for (name, sha) in [
        ("--source-main", source),
        ("--swarm-base", base),
        ("--preview", preview),
    ] {
        validate_identity(name, sha)?;
    }
    let repo = std::env::current_dir().map_err(|e| e.to_string())?;
    let preflight_path = Path::new(required("--preflight")?);
    let ledger_path = Path::new(required("--survivors")?);
    let report = verify_current(
        &repo,
        preflight_path,
        ledger_path,
        source,
        base,
        preview,
        super::super::source_promotion_acceptance::revalidate,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn verify_current(
    repo: &Path,
    preflight_path: &Path,
    ledger_path: &Path,
    source: &str,
    base: &str,
    preview: &str,
    mut acceptance: impl FnMut(&Value, &Path) -> Result<(), String>,
) -> Result<Value, String> {
    let preflight_bytes = read_preflight_bytes(preflight_path)?;
    let ledger_bytes = read_bounded(ledger_path)?;
    let preflight: Value =
        serde_json::from_slice(&preflight_bytes).map_err(|e| format!("staged preflight: {e}"))?;
    let ledger: Ledger =
        serde_json::from_slice(&ledger_bytes).map_err(|e| format!("staged ledger: {e}"))?;
    acceptance(&preflight, repo)?;
    let mut report = verify(
        repo,
        &preflight,
        &preflight_bytes,
        &ledger,
        source,
        base,
        preview,
    )?;
    if read_preflight_bytes(preflight_path)? != preflight_bytes
        || read_bounded(ledger_path)? != ledger_bytes
    {
        return Err("staged input changed during verification".into());
    }
    acceptance(&preflight, repo)?;
    report["survivor_ledger_sha256"] = Value::String(digest_bytes(&ledger_bytes));
    Ok(report)
}

fn bounded_git(repo: &Path, args: &[String]) -> Result<Vec<u8>, String> {
    let mut full = vec!["--no-replace-objects".to_string()];
    full.extend_from_slice(args);
    let output = crate::run::capture_bytes_in_dir_with_budget(
        Path::new("git"),
        &full,
        (repo, None),
        &[],
        crate::run::ByteCaptureBudget {
            timeout: Duration::from_secs(30),
            stdout_bytes: LIMIT,
            stderr_bytes: 65536,
        },
        "staged inventory",
    )?;
    if output.timed_out || output.status.is_none_or(|s| !s.success()) {
        return Err("staged Git inventory failed, timed out or exceeded its limit".into());
    }
    Ok(output.stdout)
}

fn entry(repo: &Path, commit: &str, path: &str) -> Result<Option<String>, String> {
    let bytes = bounded_git(
        repo,
        &[
            "ls-tree".into(),
            "-z".into(),
            "--full-tree".into(),
            commit.into(),
            "--".into(),
            format!(":(literal){path}"),
        ],
    )?;
    if bytes.is_empty() {
        return Ok(None);
    }
    let record = bytes.strip_suffix(&[0]).ok_or("unterminated tree entry")?;
    let separator = record
        .iter()
        .position(|b| *b == b'\t')
        .ok_or("malformed tree entry")?;
    let (header, remainder) = record.split_at(separator);
    let observed = &remainder[1..];
    if observed != path.as_bytes() || header.contains(&0) {
        return Err("tree entry path mismatch".into());
    }
    Ok(Some(
        String::from_utf8(header.to_vec()).map_err(|e| e.to_string())?,
    ))
}

fn verify(
    repo: &Path,
    preflight: &Value,
    bytes: &[u8],
    ledger: &Ledger,
    source: &str,
    base: &str,
    preview: &str,
) -> Result<Value, String> {
    if preflight.get("schema").and_then(Value::as_str) != Some("ripr.source_promotion_preflight.v2")
    {
        return Err("staged return requires native preflight v2".into());
    }
    validate_preflight_mode(preflight, source, "fast_forward")?;
    let candidate = string_field(preflight, "swarm_parent")?;
    validate_identity("candidate", candidate)?;
    for (name, value) in [
        ("source", source),
        ("base", base),
        ("candidate", candidate),
        ("preview", preview),
    ] {
        validate_identity(name, value)?;
        git_exact_commit(repo, value, name)?;
    }
    if source == candidate || base == candidate || string_field(preflight, "merge_base")? != source
    {
        return Err("staged source/base/candidate geometry differs".into());
    }
    require_ancestor(
        repo,
        source,
        candidate,
        "source is not an ancestor of staged candidate",
    )?;
    require_ancestor(
        repo,
        base,
        candidate,
        "swarm base is not an ancestor of staged candidate",
    )?;
    let prior = git_command(repo, ["merge-base", "--is-ancestor", source, base])
        .output()
        .map_err(|e| format!("pre-import ancestry check failed: {e}"))?;
    match prior.status.code() {
        Some(1) => (),
        Some(0) => return Err("source was already reachable from the pre-import base".into()),
        _ => return Err("pre-import ancestry check failed".into()),
    }
    let transitions = bounded_git(
        repo,
        &[
            "rev-list".into(),
            "--first-parent".into(),
            "--reverse".into(),
            format!("{base}..{candidate}"),
        ],
    )?;
    let transitions = std::str::from_utf8(&transitions).map_err(|_| "invalid transition output")?;
    let import = transitions
        .lines()
        .next()
        .ok_or("empty staged first-parent range")?;
    validate_identity("first import transition", import)?;
    let parent_line = git(repo, &["rev-list", "--parents", "-n", "1", import])?;
    let parents: Vec<_> = parent_line.split_whitespace().collect();
    if parents.len() != 3
        || parents.first().copied() != Some(import)
        || parents.get(1).copied() != Some(base)
    {
        return Err("declared pre-import base is not first parent of an import merge".into());
    }
    require_ancestor(
        repo,
        source,
        import,
        "first swarm transition does not import source",
    )?;
    let tree = git(repo, &["rev-parse", &format!("{candidate}^{{tree}}")])?
        .trim()
        .to_string();
    let dry = object_field(preflight, "dry_merge")?;
    if dry.get("preview_tree").and_then(Value::as_str) != Some(tree.as_str())
        || dry.get("reviewed_resolved_tree").and_then(Value::as_str) != Some(tree.as_str())
        || dry
            .get("reviewed_resolved_tree_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || dry.get("status").and_then(Value::as_str) != Some("clean")
        || !dry
            .get("conflicts")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        return Err("staged candidate tree is not the clean reviewed producer tree".into());
    }
    let options = Options {
        repo: repo.to_path_buf(),
        preflight: PathBuf::new(),
        manifest: PathBuf::new(),
        join: preview.into(),
        source_main: source.into(),
        main: None,
        out: PathBuf::new(),
    };
    let graph = verify_graph(&options, preflight)?;
    verify_release_metadata(repo, preview, source)?;
    if ledger.schema != "ripr.staged_survivors.v1"
        || ledger.preflight_sha256 != digest_bytes(bytes)
        || ledger.source_parent != source
        || ledger.swarm_base != base
        || ledger.candidate != candidate
        || ledger.candidate_tree != tree
    {
        return Err("staged survivor ledger subject or preflight digest differs".into());
    }
    let merge_base = unique_merge_base(&git(repo, &["merge-base", "--all", base, source])?)?;
    let paths = bounded_git(
        repo,
        &[
            "diff".into(),
            "--no-ext-diff".into(),
            "--no-textconv".into(),
            "--no-renames".into(),
            "--name-only".into(),
            "-z".into(),
            merge_base,
            source.into(),
            "--".into(),
        ],
    )?;
    let mut expected = BTreeSet::new();
    if !paths.is_empty() && !paths.ends_with(&[0]) {
        return Err("unterminated survivor inventory".into());
    }
    for raw in paths.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = std::str::from_utf8(raw).map_err(|_| "non-UTF-8 survivor path")?;
        if !expected.insert(path.to_string()) {
            return Err("duplicate survivor path".into());
        }
    }
    let mut actual = BTreeSet::new();
    for row in &ledger.rows {
        if !actual.insert(row.path.clone())
            || row.disposition.trim().is_empty()
            || row.rationale.trim().is_empty()
            || row.evidence.trim().is_empty()
        {
            return Err("duplicate or incomplete survivor disposition".into());
        }
        if row.source_entry != entry(repo, source, &row.path)?
            || row.candidate_entry != entry(repo, candidate, &row.path)?
        {
            return Err(format!("survivor tree entry differs for {:?}", row.path));
        }
    }
    if actual != expected {
        return Err(
            "survivor rows do not exactly cover public changes since the pre-import base".into(),
        );
    }
    Ok(
        serde_json::json!({"schema":"ripr.staged_return_geometry.v1", "status":"verified_geometry", "source_parent":source, "swarm_base":base, "import_transition":import, "candidate":candidate, "candidate_tree":tree, "preview":preview, "parents":graph.parents, "preflight_sha256":digest_bytes(bytes), "survivor_count":actual.len(), "non_claims":["Survivor dispositions require independent semantic review; this receipt does not verify their rationale.","This is not public required-check, protected merge, final landing, package or release authorization."]}),
    )
}

#[cfg(test)]
mod tests {
    use super::super::tests::{test_git_output as git_test, test_git_output_with_input};
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn commit_tree(repo: &Path, tree: &str, a: &str, b: &str) -> Result<String, String> {
        test_git_output_with_input(
            repo,
            &["commit-tree", tree, "-p", a, "-p", b],
            "staged preview\n",
        )
    }
    #[test]
    fn actual_producer_to_staged_consumer_refuses_subject_ancestry_and_survivor_drift()
    -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "staged-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        ));
        let source_repo = root.join("source");
        let swarm_repo = root.join("swarm");
        fs::create_dir_all(source_repo.join("crates/ripr")).map_err(|e| e.to_string())?;
        fs::create_dir_all(source_repo.join("editors/vscode")).map_err(|e| e.to_string())?;
        let run = (|| {
            git_test(&source_repo, &["init", "--quiet"])?;
            git_test(&source_repo, &["config", "user.name", "Fixture"])?;
            git_test(
                &source_repo,
                &["config", "user.email", "fixture@example.invalid"],
            )?;
            for (path, text) in [
                ("Cargo.toml", "[workspace]\nmembers=['crates/ripr']\n"),
                (
                    "crates/ripr/Cargo.toml",
                    "[package]\nname='ripr'\nversion='0.11.0'\n",
                ),
                (
                    "Cargo.lock",
                    "version=3\n[[package]]\nname='ripr'\nversion='0.11.0'\n",
                ),
                ("editors/vscode/package.json", "{\"version\":\"0.11.0\"}"),
                (
                    "editors/vscode/package-lock.json",
                    "{\"version\":\"0.11.0\",\"packages\":{\"\":{\"version\":\"0.11.0\"}}}",
                ),
                ("CHANGELOG.md", "# Changelog\n"),
            ] {
                fs::write(source_repo.join(path), text).map_err(|e| e.to_string())?;
            }
            git_test(&source_repo, &["add", "."])?;
            git_test(&source_repo, &["commit", "--quiet", "-m", "common"])?;
            let common = git_test(&source_repo, &["rev-parse", "HEAD"])?;
            fs::write(source_repo.join("public\nname.txt"), "source\n")
                .map_err(|e| e.to_string())?;
            git_test(&source_repo, &["add", "."])?;
            git_test(&source_repo, &["commit", "--quiet", "-m", "public"])?;
            let source = git_test(&source_repo, &["rev-parse", "HEAD"])?;
            git_test(
                &source_repo,
                &[
                    "remote",
                    "add",
                    "origin",
                    "https://github.com/EffortlessMetrics/ripr.git",
                ],
            )?;
            git_test(
                &source_repo,
                &["clone", "--quiet", ".", swarm_repo.to_str().ok_or("path")?],
            )?;
            git_test(&swarm_repo, &["config", "user.name", "Fixture"])?;
            git_test(
                &swarm_repo,
                &["config", "user.email", "fixture@example.invalid"],
            )?;
            git_test(
                &swarm_repo,
                &[
                    "remote",
                    "set-url",
                    "origin",
                    "https://github.com/EffortlessMetrics/ripr-swarm.git",
                ],
            )?;
            git_test(&swarm_repo, &["checkout", "--quiet", "-B", "main", &common])?;
            fs::write(swarm_repo.join("swarm.txt"), "swarm\n").map_err(|e| e.to_string())?;
            git_test(&swarm_repo, &["add", "."])?;
            git_test(&swarm_repo, &["commit", "--quiet", "-m", "swarm"])?;
            let base = git_test(&swarm_repo, &["rev-parse", "HEAD"])?;
            git_test(
                &swarm_repo,
                &["merge", "--quiet", "--no-ff", "-m", "import", &source],
            )?;
            let inner = git_test(&swarm_repo, &["rev-parse", "HEAD"])?;
            let inner_tree = git_test(&swarm_repo, &["rev-parse", "HEAD^{tree}"])?;
            let outer = commit_tree(&swarm_repo, &inner_tree, &base, &inner)?;
            git_test(&swarm_repo, &["reset", "--quiet", "--hard", &outer])?;
            fs::write(swarm_repo.join("repair.txt"), "repair\n").map_err(|e| e.to_string())?;
            git_test(&swarm_repo, &["add", "."])?;
            git_test(&swarm_repo, &["commit", "--quiet", "-m", "repair"])?;
            let candidate = git_test(&swarm_repo, &["rev-parse", "HEAD"])?;
            let tree = git_test(&swarm_repo, &["rev-parse", "HEAD^{tree}"])?;
            let reference = format!("refs/tags/ripr-release-0.11.0-{candidate}");
            git_test(&swarm_repo, &["update-ref", &reference, &candidate])?;
            let preview = commit_tree(&swarm_repo, &tree, &source, &candidate)?;
            let mut preflight = super::super::super::source_promotion::staged_geometry_fixture(
                &source_repo,
                &swarm_repo,
                &source,
                &candidate,
                &tree,
            )?;
            if preflight["mode"] != "fast_forward"
                || preflight["source_survivor_candidates"] != serde_json::json!([])
            {
                return Err("producer did not exercise staged empty-source-range case".into());
            }
            let portable =
                super::super::super::source_promotion_acceptance::tests::fixture_for_candidate(
                    &candidate, &tree, &reference,
                )?;
            preflight["acceptance"] = portable["acceptance"].clone();
            let bytes = serde_json::to_vec(&preflight).map_err(|e| e.to_string())?;
            let preflight_path = root.join("preflight.json");
            fs::write(&preflight_path, &bytes).map_err(|e| e.to_string())?;
            let path = "public\nname.txt";
            let mut ledger = serde_json::json!({"schema":"ripr.staged_survivors.v1","preflight_sha256":digest_bytes(&bytes),"source_parent":source,"swarm_base":base,"candidate":candidate,"candidate_tree":tree,"rows":[{"path":path,"source_entry":entry(&swarm_repo,&source,path)?,"candidate_entry":entry(&swarm_repo,&candidate,path)?,"disposition":"retained","rationale":"same blob","evidence":"fixture review"}]});
            let ledger_path = root.join("survivors.json");
            fs::write(
                &ledger_path,
                serde_json::to_vec(&ledger).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let mut reads = 0;
            let result = verify_current(
                &swarm_repo,
                &preflight_path,
                &ledger_path,
                &source,
                &base,
                &preview,
                |v, r| {
                    super::super::super::source_promotion_acceptance::revalidate_with_reader_and_tree(v,r,|reference,owner| { reads+=1; super::super::super::source_promotion_acceptance::tests::responses(v,reference,owner) })
                },
            )?;
            if reads != 6
                || result["survivor_count"] != 1
                || result["status"] != "verified_geometry"
                || result["import_transition"] != outer
            {
                return Err(
                    "positive staged route did not consume exact native snapshots and survivor"
                        .into(),
                );
            }
            let drift = verify_current(
                &swarm_repo,
                &preflight_path,
                &ledger_path,
                &source,
                &base,
                &preview,
                |v, r| {
                    super::super::super::source_promotion_acceptance::revalidate_with_reader_and_tree(v, r, |reference, owner| {
                    let response = super::super::super::source_promotion_acceptance::tests::responses(v, reference, owner)?;
                    let mut value: Value = serde_json::from_slice(&response).map_err(|e| e.to_string())?;
                    if owner == 2769 { value["body"] = Value::String("changed decision".into()); }
                    serde_json::to_vec(&value).map_err(|e| e.to_string())
                })
                },
            );
            if !drift.is_err_and(|reason| reason.contains("changed since preflight")) {
                return Err("changed cross-repository decision was not refused".into());
            }
            if validate_preflight(&preflight, &source).is_ok() {
                return Err("historical direct-join validator accepted staged geometry".into());
            }
            let parsed: Ledger =
                serde_json::from_value(ledger.clone()).map_err(|e| e.to_string())?;
            let swapped = commit_tree(&swarm_repo, &tree, &candidate, &source)?;
            if verify(
                &swarm_repo,
                &preflight,
                &bytes,
                &parsed,
                &source,
                &base,
                &swapped,
            )
            .is_ok()
            {
                return Err("swapped preview accepted".into());
            }
            if verify(
                &swarm_repo,
                &preflight,
                &bytes,
                &parsed,
                &source,
                &common,
                &preview,
            )
            .is_ok()
            {
                return Err("wrong base accepted".into());
            }
            ledger["rows"] = serde_json::json!([]);
            let missing: Ledger =
                serde_json::from_value(ledger.clone()).map_err(|e| e.to_string())?;
            if verify(
                &swarm_repo,
                &preflight,
                &bytes,
                &missing,
                &source,
                &base,
                &preview,
            )
            .is_ok()
            {
                return Err("missing survivor accepted".into());
            }
            ledger["rows"] = serde_json::json!([{"path":path,"source_entry":null,"candidate_entry":null,"disposition":"retained","rationale":"fake","evidence":"fake"}]);
            let wrong: Ledger = serde_json::from_value(ledger).map_err(|e| e.to_string())?;
            if verify(
                &swarm_repo,
                &preflight,
                &bytes,
                &wrong,
                &source,
                &base,
                &preview,
            )
            .is_ok()
            {
                return Err("wrong tree entries accepted".into());
            }
            let mut cross = preflight.clone();
            cross["acceptance"]["subject"]["candidate_sha"] = Value::String(source.clone());
            if verify(
                &swarm_repo,
                &cross,
                &bytes,
                &parsed,
                &source,
                &base,
                &preview,
            )
            .is_ok()
            {
                return Err("cross-repository subject mismatch accepted".into());
            }
            let mut legacy = preflight.clone();
            legacy["schema"] = Value::String("ripr.source_promotion_preflight.v1".into());
            if verify(
                &swarm_repo,
                &legacy,
                &bytes,
                &parsed,
                &source,
                &base,
                &preview,
            )
            .is_ok()
            {
                return Err("historical preflight accepted by staged route".into());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&root);
        run
    }
}
