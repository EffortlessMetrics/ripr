//! Hostile-repository journeys: the built `ripr` binary against repositories
//! shaped to break tooling assumptions.
//!
//! Each case builds a real git repository (a commit on `main`, then a
//! committed change on `feat`) and runs the public CLI. A journey passes only
//! when ripr either produces the finding the change deserves or refuses with a
//! named, actionable diagnostic. Neither a panic, a signal, a hang, a dropped
//! file, nor a quiet "clean" result for input ripr could not read is accepted.
//!
//! Cases: unusual in-repo file names, option-shaped `--base` values, a
//! non-UTF-8 source file, symlink loops, an oversized diff, shallow clones,
//! detached HEAD, linked worktrees, submodules, a repository without a usable
//! base, and user git configuration that changes `git diff` output.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[path = "common/mod.rs"]
mod common;

use common::fixture_git::fixture_git_ok;

static NEXT_BASE: AtomicU64 = AtomicU64::new(0);

const MANIFEST: &str =
    "[package]\nname = \"hx\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n";
const BASE_LIB: &str = "pub fn total(a: u32) -> u32 { a + 2 }\n";
const CHANGED_LIB: &str = "pub fn total(a: u32) -> u32 { a + 1 }\n";
const TEST: &str = "use hx::total;\n#[test]\nfn t() { assert_eq!(total(1), 3); }\n";

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Result<Self, String> {
        Self::new_under(&std::env::temp_dir(), label)
    }

    fn new_under(parent: &Path, label: &str) -> Result<Self, String> {
        let n = NEXT_BASE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("ripr-hostile-{label}-{}-{n}", std::process::id()));
        // `create_dir` (not `_all`) on the leaf: a path another process
        // pre-created or symlinked must fail the test, never be adopted and
        // later removed recursively.
        fs::create_dir_all(parent).map_err(|e| format!("create scratch parent failed: {e}"))?;
        fs::create_dir(&path).map_err(|e| format!("create scratch failed: {e}"))?;
        Ok(Self { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Upper bound for one CLI invocation. A hang on a hostile fixture must fail the
/// test with the child reaped, not stall the suite until the job times out.
const RIPR_DEADLINE: Duration = Duration::from_mins(2);

fn drain<R: Read + Send + 'static>(stream: Option<R>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            let _ = stream.read_to_end(&mut bytes);
        }
        bytes
    })
}

fn ripr(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Result<Ran, String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ripr"));
    command
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn ripr failed: {e}"))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= RIPR_DEADLINE => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "ripr {args:?} exceeded {RIPR_DEADLINE:?} and was terminated"
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("wait for ripr failed: {e}")),
        }
    };
    let collect = |handle: thread::JoinHandle<Vec<u8>>| {
        handle
            .join()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|_panic| "output reader panicked".to_string())
    };
    Ok(Ran {
        code: status.code(),
        stdout: collect(stdout)?,
        stderr: collect(stderr)?,
    })
}

/// A run that ends by signal, panics, or leaks a backtrace is never acceptable.
fn assert_sane(ran: &Ran, what: &str) -> Result<(), String> {
    if !matches!(ran.code, Some(0..=2)) {
        return Err(format!("{what}: exit {:?}\n{}", ran.code, ran.stderr));
    }
    if ran.stderr.contains("panicked at") || ran.stderr.contains("RUST_BACKTRACE") {
        return Err(format!("{what}: panic\n{}", ran.stderr));
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let mut full = vec![
        "-c",
        "user.email=h@example.invalid",
        "-c",
        "user.name=hostile",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "protocol.file.allow=always",
    ];
    full.extend_from_slice(args);
    fixture_git_ok(dir, &full)
}

/// A repository with `main` at the base commit and `feat` checked out with the
/// change committed. `edit` rewrites the working tree between the two commits.
fn repo(root: &Path, edit: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let w = |rel: &str, body: &str| -> Result<(), String> {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir failed: {e}"))?;
        }
        fs::write(path, body).map_err(|e| format!("write {rel} failed: {e}"))
    };
    fs::create_dir_all(root).map_err(|e| format!("mkdir root failed: {e}"))?;
    git(root, &["init", "-q", "-b", "main"])?;
    w("Cargo.toml", MANIFEST)?;
    w("src/lib.rs", BASE_LIB)?;
    w("tests/t.rs", TEST)?;
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", "base"])?;
    git(root, &["checkout", "-q", "-b", "feat"])?;
    edit(root)?;
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", "change"])
}

fn change_lib(root: &Path) -> Result<(), String> {
    fs::write(root.join("src/lib.rs"), CHANGED_LIB).map_err(|e| format!("write failed: {e}"))
}

fn plain(scratch: &Scratch, name: &str) -> Result<PathBuf, String> {
    let root = scratch.path.join(name);
    repo(&root, change_lib)?;
    Ok(root)
}

fn summary_line(ran: &Ran) -> &str {
    ran.stdout
        .lines()
        .find(|line| line.starts_with("Summary:"))
        .unwrap_or("")
}

fn assert_found_change(ran: &Ran, what: &str) -> Result<(), String> {
    assert_sane(ran, what)?;
    if ran.code != Some(0) || !summary_line(ran).starts_with("Summary: 1 probe(s)") {
        return Err(format!(
            "{what}: expected one probe, got {:?}\n{}\n{}",
            ran.code, ran.stdout, ran.stderr
        ));
    }
    Ok(())
}

#[test]
fn baseline_repo_reports_the_change() -> Result<(), String> {
    let scratch = Scratch::new("baseline")?;
    let root = plain(&scratch, "plain")?;
    assert_found_change(&ripr(&root, &["check"], &[])?, "baseline")
}

#[test]
fn option_shaped_base_is_rejected_without_side_effects() -> Result<(), String> {
    let scratch = Scratch::new("optbase")?;
    let root = plain(&scratch, "plain")?;
    let victim = scratch.path.join("injected");
    let output_flag = format!("--output={}", victim.display());
    for base in [output_flag.as_str(), "--no-index", "-h"] {
        let ran = ripr(&root, &["check", "--base", base], &[])?;
        assert_sane(&ran, base)?;
        if ran.code != Some(2) || !ran.stderr.contains("starts with `-`") {
            return Err(format!("{base}: expected named refusal\n{}", ran.stderr));
        }
    }
    if victim.exists() {
        return Err("--output flag reached git".to_string());
    }
    Ok(())
}

#[test]
fn shell_metacharacter_root_name_does_not_execute() -> Result<(), String> {
    let scratch = Scratch::new("meta")?;
    let marker = "PWNED";
    let root = scratch
        .path
        .join("d$(touch PWNED)`touch PWNED`;touch PWNED&|");
    repo(&root, change_lib)?;
    assert_found_change(&ripr(&root, &["check"], &[])?, "metacharacter root")?;
    let ran = ripr(&root, &["pilot", "--root", ".", "--quiet"], &[])?;
    assert_sane(&ran, "pilot in metacharacter root")?;
    for dir in [&root, &scratch.path, &std::env::temp_dir()] {
        if dir.join(marker).exists() {
            return Err(format!("{} was created by command injection", marker));
        }
    }
    Ok(())
}

#[test]
fn leading_dash_root_directory_is_not_parsed_as_a_flag() -> Result<(), String> {
    let scratch = Scratch::new("dashroot")?;
    let root = plain(&scratch, "--flaglike")?;
    assert_found_change(&ripr(&root, &["check"], &[])?, "cwd in --flaglike")?;
    let ran = ripr(&scratch.path, &["check", "--root", "./--flaglike"], &[])?;
    assert_found_change(&ran, "--root ./--flaglike")
}

/// Names git quotes in raw diff output (octal escapes, backslash escapes) must
/// still resolve to one probe per changed file.
#[cfg(unix)]
#[test]
fn awkward_file_names_each_produce_a_probe() -> Result<(), String> {
    let scratch = Scratch::new("names")?;
    let names = [
        "ünï.rs",
        "it's.rs",
        "dq\"x.rs",
        "sp ace.rs",
        "tab\tx.rs",
        "new\nline.rs",
        "-dash.rs",
        "日本語.rs",
        "back\\slash.rs",
    ];
    let body = |i: usize, delta: u32| format!("pub fn f{i}(a: u32) -> u32 {{ a + {delta} }}\n");
    let root = scratch.path.join("names");
    fs::create_dir_all(root.join("src")).map_err(|e| format!("mkdir failed: {e}"))?;
    fs::write(root.join("Cargo.toml"), MANIFEST).map_err(|e| format!("write failed: {e}"))?;
    fs::write(root.join("src/lib.rs"), "").map_err(|e| format!("write failed: {e}"))?;
    for (i, name) in names.iter().enumerate() {
        fs::write(root.join("src").join(name), body(i, 2))
            .map_err(|e| format!("write {name:?} failed: {e}"))?;
    }
    git(&root, &["init", "-q", "-b", "main"])?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-q", "-m", "base"])?;
    git(&root, &["checkout", "-q", "-b", "feat"])?;
    for (i, name) in names.iter().enumerate() {
        fs::write(root.join("src").join(name), body(i, 1))
            .map_err(|e| format!("write {name:?} failed: {e}"))?;
    }
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-q", "-m", "change"])?;

    let ran = ripr(&root, &["check", "--format", "json"], &[])?;
    assert_sane(&ran, "awkward names")?;
    let report: serde_json::Value = serde_json::from_str(&ran.stdout)
        .map_err(|e| format!("invalid JSON from check: {e}\n{}", ran.stderr))?;
    let summary = &report["summary"];
    let expected = u64::try_from(names.len()).map_err(|e| e.to_string())?;
    if ran.code != Some(0)
        || summary["changed_rust_files"].as_u64() != Some(expected)
        || summary["probes"].as_u64() != Some(expected)
    {
        return Err(format!(
            "expected {expected} changed files and probes, got {summary}\n{}",
            ran.stderr
        ));
    }
    let findings = report["findings"]
        .as_array()
        .ok_or_else(|| "check JSON is missing findings".to_string())?;
    let mut reported_paths = Vec::with_capacity(findings.len());
    for finding in findings {
        let path = finding["probe"]["file"]
            .as_str()
            .ok_or_else(|| "check JSON finding is missing probe.file".to_string())?;
        reported_paths.push(path.strip_prefix("./").unwrap_or(path).to_owned());
    }
    // The renderer writes every path with forward slashes (and a leading
    // `./`), so a literal backslash in a file name is reported as a separator.
    let mut expected_paths: Vec<String> = names
        .iter()
        .map(|name| format!("src/{}", name.replace('\\', "/")))
        .collect();
    expected_paths.sort();
    reported_paths.sort();
    if reported_paths != expected_paths {
        return Err(format!(
            "expected check finding paths {expected_paths:?}, got {reported_paths:?}\n{}",
            ran.stderr
        ));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn non_utf8_source_is_limited_not_clean() -> Result<(), String> {
    let scratch = Scratch::new("nonutf8")?;
    let root = scratch.path.join("r");
    repo(&root, |root| {
        fs::write(
            root.join("src/lib.rs"),
            b"pub fn total(a: u32) -> u32 { a + 1 } // \xff\xfe\n",
        )
        .map_err(|e| format!("write failed: {e}"))
    })?;
    let ran = ripr(&root, &["check"], &[])?;
    assert_sane(&ran, "non-utf8 source")?;
    if ran.stdout.contains("0 of 0 finding(s)") || !ran.stdout.contains("static_unknown") {
        return Err(format!("expected a disclosed limitation\n{}", ran.stdout));
    }
    if !ran.stdout.contains("Save the file as UTF-8") {
        return Err(format!("expected UTF-8 repair route\n{}", ran.stdout));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_loops_and_dangling_links_complete() -> Result<(), String> {
    let scratch = Scratch::new("symlinks")?;
    let root = scratch.path.join("r");
    repo(&root, |root| {
        change_lib(root)?;
        for (target, link) in [
            (".", "src/loop"),
            ("../tests", "src/tl"),
            ("nope", "src/dangling"),
        ] {
            std::os::unix::fs::symlink(target, root.join(link))
                .map_err(|e| format!("symlink {link} failed: {e}"))?;
        }
        Ok(())
    })?;
    assert_found_change(&ripr(&root, &["check"], &[])?, "symlink loop check")?;
    let ran = ripr(&root, &["pilot", "--root", ".", "--quiet"], &[])?;
    assert_sane(&ran, "symlink loop pilot")
}

#[test]
fn oversized_diff_is_refused_with_a_repair_route() -> Result<(), String> {
    let scratch = Scratch::new("huge")?;
    let root = scratch.path.join("r");
    repo(&root, |root| {
        change_lib(root)?;
        let mut generated = String::new();
        for i in 0..5_000 {
            generated.push_str(&format!("pub fn g{i}(a: u32) -> u32 {{ a + {i} }}\n"));
        }
        fs::write(root.join("src/gen.rs"), generated).map_err(|e| format!("write failed: {e}"))
    })?;
    let ran = ripr(
        &root,
        &["check"],
        &[("RIPR_MAX_DIFF_CHANGED_RUST_LINES", "1000")],
    )?;
    assert_sane(&ran, "oversized diff")?;
    if ran.code != Some(2)
        || !ran.stderr.contains("diff_scope_oversized")
        || !ran.stderr.contains("RIPR_MAX_DIFF_CHANGED_RUST_LINES")
    {
        return Err(format!("expected oversized-diff refusal\n{}", ran.stderr));
    }
    Ok(())
}

#[test]
fn detached_head_worktree_and_submodule_roots_work() -> Result<(), String> {
    let scratch = Scratch::new("topology")?;

    let detached = plain(&scratch, "detached")?;
    git(&detached, &["checkout", "-q", "--detach"])?;
    assert_found_change(&ripr(&detached, &["check"], &[])?, "detached HEAD")?;

    let main_repo = plain(&scratch, "wtmain")?;
    let linked = scratch.path.join("wt tree 'x'");
    let linked_arg = linked.to_string_lossy().into_owned();
    git(
        &main_repo,
        &["worktree", "add", "-q", &linked_arg, "-b", "wtb", "feat"],
    )?;
    assert_found_change(&ripr(&linked, &["check"], &[])?, "linked worktree")?;

    let inner = plain(&scratch, "subinner")?;
    let outer = plain(&scratch, "subouter")?;
    let inner_arg = inner.to_string_lossy().into_owned();
    git(
        &outer,
        &["submodule", "add", "-q", &inner_arg, "vendor/sub mod"],
    )?;
    git(&outer, &["commit", "-q", "-m", "add submodule"])?;
    assert_found_change(&ripr(&outer, &["check"], &[])?, "repo with submodule")?;
    assert_found_change(
        &ripr(
            &outer.join("vendor/sub mod"),
            &["check", "--base", "origin/main"],
            &[],
        )?,
        "inside the submodule",
    )
}

// `file://` clone URLs are POSIX-shaped; Windows needs `file:///C:/...`.
#[cfg(unix)]
#[test]
fn shallow_clone_without_merge_base_names_the_repair() -> Result<(), String> {
    let scratch = Scratch::new("shallow")?;
    let source = plain(&scratch, "source")?;
    let clone = scratch.path.join("clone");
    let url = format!("file://{}", source.display());
    let clone_arg = clone.to_string_lossy().into_owned();
    git(
        &scratch.path,
        &[
            "clone", "-q", "--depth", "1", "-b", "feat", &url, &clone_arg,
        ],
    )?;
    git(
        &clone,
        &[
            "fetch",
            "-q",
            "--depth",
            "1",
            "origin",
            "main:refs/remotes/origin/main",
        ],
    )?;
    let ran = ripr(&clone, &["check", "--base", "origin/main"], &[])?;
    assert_sane(&ran, "shallow clone")?;
    if ran.code != Some(2) || !ran.stderr.contains("git fetch --unshallow") {
        return Err(format!("expected unshallow repair route\n{}", ran.stderr));
    }
    Ok(())
}

#[test]
fn repository_without_a_usable_base_refuses_with_a_route() -> Result<(), String> {
    let scratch = Scratch::new("nobase")?;

    let empty = scratch.path.join("empty");
    fs::create_dir_all(&empty).map_err(|e| format!("mkdir failed: {e}"))?;
    git(&empty, &["init", "-q", "-b", "main"])?;
    let ran = ripr(&empty, &["check"], &[])?;
    assert_sane(&ran, "no commits")?;
    if ran.code != Some(2) || !ran.stderr.contains("no commits yet") {
        return Err(format!("expected no-commits route\n{}", ran.stderr));
    }

    let renamed = plain(&scratch, "trunk")?;
    git(&renamed, &["branch", "-m", "main", "trunk"])?;
    let ran = ripr(&renamed, &["check"], &[])?;
    assert_sane(&ran, "no main branch")?;
    if ran.code != Some(2) || !ran.stderr.contains("--base trunk") {
        return Err(format!(
            "expected the other branch to be named\n{}",
            ran.stderr
        ));
    }

    // The workspace's cargo config points TMPDIR inside the checkout, and ripr
    // resolves a workspace root by walking up, so a directory outside every
    // repository is needed to observe the not-a-repository refusal.
    #[cfg(unix)]
    {
        let outside = Scratch::new_under(Path::new("/tmp"), "notgit")?;
        // Some hosts keep /tmp inside a work tree; the refusal cannot be
        // observed there.
        let enclosing = Command::new("git")
            .current_dir(&outside.path)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .map_err(|e| format!("spawn git failed: {e}"))?;
        if enclosing.status.success() {
            return Ok(());
        }
        let ran = ripr(&outside.path, &["check"], &[])?;
        assert_sane(&ran, "not a git repository")?;
        if ran.code != Some(2) || !ran.stderr.contains("not inside a Git work tree") {
            return Err(format!("expected not-a-repository refusal\n{}", ran.stderr));
        }
    }
    Ok(())
}

#[test]
fn user_git_configuration_does_not_change_the_result() -> Result<(), String> {
    let scratch = Scratch::new("gitcfg")?;
    let root = plain(&scratch, "r")?;
    let settings = [
        ("diff.noprefix", "true"),
        ("diff.mnemonicPrefix", "true"),
        ("diff.srcPrefix", "X/"),
        ("diff.dstPrefix", "Y/"),
        ("color.ui", "always"),
        ("color.diff", "always"),
        ("diff.external", "/bin/false"),
        ("diff.renames", "copies"),
        ("diff.algorithm", "patience"),
        ("diff.context", "0"),
        ("diff.interHunkContext", "5"),
        ("core.quotepath", "false"),
        ("core.autocrlf", "true"),
        ("core.pager", "/bin/false"),
        ("log.showSignature", "true"),
    ];
    for (key, value) in settings {
        let ran = ripr(
            &root,
            &["check"],
            &[
                ("GIT_CONFIG_COUNT", "1"),
                ("GIT_CONFIG_KEY_0", key),
                ("GIT_CONFIG_VALUE_0", value),
            ],
        )?;
        assert_found_change(&ran, &format!("git config {key}={value}"))?;
    }
    Ok(())
}

#[test]
fn unreadable_config_is_a_loud_error_not_a_default() -> Result<(), String> {
    let scratch = Scratch::new("config")?;
    let root = plain(&scratch, "r")?;
    fs::write(root.join("ripr.toml"), b"\xff\xfe[bad").map_err(|e| format!("write failed: {e}"))?;
    let ran = ripr(&root, &["check"], &[])?;
    assert_sane(&ran, "non-utf8 ripr.toml")?;
    if ran.code != Some(2) || !ran.stderr.contains("ripr.toml") {
        return Err(format!("expected config refusal\n{}", ran.stderr));
    }
    Ok(())
}
