//! Input availability must survive the real CLI's JSON, human and badge projections.
//! The retained behavioral patch has one finding before and after source restoration.
use super::{run_command, run_git, unique_temp_workspace};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const SOURCE: &str = "def discount(total):\n    return total >= 100\n";
const PATCH: &str = "diff --git a/src/discount.py b/src/discount.py\nindex ec6175f..8f07f5e 100644\n--- a/src/discount.py\n+++ b/src/discount.py\n@@ -1,2 +1,2 @@\n def discount(total):\n-    return total > 100\n+    return total >= 100\n";

fn fixture(label: &str) -> TestResult<PathBuf> {
    let root = unique_temp_workspace(label);
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("tests"))?;
    fs::write(
        root.join("ripr.toml"),
        "[languages]\nenabled = [\"rust\", \"python\"]\n",
    )?;
    fs::write(root.join("src/discount.py"), SOURCE)?;
    fs::write(
        root.join("tests/test_discount.py"),
        "from src.discount import discount\n\ndef test_discount_boundary():\n    assert discount(100) is True\n",
    )?;
    fs::write(root.join("change.patch"), PATCH)?;
    run_git(&root, &["init", "-b", "main"])?;
    Ok(root)
}

fn check(root: &Path, format: &str) -> TestResult<String> {
    let output = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(root),
        &[
            "check",
            "--root",
            ".",
            "--diff",
            "change.patch",
            "--mode",
            "fast",
            "--format",
            format,
        ],
    )?;
    assert!(
        output.status.success(),
        "{format}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

fn json(root: &Path) -> TestResult<Value> {
    Ok(serde_json::from_str(&check(root, "json")?)?)
}

fn outcome(report: &Value) -> &Value {
    &report["analysis_outcome"]["outcome"]
}

fn python_count(report: &Value) -> Option<u64> {
    report["summary"]["changed_files_by_language"]
        .as_array()?
        .iter()
        .find(|entry| entry["language"] == "python")?["files"]
        .as_u64()
}

/// Hold Git's real behavioral diff fixed while changing only the worktree
/// source's admission. All link targets are inert data inside this fixture.
#[cfg(unix)]
#[test]
fn changed_python_symlink_source_is_incomplete_across_root_routes() -> TestResult {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let root = git_path_fixture("python-symlink-admission", &["src/discount.py"], SOURCE)?;
    let (run_id, run_attempt) = match (
        std::env::var("GITHUB_RUN_ID").ok(),
        std::env::var("GITHUB_RUN_ATTEMPT").ok(),
    ) {
        (Some(run_id), Some(run_attempt)) => {
            for value in [&run_id, &run_attempt] {
                assert!(
                    !value.is_empty()
                        && value.len() <= 20
                        && value.bytes().all(|byte| byte.is_ascii_digit()),
                    "observational transcript requires bounded ASCII run identifiers"
                );
            }
            (run_id, run_attempt)
        }
        (None, None) if std::env::var_os("GITHUB_ACTIONS").is_none() => (
            format!(
                "local-{}",
                root.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or("local fixture name is not UTF-8")?
            ),
            "0".to_string(),
        ),
        _ => {
            return Err("GitHub transcript requires actual run and attempt identifiers".into());
        }
    };
    let nested = root.join("nested/inner");
    fs::create_dir_all(&nested)?;
    let patch = fs::read(root.join("change.patch"))?;
    eprintln!("retained Git diff: {}", String::from_utf8_lossy(&patch));
    eprintln!("Git diff SHA256: {:x}", Sha256::digest(&patch));
    let binary = env!("CARGO_BIN_EXE_ripr");
    let mut binary_file = fs::File::open(binary)?;
    let mut binary_digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = binary_file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        binary_digest.update(&buffer[..read]);
    }
    let binary_sha256 = format!("{:x}", binary_digest.finalize());
    eprintln!("actual CLI binary: {binary}; SHA256: {binary_sha256}");
    let version = run_command(binary, Some(&root), &["--version"])?;
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout)?;
    eprintln!("actual CLI producing identity: {}", version.trim_end());
    assert_eq!(version, super::expected_version_line());
    let mut transcript = format!(
        "Observational public CLI test transcript; not a typed receipt, native owner acceptance, or release qualification.\nRun ID: {run_id}\nRun attempt: {run_attempt}\nGit diff SHA256: {:x}\nretained Git diff: {}\nactual CLI binary: {binary}; SHA256: {binary_sha256}\nactual CLI producing identity: {version}\n",
        Sha256::digest(&patch),
        String::from_utf8_lossy(&patch),
    );
    // The existing always-uploaded test artifact retains this one observational
    // file without a profile override or broad successful-output capture. Its
    // per-test name is isolated from typed product/native acceptance receipts.
    let transcript_path = super::workspace_root().join(format!(
        "target/nextest/ci/python-symlink-source-admission-cli-{run_id}-{run_attempt}.txt"
    ));
    fs::create_dir_all(
        transcript_path
            .parent()
            .ok_or("transcript parent missing")?,
    )?;
    assert!(
        transcript.len() <= 1024 * 1024,
        "observational CLI transcript exceeds its 1 MiB artifact bound"
    );
    fs::write(&transcript_path, &transcript)?;
    let root_arg = root.to_str().ok_or("fixture root is not UTF-8")?;
    let root_alias = root.join("selected-root-alias");
    std::os::unix::fs::symlink(&root, &root_alias)?;
    assert!(fs::symlink_metadata(&root_alias)?.file_type().is_symlink());
    let alias_arg = root_alias.to_str().ok_or("fixture alias is not UTF-8")?;
    let diff = root.join("change.patch");
    let diff_arg = diff.to_str().ok_or("fixture diff is not UTF-8")?;
    let mut collect = |state: &str| -> TestResult<Vec<(Value, String, Value)>> {
        let mut reports = Vec::new();
        for (route, cwd, selected) in [
            ("explicit-root", nested.as_path(), Some(root_arg)),
            ("implicit-repo-root", root.as_path(), None),
            ("implicit-nested-cwd", nested.as_path(), None),
            ("explicit-root-alias", nested.as_path(), Some(alias_arg)),
        ] {
            let mut outputs = Vec::new();
            for format in ["json", "human", "badge-json"] {
                let mut args = vec![
                    "check", "--diff", diff_arg, "--mode", "fast", "--format", format,
                ];
                if let Some(selected) = selected {
                    args.extend(["--root", selected]);
                }
                let output = run_command(binary, Some(cwd), &args)?;
                let receipt = format!(
                    "CLI receipt state={state} route={route} format={format} cwd={} args={args:?} status={}\nstdout={}\nstderr={}",
                    cwd.display(),
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                eprintln!("{receipt}");
                transcript.push_str(&receipt);
                transcript.push('\n');
                assert!(
                    transcript.len() <= 1024 * 1024,
                    "observational CLI transcript exceeds its 1 MiB artifact bound"
                );
                fs::write(&transcript_path, &transcript)?;
                assert!(
                    output.status.success(),
                    "CLI invocation failed before outcome discrimination"
                );
                outputs.push(String::from_utf8(output.stdout)?);
            }
            reports.push((
                serde_json::from_str(&outputs[0])?,
                outputs[1].clone(),
                serde_json::from_str(&outputs[2])?,
            ));
        }
        Ok(reports)
    };
    let present = collect("regular")?;
    let source = root.join("src/discount.py");
    let target = root.join("held-source.txt");
    fs::rename(&source, &target)?;
    std::os::unix::fs::symlink(&target, &source)?;
    assert!(fs::symlink_metadata(&source)?.file_type().is_symlink());
    assert!(fs::metadata(&source)?.is_file());
    assert_eq!(fs::read(&source)?, SOURCE.as_bytes());
    let linked = collect("owned-regular-target-symlink")?;
    fs::remove_file(&source)?;
    fs::rename(&target, &source)?;
    let restored = collect("restored-regular")?;
    let source_dir = root.join("src");
    let held_dir = root.join("target/held-src");
    fs::create_dir_all(root.join("target"))?;
    fs::rename(&source_dir, &held_dir)?;
    std::os::unix::fs::symlink(&held_dir, &source_dir)?;
    assert!(fs::symlink_metadata(&source_dir)?.file_type().is_symlink());
    assert!(fs::metadata(&source_dir)?.is_dir());
    assert_eq!(fs::read(&source)?, SOURCE.as_bytes());
    let directory_linked = collect("owned-ignored-target-directory-symlink")?;
    fs::remove_file(&source_dir)?;
    fs::rename(&held_dir, &source_dir)?;
    let directory_restored = collect("restored-regular-after-directory-link")?;
    eprintln!(
        "observational CLI transcript: {}",
        transcript_path.display()
    );
    assert_eq!(
        fs::read(&diff)?,
        patch,
        "the Git diff must remain byte-identical"
    );
    fs::remove_file(&root_alias)?;
    fs::remove_dir_all(&root)?;
    // Gather every route and format before evaluating the disputed predicate:
    // a first-route failure must not hide the remaining actual CLI receipts.
    assert_eq!(present.len(), 4);
    assert_eq!(linked.len(), 4);
    assert_eq!(restored.len(), 4);
    assert_eq!(directory_linked.len(), 4);
    assert_eq!(directory_restored.len(), 4);
    for (index, (report, human, badge)) in present.iter().enumerate() {
        assert_eq!(
            report["summary"]["findings"], 1,
            "regular positive route {index}: {report}"
        );
        assert_eq!(python_count(report), Some(1));
        assert_eq!(report["analysis_outcome"]["analysis_complete"], true);
        assert!(human.contains("1 Python file analyzed"));
        assert_eq!(badge["analysis_complete"], true);
        for restored in [&restored[index], &directory_restored[index]] {
            assert_eq!(&restored.0, report, "restoration JSON route {index}");
            assert_eq!(&restored.1, human, "restoration human route {index}");
            assert_eq!(&restored.2, badge, "restoration badge route {index}");
        }
        for (refused, refused_human, refused_badge) in [&linked[index], &directory_linked[index]] {
            assert_missing(refused, "src/discount.py");
            assert_eq!(refused["summary"]["findings"], 0);
            assert_eq!(python_count(refused), Some(0));
            assert!(refused.get("preview_languages").is_none(), "{refused}");
            assert!(!refused_human.contains("1 Python file analyzed"));
            assert!(
                refused_human.lines().any(|line| {
                    line.trim_start().starts_with("Limitation:")
                        && line.contains("file: src/discount.py;")
                }),
                "{refused_human}"
            );
            assert_eq!(refused_badge["analysis_complete"], false);
            assert_eq!(refused_badge["analysis_outcome"], *outcome(refused));
            assert_ne!(refused_badge["color"], "brightgreen");
            assert_ne!(refused_badge["status"], "pass");
        }
    }
    Ok(())
}

fn assert_missing(report: &Value, path: &str) {
    assert_eq!(
        report["analysis_outcome"]["analysis_complete"], false,
        "{report}"
    );
    assert_eq!(outcome(report)["kind"], "partial_with_limitations");
    let limitations = &outcome(report)["limitations"];
    assert_eq!(
        limitations.as_array().map(Vec::len),
        Some(1),
        "{limitations}"
    );
    assert_eq!(limitations[0]["kind"], "changed_file_absent_from_worktree");
    assert_eq!(limitations[0]["path"], path);
    assert_eq!(limitations[0]["affected_items"], 1);
    assert_eq!(limitations[0]["recovery"]["kind"], "retry");
    assert!(
        limitations[0]["recovery"]["detail"]
            .as_str()
            .is_some_and(|s| s.contains("Check out")),
        "{limitations}"
    );
}

#[test]
fn missing_changed_python_source_is_incomplete_and_restorable() -> TestResult {
    let root = fixture("python-source-restoration")?;
    let present = json(&root)?;
    let present_human = check(&root, "human")?;
    let present_badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_eq!(
        present["summary"]["weakly_exposed"], 1,
        "positive fixture must exercise behavior"
    );
    assert_eq!(outcome(&present)["kind"], "complete_with_findings");
    assert_eq!(present["preview_languages"][0]["file_count"], 1);
    assert_eq!(present["preview_languages"][0]["analyzed"], true);
    assert!(present_human.contains("1 Python file analyzed"));

    // The patch, tests, selected root and process entry point stay identical.
    // Moving the source outside the walk also exercises a repeated same-root run.
    fs::rename(root.join("src/discount.py"), root.join("held-source.txt"))?;
    let absent = json(&root)?;
    let absent_human = check(&root, "human")?;
    let absent_badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_missing(&absent, "src/discount.py");
    assert_eq!(absent["summary"]["findings"], 0);
    assert_eq!(python_count(&absent), Some(0));
    assert_eq!(outcome(&absent)["counts"]["changed_file_count"], 1);
    assert!(absent.get("preview_languages").is_none(), "{absent}");
    assert!(absent_human.contains("src/discount.py"));
    assert!(absent_human.contains("Check out the missing file"));
    assert!(!absent_human.contains("1 Python file analyzed"));
    assert_eq!(absent_badge["analysis_complete"], false);
    assert_eq!(absent_badge["analysis_outcome"], *outcome(&absent));
    assert_ne!(absent_badge["color"], "brightgreen");
    assert_ne!(absent_badge["status"], "pass");

    fs::rename(root.join("held-source.txt"), root.join("src/discount.py"))?;
    assert_eq!(fs::read_to_string(root.join("src/discount.py"))?, SOURCE);
    assert_eq!(json(&root)?, present);
    assert_eq!(check(&root, "human")?, present_human);
    assert_eq!(
        serde_json::from_str::<Value>(&check(&root, "badge-json")?)?,
        present_badge
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn mixed_missing_python_source_preserves_available_findings_and_counts() -> TestResult {
    let root = fixture("python-source-mixed")?;
    let present = json(&root)?;
    assert_eq!(present["summary"]["weakly_exposed"], 1);
    let missing_patch = PATCH
        .replace("discount.py", "missing.py")
        .replace("discount(total)", "missing(total)");
    fs::write(root.join("change.patch"), format!("{PATCH}{missing_patch}"))?;
    let mixed = json(&root)?;
    assert_missing(&mixed, "src/missing.py");
    assert_eq!(mixed["findings"], present["findings"]);
    assert_eq!(python_count(&mixed), Some(1));
    assert_eq!(outcome(&mixed)["counts"]["changed_file_count"], 2);
    assert_eq!(mixed["preview_languages"][0]["file_count"], 1);
    assert_eq!(
        mixed["preview_languages"][0]["sample_paths"],
        serde_json::json!(["src/discount.py"])
    );
    let human = check(&root, "human")?;
    assert!(human.contains("1 Python file analyzed"));
    assert!(human.contains("src/missing.py"));
    let badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_eq!(badge["analysis_complete"], false);
    assert_eq!(badge["analysis_outcome"], *outcome(&mixed));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn python_source_admission_preserves_read_failure_and_honest_comment_zero() -> TestResult {
    let root = fixture("python-source-controls")?;
    fs::write(root.join("src/discount.py"), [0xff, b'\n'])?;
    let unreadable = json(&root)?;
    assert_eq!(unreadable["analysis_outcome"]["analysis_complete"], false);
    assert_eq!(
        outcome(&unreadable)["limitations"][0]["kind"],
        "language_scope_unsupported"
    );
    assert_eq!(
        outcome(&unreadable)["limitations"][0]["path"],
        "src/discount.py"
    );
    let badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_ne!(badge["color"], "brightgreen");

    fs::write(
        root.join("src/discount.py"),
        format!("# New comment\n{SOURCE}"),
    )?;
    fs::write(
        root.join("change.patch"),
        "diff --git a/src/discount.py b/src/discount.py\n--- a/src/discount.py\n+++ b/src/discount.py\n@@ -1,3 +1,3 @@\n-# Old comment\n+# New comment\n def discount(total):\n     return total >= 100\n",
    )?;
    let comment = json(&root)?;
    assert_eq!(comment["analysis_outcome"]["analysis_complete"], true);
    assert_eq!(outcome(&comment)["kind"], "no_behavioral_candidates");
    assert_eq!(comment["summary"]["findings"], 0);
    assert_eq!(python_count(&comment), Some(1));
    let badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_eq!(badge["color"], "brightgreen");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn python_source_admission_preserves_exclusions_and_genuine_git_deletions() -> TestResult {
    let root = fixture("python-source-intentional-absence")?;
    for path in ["vendor/discount.py", "src/discount_pb2.py"] {
        fs::write(
            root.join("change.patch"),
            PATCH.replace("src/discount.py", path),
        )?;
        let excluded = json(&root)?;
        assert_eq!(excluded["summary"]["findings"], 0);
        assert_eq!(python_count(&excluded), Some(0));
        assert_eq!(excluded["analysis_outcome"]["analysis_complete"], false);
        assert!(
            !outcome(&excluded)["limitations"]
                .as_array()
                .is_some_and(|items| items
                    .iter()
                    .any(|item| item["kind"] == "changed_file_absent_from_worktree")),
            "{excluded}"
        );
    }
    fs::rename(root.join("src/discount.py"), root.join("held-source.txt"))?;
    fs::write(
        root.join("change.patch"),
        "diff --git a/src/discount.py b/src/discount.py\ndeleted file mode 100644\n--- a/src/discount.py\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-def discount(total):\n-    return total >= 100\n",
    )?;
    let deleted = json(&root)?;
    assert_eq!(
        deleted["analysis_outcome"]["analysis_complete"], true,
        "{deleted}"
    );
    assert_eq!(outcome(&deleted)["counts"]["changed_file_count"], 0);
    assert_eq!(outcome(&deleted)["limitations"], serde_json::json!([]));
    assert_eq!(deleted["summary"]["findings"], 0);
    fs::remove_dir_all(root)?;
    Ok(())
}

fn git_path_fixture(label: &str, paths: &[&str], source: &str) -> TestResult<PathBuf> {
    let root = unique_temp_workspace(label);
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("ripr.toml"),
        "[languages]\nenabled = [\"rust\", \"python\"]\n",
    )?;
    run_git(&root, &["init", "-b", "main"])?;
    for path in paths {
        let file = root.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file, source.replace(">=", ">"))?;
        run_git(&root, &["add", "--", path])?;
        fs::write(file, source)?;
    }
    // Git owns quoting and the marker's tab delimiter. Handwritten headers
    // would miss the path-identity boundary exercised by these fixtures.
    run_git(
        &root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-color",
            "--output=change.patch",
        ],
    )?;
    let patch = fs::read_to_string(root.join("change.patch"))?;
    assert!(patch.contains("@@"), "Git must produce behavioral hunks");
    Ok(root)
}

fn assert_path_restoration(path: &str) -> TestResult {
    let root = git_path_fixture("python-git-path-restoration", &[path], SOURCE)?;
    let present = json(&root)?;
    let human = check(&root, "human")?;
    let badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_eq!(present["summary"]["findings"], 1, "{present}");
    assert_eq!(present["analysis_outcome"]["analysis_complete"], true);
    assert_eq!(
        present["preview_languages"][0]["sample_paths"],
        serde_json::json!([path])
    );
    if path.contains('\t') {
        assert!(fs::read_to_string(root.join("change.patch"))?.contains("+++ \"b/"));
    }

    fs::rename(root.join(path), root.join("held-source.txt"))?;
    let absent = json(&root)?;
    assert_missing(&absent, path);
    assert_eq!(python_count(&absent), Some(0));
    assert_eq!(absent["summary"]["findings"], 0);
    assert!(absent.get("preview_languages").is_none(), "{absent}");
    let absent_human = check(&root, "human")?;
    assert!(
        absent_human.contains(&format!("file: {path};")),
        "{absent_human}"
    );
    assert!(!absent_human.contains("Python file analyzed"));
    let absent_badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_eq!(absent_badge["analysis_outcome"], *outcome(&absent));
    assert_ne!(absent_badge["color"], "brightgreen");

    fs::rename(root.join("held-source.txt"), root.join(path))?;
    assert_eq!(json(&root)?, present);
    assert_eq!(check(&root, "human")?, human);
    assert_eq!(
        serde_json::from_str::<Value>(&check(&root, "badge-json")?)?,
        badge
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn git_generated_python_paths_preserve_whitespace_and_restore() -> TestResult {
    for path in ["leading.py", " leading.py", " spaced/discount.py"] {
        assert_path_restoration(path)?;
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn git_quoted_python_path_preserves_tab_identity() -> TestResult {
    assert_path_restoration(" quoted\t.py")
}

#[test]
fn check_json_source_subject_preserves_whitespace_path_identity() -> TestResult {
    use sha2::{Digest, Sha256};
    let root = git_path_fixture(
        "python-source-subject-space-paths",
        &["leading.py", " leading.py", " spaced/discount.py"],
        SOURCE,
    )?;
    fs::write(root.join(" leading.py"), format!("{SOURCE}# spaced\n"))?;
    fs::write(root.join("leading.py"), format!("{SOURCE}# plain\n"))?;
    fs::write(
        root.join(" spaced/discount.py"),
        format!("{SOURCE}# nested\n"),
    )?;
    let present = json(&root)?;
    assert_eq!(present["summary"]["findings"], 3, "{present}");
    let probe_files: Vec<&str> = present["findings"]
        .as_array()
        .ok_or("missing findings")?
        .iter()
        .filter_map(|finding| finding["probe"]["file"].as_str())
        .collect();
    assert!(probe_files.contains(&" leading.py"), "{probe_files:?}");
    assert!(probe_files.contains(&"leading.py"), "{probe_files:?}");
    assert!(
        probe_files.contains(&" spaced/discount.py"),
        "{probe_files:?}"
    );

    let files = present["source_subject"]["files"]
        .as_array()
        .ok_or("missing source_subject.files")?;
    let stamped: Vec<(&str, Option<&str>)> = files
        .iter()
        .map(|file| (file["path"].as_str().unwrap_or(""), file["digest"].as_str()))
        .collect();
    let paths: Vec<&str> = stamped.iter().map(|(path, _)| *path).collect();
    assert!(paths.contains(&" leading.py"), "{stamped:?}");
    assert!(paths.contains(&"leading.py"), "{stamped:?}");
    assert!(paths.contains(&" spaced/discount.py"), "{stamped:?}");
    assert!(
        !paths.contains(&"spaced/discount.py"),
        "trimmed directory identity must not replace the spaced path: {stamped:?}"
    );

    let digest_of = |relative: &str| -> TestResult<String> {
        Ok(format!(
            "sha256:{:x}",
            Sha256::digest(fs::read(root.join(relative))?)
        ))
    };
    let spaced = stamped
        .iter()
        .find(|(path, _)| *path == " leading.py")
        .ok_or("missing spaced stamp")?;
    let plain = stamped
        .iter()
        .find(|(path, _)| *path == "leading.py")
        .ok_or("missing plain stamp")?;
    let nested = stamped
        .iter()
        .find(|(path, _)| *path == " spaced/discount.py")
        .ok_or("missing nested stamp")?;
    assert_ne!(
        spaced.1, plain.1,
        "distinct contents must not share a digest"
    );
    let spaced_digest = digest_of(" leading.py")?;
    let plain_digest = digest_of("leading.py")?;
    let nested_digest = digest_of(" spaced/discount.py")?;
    assert_eq!(spaced.1, Some(spaced_digest.as_str()));
    assert_eq!(plain.1, Some(plain_digest.as_str()));
    assert_eq!(nested.1, Some(nested_digest.as_str()));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn check_json_source_subject_preserves_git_quoted_tab_path() -> TestResult {
    use sha2::{Digest, Sha256};
    let path = " quoted\t.py";
    let root = git_path_fixture("python-source-subject-tab-path", &[path], SOURCE)?;
    fs::write(root.join(path), format!("{SOURCE}# tab\n"))?;
    let present = json(&root)?;
    assert_eq!(present["summary"]["findings"], 1, "{present}");
    let files = present["source_subject"]["files"]
        .as_array()
        .ok_or("missing source_subject.files")?;
    let stamped = files
        .iter()
        .find(|file| file["path"].as_str() == Some(path))
        .ok_or_else(|| format!("missing tab stamp in {files:?}"))?;
    let digest = format!("sha256:{:x}", Sha256::digest(fs::read(root.join(path))?));
    assert_eq!(stamped["digest"].as_str(), Some(digest.as_str()));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn mixed_python_paths_keep_whitespace_distinct_from_available_sibling() -> TestResult {
    let root = git_path_fixture(
        "python-distinct-space-paths",
        &["leading.py", " leading.py"],
        SOURCE,
    )?;
    let present = json(&root)?;
    assert_eq!(present["summary"]["findings"], 2);
    let expected: Vec<Value> = present["findings"]
        .as_array()
        .ok_or("missing findings")?
        .iter()
        .filter(|finding| finding["probe"]["file"] == "leading.py")
        .cloned()
        .collect();
    assert_eq!(expected.len(), 1);
    fs::rename(root.join(" leading.py"), root.join("held-source.txt"))?;
    let mixed = json(&root)?;
    assert_missing(&mixed, " leading.py");
    assert_eq!(mixed["findings"], serde_json::json!(expected));
    assert_eq!(python_count(&mixed), Some(1));
    assert_eq!(mixed["preview_languages"][0]["file_count"], 1);
    assert_eq!(mixed["preview_languages"][0]["analyzed"], true);
    assert_eq!(
        mixed["preview_languages"][0]["sample_paths"],
        serde_json::json!(["leading.py"])
    );
    let human = check(&root, "human")?;
    assert!(human.contains("file:  leading.py;"), "{human}");
    assert!(human.contains("1 Python file analyzed"));
    let badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_eq!(badge["analysis_outcome"], *outcome(&mixed));
    assert_ne!(badge["color"], "brightgreen");
    fs::rename(root.join("held-source.txt"), root.join(" leading.py"))?;
    assert_eq!(json(&root)?, present);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn shared_absence_disclosure_preserves_rust_path_identity() -> TestResult {
    let path = " leading/src/lib.rs";
    let source =
        "pub fn discount(total: i32) -> bool {\n    if total >= 100 { true } else { false }\n}\n";
    let root = git_path_fixture("rust-space-path-restoration", &[path], source)?;
    fs::write(
        root.join(" leading/Cargo.toml"),
        "[package]\nname = \"space_path\"\nversion = \"0.1.0\"\nedition = \"2024\"",
    )?;
    let present = json(&root)?;
    assert_eq!(present["summary"]["findings"], 1, "{present}");
    assert_eq!(present["analysis_outcome"]["analysis_complete"], true);
    fs::rename(root.join(path), root.join("held-source.txt"))?;
    let absent = json(&root)?;
    assert_missing(&absent, path);
    assert_eq!(absent["summary"]["findings"], 0);
    assert!(check(&root, "human")?.contains("file:  leading/src/lib.rs;"));
    let badge: Value = serde_json::from_str(&check(&root, "badge-json")?)?;
    assert_eq!(badge["analysis_outcome"], *outcome(&absent));
    fs::rename(root.join("held-source.txt"), root.join(path))?;
    assert_eq!(json(&root)?, present);
    fs::remove_dir_all(root)?;
    Ok(())
}
