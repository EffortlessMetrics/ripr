//! Exercise the workflow's real preview-identity gate against Git objects.
use std::fs;
use std::path::Path;

fn workflow_section(begin: &str, end: &str) -> Result<String, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("missing root")?;
    let workflow = fs::read_to_string(root.join(".github/workflows/source-promotion-contract.yml"))
        .map_err(|e| e.to_string())?;
    let begin = format!("          # {begin}\n");
    let end = format!("          # {end}\n");
    if workflow.matches(begin.as_str()).count() != 1 || workflow.matches(end.as_str()).count() != 1
    {
        return Err("workflow gate markers are missing or duplicated".into());
    }
    let start = workflow
        .find(begin.as_str())
        .ok_or("workflow gate start marker is missing")?;
    let rest = &workflow[start..];
    let end = rest
        .find(end.as_str())
        .ok_or("workflow gate end marker is missing or precedes the start")?;
    Ok(rest[..end]
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn identity_gate() -> Result<String, String> {
    workflow_section("staged-preview-begin", "staged-preview-end")
}

#[test]
fn composition_check_revalidates_body_edits() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("missing root")?;
    let path = root.join(".github/workflows/source-promotion-composition.yml");
    let workflow = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if !workflow.contains("types: [opened, synchronize, reopened, edited]") {
        return Err("promotion control body edits must retrigger qualification".into());
    }
    Ok(())
}

#[test]
fn workflow_exposes_a_staged_preview_identity_gate() -> Result<(), String> {
    let gate = identity_gate()?;
    if !gate.contains("refs/pull/") {
        return Err("staged gate does not fetch the real PR preview".into());
    }
    Ok(())
}

fn staged_post_gate() -> Result<String, String> {
    workflow_section("staged-post-begin", "staged-post-end")
}

fn staged_receipt_gate() -> Result<String, String> {
    workflow_section("staged-receipt-begin", "staged-receipt-end")
}

#[test]
fn workflow_records_staged_preview_and_actual_separately() -> Result<(), String> {
    let gate = staged_post_gate()?;
    if !gate.contains("staged_preview") || !gate.contains("staged_actual") {
        return Err("staged workflow must verify preview and actual separately".into());
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn documented_and_rendered_merge_commands_bind_live_head_in_both_modes() -> Result<(), String> {
    use std::process::Command;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("missing root")?;
    let workflow = fs::read_to_string(root.join(".github/workflows/source-promotion-contract.yml"))
        .map_err(|e| e.to_string())?;
    let spec =
        fs::read_to_string(root.join("docs/specs/RIPR-SPEC-0177-source-promotion-ci-contract.md"))
            .map_err(|e| e.to_string())?;
    let documented = spec
        .split("```bash\n")
        .nth(1)
        .and_then(|block| block.split("```").next())
        .ok_or("missing documented merge command")?;
    let guard_start = workflow
        .find("          case \"$merge_command\" in")
        .ok_or("missing command guard")?;
    let guard = workflow[guard_start..]
        .split("          esac")
        .next()
        .ok_or("missing guard end")?;
    let guard = format!("{guard}\n          esac\n");
    let render_start = workflow
        .find("            echo \"gh pr merge $PR_NUMBER")
        .ok_or("missing command renderer")?;
    let renderer = workflow[render_start..]
        .split("            echo '```'")
        .next()
        .ok_or("missing renderer end")?;
    let normalize = |command: &str| {
        command
            .replace("\\\n", " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    for (mode, head, join) in [
        (
            "two_parent_join",
            "1111111111111111111111111111111111111111",
            "1111111111111111111111111111111111111111",
        ),
        (
            "fast_forward",
            "2222222222222222222222222222222222222222",
            "3333333333333333333333333333333333333333",
        ),
    ] {
        let expected = format!(
            "gh pr merge 123 --repo EffortlessMetrics/ripr --merge --match-head-commit {head}"
        );
        let command = normalize(
            &documented
                .replace("<PR>", "123")
                .replace("<PR_HEAD_SHA>", head)
                .replace("<JOIN_SHA>", join),
        );
        if command != expected {
            return Err(format!("{mode}: documentation binds the wrong identity"));
        }
        let rendered = Command::new("bash")
            .args(["-c", renderer])
            .env("PR_NUMBER", "123")
            .env("PR_HEAD", head)
            .output()
            .map_err(|e| e.to_string())?;
        if !rendered.status.success()
            || normalize(&String::from_utf8_lossy(&rendered.stdout)) != expected
        {
            return Err(format!(
                "{mode}: rendered command differs from specification"
            ));
        }
        for (candidate, accepted) in [
            (command, true),
            (
                expected.replace(head, "4444444444444444444444444444444444444444"),
                false,
            ),
        ] {
            let output = Command::new("bash")
                .args(["-c", &format!("fail() {{ exit 1; }}\n{guard}")])
                .env("PR_NUMBER", "123")
                .env("PR_HEAD", head)
                .env("merge_command", candidate)
                .output()
                .map_err(|e| e.to_string())?;
            if output.status.success() != accepted {
                return Err(format!(
                    "{mode}: real workflow guard accepted the wrong identity"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
mod git_fixture {
    use super::{identity_gate, staged_post_gate, staged_receipt_gate, workflow_section};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture {
        root: PathBuf,
        public: String,
        swarm: String,
        tree: String,
        preview: String,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    fn isolated(command: &mut Command) -> &mut Command {
        // The fixture's Git graph must not depend on the developer's hooks,
        // commit-signing policy, init templates, or system/global aliases.
        // In particular, inherited GIT_DIR/GIT_WORK_TREE/GIT_INDEX_FILE and
        // object-directory overrides must never redirect fixture writes into
        // the repository running these tests. Explicit fixture identity below
        // is applied only after inherited Git state has been removed.
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                command.env_remove(name);
            }
        }
        command
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
    }
    fn git(root: &Path, args: &[&str]) -> Result<String, String> {
        let output = isolated(&mut Command::new("git"))
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        String::from_utf8(output.stdout)
            .map(|s| s.trim().to_string())
            .map_err(|e| e.to_string())
    }
    impl Fixture {
        fn new() -> Result<Self, String> {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos();
            let temp_base = std::env::temp_dir();
            std::fs::create_dir_all(&temp_base).map_err(|e| e.to_string())?;
            let root = temp_base.join(format!(
                "ripr-staged-workflow-{}-{stamp}",
                std::process::id()
            ));
            std::fs::create_dir(&root).map_err(|e| e.to_string())?;
            git(&root, &["init", "-q"])?;
            std::fs::write(root.join("file"), "public\n").map_err(|e| e.to_string())?;
            git(&root, &["add", "file"])?;
            git(&root, &["commit", "-qm", "public"])?;
            let public = git(&root, &["rev-parse", "HEAD"])?;
            std::fs::write(root.join("file"), "swarm\n").map_err(|e| e.to_string())?;
            git(&root, &["add", "file"])?;
            git(&root, &["commit", "-qm", "swarm"])?;
            let swarm = git(&root, &["rev-parse", "HEAD"])?;
            let tree = git(&root, &["rev-parse", "HEAD^{tree}"])?;
            let preview = git(
                &root,
                &[
                    "commit-tree",
                    &tree,
                    "-p",
                    &public,
                    "-p",
                    &swarm,
                    "-m",
                    "preview",
                ],
            )?;
            git(&root, &["update-ref", "refs/pull/42/merge", &preview])?;
            let root_arg = root.to_str().ok_or("non-UTF-8 fixture root")?;
            git(&root, &["remote", "add", "origin", root_arg])?;
            Ok(Self {
                root,
                public,
                swarm,
                tree,
                preview,
            })
        }
        fn run(&self, mode: &str, identity: [&str; 6]) -> Result<Output, String> {
            let [public, base, head, swarm, preview, tree] = identity;
            let preflight = self.root.join("preflight.json");
            std::fs::write(&preflight,format!("{{\"mode\":\"{mode}\",\"dry_merge\":{{\"reviewed_resolved_tree\":null,\"preview_tree\":\"{tree}\"}}}}"))
                .map_err(|e|e.to_string())?;
            let script = format!(
                "set -e\nfail() {{ printf '%s\\n' \"$1\" >&2; exit 1; }}\n{}\n",
                identity_gate()?
            );
            isolated(&mut Command::new("bash"))
                .arg("-c")
                .arg(script)
                .current_dir(&self.root)
                .env("source_main", public)
                .env("BASE_SHA", base)
                .env("PR_HEAD", head)
                .env("swarm_parent", swarm)
                .env("join_head", preview)
                .env("PR_NUMBER", "42")
                .env("preflight", preflight)
                .output()
                .map_err(|e| e.to_string())
        }
        fn staged(
            &self,
            public: &str,
            base: &str,
            head: &str,
            swarm: &str,
            preview: &str,
            tree: &str,
        ) -> Result<Output, String> {
            self.run("fast_forward", [public, base, head, swarm, preview, tree])
        }
    }
    fn rejects(output: Output, reason: &str) -> Result<(), String> {
        let text = String::from_utf8_lossy(&output.stderr);
        if output.status.success() || !text.contains(reason) {
            return Err(format!(
                "expected rejection {reason:?}; status={}, stderr={text}",
                output.status
            ));
        }
        Ok(())
    }
    #[test]
    fn actual_workflow_gate_checks_staged_preview_and_preserves_legacy() -> Result<(), String> {
        let f = Fixture::new()?;
        let accepted = f.staged(
            &f.public, &f.public, &f.swarm, &f.swarm, &f.preview, &f.tree,
        )?;
        if !accepted.status.success() {
            return Err(format!(
                "valid staged preview rejected: {}",
                String::from_utf8_lossy(&accepted.stderr)
            ));
        }
        rejects(
            f.staged(
                &f.public, &f.public, &f.public, &f.swarm, &f.preview, &f.tree,
            )?,
            "staged swarm parent does not equal",
        )?;
        rejects(
            f.staged(&f.public, &f.swarm, &f.swarm, &f.swarm, &f.preview, &f.tree)?,
            "staged source parent does not equal",
        )?;
        rejects(
            f.staged(&f.public, &f.public, &f.swarm, &f.swarm, &f.swarm, &f.tree)?,
            "control join_head does not equal",
        )?;
        let old_tree = git(&f.root, &["rev-parse", &format!("{}^{{tree}}", f.public)])?;
        rejects(
            f.staged(
                &f.public, &f.public, &f.swarm, &f.swarm, &f.preview, &old_tree,
            )?,
            "PR head tree differs",
        )?;
        let reversed = git(
            &f.root,
            &[
                "commit-tree",
                &f.tree,
                "-p",
                &f.swarm,
                "-p",
                &f.public,
                "-m",
                "reversed",
            ],
        )?;
        git(&f.root, &["update-ref", "refs/pull/42/merge", &reversed])?;
        rejects(
            f.staged(&f.public, &f.public, &f.swarm, &f.swarm, &reversed, &f.tree)?,
            "PR preview parents are not ordered",
        )?;
        let wrong_tree_preview = git(
            &f.root,
            &[
                "commit-tree",
                &old_tree,
                "-p",
                &f.public,
                "-p",
                &f.swarm,
                "-m",
                "wrong tree",
            ],
        )?;
        git(
            &f.root,
            &["update-ref", "refs/pull/42/merge", &wrong_tree_preview],
        )?;
        rejects(
            f.staged(
                &f.public,
                &f.public,
                &f.swarm,
                &f.swarm,
                &wrong_tree_preview,
                &f.tree,
            )?,
            "PR preview tree differs",
        )?;
        let unrelated = git(&f.root, &["commit-tree", &f.tree, "-m", "unrelated"])?;
        rejects(
            f.staged(
                &unrelated,
                &unrelated,
                &f.swarm,
                &f.swarm,
                &wrong_tree_preview,
                &f.tree,
            )?,
            "staged source is not an ancestor",
        )?;
        let legacy = f.run(
            "two_parent_join",
            [&f.public, &f.public, &f.swarm, &f.swarm, &f.swarm, &f.tree],
        )?;
        if !legacy.status.success() {
            return Err(format!(
                "legacy route rejected: {}",
                String::from_utf8_lossy(&legacy.stderr)
            ));
        }
        rejects(
            f.run(
                "two_parent_join",
                [
                    &f.public, &f.public, &f.swarm, &f.swarm, &f.preview, &f.tree,
                ],
            )?,
            "declared join_head does not equal the live PR head",
        )?;
        rejects(
            f.run(
                "unknown",
                [&f.public, &f.public, &f.swarm, &f.swarm, &f.swarm, &f.tree],
            )?,
            "unsupported preflight mode",
        )?;
        Ok(())
    }
    #[test]
    fn actual_post_gate_checks_distinct_preview_and_merge() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new()?;
        let actual = git(
            &f.root,
            &[
                "commit-tree",
                &f.tree,
                "-p",
                &f.public,
                "-p",
                &f.swarm,
                "-m",
                "actual",
            ],
        )?;
        git(&f.root, &["update-ref", "refs/heads/main", &actual])?;
        git(&f.root, &["checkout", "--quiet", "--detach", &actual])?;
        let preflight = f.root.join("post-preflight.json");
        std::fs::write(&preflight, format!(
            "{{\"schema\":\"ripr.source_promotion_preflight.v2\",\"mode\":\"fast_forward\",\"swarm_parent\":\"{}\",\"dry_merge\":{{\"reviewed_resolved_tree\":null,\"preview_tree\":\"{}\"}},\"acceptance\":{{\"subject\":{{\"candidate_sha\":\"{}\",\"candidate_tree\":\"{}\"}}}}}}",
            f.swarm, f.tree, f.swarm, f.tree
        )).map_err(|e| e.to_string())?;
        let preview_object = f.root.join("preview.git-object");
        let raw = isolated(&mut Command::new("git"))
            .current_dir(&f.root)
            .args(["--no-replace-objects", "cat-file", "commit", &f.preview])
            .output()
            .map_err(|e| e.to_string())?;
        if !raw.status.success() {
            return Err("could not read preview commit object".into());
        }
        std::fs::write(&preview_object, &raw.stdout).map_err(|e| e.to_string())?;
        let verifier = f.root.join("verifier");
        std::fs::write(&verifier, r##"#!/bin/sh
set -eu
shift 2
head= source= main= out=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --join-head) head=$2;; --source-main) source=$2;; --main-head) main=$2;; --out) out=$2;;
  esac
  shift 2
done
mkdir -p "$out"
if [ -n "$main" ]; then reach=passed; else reach=not_run; fi
jq -n --arg head "$head" --arg source "$source" --arg swarm "$MOCK_SWARM" --arg tree "$MOCK_TREE" --arg main "$main" --arg reach "$reach" \
  '{status:"verified",join_head:$head,source_main:$source,main_head:(if $main == "" then null else $main end),parents:[$source,$swarm],tree:$tree,checks:{main_reachability:$reach,native_selection_acceptance_current:true,native_qualification_acceptance_current:true}}' > "$out/source-promotion-verification.json"
"##).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&verifier, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let runner = f.root.join("post-runner");
        let source = f.root.to_str().ok_or("non-UTF-8 source fixture")?;
        let destination = runner.to_str().ok_or("non-UTF-8 runner fixture")?;
        git(
            &f.root,
            &[
                "clone",
                "--quiet",
                "--no-local",
                "--no-tags",
                "--branch",
                "main",
                source,
                destination,
            ],
        )?;
        let absent = isolated(&mut Command::new("git"))
            .current_dir(&runner)
            .args(["cat-file", "-e", &f.preview])
            .status()
            .map_err(|e| e.to_string())?;
        if absent.success() {
            return Err("fixture unexpectedly carried the preview object".into());
        }
        let out = f.root.join("post-out");
        let run = |main: &str, tree: &str| -> Result<Output, String> {
            if out.exists() {
                std::fs::remove_dir_all(&out).map_err(|e| e.to_string())?;
            }
            let script = format!("set -euo pipefail\n{}\n", staged_post_gate()?);
            isolated(&mut Command::new("bash"))
                .arg("-c")
                .arg(script)
                .current_dir(&runner)
                .env("PREFLIGHT", &preflight)
                .env("PREVIEW_OBJECT", &preview_object)
                .env("RESOLUTION_MANIFEST", f.root.join("resolution.json"))
                .env("SOURCE_PARENT", &f.public)
                .env("SOURCE_MAIN", &f.public)
                .env("JOIN_HEAD", &f.preview)
                .env("MAIN_HEAD", main)
                .env("TRUSTED_VERIFIER", &verifier)
                .env("SOURCE_PROMOTION_OUT", &out)
                .env("MOCK_SWARM", &f.swarm)
                .env("MOCK_TREE", tree)
                .output()
                .map_err(|e| e.to_string())
        };
        let accepted = run(&actual, &f.tree)?;
        if !accepted.status.success() {
            return Err(format!(
                "valid staged actual rejected: {}",
                String::from_utf8_lossy(&accepted.stderr)
            ));
        }
        let receipt = std::fs::read_to_string(out.join("source-promotion-verification.json"))
            .map_err(|e| e.to_string())?;
        if !receipt.contains(&actual) || receipt.contains(&f.preview) {
            return Err("actual receipt did not bind actual separately".into());
        }
        std::fs::write(&preview_object, b"tampered").map_err(|e| e.to_string())?;
        if run(&actual, &f.tree)?.status.success() {
            return Err("tampered preview object was accepted".into());
        }
        std::fs::write(&preview_object, &raw.stdout).map_err(|e| e.to_string())?;
        if !run(&actual, &f.tree)?.status.success() {
            return Err("restored preview object was rejected".into());
        }
        let receipt_script = format!("set -euo pipefail\n{}\n", staged_receipt_gate()?);
        let write_receipt = |outcome: &str| -> Result<Output, String> {
            isolated(&mut Command::new("bash"))
                .arg("-c")
                .arg(&receipt_script)
                .current_dir(&f.root)
                .env("PREFLIGHT", &preflight)
                .env("SOURCE_PARENT", &f.public)
                .env("SOURCE_MAIN", &f.public)
                .env("JOIN_HEAD", &f.preview)
                .env("MAIN_HEAD", &actual)
                .env("CONTROL_COMMIT", &f.public)
                .env("POST_VERIFY_OUTCOME", outcome)
                .env("SOURCE_PROMOTION_OUT", &out)
                .output()
                .map_err(|e| e.to_string())
        };
        if !write_receipt("success")?.status.success() {
            return Err("could not write staged receipt".into());
        }
        let receipt_path = out.join("source-promotion-post-merge-contract.json");
        let check = Command::new("jq").args(["-e", "--arg", "preview", &f.preview, "--arg", "actual", &actual,
            ".schema == \"ripr.source_promotion_staged_post_merge_contract.v1\" and .status == \"verified\" and .preview_head == $preview and .actual_merge_head == $actual"])
            .arg(&receipt_path).output().map_err(|e| e.to_string())?;
        if !check.status.success() {
            return Err("staged receipt did not bind preview and actual".into());
        }
        if !write_receipt("failure")?.status.success() {
            return Err("could not write failure receipt".into());
        }
        let check = Command::new("jq")
            .args(["-e", ".status == \"rejected\""])
            .arg(&receipt_path)
            .output()
            .map_err(|e| e.to_string())?;
        if !check.status.success() {
            return Err("failed workflow reused a verified receipt".into());
        }
        let wrong = git(
            &f.root,
            &[
                "commit-tree",
                &f.tree,
                "-p",
                &f.swarm,
                "-p",
                &f.public,
                "-m",
                "wrong",
            ],
        )?;
        // The negative must reach the parent-order check: wrong must exist in
        // the runner AND be the actual current origin/main tip. If it exists
        // only locally, the later origin-main identity check rejects it even
        // if the parent-order assertion was accidentally removed.
        git(&f.root, &["update-ref", "refs/heads/main", &wrong])?;
        git(&runner, &["fetch", "origin", "refs/heads/main"])?;
        let present = git(&runner, &["cat-file", "-t", &wrong])?;
        if present != "commit" {
            return Err("wrong-parent commit is absent from the execution repository".into());
        }
        let live = git(&runner, &["ls-remote", "origin", "refs/heads/main"])?;
        if live.split_whitespace().next() != Some(wrong.as_str()) {
            return Err("wrong-parent commit is not the current origin/main tip".into());
        }
        if run(&wrong, &f.tree)?.status.success() {
            return Err("wrong actual parents were accepted".into());
        }
        git(&f.root, &["update-ref", "refs/heads/main", &actual])?;
        git(&runner, &["fetch", "origin", "refs/heads/main"])?;
        if run(&actual, &f.public)?.status.success() {
            return Err("forged verifier tree was accepted".into());
        }
        git(&f.root, &["update-ref", "refs/heads/main", &f.swarm])?;
        if run(&actual, &f.tree)?.status.success() {
            return Err("moved public main was accepted".into());
        }
        Ok(())
    }
    #[test]
    fn retained_preview_requires_exact_regular_git_object() -> Result<(), String> {
        let f = Fixture::new()?;
        let control = f.root.join("ripr-promotion-control");
        std::fs::create_dir(&control).map_err(|e| e.to_string())?;
        git(&control, &["init", "-q"])?;
        let name = "docs/release/source-promotion/preview-commit.git-object";
        let path = control.join(name);
        std::fs::create_dir_all(path.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
        let object = isolated(&mut Command::new("git"))
            .current_dir(&f.root)
            .args(["cat-file", "commit", &f.preview])
            .output()
            .map_err(|e| e.to_string())?;
        if !object.status.success() {
            return Err("cannot read preview".into());
        }
        std::fs::write(&path, &object.stdout).map_err(|e| e.to_string())?;
        git(&control, &["add", name])?;
        git(&control, &["commit", "-qm", "retain preview"])?;
        let valid = git(&control, &["rev-parse", "HEAD"])?;
        std::fs::write(&path, b"tampered").map_err(|e| e.to_string())?;
        git(&control, &["add", name])?;
        git(&control, &["commit", "-qm", "tamper"])?;
        let tampered = git(&control, &["rev-parse", "HEAD"])?;
        git(&control, &["rm", name])?;
        git(&control, &["commit", "-qm", "remove"])?;
        let missing = git(&control, &["rev-parse", "HEAD"])?;
        let script = format!(
            "set -euo pipefail\n{}",
            workflow_section("staged-retain-begin", "staged-retain-end")?
        );
        let run = |commit: &str| {
            isolated(&mut Command::new("bash"))
                .args(["-c", &script])
                .current_dir(&f.root)
                .env("RUNNER_TEMP", &f.root)
                .env("CONTROL_COMMIT", commit)
                .env("JOIN_HEAD", &f.preview)
                .env("PREVIEW_OBJECT_PATH", name)
                .output()
                .map_err(|e| e.to_string())
        };
        if !run(&valid)?.status.success() {
            return Err("valid retained preview rejected".into());
        }
        if run(&tampered)?.status.success() || run(&missing)?.status.success() {
            return Err("invalid retained preview accepted".into());
        }
        Ok(())
    }
}
