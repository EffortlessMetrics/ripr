//! New-developer first-run walk (`cargo xtask first-run`).
//!
//! Replays what a developer who has never used ripr does on a fresh machine,
//! against pinned third-party crates ripr has not been tuned on: install,
//! `ripr doctor`, `ripr check`, `ripr pilot`, the follow-up command `check`
//! prints, then `ripr init --ci github` and a second `ripr doctor`. Every step
//! is timed and its friction recorded (nonzero exit, stderr noise, oversized
//! output, a missing next step, a blown time budget), and the report is
//! written as JSON and Markdown so two releases can be compared.
//!
//! The walk observes; it does not gate. A static verdict is recorded per case
//! but never asserted here. Verdict accuracy belongs to a labeled corpus, and
//! this report only names which verdict each release produced.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Value, json};

use crate::run::{CapturedOutput, capture_output_in_dir};

const DEFAULT_OUT: &str = "target/ripr/first-run";
const SCHEMA_VERSION: &str = "first_run.v1";
/// A developer is asked to review the generated workflow before committing it.
const MAX_WORKFLOW_LINES: usize = 1500;
const INSTALL_STDERR_LOG: &str = "install_published.stderr.log";
const OUT_MARKER: &str = ".first-run-output";
const PROGRESS_PREFIX: &str = "ripr progress:";
const VERDICT_CLASSES: [&str; 7] = [
    "exposed",
    "weakly_exposed",
    "reachable_unrevealed",
    "no_static_path",
    "infection_unknown",
    "propagation_unknown",
    "static_unknown",
];

/// One pinned crate and the one-line behavior change applied on a feature
/// branch. The edit must match exactly once on the named line, so a version
/// drift fails loudly instead of measuring a different change.
struct Case {
    krate: &'static str,
    version: &'static str,
    file: &'static str,
    line: usize,
    from: &'static str,
    to: &'static str,
}

const CASES: [Case; 3] = [
    Case {
        krate: "semver",
        version: "1.0.23",
        file: "src/parse.rs",
        line: 166,
        from: "digit > b'9'",
        to: "digit >= b'9'",
    },
    Case {
        krate: "fastrand",
        version: "2.3.0",
        file: "src/lib.rs",
        line: 684,
        from: "val >= surrogate_start",
        to: "val > surrogate_start",
    },
    Case {
        krate: "bytesize",
        version: "1.3.0",
        file: "src/lib.rs",
        line: 192,
        from: "bytes < unit",
        to: "bytes <= unit",
    },
];

/// Wall-clock budget per step. A blown budget is friction, not a failure.
fn budget_secs(step: &str) -> f64 {
    match step {
        "check" | "check_json" => 10.0,
        "pilot" => 30.0,
        "explain" => 10.0,
        _ => 5.0,
    }
}

/// Stdout lines above which a step reads as a wall of text.
fn max_stdout_lines(step: &str) -> Option<usize> {
    match step {
        "check" | "init_ci" | "doctor" | "doctor_after" => Some(60),
        "pilot" => Some(40),
        _ => None,
    }
}

struct Options {
    ripr: String,
    out: PathBuf,
    install_published: bool,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        ripr: "ripr".to_string(),
        out: PathBuf::from(DEFAULT_OUT),
        install_published: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--ripr" => {
                options.ripr = iter
                    .next()
                    .ok_or_else(|| "--ripr needs a path to the ripr binary".to_string())?
                    .clone();
            }
            "--out" => {
                options.out = PathBuf::from(
                    iter.next()
                        .ok_or_else(|| "--out needs a directory".to_string())?,
                );
            }
            "--install-published" => options.install_published = true,
            other => {
                return Err(format!(
                    "unknown first-run argument `{other}`\nusage: cargo xtask first-run [--ripr <path>] [--out <dir>] [--install-published]"
                ));
            }
        }
    }
    Ok(options)
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let options = parse_options(args)?;
    let out = absolute(&options.out)?;
    prepare_out_dir(&out)?;

    let mut setup_steps = Vec::new();
    let ripr = if options.install_published {
        let root = out.join("install-root");
        // Outside the repository so its rust-toolchain and cargo config do not
        // decide how the published crate builds.
        let step = timed(
            &std::env::temp_dir(),
            "install_published",
            "cargo",
            &[
                "install".into(),
                "ripr".into(),
                "--locked".into(),
                "--root".into(),
                root.display().to_string(),
            ],
        )?;
        let failed = step.exit != Some(0);
        if failed {
            // The reports keep a count; the cause lives in the full stderr.
            fs::write(out.join(INSTALL_STDERR_LOG), &step.stderr)
                .map_err(|err| format!("failed to write {INSTALL_STDERR_LOG}: {err}"))?;
        }
        setup_steps.push(step);
        if failed {
            return finish(&options, &out, "unavailable", setup_steps, Vec::new());
        }
        root.join("bin").join("ripr").display().to_string()
    } else {
        options.ripr.clone()
    };

    let version = capture_output_in_dir(&ripr, &["--version".to_string()], &out, "ripr --version")
        .map(|captured| captured.stdout.trim().to_string())
        .map_err(|err| {
            format!(
                "{err}\nbuild or install ripr first, or pass --ripr <path> / --install-published"
            )
        })?;

    setup_steps.push(fetch_sources(&out)?);
    let mut cases = Vec::new();
    for case in &CASES {
        cases.push(walk_case(&out, &ripr, case)?);
    }
    finish(&options, &out, &version, setup_steps, cases)
}

/// A stale tree would let one release's leftovers pass for the next one's run,
/// so a previous walk's directory is replaced. Only a directory this command
/// created (it carries the marker) or an empty one is replaced; `--out` pointed
/// at anything else is refused rather than deleted.
fn prepare_out_dir(out: &Path) -> Result<(), String> {
    if out.exists() {
        let owned = out.join(OUT_MARKER).is_file();
        let empty = fs::read_dir(out)
            .map_err(|err| format!("failed to read {}: {err}", out.display()))?
            .next()
            .is_none();
        if !owned && !empty {
            return Err(format!(
                "{} exists and was not created by `cargo xtask first-run`, so it is not cleared\npass --out <new or empty directory>",
                out.display()
            ));
        }
        // Clearing the directory the command runs from would leave every child
        // process in a deleted working directory.
        let canonical = out
            .canonicalize()
            .map_err(|err| format!("failed to resolve {}: {err}", out.display()))?;
        if std::env::current_dir().is_ok_and(|cwd| cwd.starts_with(&canonical)) {
            return Err(format!(
                "{} contains the current directory, so it is not cleared\npass --out <a directory elsewhere>",
                out.display()
            ));
        }
        fs::remove_dir_all(out)
            .map_err(|err| format!("failed to clear {}: {err}", out.display()))?;
    }
    fs::create_dir_all(out).map_err(|err| format!("failed to create {}: {err}", out.display()))?;
    fs::write(out.join(OUT_MARKER), "first-run output; safe to replace\n")
        .map_err(|err| format!("failed to mark {}: {err}", out.display()))
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|err| format!("failed to read the working directory: {err}"))
}

struct StepResult {
    name: String,
    command: String,
    exit: Option<i32>,
    secs: f64,
    stdout: String,
    stderr: String,
}

impl StepResult {
    fn stderr_noise(&self) -> Vec<&str> {
        self.stderr
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with(PROGRESS_PREFIX))
            .collect()
    }
}

fn timed(cwd: &Path, name: &str, program: &str, args: &[String]) -> Result<StepResult, String> {
    let started = Instant::now();
    let captured = capture_output_in_dir(program, args, cwd, name)?;
    Ok(step_result(name, program, args, started, captured))
}

fn step_result(
    name: &str,
    program: &str,
    args: &[String],
    started: Instant,
    captured: CapturedOutput,
) -> StepResult {
    StepResult {
        name: name.to_string(),
        command: format!("{program} {}", args.join(" ")).trim().to_string(),
        exit: captured.status.code(),
        secs: started.elapsed().as_secs_f64(),
        stdout: captured.stdout,
        stderr: captured.stderr,
    }
}

/// The `[workspace]` table keeps cargo from adopting this scratch package into
/// the repository's workspace when `--out` sits inside it (the default does).
fn fetch_manifest() -> String {
    let mut manifest = String::from(
        "[package]\nname = \"first-run-fetch\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dependencies]\n",
    );
    for case in &CASES {
        manifest.push_str(&format!("{} = \"={}\"\n", case.krate, case.version));
    }
    manifest
}

/// Fetches the pinned crates through cargo itself (registry, proxy and
/// credentials as the developer's machine already configures them), so the walk
/// adds no network client of its own.
fn fetch_sources(out: &Path) -> Result<StepResult, String> {
    let project = out.join("fetch");
    fs::create_dir_all(project.join("src"))
        .map_err(|err| format!("failed to create the fetch project: {err}"))?;
    fs::write(project.join("src").join("lib.rs"), "")
        .map_err(|err| format!("failed to write the fetch project: {err}"))?;
    let manifest = fetch_manifest();
    fs::write(project.join("Cargo.toml"), manifest)
        .map_err(|err| format!("failed to write the fetch manifest: {err}"))?;
    let step = timed(
        &project,
        "fetch_sources",
        "cargo",
        &[
            "vendor".into(),
            "--versioned-dirs".into(),
            "vendored".into(),
        ],
    )?;
    if step.exit != Some(0) {
        return Err(format!(
            "could not fetch the pinned crates (exit {:?}):\n{}\nread the cargo error above; the usual causes are no registry access (check that `cargo fetch` works here) or a changed registry configuration",
            step.exit, step.stderr
        ));
    }
    Ok(step)
}

fn run_checked(cwd: &Path, program: &str, args: &[&str]) -> Result<CapturedOutput, String> {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let captured = capture_output_in_dir(program, &owned, cwd, program)?;
    if !captured.status.success() {
        return Err(format!(
            "`{program} {}` failed in {}: {}\n`first-run` needs git on PATH with a working `git commit` (set user.name and user.email if git asks)",
            args.join(" "),
            cwd.display(),
            captured.stderr.trim()
        ));
    }
    Ok(captured)
}

/// Builds the developer's starting state: a clone of an `origin` whose default
/// branch is `main`, checked out on a feature branch with one committed edit.
fn prepare_case(out: &Path, case: &Case) -> Result<PathBuf, String> {
    let name = format!("{}-{}", case.krate, case.version);
    let vendored = out.join("fetch").join("vendored").join(&name);
    let seed = out.join("seed").join(&name);
    let origin = out.join("origin").join(format!("{name}.git"));
    let work = out.join("work").join(&name);
    for parent in [seed.parent(), origin.parent(), work.parent()]
        .into_iter()
        .flatten()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    copy_tree(&vendored, &seed)?;
    // Cargo's vendor bookkeeping is not part of a repository a developer clones.
    let _ = fs::remove_file(seed.join(".cargo-checksum.json"));
    fs::write(seed.join(".gitignore"), "/target\n")
        .map_err(|err| format!("failed to write .gitignore: {err}"))?;

    run_checked(&seed, "git", &["init", "-q", "-b", "main"])?;
    run_checked(
        &seed,
        "git",
        &["config", "user.email", "first-run@example.invalid"],
    )?;
    run_checked(&seed, "git", &["config", "user.name", "first-run"])?;
    run_checked(&seed, "git", &["add", "-A"])?;
    run_checked(&seed, "git", &["commit", "-q", "-m", "base"])?;
    let origin_arg = origin.display().to_string();
    let work_arg = work.display().to_string();
    run_checked(
        out,
        "git",
        &[
            "clone",
            "-q",
            "--bare",
            &seed.display().to_string(),
            &origin_arg,
        ],
    )?;
    run_checked(out, "git", &["clone", "-q", &origin_arg, &work_arg])?;
    run_checked(
        &work,
        "git",
        &["config", "user.email", "first-run@example.invalid"],
    )?;
    run_checked(&work, "git", &["config", "user.name", "first-run"])?;
    run_checked(&work, "git", &["checkout", "-q", "-b", "change"])?;

    let target = work.join(case.file);
    let source = fs::read_to_string(&target)
        .map_err(|err| format!("failed to read {}: {err}", target.display()))?;
    let Some(edited) = apply_edit(&source, case) else {
        return Err(format!(
            "{name}: expected exactly one `{}` on {}:{}; the pinned source no longer matches the recorded edit\nupdate the `Case` record in xtask/src/first_run.rs for the pinned version, or restore the pinned source",
            case.from, case.file, case.line
        ));
    };
    fs::write(&target, edited)
        .map_err(|err| format!("failed to write {}: {err}", target.display()))?;
    run_checked(&work, "git", &["commit", "-q", "-a", "-m", "change"])?;
    Ok(work)
}

/// The source with the recorded edit applied, or None unless `from` occurs
/// exactly once on the recorded line.
fn apply_edit(source: &str, case: &Case) -> Option<String> {
    let mut edited = String::new();
    let mut matched = false;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        if index + 1 == case.line && line.matches(case.from).count() == 1 {
            edited.push_str(&line.replacen(case.from, case.to, 1));
            matched = true;
        } else {
            edited.push_str(line);
        }
    }
    matched.then_some(edited)
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|err| format!("failed to create {}: {err}", to.display()))?;
    let entries =
        fs::read_dir(from).map_err(|err| format!("failed to read {}: {err}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read {}: {err}", from.display()))?;
        let destination = to.join(entry.file_name());
        let kind = entry
            .file_type()
            .map_err(|err| format!("failed to stat {}: {err}", entry.path().display()))?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &destination)
                .map_err(|err| format!("failed to copy {}: {err}", entry.path().display()))?;
        }
    }
    Ok(())
}

struct CaseResult {
    name: String,
    steps: Vec<(StepResult, Vec<String>)>,
    verdict: Option<&'static str>,
    workflow_lines: Option<usize>,
    workflow_installs_with_cargo: Option<bool>,
}

fn walk_case(out: &Path, ripr: &str, case: &Case) -> Result<CaseResult, String> {
    let work = prepare_case(out, case)?;
    let name = format!("{}-{}", case.krate, case.version);
    let mut steps: Vec<StepResult> = Vec::new();
    push_step(&mut steps, &work, ripr, "doctor", &["doctor"])?;
    let check = push_step(&mut steps, &work, ripr, "check", &["check"])?;
    push_step(
        &mut steps,
        &work,
        ripr,
        "check_json",
        &["check", "--format", "json"],
    )?;
    push_step(&mut steps, &work, ripr, "pilot", &["pilot", "--root", "."])?;

    // Run the drill-down exactly as `check` printed it, the way a developer would.
    let printed = steps[check]
        .stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("ripr explain "))
        .map(str::to_string);
    let explain_missing = printed.is_none();
    let verdict = verdict_of(&steps[check].stdout);
    if let Some(command) = printed {
        let words: Vec<&str> = command.split_whitespace().skip(1).collect();
        push_step(&mut steps, &work, ripr, "explain", &words)?;
    }
    push_step(
        &mut steps,
        &work,
        ripr,
        "init_ci",
        &["init", "--ci", "github"],
    )?;
    push_step(&mut steps, &work, ripr, "doctor_after", &["doctor"])?;

    let workflow = fs::read_to_string(work.join(".github").join("workflows").join("ripr.yml")).ok();
    let workflow_lines = workflow.as_ref().map(|text| text.lines().count());
    let workflow_installs_with_cargo = workflow
        .as_ref()
        .map(|text| text.contains("cargo install ripr"));

    let mut annotated = Vec::new();
    for result in steps {
        let mut flags = friction(&result, workflow.as_deref());
        if explain_missing && result.name == "check" {
            flags.push("no `ripr explain` command printed to drill into".to_string());
        }
        annotated.push((result, flags));
    }
    Ok(CaseResult {
        name,
        steps: annotated,
        verdict,
        workflow_lines,
        workflow_installs_with_cargo,
    })
}

fn push_step(
    steps: &mut Vec<StepResult>,
    cwd: &Path,
    ripr: &str,
    label: &str,
    args: &[&str],
) -> Result<usize, String> {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    steps.push(timed(cwd, label, ripr, &owned)?);
    Ok(steps.len() - 1)
}

/// The first verdict class named after the "Static exposure" label, whichever
/// release wording surrounds it.
fn verdict_of(check_stdout: &str) -> Option<&'static str> {
    let start = check_stdout.find("Static exposure")?;
    let tail = &check_stdout[start..];
    VERDICT_CLASSES
        .iter()
        .filter_map(|class| tail.find(class).map(|at| (at, *class)))
        .min_by_key(|(at, _)| *at)
        .map(|(_, class)| class)
}

fn friction(result: &StepResult, workflow: Option<&str>) -> Vec<String> {
    let mut flags = Vec::new();
    if result.exit != Some(0) {
        flags.push(format!("exit {:?}, expected 0", result.exit));
    }
    let noise = result.stderr_noise();
    if !noise.is_empty() {
        flags.push(format!(
            "{} stderr line(s) beyond progress, first: {}",
            noise.len(),
            noise[0]
        ));
    }
    if let Some(limit) = max_stdout_lines(&result.name) {
        let lines = result.stdout.lines().count();
        if lines > limit {
            flags.push(format!(
                "{lines} stdout lines, over the {limit}-line read budget"
            ));
        }
    }
    let budget = budget_secs(&result.name);
    if result.secs > budget {
        flags.push(format!("{:.1}s, over the {budget:.0}s budget", result.secs));
    }
    if result.name == "check" {
        let has_next = result.stdout.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("Next") || line.starts_with("ripr explain ")
        });
        if !has_next {
            flags.push("no next step printed".to_string());
        }
    }
    if result.name == "init_ci" {
        match workflow {
            None => flags.push("no .github/workflows/ripr.yml written".to_string()),
            Some(text) => {
                let lines = text.lines().count();
                if lines > MAX_WORKFLOW_LINES {
                    flags.push(format!(
                        "generated workflow is {lines} lines, over the {MAX_WORKFLOW_LINES}-line review budget"
                    ));
                }
                if text.contains("cargo install ripr") && !text.contains("sha256") {
                    flags.push(
                        "generated workflow compiles ripr from source on every run".to_string(),
                    );
                }
            }
        }
    }
    flags
}

fn finish(
    options: &Options,
    out: &Path,
    version: &str,
    setup: Vec<StepResult>,
    cases: Vec<CaseResult>,
) -> Result<(), String> {
    let step_json = |result: &StepResult, flags: &[String]| {
        json!({
            "step": result.name,
            "command": result.command,
            "exit": result.exit,
            "secs": (result.secs * 100.0).round() / 100.0,
            "stdout_lines": result.stdout.lines().count(),
            "stderr_lines_beyond_progress": result.stderr_noise().len(),
            "friction": flags,
        })
    };
    let document = json!({
        "schema_version": SCHEMA_VERSION,
        "ripr": version,
        "binary": if options.install_published { "cargo install ripr --locked".to_string() } else { options.ripr.clone() },
        "setup": setup.iter().map(|s| {
            let mut entry = step_json(s, &setup_friction(s));
            if s.name == "install_published" && s.exit != Some(0) {
                entry["stderr_log"] = json!(INSTALL_STDERR_LOG);
            }
            entry
        }).collect::<Vec<Value>>(),
        "cases": cases.iter().map(|case| json!({
            "case": case.name,
            "verdict": case.verdict,
            "workflow_lines": case.workflow_lines,
            "workflow_installs_with_cargo": case.workflow_installs_with_cargo,
            "steps": case.steps.iter().map(|(s, f)| step_json(s, f)).collect::<Vec<Value>>(),
        })).collect::<Vec<Value>>(),
    });
    let rendered = serde_json::to_string_pretty(&document)
        .map_err(|err| format!("failed to render first-run.json: {err}"))?;
    fs::write(out.join("first-run.json"), format!("{rendered}\n"))
        .map_err(|err| format!("failed to write first-run.json: {err}"))?;
    let markdown = render_markdown(version, &setup, &cases);
    fs::write(out.join("first-run.md"), &markdown)
        .map_err(|err| format!("failed to write first-run.md: {err}"))?;
    print!("{markdown}");
    println!("\nWrote {}", out.join("first-run.json").display());
    Ok(())
}

/// Setup steps carry no time or size budget (an install legitimately takes
/// minutes); only a failure is friction.
fn setup_friction(step: &StepResult) -> Vec<String> {
    if step.exit == Some(0) {
        Vec::new()
    } else {
        vec![format!("exit {:?}, expected 0", step.exit)]
    }
}

fn render_markdown(version: &str, setup: &[StepResult], cases: &[CaseResult]) -> String {
    let mut text = format!("# ripr first-run walk\n\nripr: `{version}`\n\n## Setup\n\n");
    let mut friction_total = 0usize;
    for step in setup {
        friction_total += setup_friction(step).len();
        text.push_str(&format!(
            "- `{}`: {:.1}s, exit {:?}\n",
            step.name, step.secs, step.exit
        ));
        if step.name == "install_published" && step.exit != Some(0) {
            text.push_str(&format!("  stderr: `{INSTALL_STDERR_LOG}`\n"));
        }
    }
    for case in cases {
        text.push_str(&format!(
            "\n## {}\n\nverdict: `{}`; generated workflow: {} lines, installs with cargo: {}\n\n| step | exit | secs | stdout lines | friction |\n| --- | --- | --- | --- | --- |\n",
            case.name,
            case.verdict.unwrap_or("none printed"),
            case.workflow_lines
                .map_or_else(|| "none".to_string(), |lines| lines.to_string()),
            case.workflow_installs_with_cargo
                .map_or_else(|| "unknown".to_string(), |flag| flag.to_string()),
        ));
        for (step, flags) in &case.steps {
            friction_total += flags.len();
            text.push_str(&format!(
                "| {} | {} | {:.2} | {} | {} |\n",
                step.name,
                step.exit
                    .map_or_else(|| "signal".to_string(), |c| c.to_string()),
                step.secs,
                step.stdout.lines().count(),
                if flags.is_empty() {
                    "-".to_string()
                } else {
                    flags.join("; ")
                }
            ));
        }
    }
    text.push_str(&format!("\nFriction flags: {friction_total}\n"));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(name: &str, exit: i32, stdout: &str, stderr: &str, secs: f64) -> StepResult {
        StepResult {
            name: name.to_string(),
            command: name.to_string(),
            exit: Some(exit),
            secs,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn verdict_is_the_first_class_after_the_static_exposure_label() {
        // The summary line names every class (all counts), so only text after the
        // label may decide the verdict.
        let published = "Summary: 1 probe(s), 0 exposed, 0 weak\n\nStatic exposure\n  reachable_unrevealed (warning)\n";
        let development = "Summary: 0 exposed, 1 infection_unknown\nStatic exposure: unknown (infection_unknown, warning)\n";
        assert_eq!(verdict_of(published), Some("reachable_unrevealed"));
        assert_eq!(verdict_of(development), Some("infection_unknown"));
        assert_eq!(verdict_of("No findings"), None);
    }

    #[test]
    fn progress_lines_are_not_friction_but_other_stderr_is() {
        let quiet = step(
            "check",
            0,
            "Next: ripr explain x\n",
            "ripr progress: completed [diff]\n",
            0.1,
        );
        assert!(friction(&quiet, None).is_empty());
        let noisy = step(
            "check",
            0,
            "Next: x\n",
            "ripr progress: a\nwarning: odd\n",
            0.1,
        );
        let flags = friction(&noisy, None);
        assert_eq!(flags.len(), 1);
        assert!(flags[0].contains("warning: odd"), "{flags:?}");
    }

    #[test]
    fn exit_budget_length_and_missing_next_step_each_flag_independently() {
        let wall = "line\n".repeat(61);
        let slow = step("check", 2, &wall, "", 11.0);
        let flags = friction(&slow, None);
        assert_eq!(flags.len(), 4, "{flags:?}");
        assert!(flags.iter().any(|f| f.contains("exit Some(2)")));
        assert!(flags.iter().any(|f| f.contains("61 stdout lines")));
        assert!(flags.iter().any(|f| f.contains("budget")));
        assert!(flags.iter().any(|f| f.contains("no next step")));
    }

    #[test]
    fn init_without_a_workflow_file_is_friction() {
        let init = step("init_ci", 0, "Wrote ./ripr.toml\n", "", 0.1);
        assert_eq!(friction(&init, None).len(), 1);
        assert!(friction(&init, Some("name: ripr\n")).is_empty());
    }

    #[test]
    fn a_long_source_compiling_workflow_is_friction_but_a_checksummed_prebuilt_one_is_not() {
        let init = step("init_ci", 0, "Wrote ./ripr.toml\n", "", 0.1);
        let compiled = format!(
            "run: cargo install ripr --version 1 --locked\n{}",
            "x\n".repeat(1600)
        );
        let flags = friction(&init, Some(&compiled));
        assert_eq!(flags.len(), 2, "{flags:?}");
        let prebuilt = "run: sha256sum -c; fallback: cargo install ripr --version 1 --locked\n";
        assert!(friction(&init, Some(prebuilt)).is_empty());
    }

    #[test]
    fn an_unowned_non_empty_out_directory_is_refused_and_an_owned_one_is_replaced()
    -> Result<(), String> {
        let io = |err: std::io::Error| err.to_string();
        let base = std::env::temp_dir().join(format!("first-run-out-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let foreign = base.join("foreign");
        fs::create_dir_all(&foreign).map_err(io)?;
        fs::write(foreign.join("keep.txt"), "mine").map_err(io)?;
        let err = prepare_out_dir(&foreign).err().unwrap_or_default();
        assert!(err.contains("not cleared"), "{err}");
        assert!(foreign.join("keep.txt").is_file(), "foreign file remained");

        let owned = base.join("owned");
        prepare_out_dir(&owned)?;
        fs::write(owned.join("stale.json"), "old").map_err(io)?;
        prepare_out_dir(&owned)?;
        assert!(!owned.join("stale.json").exists(), "stale output replaced");
        assert!(owned.join(OUT_MARKER).is_file());
        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn only_a_failed_setup_step_is_friction_however_long_it_ran() {
        let slow_install = step("install_published", 0, "", "", 600.0);
        assert!(setup_friction(&slow_install).is_empty());
        let failed = step("install_published", 101, "", "error\n", 3.0);
        assert_eq!(setup_friction(&failed), vec!["exit Some(101), expected 0"]);
        let report = render_markdown("v", &[failed], &[]);
        assert!(report.contains("Friction flags: 1"), "{report}");
    }

    #[test]
    fn the_fetch_manifest_is_its_own_workspace_and_pins_every_case() {
        let manifest = fetch_manifest();
        assert!(manifest.contains("\n[workspace]\n"), "{manifest}");
        for case in &CASES {
            assert!(manifest.contains(&format!("{} = \"={}\"", case.krate, case.version)));
        }
    }

    #[test]
    fn an_edit_applies_only_when_the_pattern_occurs_once_on_the_recorded_line() {
        let case = Case {
            krate: "k",
            version: "1",
            file: "f",
            line: 2,
            from: "a > b",
            to: "a >= b",
        };
        assert_eq!(
            apply_edit("x\nif a > b {\ny\n", &case).as_deref(),
            Some("x\nif a >= b {\ny\n")
        );
        assert_eq!(apply_edit("x\nif a > b && a > b {\n", &case), None);
        assert_eq!(apply_edit("x\nif a > c {\n", &case), None);
        assert_eq!(apply_edit("if a > b {\nx\n", &case), None, "wrong line");
    }

    #[test]
    fn unknown_arguments_name_the_usage() {
        let err = parse_options(&["--bogus".to_string()])
            .err()
            .unwrap_or_default();
        assert!(err.contains("usage: cargo xtask first-run"), "{err}");
        assert!(parse_options(&["--ripr".to_string()]).is_err());
    }
}
