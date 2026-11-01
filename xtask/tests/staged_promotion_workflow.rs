//! Exercise the workflow's real preview-identity gate against Git objects.
use std::fs;
use std::path::Path;

fn identity_gate() -> Result<String, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("missing root")?;
    let workflow = fs::read_to_string(root.join(".github/workflows/source-promotion-contract.yml"))
        .map_err(|e| e.to_string())?;
    let start = workflow
        .find("          # staged-preview-begin\n")
        .ok_or("staged preview identity gate is missing")?;
    let rest = &workflow[start..];
    let end = rest
        .find("          # staged-preview-end\n")
        .ok_or("staged preview identity gate is unterminated")?;
    Ok(rest[..end]
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n"))
}

#[test]
fn workflow_exposes_a_staged_preview_identity_gate() -> Result<(), String> {
    let gate = identity_gate()?;
    if !gate.contains("refs/pull/") {
        return Err("staged gate does not fetch the real PR preview".into());
    }
    Ok(())
}

#[cfg(unix)]
mod git_fixture {
    use super::identity_gate;
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
    fn git(root: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
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
            std::fs::write(&preflight,format!("{{\"mode\":\"{mode}\",\"dry_merge\":{{\"reviewed_resolved_tree\":\"{tree}\"}}}}"))
                .map_err(|e|e.to_string())?;
            let script = format!(
                "set -e\nfail() {{ printf '%s\\n' \"$1\" >&2; exit 1; }}\n{}\n",
                identity_gate()?
            );
            Command::new("bash")
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
}
