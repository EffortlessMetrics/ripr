use std::error::Error;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "common/mod.rs"]
mod common;

use common::fixture_git::{fixture_git_ok, fixture_git_output};

#[test]
fn generated_workflow_batches_compact_review_comments() -> Result<(), Box<dyn Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-generated-review-workflow-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;

    let output = Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(["init", "--root"])
        .arg(&root)
        .args(["--ci", "github"])
        .output()?;
    assert!(
        output.status.success(),
        "ripr init failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let workflow = fs::read_to_string(root.join(".github/workflows/ripr.yml"))?;
    let review_endpoint = "gh api --method POST \"repos/${{ github.repository }}/pulls/${{ github.event.pull_request.number }}/reviews\"";
    let legacy_create_endpoint = "gh api --method POST \"repos/${{ github.repository }}/pulls/${{ github.event.pull_request.number }}/comments\"";
    let update_endpoint =
        "gh api --method PATCH \"repos/${{ github.repository }}/pulls/comments/$comment_id\"";
    assert_eq!(workflow.matches(review_endpoint).count(), 1);
    assert!(!workflow.contains(legacy_create_endpoint));
    assert!(workflow.contains(update_endpoint));
    assert!(workflow.contains("event: \"COMMENT\""));
    assert!(workflow.contains("comments: ["));
    assert!(workflow.contains("<details><summary>Full RIPR repair card</summary>"));
    assert!(workflow.contains("presentation=compact-v1"));
    // #3906: a card that carries the repair start leads the compact comment
    // with it; only cards without one fall back to the Verify line.
    assert!(
        workflow.contains(r#"captured("\nStart the repair:\n`(?<value>[^`]+)`"; "")) as $start"#)
    );
    assert!(workflow.contains(
        r#"(if $start then "Start the repair: `\($start)`" else "Verify: `\($verify)`" end) as $next"#
    ));
    assert!(workflow.contains("__ripr_legacy_presentation__"));
    assert!(workflow.contains("__ripr_compact_presentation_unreadable__"));
    assert!(workflow.contains("additional recommendation"));
    assert!(workflow.contains("target/ripr/review/comments.json"));
    assert!(workflow.contains("target/ripr/review/comments.md"));
    assert!(workflow.contains("Created one RIPR review with $create_count inline comment(s)."));

    fs::remove_dir_all(root)?;
    Ok(())
}

/// #3906 (F60-1, F60-2, F60-3, F60-14): replay the generated workflow's `run:`
/// steps, in order, on a PR-shaped fixture repository, then hold the job
/// summary to what a new adopter can act on.
///
/// - Every executed step exits 0. The agent-loop step used to exit 2 on every
///   run (verify had no movement between two snapshots of one HEAD) and left
///   a 0-byte `agent-verify.json` in the upload.
/// - No JSON artifact under `target/ripr` is empty or unparseable.
/// - The `First-run status` block leads with the repair start that
///   start-here carries, instead of `Safe next action command: none`.
/// - Every `ripr ...` command the summary prints either runs as printed
///   (exit 0) from a fresh copy of the post-CI workspace, or is labelled as a
///   step that runs after the test edit or the repair.
#[cfg(unix)]
#[test]
fn generated_workflow_replay_prints_only_runnable_next_steps() -> Result<(), Box<dyn Error>> {
    for tool in ["bash", "git", "jq"] {
        if !replay::tool_available(tool) {
            // A hosted runner always has these tools, so a missing one there
            // is a broken runner, not a reason to skip the strongest oracle
            // for the generated workflow.
            if std::env::var_os("GITHUB_ACTIONS").is_some() {
                return Err(format!(
                    "`{tool}` is not on PATH under GitHub Actions; the generated-workflow replay cannot be skipped in CI"
                )
                .into());
            }
            eprintln!(
                "SKIPPED generated_workflow_replay_prints_only_runnable_next_steps: `{tool}` is not on PATH; the generated workflow needs it"
            );
            return Ok(());
        }
    }
    let base = replay::unique_temp_dir("replay")?;
    let root = base.join("repo");
    replay::write_pr_fixture(&root)?;

    let init = replay::ripr(&root, &["init", "--root", ".", "--ci", "github"])?;
    assert!(
        init.status.success(),
        "ripr init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    replay::git(&root, &["add", "-A"])?;
    replay::git(&root, &["commit", "-q", "-m", "add ripr advisory workflow"])?;

    let workflow = fs::read_to_string(root.join(".github/workflows/ripr.yml"))?;
    let steps = replay::parse_steps(&workflow);
    assert!(
        steps.len() > 20,
        "workflow step parser found {} steps; the template layout changed",
        steps.len()
    );
    let runs = replay::run_workflow(&root, &base, &steps)?;

    // Precondition: the agent-loop step ran for a real top seam.
    let agent_loop = runs
        .iter()
        .find(|run| run.name == "Generate RIPR agent loop artifacts")
        .ok_or("the agent-loop step did not run; the fixture produced no top seam")?;
    assert_eq!(
        agent_loop.exit_code,
        Some(0),
        "agent-loop step failed:\n{}",
        agent_loop.output
    );
    let failed = runs
        .iter()
        .filter(|run| run.exit_code != Some(0))
        .map(|run| format!("{} (exit {:?}):\n{}", run.name, run.exit_code, run.output))
        .collect::<Vec<_>>();
    assert!(
        failed.is_empty(),
        "workflow steps failed:\n{}",
        failed.join("\n")
    );

    // No artifact the upload step would ship is an empty or invalid JSON.
    let json_files = replay::json_files(&root.join("target/ripr"))?;
    assert!(
        json_files.len() > 10,
        "only {} JSON artifacts were written",
        json_files.len()
    );
    for path in &json_files {
        let text = fs::read_to_string(path)?;
        assert!(
            !text.trim().is_empty(),
            "empty JSON artifact: {}",
            path.display()
        );
        if let Err(err) = serde_json::from_str::<serde_json::Value>(&text) {
            return Err(format!("invalid JSON artifact {}: {err}", path.display()).into());
        }
    }
    assert!(
        root.join("target/ripr/workflow/agent-packet.json")
            .is_file(),
        "the agent-loop step must still write the packet the repair starts from"
    );
    assert!(
        !root.join("target/ripr/workflow/agent-verify.json").exists(),
        "CI has no test edit between snapshots, so it must not write a verify artifact"
    );

    // Precondition: start-here carries the repair start the summary leads with.
    let start_here: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("target/ripr/reports/start-here.json"),
    )?)?;
    assert_eq!(start_here["selected"]["state"], "top_gap", "{start_here}");
    let repair_command = start_here["selected"]["repair_command"]
        .as_str()
        .ok_or("start-here.json carries no selected.repair_command")?;
    assert!(
        repair_command.starts_with("ripr agent repair --root . --seam-id ")
            && repair_command.ends_with(" --phase before"),
        "{repair_command}"
    );

    let summary = fs::read_to_string(base.join("step-summary.md"))?;
    let first_block = summary
        .split("#### First-run status\n")
        .nth(1)
        .ok_or("summary has no First-run status block")?;
    assert!(
        first_block.starts_with(&format!("- Start repair: `{repair_command}`\n")),
        "the First-run status block must lead with the repair start:\n{}",
        first_block.lines().take(4).collect::<Vec<_>>().join("\n")
    );
    assert!(
        !summary.contains("Safe next action command: `none`"),
        "summary still says there is no safe next action"
    );
    // Each full report is collapsed under its at-a-glance lines: every
    // `Full report` opener has its own closer, and no report's top-level
    // heading (the workflow's own headings are `##` and deeper) is visible
    // outside a collapsed block.
    let openers = summary.matches("<details><summary>Full report: ").count();
    assert!(openers > 0, "summary collapses no full report:\n{summary}");
    assert_eq!(
        openers,
        summary.matches("</details>").count(),
        "every collapsed full report must close"
    );
    let mut collapsed = false;
    for line in summary.lines() {
        if line.starts_with("<details><summary>Full report: ") {
            collapsed = true;
        } else if line == "</details>" {
            collapsed = false;
        } else if !collapsed {
            assert!(
                !line.starts_with("# "),
                "a full report's heading is visible outside its collapsed block: {line}"
            );
        }
    }

    // The annotation GitHub places on the changed line carries the same
    // repair start, and nothing that points into this runner's checkout
    // (F60-7): the reader is on another machine.
    let annotations = runs
        .iter()
        .find(|run| run.name == "Emit RIPR PR guidance annotations")
        .ok_or("the annotation step did not run")?;
    let warnings = annotations
        .output
        .lines()
        .filter(|line| line.starts_with("::warning "))
        .collect::<Vec<_>>();
    assert!(
        !warnings.is_empty(),
        "no annotation was emitted:\n{}",
        annotations.output
    );
    assert!(
        warnings
            .iter()
            .any(|line| line.ends_with(&format!(" Start the repair: {repair_command}"))),
        "no annotation names the repair start:\n{}",
        warnings.join("\n")
    );
    let checkout = root.display().to_string();
    for line in &warnings {
        assert!(
            !line.contains(&checkout) && !line.contains("agent brief"),
            "annotation points into the runner checkout or at the brief: {line}"
        );
    }

    // Every at-a-glance block whose artifact carries the same repair start
    // leads with it and its after phase (#3906, F60-14). Precondition: each
    // artifact really carries the command, so the block is held to it.
    for (artifact, pointer, heading) in [
        (
            "target/ripr/reports/first-useful-action.json",
            "/commands/repair",
            "#### Recommended next test at a glance\n",
        ),
        (
            "target/ripr/reports/pr-review-front-panel.json",
            "/top_issue/repair_command",
            "#### PR review at a glance\n",
        ),
        (
            "target/ripr/reports/pr-evidence-ledger.json",
            "/top_repair_route/repair_command",
            "#### PR movement at a glance\n",
        ),
    ] {
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join(artifact))?)?;
        assert_eq!(
            value.pointer(pointer).and_then(serde_json::Value::as_str),
            Some(repair_command),
            "{artifact} must carry the repair start at {pointer}"
        );
        let block = summary
            .split(heading)
            .nth(1)
            .ok_or_else(|| format!("summary has no {heading:?} block"))?;
        assert!(
            block.starts_with(&format!(
                "- Repair start: `{repair_command}`\n- After the test edit: run the `--attempt ... --phase after` command"
            )),
            "{heading:?} must lead with the repair start:\n{}",
            block.lines().take(4).collect::<Vec<_>>().join("\n")
        );
    }

    // Before any test edit the agent review packet names the missing
    // receipt as expected and leads with the same repair start and its
    // after phase (#3906, N5), instead of the review summary's post-edit
    // snapshot loop with checkout-absolute redirect paths.
    let packet = summary
        .split("### Agent review packet\n")
        .nth(1)
        .and_then(|rest| rest.split("\n### ").next())
        .ok_or("summary has no Agent review packet block")?;
    assert!(
        packet.starts_with(&format!(
            "- Receipt: No agent receipt yet. None is expected before a repair: the `--attempt ... --phase after` command writes it after the focused test edit.\n- Start repair: `{repair_command}`\n- After the test edit: run the `--attempt ... --phase after` command"
        )),
        "the review packet must lead with the repair start:\n{packet}"
    );
    let root_text = root.to_str().ok_or("non-utf8 path")?;
    for stale in [
        "Movement: missing_artifact",
        "Run the next command listed by agent status",
        root_text,
    ] {
        assert!(
            !packet.contains(stale),
            "the review packet must not print `{stale}` before a repair:\n{packet}"
        );
    }

    let commands = replay::printed_commands(&summary);
    let runnable = commands
        .iter()
        .filter(|command| !command.after_edit)
        .map(|command| command.text.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let deferred = commands
        .iter()
        .filter(|command| command.after_edit)
        .map(|command| command.text.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        runnable.contains(repair_command),
        "the repair start must be printed as runnable now: {runnable:?}"
    );
    assert!(
        !deferred
            .iter()
            .any(|command| command.contains("--phase before")),
        "a before-phase command must not be labelled as a post-edit step: {deferred:?}"
    );
    for label in [
        "ripr agent verify ",
        "ripr agent receipt ",
        "ripr assistant-loop proof ",
    ] {
        assert!(
            deferred.iter().any(|command| command.starts_with(label)),
            "expected the summary to print `{label}...` as a post-edit step: {deferred:?}"
        );
    }

    // Steps that compare against an after snapshot have nothing to compare
    // before the test edit, even when the command itself exits 0.
    let premature = runnable
        .iter()
        .filter(|command| {
            command.contains("after.repo-exposure.json")
                || command.starts_with("ripr agent verify ")
                || command.starts_with("ripr agent receipt ")
                || command.starts_with("ripr outcome ")
        })
        .collect::<Vec<_>>();
    assert!(
        premature.is_empty(),
        "post-edit steps are printed as if they could run now: {premature:?}"
    );

    let mut failures = Vec::new();
    for (index, command) in runnable.iter().enumerate() {
        let copy = base.join(format!("fresh-{index}"));
        replay::copy_tree(&root, &copy)?;
        let output = replay::bash(&copy, command, &[])?;
        if !output.status.success() {
            failures.push(format!(
                "`{command}` exited {:?}:\n{}{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "summary commands printed as runnable failed as printed:\n{}",
        failures.join("\n")
    );

    // The assistant-proof regeneration is labelled as post-repair, but it
    // must still be a complete command: a bare `--out` form exits 2 because
    // the proof refuses to run without explicit inputs.
    for (index, command) in deferred
        .iter()
        .filter(|command| command.starts_with("ripr assistant-loop proof "))
        .enumerate()
    {
        let copy = base.join(format!("proof-{index}"));
        replay::copy_tree(&root, &copy)?;
        let output = replay::bash(&copy, command, &[])?;
        assert!(
            output.status.success(),
            "`{command}` is not a complete command: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // The repair start also runs from a pristine clone with no CI artifacts.
    let clone = base.join("clone");
    replay::git(
        &base,
        &[
            "clone",
            "-q",
            root.to_str().ok_or("non-utf8 path")?,
            clone.to_str().ok_or("non-utf8 path")?,
        ],
    )?;
    let started = replay::bash(&clone, repair_command, &[])?;
    assert!(
        started.status.success(),
        "`{repair_command}` failed from a fresh clone: {}",
        String::from_utf8_lossy(&started.stderr)
    );

    fs::remove_dir_all(base)?;
    Ok(())
}

#[cfg(unix)]
mod replay {
    use std::collections::BTreeMap;
    use std::error::Error;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    type TestResult<T> = Result<T, Box<dyn Error>>;

    const LIB_BASE: &str = "pub const DISCOUNT_THRESHOLD: u64 = 10_000;

pub fn discounted_total(amount: u64) -> u64 {
    if amount > DISCOUNT_THRESHOLD {
        amount - amount / 10
    } else {
        amount
    }
}
";

    const TESTS: &str = "use pricing::discounted_total;

#[test]
fn below_threshold_has_no_discount() {
    assert_eq!(discounted_total(5_000), 5_000);
}

#[test]
fn far_above_threshold_discounts() {
    assert_eq!(discounted_total(20_000), 18_000);
}
";

    /// Phrases that mark a printed command as a step for after the test edit
    /// or the repair, rather than one to run on the checkout as it is.
    const AFTER_EDIT_MARKERS: &[&str] = &[
        "after the test edit",
        "after the focused test edit",
        "after adding one focused test",
        "after verify",
        "without a repair attempt",
        "after the repair's after phase",
    ];

    #[derive(Debug, Default)]
    pub(super) struct Step {
        pub(super) name: String,
        condition: Option<String>,
        run: Option<String>,
        env: Vec<(String, String)>,
    }

    pub(super) struct StepRun {
        pub(super) name: String,
        pub(super) exit_code: Option<i32>,
        pub(super) output: String,
    }

    pub(super) struct PrintedCommand {
        pub(super) text: String,
        pub(super) after_edit: bool,
    }

    pub(super) fn tool_available(tool: &str) -> bool {
        Command::new(tool)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    pub(super) fn unique_temp_dir(label: &str) -> TestResult<PathBuf> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ripr-generated-workflow-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// A one-crate repo whose PR moves `>` to `>=` on a named threshold that
    /// the tests never exercise at the boundary: a repair-ready gap. The PR
    /// targets `trunk`, not `main`, so a step that falls back to a
    /// hardcoded `origin/main` fails here instead of passing by accident.
    pub(super) fn write_pr_fixture(root: &Path) -> TestResult<()> {
        fs::create_dir_all(root.join("src"))?;
        fs::create_dir_all(root.join("tests"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"pricing\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        )?;
        fs::write(root.join("src/lib.rs"), LIB_BASE)?;
        fs::write(root.join("tests/pricing.rs"), TESTS)?;
        git(root, &["init", "-q", "-b", "trunk"])?;
        git(root, &["add", "-A"])?;
        git(root, &["commit", "-q", "-m", "initial pricing crate"])?;
        git(root, &["update-ref", "refs/remotes/origin/trunk", "HEAD"])?;
        git(root, &["checkout", "-q", "-b", "feature"])?;
        fs::write(
            root.join("src/lib.rs"),
            LIB_BASE.replace(
                "amount > DISCOUNT_THRESHOLD",
                "amount >= DISCOUNT_THRESHOLD",
            ),
        )?;
        git(
            root,
            &["commit", "-q", "-a", "-m", "discount at the threshold"],
        )?;
        Ok(())
    }

    pub(super) fn git(dir: &Path, args: &[&str]) -> TestResult<()> {
        let output = Command::new("git")
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "ripr test")
            .env("GIT_AUTHOR_EMAIL", "ripr-test@example.invalid")
            .env("GIT_COMMITTER_NAME", "ripr test")
            .env("GIT_COMMITTER_EMAIL", "ripr-test@example.invalid")
            .stdin(Stdio::null())
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into())
        }
    }

    pub(super) fn ripr(dir: &Path, args: &[&str]) -> TestResult<Output> {
        Ok(Command::new(env!("CARGO_BIN_EXE_ripr"))
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .output()?)
    }

    fn path_with_ripr() -> TestResult<String> {
        let bin = Path::new(env!("CARGO_BIN_EXE_ripr"))
            .parent()
            .ok_or("ripr binary has no parent directory")?;
        let inherited = std::env::var("PATH").unwrap_or_default();
        Ok(format!("{}:{inherited}", bin.display()))
    }

    /// Run a script the way a GitHub-hosted Linux step does without a
    /// `shell:` key (`bash -e {0}`), with the freshly built `ripr` first on
    /// PATH.
    pub(super) fn bash(dir: &Path, script: &str, env: &[(String, String)]) -> TestResult<Output> {
        let mut command = Command::new("bash");
        command
            .args(["--noprofile", "--norc", "-e", "-c", script])
            .current_dir(dir)
            .env("PATH", path_with_ripr()?)
            .stdin(Stdio::null());
        for (key, value) in env {
            command.env(key, value);
        }
        Ok(command.output()?)
    }

    pub(super) fn copy_tree(from: &Path, to: &Path) -> TestResult<()> {
        let status = Command::new("cp")
            .arg("-a")
            .arg(from)
            .arg(to)
            .stdin(Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("cp -a {} {} failed", from.display(), to.display()).into())
        }
    }

    /// Parse the generated workflow's job steps. The template is regular:
    /// steps open at six spaces (`- `), keys sit at eight, and `run: |`
    /// bodies and step `env:` entries at ten.
    pub(super) fn parse_steps(workflow: &str) -> Vec<Step> {
        let lines = workflow.lines().collect::<Vec<_>>();
        let Some(start) = lines.iter().position(|line| *line == "    steps:") else {
            return Vec::new();
        };
        let mut steps: Vec<Step> = Vec::new();
        let mut index = start + 1;
        while index < lines.len() {
            let line = lines[index];
            index += 1;
            let entry = if let Some(rest) = line.strip_prefix("      - ") {
                steps.push(Step::default());
                rest
            } else if let Some(rest) = line.strip_prefix("        ") {
                if rest.starts_with(' ') || rest.starts_with('#') {
                    continue;
                }
                rest
            } else {
                continue;
            };
            let Some(step) = steps.last_mut() else {
                continue;
            };
            let Some((key, value)) = entry.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match key {
                "name" => step.name = value.to_string(),
                "uses" if step.name.is_empty() => step.name = value.to_string(),
                "if" => step.condition = Some(value.to_string()),
                "run" if value == "|" => {
                    let mut body = Vec::new();
                    while index < lines.len() {
                        let next = lines[index];
                        if next.trim().is_empty() {
                            body.push("");
                        } else if let Some(stripped) = next.strip_prefix("          ") {
                            body.push(stripped);
                        } else {
                            break;
                        }
                        index += 1;
                    }
                    while body.last() == Some(&"") {
                        body.pop();
                    }
                    step.run = Some(body.join("\n"));
                }
                "run" => step.run = Some(value.to_string()),
                "env" => {
                    while index < lines.len() {
                        let Some(pair) = lines[index].strip_prefix("          ") else {
                            break;
                        };
                        if let Some((name, value)) = pair.split_once(':') {
                            step.env
                                .push((name.trim().to_string(), value.trim().to_string()));
                        }
                        index += 1;
                    }
                }
                _ => {}
            }
        }
        steps
    }

    fn github_expression(expression: &str, head_sha: &str) -> Option<String> {
        let value = match expression {
            "github.base_ref" => "trunk",
            // A pull_request event carries base_ref, so the default-branch
            // fallback never applies in this replay.
            "github.base_ref || github.event.repository.default_branch" => "trunk",
            "github.event_name" => "pull_request",
            "github.repository" => "ripr-test/pricing",
            "github.event.number" | "github.event.pull_request.number" => "1",
            "github.event.pull_request.head.repo.full_name" => "ripr-test/pricing",
            "github.event.pull_request.head.sha" => head_sha,
            "github.token" => "replay-token-unused",
            "vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only'" => "true",
            _ => return None,
        };
        Some(value.to_string())
    }

    fn substitute(text: &str, head_sha: &str) -> TestResult<String> {
        let mut out = String::new();
        let mut rest = text;
        while let Some(open) = rest.find("${{") {
            out.push_str(&rest[..open]);
            let after = &rest[open + 3..];
            let close = after.find("}}").ok_or("unterminated workflow expression")?;
            let expression = after[..close].trim();
            let value = github_expression(expression, head_sha)
                .ok_or_else(|| format!("replay does not model `${{{{ {expression} }}}}`"))?;
            out.push_str(&value);
            rest = &after[close + 2..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn operand(text: &str, root: &Path, env: &BTreeMap<String, String>) -> TestResult<String> {
        let text = text.trim();
        if let Some(path) = text
            .strip_prefix("hashFiles('")
            .and_then(|rest| rest.strip_suffix("')"))
        {
            return Ok(if root.join(path).exists() {
                "hash".to_string()
            } else {
                String::new()
            });
        }
        if let Some(literal) = text
            .strip_prefix('\'')
            .and_then(|rest| rest.strip_suffix('\''))
        {
            return Ok(literal.to_string());
        }
        if let Some(name) = text.strip_prefix("env.") {
            return Ok(env.get(name).cloned().unwrap_or_default());
        }
        if text == "github.event_name" {
            return Ok("pull_request".to_string());
        }
        Err(format!("replay does not model the condition operand `{text}`").into())
    }

    fn condition_holds(
        condition: &str,
        root: &Path,
        env: &BTreeMap<String, String>,
    ) -> TestResult<bool> {
        for atom in condition.split("&&").map(str::trim) {
            let holds = if atom == "always()" {
                true
            } else if let Some((left, right)) = atom.split_once("!=") {
                operand(left, root, env)? != operand(right, root, env)?
            } else if let Some((left, right)) = atom.split_once("==") {
                operand(left, root, env)? == operand(right, root, env)?
            } else {
                return Err(format!("replay does not model the condition `{atom}`").into());
            };
            if !holds {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Replay every `run:` step in order. `uses:` steps and the crates.io
    /// install are skipped: the checkout is the fixture and `ripr` is the
    /// binary under test. `$GITHUB_ENV` writes carry into later steps.
    pub(super) fn run_workflow(
        root: &Path,
        base: &Path,
        steps: &[Step],
    ) -> TestResult<Vec<StepRun>> {
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()?;
        let head_sha = String::from_utf8(head.stdout)?.trim().to_string();
        let github_env = base.join("github-env");
        let summary = base.join("step-summary.md");
        let event = base.join("event.json");
        fs::write(&github_env, "")?;
        fs::write(&summary, "")?;
        fs::write(base.join("github-output"), "")?;
        fs::write(
            &event,
            format!(
                "{{\"pull_request\":{{\"number\":1,\"labels\":[],\"head\":{{\"sha\":\"{head_sha}\"}}}}}}"
            ),
        )?;
        let mut env = BTreeMap::from([
            ("RIPR_UPLOAD_SARIF".to_string(), "true".to_string()),
            ("RIPR_GATE_MODE".to_string(), String::new()),
            ("RIPR_GATE_BASELINE".to_string(), String::new()),
            ("RIPR_COMMENT_MODE".to_string(), "off".to_string()),
            ("GITHUB_ENV".to_string(), github_env.display().to_string()),
            (
                "GITHUB_STEP_SUMMARY".to_string(),
                summary.display().to_string(),
            ),
            (
                "GITHUB_OUTPUT".to_string(),
                base.join("github-output").display().to_string(),
            ),
            ("GITHUB_EVENT_PATH".to_string(), event.display().to_string()),
            ("GITHUB_EVENT_NAME".to_string(), "pull_request".to_string()),
            ("GITHUB_BASE_REF".to_string(), "trunk".to_string()),
            ("GITHUB_HEAD_REF".to_string(), "feature".to_string()),
            (
                "GITHUB_REPOSITORY".to_string(),
                "ripr-test/pricing".to_string(),
            ),
            ("GITHUB_SHA".to_string(), head_sha.clone()),
            ("GITHUB_WORKSPACE".to_string(), root.display().to_string()),
            ("RUNNER_TEMP".to_string(), base.display().to_string()),
        ]);
        let mut runs = Vec::new();
        for step in steps {
            let Some(script) = &step.run else {
                continue;
            };
            if script.starts_with("cargo install ripr") {
                continue;
            }
            if let Some(condition) = &step.condition
                && !condition_holds(condition, root, &env)?
            {
                continue;
            }
            let script = substitute(script, &head_sha)?;
            let mut step_env = env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>();
            for (name, value) in &step.env {
                step_env.push((name.clone(), substitute(value, &head_sha)?));
            }
            let output = bash(root, &script, &step_env)?;
            runs.push(StepRun {
                name: step.name.clone(),
                exit_code: output.status.code(),
                output: format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
            for line in fs::read_to_string(&github_env)?.lines() {
                if let Some((name, value)) = line.split_once('=') {
                    env.insert(name.to_string(), value.to_string());
                }
            }
        }
        Ok(runs)
    }

    pub(super) fn json_files(dir: &Path) -> TestResult<Vec<PathBuf>> {
        let mut found = Vec::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(next) = pending.pop() {
            for entry in fs::read_dir(&next)? {
                let path = entry?.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|ext| ext == "json") {
                    found.push(path);
                }
            }
        }
        Ok(found)
    }

    /// Code spans on one Markdown line, honoring `\`` escapes, with the byte
    /// offset where each span opens.
    fn code_spans(line: &str) -> Vec<(usize, String)> {
        let mut spans = Vec::new();
        let mut current: Option<(usize, String)> = None;
        let mut chars = line.char_indices().peekable();
        while let Some((offset, ch)) = chars.next() {
            if ch == '\\' && chars.peek().is_some_and(|(_, next)| *next == '`') {
                if let Some((_, span)) = current.as_mut() {
                    span.push('`');
                }
                chars.next();
            } else if ch == '`' {
                match current.take() {
                    Some(span) => spans.push(span),
                    None => current = Some((offset, String::new())),
                }
            } else if let Some((_, span)) = current.as_mut() {
                span.push(ch);
            }
        }
        spans
    }

    /// The nearest preceding line that labels a command, skipping blank
    /// lines, fences, and the shared shell disclosures.
    fn label_before(lines: &[&str], index: usize) -> String {
        lines[..index]
            .iter()
            .rev()
            .map(|line| line.trim())
            .find(|line| {
                !line.is_empty()
                    && !line.starts_with("```")
                    && !line.starts_with("Each command includes")
                    && !line.starts_with("The first form is written")
            })
            .unwrap_or_default()
            .to_string()
    }

    fn after_edit(label: &str) -> bool {
        let label = label.to_ascii_lowercase();
        AFTER_EDIT_MARKERS
            .iter()
            .any(|marker| label.contains(marker))
    }

    /// Every `ripr` command line the summary prints: bash fence lines and
    /// code spans that start with `ripr ` and carry a flag. Bare mentions
    /// such as `ripr gate evaluate` in prose are not commands to run.
    pub(super) fn printed_commands(summary: &str) -> Vec<PrintedCommand> {
        let lines = summary.lines().collect::<Vec<_>>();
        let mut commands = Vec::new();
        let mut fence: Option<(bool, String)> = None;
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if let Some(info) = trimmed.strip_prefix("```") {
                fence = match fence {
                    Some(_) => None,
                    None => Some((info == "bash", label_before(&lines, index))),
                };
                continue;
            }
            if let Some((is_bash, label)) = &fence {
                if *is_bash && trimmed.starts_with("ripr ") {
                    commands.push(PrintedCommand {
                        text: trimmed.to_string(),
                        after_edit: after_edit(label),
                    });
                }
                continue;
            }
            for (offset, span) in code_spans(line) {
                if !span.starts_with("ripr ") || !span.contains(" --") {
                    continue;
                }
                let before = line[..offset].trim();
                let label = if before.is_empty() || before == "-" {
                    label_before(&lines, index)
                } else {
                    before.to_string()
                };
                commands.push(PrintedCommand {
                    text: span,
                    after_edit: after_edit(&label),
                });
            }
        }
        commands
    }
}

/// #4005: the generated "Capture pull request diff" step must use the pinned
/// diff contract (same presentation pins as the production loaders), resolve
/// and retain the exact base/head identities, record the patch digest, and
/// fail closed instead of handing RIPR an ambient-presentation patch.
#[test]
fn generated_capture_step_uses_pinned_diff_contract() -> Result<(), Box<dyn Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-generated-capture-contract-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;

    let output = run_ripr_init(&root)?;
    assert!(
        output.status.success(),
        "ripr init failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let workflow = fs::read_to_string(root.join(".github/workflows/ripr.yml"))?;
    let step = capture_pull_request_diff_step(&workflow)
        .ok_or("generated workflow has no 'Capture pull request diff' step")?;
    for pin in [
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--unified=3",
        "--inter-hunk-context=0",
        "core.quotePath",
        "rev-parse",
        "sha256sum",
        "mktemp",
    ] {
        assert!(
            step.contains(pin),
            "capture step must pin {pin}; got:\n{step}"
        );
    }

    fs::remove_dir_all(root)?;
    Ok(())
}

/// #4005: docs/CI.md documents the same capture step the generator emits.
/// The doc copy and the template copy drifted before (unpinned `git diff
/// --binary` in both); the capture block must stay byte-identical so the
/// documented recipe cannot promise a different patch than `ripr init` ships.
#[test]
fn docs_ci_capture_step_matches_generated_template() -> Result<(), Box<dyn Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-generated-capture-doc-sync-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;

    let output = run_ripr_init(&root)?;
    assert!(
        output.status.success(),
        "ripr init failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let workflow = fs::read_to_string(root.join(".github/workflows/ripr.yml"))?;
    let generated = capture_pull_request_diff_step(&workflow)
        .ok_or("generated workflow has no 'Capture pull request diff' step")?;
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let doc = fs::read_to_string(manifest_dir.join("../../docs/CI.md"))?;
    let documented = capture_pull_request_diff_step(&doc)
        .ok_or("docs/CI.md has no 'Capture pull request diff' step")?;
    assert_eq!(
        generated, documented,
        "docs/CI.md capture step drifted from the generated template"
    );

    fs::remove_dir_all(root)?;
    Ok(())
}

/// #4005: the pinned capture flags must retain a source edit that a
/// configured textconv driver hides. Control first: the pre-repair recipe
/// (`diff --binary` with ambient presentation) yields an empty patch, which
/// proves the fixture hides the edit. Then the repaired flag set — the same
/// pins the generated template now carries — must retain the edit with its
/// three-line context and no color bytes.
#[test]
fn pinned_capture_flags_retain_edit_hidden_by_textconv() -> Result<(), Box<dyn Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-generated-capture-textconv-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("src"))?;
    fixture_git_ok(&root, &["init", "--initial-branch=main"])?;
    fixture_git_ok(
        &root,
        &["config", "--local", "user.name", "Capture Contract"],
    )?;
    fixture_git_ok(
        &root,
        &["config", "--local", "user.email", "capture@example.com"],
    )?;
    fixture_git_ok(&root, &["config", "--local", "commit.gpgsign", "false"])?;
    fs::write(root.join(".gitattributes"), "src/lib.rs diff=audit\n")?;
    fs::write(
        root.join("src/lib.rs"),
        "pub const A: u32 = 1;\npub const B: u32 = 2;\npub const C: u32 = 3;\npub const VALUE: u32 = 1;\npub const D: u32 = 4;\npub const E: u32 = 5;\npub const F: u32 = 6;\n",
    )?;
    fixture_git_ok(&root, &["add", "."])?;
    fixture_git_ok(&root, &["commit", "--quiet", "-m", "base"])?;
    fixture_git_ok(&root, &["tag", "capture-base"])?;
    fs::write(
        root.join("src/lib.rs"),
        "pub const A: u32 = 1;\npub const B: u32 = 2;\npub const C: u32 = 3;\npub const VALUE: u32 = 2;\npub const D: u32 = 4;\npub const E: u32 = 5;\npub const F: u32 = 6;\n",
    )?;
    fixture_git_ok(&root, &["add", "src/lib.rs"])?;
    fixture_git_ok(&root, &["commit", "--quiet", "-m", "edit"])?;
    // Git itself is the constant-output helper on Unix and Windows; no
    // shell script or executable bit needed.
    fixture_git_ok(
        &root,
        &["config", "--local", "diff.audit.textconv", "git --version"],
    )?;
    let range = "capture-base...HEAD";
    let hidden = fixture_git_output(&root, &["diff", "--binary", range])?;
    assert!(
        hidden.trim().is_empty(),
        "the constant textconv must hide the source edit"
    );
    // Repaired recipe: the presentation pins the generated "Capture pull
    // request diff" step now carries (init.rs template + docs/CI.md).
    let retained = fixture_git_output(
        &root,
        &[
            "-c",
            "core.quotePath=true",
            "diff",
            "--binary",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--unified=3",
            "--inter-hunk-context=0",
            range,
        ],
    )?;
    assert!(
        retained.contains("pub const VALUE"),
        "pinned capture must retain the source edit despite textconv"
    );
    assert!(
        retained.contains("pub const C: u32 = 3;") && retained.contains("pub const D: u32 = 4;"),
        "pinned capture must keep context around the edit"
    );
    assert!(
        !retained.contains('\u{1b}'),
        "pinned capture must not contain color"
    );

    fs::remove_dir_all(root)?;
    Ok(())
}

/// #4005: execute the generated capture step itself in a disposable
/// repository. Unix-only: the generated workflow declares `runs-on:
/// ubuntu-latest`, so `sh` plus coreutils/`jq` are the faithful runner.
/// The `${{ github.base_ref }}` expression is substituted with a real local
/// branch (there is no origin in a fixture); everything else runs verbatim,
/// so this fails when the template stops being executable shell.
#[cfg(unix)]
#[test]
fn generated_capture_step_runs_end_to_end() -> Result<(), Box<dyn Error>> {
    // Runner tools: the generated workflow targets ubuntu-latest, where
    // bash, coreutils, and jq exist. On minimal local Unix environments
    // without them, skip loudly instead of failing the suite.
    let tools = run_sh(
        "command -v bash >/dev/null && command -v sha256sum >/dev/null && command -v jq >/dev/null",
        std::env::temp_dir().as_path(),
    )?;
    if !tools.status.success() {
        eprintln!(
            "skipping generated_capture_step_runs_end_to_end: bash, sha256sum, or jq not available"
        );
        return Ok(());
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let init_root = std::env::temp_dir().join(format!(
        "ripr-generated-capture-exec-init-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&init_root)?;
    let output = run_ripr_init(&init_root)?;
    assert!(
        output.status.success(),
        "ripr init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let workflow = fs::read_to_string(init_root.join(".github/workflows/ripr.yml"))?;
    let step = capture_pull_request_diff_step(&workflow)
        .ok_or("generated workflow has no 'Capture pull request diff' step")?;
    let body: Vec<String> = step
        .lines()
        .skip_while(|line| !line.trim_start_matches(' ').starts_with("run: |"))
        .skip(1)
        .map(|line| {
            line.strip_prefix("          ")
                .ok_or("capture run-block line lost its 10-space indent")
                .map(str::to_string)
        })
        .collect::<Result<Vec<String>, &str>>()
        .map_err(|err| err.to_string())?;
    assert!(
        !body.is_empty(),
        "capture step has no shell body to execute"
    );

    let repo = std::env::temp_dir().join(format!(
        "ripr-generated-capture-exec-repo-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&repo)?;
    fixture_git_ok(&repo, &["init", "--initial-branch=main"])?;
    fixture_git_ok(&repo, &["config", "--local", "user.name", "Capture Exec"])?;
    fixture_git_ok(
        &repo,
        &[
            "config",
            "--local",
            "user.email",
            "capture-exec@example.com",
        ],
    )?;
    fixture_git_ok(&repo, &["config", "--local", "commit.gpgsign", "false"])?;
    fs::write(repo.join("probe.txt"), "before\n")?;
    fixture_git_ok(&repo, &["add", "."])?;
    fixture_git_ok(&repo, &["commit", "--quiet", "-m", "base"])?;
    fixture_git_ok(&repo, &["checkout", "--quiet", "-b", "feature"])?;
    fs::write(repo.join("probe.txt"), "after\n")?;
    fixture_git_ok(&repo, &["add", "probe.txt"])?;
    fixture_git_ok(&repo, &["commit", "--quiet", "-m", "edit"])?;
    let base_sha = fixture_git_output(&repo, &["rev-parse", "--verify", "main^{commit}"])?;
    let head_sha = fixture_git_output(&repo, &["rev-parse", "--verify", "HEAD^{commit}"])?;

    // Positive: the step resolves main, captures the edit, and retains a
    // receipt whose identities and byte count match the run.
    let script = body
        .join("\n")
        .replace("origin/${{ github.base_ref }}", "main");
    let run = run_sh(&script, &repo)?;
    assert!(
        run.status.success(),
        "capture step failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let patch = fs::read(repo.join("target/ripr/reports/pr.diff"))?;
    assert!(
        String::from_utf8_lossy(&patch).contains("after"),
        "captured patch must contain the feature-branch edit"
    );
    let receipt: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        repo.join("target/ripr/reports/pr-diff.receipt.json"),
    )?)?;
    let field = |name: &str| {
        receipt
            .get(name)
            .ok_or(format!("receipt is missing field {name:?}"))
    };
    let as_str = |name: &str| {
        field(name)?
            .as_str()
            .ok_or(format!("receipt field {name:?} is not a string"))
    };
    assert_eq!(
        as_str("tool")?,
        "ripr",
        "receipt must identify its producer"
    );
    assert_eq!(
        as_str("kind")?,
        "pr-diff-receipt",
        "receipt must identify its kind"
    );
    assert_eq!(
        as_str("base_ref")?,
        "main",
        "receipt must retain the requested base ref"
    );
    assert_eq!(
        as_str("base_sha")?,
        base_sha.trim(),
        "receipt base_sha must equal the resolved base commit"
    );
    assert_eq!(
        as_str("head_sha")?,
        head_sha.trim(),
        "receipt head_sha must equal the resolved head commit"
    );
    assert_eq!(
        field("byte_count")?
            .as_u64()
            .ok_or("receipt field \"byte_count\" is not a number")?,
        patch.len() as u64,
        "receipt byte count must match the patch"
    );
    let expected_digest = {
        use sha2::{Digest, Sha256};
        Sha256::digest(&patch)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    assert_eq!(
        as_str("sha256")?,
        expected_digest.as_str(),
        "receipt digest must match the patch bytes"
    );

    // Negative: an unresolvable base fails closed with the named error
    // instead of handing RIPR an absent patch.
    let missing = body
        .join("\n")
        .replace("origin/${{ github.base_ref }}", "nonexistent-base-branch");
    let run = run_sh(&missing, &repo)?;
    assert!(
        !run.status.success(),
        "capture step must fail when the base cannot be resolved"
    );
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("cannot resolve base ref"),
        "missing base must name the failure; got:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );

    // Empty range: base == HEAD is an honest zero-change run — the step
    // succeeds with an empty patch and a zero byte count (exercises the
    // mktemp zero-change proof path with zero NUL bytes).
    let empty = body
        .join("\n")
        .replace("origin/${{ github.base_ref }}", "feature");
    let run = run_sh(&empty, &repo)?;
    assert!(
        run.status.success(),
        "capture step must accept a real zero-change range; got:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let empty_patch = fs::read(repo.join("target/ripr/reports/pr.diff"))?;
    assert!(
        empty_patch.is_empty(),
        "zero-change range must capture an empty patch"
    );
    let empty_receipt: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        repo.join("target/ripr/reports/pr-diff.receipt.json"),
    )?)?;
    assert_eq!(
        empty_receipt
            .get("byte_count")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "zero-change receipt must record a zero byte count"
    );

    fs::remove_dir_all(init_root)?;
    fs::remove_dir_all(repo)?;
    Ok(())
}

/// #4089: the generated annotation step must keep path and message bytes.
/// The old `@tsv` | `read` loop stored jq's transport escapes, so a later
/// GitHub workflow-command decode could not recover a backslash, tab, or
/// line break.
#[cfg(unix)]
#[test]
fn generated_annotation_script_preserves_path_and_message_bytes() -> Result<(), Box<dyn Error>> {
    let tools = run_sh(
        "command -v bash >/dev/null && command -v jq >/dev/null",
        std::env::temp_dir().as_path(),
    )?;
    if !tools.status.success() {
        eprintln!(
            "skipping generated_annotation_script_preserves_path_and_message_bytes: bash or jq missing"
        );
        return Ok(());
    }

    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-annotation-bytes-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;
    let output = run_ripr_init(&root)?;
    if !output.status.success() {
        let _ = fs::remove_dir_all(&root);
        return Err(format!(
            "ripr init failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let workflow = fs::read_to_string(root.join(".github/workflows/ripr.yml"))?;
    let script = annotation_run_script(&workflow)?;

    let path_backslash = "src\\app.py";
    let path_newline = "src/a\nb.py";
    let reason_mixed = "line one\nline\ttwo\\nkept";
    let repair = "cargo test -- --exact";
    fs::create_dir_all(root.join("target/ripr/review"))?;
    fs::write(
        root.join("target/ripr/review/comments.json"),
        format!(
            r#"{{
  "comments": [
    {{
      "placement": {{"path": {path}, "line": 12}},
      "reason": {reason},
      "llm_guidance": {{"repair_command": {repair_json}}}
    }},
    {{
      "placement": {{"path": {backslash}, "line": 4}},
      "reason": "plain",
      "llm_guidance": {{"repair_command": ""}}
    }},
    {{
      "placement": {{"path": {newline}, "line": 7}},
      "reason": {mixed},
      "llm_guidance": {{"repair_command": "null"}}
    }},
    {{
      "placement": {{"path": "src/absent.py", "line": 1}},
      "reason": "no repair field",
      "llm_guidance": {{"repair_command": null}}
    }}
  ]
}}"#,
            path = json_string("src/app.py"),
            reason = json_string("Result::Err, 100%"),
            repair_json = json_string(repair),
            backslash = json_string(path_backslash),
            newline = json_string(path_newline),
            mixed = json_string(reason_mixed),
        ),
    )?;

    let ran = run_sh(&script, &root)?;
    let stdout = String::from_utf8_lossy(&ran.stdout).to_string();
    let stderr = String::from_utf8_lossy(&ran.stderr).to_string();
    if !ran.status.success() {
        let _ = fs::remove_dir_all(&root);
        return Err(format!("annotation script failed\nstderr: {stderr}\nstdout: {stdout}").into());
    }
    let mut warnings = Vec::new();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        warnings.push(parse_warning(line.trim())?);
    }
    let _ = fs::remove_dir_all(&root);

    let ordinary = warnings
        .iter()
        .find(|row| row.0 == "src/app.py")
        .ok_or("missing ordinary annotation")?;
    assert_eq!(ordinary.1, "12");
    assert_eq!(ordinary.2, "RIPR targeted test guidance");
    assert_eq!(
        ordinary.3,
        format!("Result::Err, 100% Start the repair: {repair}")
    );

    let slashed = warnings
        .iter()
        .find(|row| row.0 == path_backslash)
        .ok_or("backslash path was rewritten")?;
    assert_eq!(slashed.1, "4");
    assert_eq!(slashed.3, "plain");

    let broken = warnings
        .iter()
        .find(|row| row.0 == path_newline)
        .ok_or("newline path was rewritten")?;
    assert_eq!(broken.1, "7");
    assert_eq!(broken.3, reason_mixed);

    let absent = warnings
        .iter()
        .find(|row| row.0 == "src/absent.py")
        .ok_or("null repair command dropped the row")?;
    assert_eq!(absent.3, "no repair field");
    assert!(!absent.3.contains("Start the repair"));
    Ok(())
}

/// Unix-only like its callers: the shell-backed tests that use this
/// helper are `#[cfg(unix)]`, and an ungated helper is dead code (and a
/// `-D warnings` failure) on Windows builds.
#[cfg(unix)]
fn annotation_run_script(workflow: &str) -> Result<String, String> {
    let marker = "- name: Emit RIPR PR guidance annotations";
    let start = workflow.find(marker).ok_or("missing annotation step")?;
    let rest = &workflow[start..];
    let run_marker = "\n        run: |\n";
    let run_at = rest.find(run_marker).ok_or("missing annotation run")?;
    let body = &rest[run_at + run_marker.len()..];
    let end = body
        .find("\n      - name:")
        .ok_or("annotation step does not end")?;
    let script = body[..end].trim_end();
    if !script.contains("def escape_data:") || script.contains("@tsv") {
        return Err("annotation script is not the jq encoder".to_string());
    }
    Ok(script.to_string())
}

/// Unix-only like its callers: the shell-backed tests that use this
/// helper are `#[cfg(unix)]`, and an ungated helper is dead code (and a
/// `-D warnings` failure) on Windows builds.
#[cfg(unix)]
fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Unix-only like its callers: the shell-backed tests that use this
/// helper are `#[cfg(unix)]`, and an ungated helper is dead code (and a
/// `-D warnings` failure) on Windows builds.
#[cfg(unix)]
fn decode_github(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16)
                && matches!(byte, b'%' | b'\r' | b'\n' | b':' | b',')
            {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).map_err(|err| format!("annotation is not utf-8: {err}"))
}

/// Unix-only like its callers: the shell-backed tests that use this
/// helper are `#[cfg(unix)]`, and an ungated helper is dead code (and a
/// `-D warnings` failure) on Windows builds.
#[cfg(unix)]
fn parse_warning(line: &str) -> Result<(String, String, String, String), String> {
    let rest = line
        .strip_prefix("::warning ")
        .ok_or_else(|| format!("not a warning: {line}"))?;
    let (props, message) = rest
        .split_once("::")
        .ok_or_else(|| format!("no message separator: {line}"))?;
    let mut file = None;
    let mut line_no = None;
    let mut title = None;
    for part in props.split(',') {
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| format!("bad property: {part}"))?;
        let decoded = decode_github(value)?;
        match key {
            "file" => file = Some(decoded),
            "line" => line_no = Some(decoded),
            "title" => title = Some(decoded),
            other => return Err(format!("unexpected property {other}")),
        }
    }
    Ok((
        file.ok_or("missing file")?,
        line_no.ok_or("missing line")?,
        title.ok_or("missing title")?,
        decode_github(message)?,
    ))
}

/// One spawn site for the built-binary `ripr init` invocations below
/// (process-policy bound).
fn run_ripr_init(root: &std::path::Path) -> Result<std::process::Output, Box<dyn Error>> {
    Ok(Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(["init", "--root"])
        .arg(root)
        .args(["--ci", "github"])
        .output()?)
}

/// One spawn site for executing extracted capture-step shell (process-policy
/// bound). GitHub Actions runs a `run:` step without `shell:` as `bash -e`
/// on ubuntu-latest, so the helper mirrors that invocation instead of a
/// bare `sh -c`, which would not enable errexit. Unix-only, so Windows
/// builds never see a dead helper.
#[cfg(unix)]
fn run_sh(script: &str, cwd: &std::path::Path) -> Result<std::process::Output, Box<dyn Error>> {
    Ok(Command::new("bash")
        .args(["-e", "-c", script])
        .current_dir(cwd)
        .output()?)
}

/// Extract the "Capture pull request diff" step block: from its `- name:`
/// line through (excluding) the next step's `- name:` line.
fn capture_pull_request_diff_step(text: &str) -> Option<String> {
    let marker = "- name: Capture pull request diff";
    let start = text.find(marker)?;
    let rest = &text[start..];
    let end = rest[marker.len()..]
        .find("\n      - name: ")
        .map(|offset| marker.len() + offset)
        .unwrap_or(rest.len());
    Some(rest[..end].trim_end().to_string())
}
