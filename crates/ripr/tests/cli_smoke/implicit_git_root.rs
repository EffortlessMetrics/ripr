//! A marker must not replace the standalone project's actual analysis context.
use super::{run_command, run_command_with_env, run_git, unique_external_workspace};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn python_fixture(label: &str) -> TestResult<(PathBuf, PathBuf)> {
    let sandbox = unique_external_workspace(label)?;
    let project = sandbox.join("project");
    fs::create_dir_all(project.join("src"))?;
    fs::create_dir_all(project.join("tests"))?;
    // Cargo points TMPDIR into this checkout. This fixture must instead be
    // outside its real Git ancestor before adding deliberately inert metadata.
    let outside = run_command_with_env(
        "git",
        &sandbox,
        &["rev-parse", "--show-toplevel"],
        &[("LC_ALL", "C")],
    )?;
    assert_eq!(outside.status.code(), Some(128));
    assert!(
        String::from_utf8_lossy(&outside.stderr).contains("not a git repository"),
        "fixture must lack an ambient Git ancestor before setup: {}",
        String::from_utf8_lossy(&outside.stderr)
    );
    fs::write(
        project.join("pyproject.toml"),
        "[project]\nname = \"discount\"\nversion = \"0.1.0\"\n",
    )?;
    fs::write(
        project.join("src/discount.py"),
        "def discount(amount):\n    if amount >= 100:\n        return 10\n    return 0\n",
    )?;
    fs::write(
        project.join("tests/test_discount.py"),
        "from src.discount import discount\n\ndef test_below():\n    assert discount(99) == 0\n\ndef test_above():\n    assert discount(101) == 10\n",
    )?;
    fs::write(
        project.join("change.diff"),
        "--- a/src/discount.py\n+++ b/src/discount.py\n@@ -1,4 +1,4 @@\n def discount(amount):\n-    if amount > 100:\n+    if amount >= 100:\n         return 10\n     return 0\n",
    )?;
    Ok((sandbox, project))
}

fn check(project: &Path, explicit: bool, env: &[(&str, &str)]) -> TestResult<Value> {
    let mut args = vec![
        "check",
        "--diff",
        "change.diff",
        "--mode",
        "fast",
        "--format",
        "json",
    ];
    if explicit {
        args.extend(["--root", "."]);
    }
    let output = run_command_with_env(env!("CARGO_BIN_EXE_ripr"), project, &args, env)?;
    assert!(
        output.status.success(),
        "check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("resolved workspace root to"),
        "a standalone project must retain its root: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["root"], ".");
    let outcome = &report["analysis_outcome"]["outcome"];
    assert_eq!(outcome["counts"]["changed_file_count"], 1);
    assert_eq!(outcome["counts"]["changed_line_count"], 2);
    assert_eq!(report["summary"]["probes"], 1);
    assert_eq!(report["summary"]["findings"], 1);
    assert_eq!(report["summary"]["weakly_exposed"], 1);
    assert_eq!(report["findings"][0]["related_tests_total"], 2);
    assert_eq!(
        report["findings"][0]["related_tests"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(report["preview_languages"][0]["enabled"], true);
    assert_eq!(report["preview_languages"][0]["analyzed"], true);
    assert_eq!(outcome["kind"], "complete_with_findings");
    assert_eq!(report["analysis_outcome"]["analysis_complete"], true);
    assert_eq!(outcome["limitations"], serde_json::json!([]));
    assert!(
        report["findings"][0]["evidence"]
            .as_array()
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item == "missing_discriminator: amount == 100")
            })
    );
    Ok(report)
}

#[test]
fn invalid_ancestor_git_markers_preserve_python_and_cache_context() -> TestResult {
    for marker in ["empty-directory", "invalid-file", "missing-gitdir"] {
        let (sandbox, project) = python_fixture(marker)?;
        match marker {
            "empty-directory" => fs::create_dir(sandbox.join(".git"))?,
            "invalid-file" => fs::write(sandbox.join(".git"), "leftover metadata\n")?,
            _ => fs::write(sandbox.join(".git"), "gitdir: missing\n")?,
        }
        // Setup must establish that Git itself refuses the marker.
        let git = run_command(
            "git",
            Some(&sandbox),
            &["--git-dir=.git", "rev-parse", "--show-toplevel"],
        )?;
        assert!(
            !git.status.success(),
            "inert marker unexpectedly became a repository"
        );
        let explicit = check(&project, true, &[])?;
        let implicit = check(&project, false, &[])?;
        assert_eq!(implicit["findings"], explicit["findings"]);
        assert_eq!(implicit["analysis_outcome"], explicit["analysis_outcome"]);

        let cache = run_command_with_env(
            env!("CARGO_BIN_EXE_ripr"),
            &project,
            &["cache", "status", "--json"],
            &[("RIPR_CACHE_DIR", "")],
        )?;
        assert!(
            cache.status.success(),
            "{}",
            String::from_utf8_lossy(&cache.stderr)
        );
        let cache: Value = serde_json::from_slice(&cache.stdout)?;
        assert_eq!(
            cache["cache_dir"].as_str().map(Path::new),
            Some(project.canonicalize()?.join("target/ripr/cache").as_path())
        );

        // The positive is an actual repository, with the exact same sources/diff.
        run_git(&project, &["init", "-b", "main"])?;
        let real_git = check(&project, false, &[])?;
        assert_eq!(real_git["findings"], explicit["findings"]);
        assert_eq!(real_git["analysis_outcome"], explicit["analysis_outcome"]);
        fs::remove_dir_all(sandbox)?;
    }
    Ok(())
}

#[test]
fn inherited_git_selectors_cannot_certify_an_inert_ancestor() -> TestResult {
    let (sandbox, project) = python_fixture("implicit-git-selectors")?;
    fs::create_dir(sandbox.join(".git"))?;
    let unrelated = sandbox.join("unrelated");
    fs::create_dir(&unrelated)?;
    run_git(&unrelated, &["init", "-b", "main"])?;
    let git_dir = unrelated.join(".git");
    let git_dir = git_dir.to_str().ok_or("fixture Git path is not UTF-8")?;
    let work_tree = sandbox.to_str().ok_or("fixture work tree is not UTF-8")?;
    let selectors = [
        ("GIT_DIR", git_dir),
        ("GIT_WORK_TREE", work_tree),
        ("GIT_COMMON_DIR", git_dir),
    ];
    let explicit = check(&project, true, &selectors)?;
    let implicit = check(&project, false, &selectors)?;
    assert_eq!(implicit["findings"], explicit["findings"]);
    fs::remove_dir_all(sandbox)?;
    Ok(())
}

#[test]
fn gitless_saved_diff_does_not_cross_a_nested_repository_boundary() -> TestResult {
    let (sandbox, project) = python_fixture("implicit-gitless-boundary")?;
    run_git(&project, &["init", "-b", "main"])?;
    fs::write(sandbox.join("Cargo.toml"), "[workspace]\nmembers = []\n")?;
    let explicit = check(&project, true, &[("PATH", "")])?;
    let implicit = check(&project, false, &[("PATH", "")])?;
    assert_eq!(implicit["findings"], explicit["findings"]);
    fs::remove_dir_all(sandbox)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn git_launch_failure_stops_root_discovery_with_consumer_recovery() -> TestResult {
    let (sandbox, project) = python_fixture("implicit-git-launch-failure")?;
    run_git(&project, &["init", "-b", "main"])?;
    fs::write(sandbox.join("Cargo.toml"), "[workspace]\nmembers = []\n")?;
    let programs = sandbox.join("programs");
    fs::create_dir(&programs)?;
    // A present, non-executable program is a launch failure, not missing Git.
    fs::write(programs.join("git"), "not executable\n")?;
    let path = programs.to_str().ok_or("fixture PATH is not UTF-8")?;
    let env = [("PATH", path), ("RIPR_CACHE_DIR", "")];
    for (args, recovery) in [
        (
            vec!["check", "--diff", "change.diff", "--format", "json"],
            "pass --root PATH",
        ),
        (vec!["cache", "status", "--json"], "set RIPR_CACHE_DIR"),
    ] {
        let output = run_command_with_env(env!("CARGO_BIN_EXE_ripr"), &project, &args, &env)?;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("cannot verify implicit Git root"),
            "{stderr}"
        );
        assert!(stderr.contains(recovery), "{stderr}");
        assert!(!stderr.contains("resolved workspace root to"), "{stderr}");
    }
    check(&project, true, &env)?;
    fs::remove_dir_all(sandbox)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn fixed_root_probe_timeout_names_only_effective_consumer_recovery() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let (sandbox, project) = python_fixture("implicit-git-fixed-timeout")?;
    run_git(&project, &["init", "-b", "main"])?;
    let programs = sandbox.join("programs");
    fs::create_dir(&programs)?;
    let shim = programs.join("git");
    fs::write(&shim, "#!/bin/sh\nexec sleep 60\n")?;
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755))?;
    let path =
        std::env::join_paths([programs.as_path(), Path::new("/bin"), Path::new("/usr/bin")])?;
    let path = path.to_str().ok_or("fixture PATH is not UTF-8")?;
    let env = [
        ("PATH", path),
        ("RIPR_CACHE_DIR", ""),
        ("RIPR_GIT_TIMEOUT", "0"),
    ];
    for (args, recovery) in [
        (
            vec!["check", "--diff", "change.diff", "--git-timeout", "0"],
            "pass --root PATH",
        ),
        (vec!["cache", "status", "--json"], "set RIPR_CACHE_DIR"),
    ] {
        let output = run_command_with_env(env!("CARGO_BIN_EXE_ripr"), &project, &args, &env)?;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("git_invocation_timeout:"), "{stderr}");
        assert!(
            stderr.contains("5000ms deadline (process terminated)"),
            "{stderr}"
        );
        assert!(stderr.contains(recovery), "{stderr}");
        for ineffective in ["--git-timeout", "RIPR_GIT_TIMEOUT", "gitTimeoutMs"] {
            assert!(!stderr.contains(ineffective), "{stderr}");
        }
    }
    fs::remove_dir_all(sandbox)?;
    Ok(())
}
