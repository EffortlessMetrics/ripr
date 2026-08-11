use std::path::Path;

pub(crate) const AGENT_LOOP_COMMAND_TEMPLATE_VERSION: &str = "0.1";

pub(crate) const WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT: &str =
    "target/ripr/workflow/before.repo-exposure.json";
pub(crate) const WORKFLOW_AFTER_SNAPSHOT_ARTIFACT: &str =
    "target/ripr/workflow/after.repo-exposure.json";
pub(crate) const WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT: &str =
    "target/ripr/workflow/analysis-outcome.json";
pub(crate) const WORKFLOW_MANIFEST_ARTIFACT: &str = "target/ripr/workflow/workflow.json";
pub(crate) const WORKFLOW_COMMANDS_MARKDOWN_ARTIFACT: &str = "target/ripr/workflow/commands.md";
pub(crate) const WORKFLOW_AGENT_SEAM_PACKETS_ARTIFACT: &str =
    "target/ripr/workflow/agent-seam-packets.json";
pub(crate) const WORKFLOW_AGENT_PACKET_ARTIFACT: &str = "target/ripr/workflow/agent-packet.json";
pub(crate) const WORKFLOW_AGENT_BRIEF_ARTIFACT: &str = "target/ripr/workflow/agent-brief.json";
pub(crate) const WORKFLOW_AGENT_VERIFY_ARTIFACT: &str = "target/ripr/workflow/agent-verify.json";
pub(crate) const WORKFLOW_AGENT_RECEIPT_ARTIFACT: &str = "target/ripr/reports/agent-receipt.json";

pub(crate) const PILOT_BEFORE_SNAPSHOT_ARTIFACT: &str = "target/ripr/pilot/repo-exposure.json";
pub(crate) const PILOT_AFTER_SNAPSHOT_ARTIFACT: &str = "target/ripr/pilot/after.repo-exposure.json";
pub(crate) const EDITOR_AGENT_PACKET_ARTIFACT: &str = "target/ripr/agent/agent-packet.json";
pub(crate) const EDITOR_AGENT_BRIEF_ARTIFACT: &str = "target/ripr/agent/agent-brief.json";
pub(crate) const EDITOR_AGENT_VERIFY_ARTIFACT: &str = "target/ripr/agent/agent-verify.json";
pub(crate) const EDITOR_AGENT_RECEIPT_ARTIFACT: &str = "target/ripr/agent/agent-receipt.json";

pub(crate) const WORKFLOW_AGENT_STATUS_ARTIFACT: &str = "target/ripr/workflow/agent-status.json";
pub(crate) const WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT: &str =
    "target/ripr/workflow/agent-status.md";
pub(crate) const WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT: &str =
    "target/ripr/workflow/agent-review-summary.json";
pub(crate) const WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT: &str =
    "target/ripr/workflow/agent-review-summary.md";

pub(crate) fn agent_start_command(root: &str, seam_id: &str, out_dir: &str) -> String {
    format!(
        "ripr agent start --root {} --seam-id {} --out {}",
        shell_arg(root),
        shell_arg(seam_id),
        shell_arg(out_dir)
    )
}

pub(crate) fn check_repo_exposure_command(root: &str, mode: &str, out_path: &str) -> String {
    check_repo_exposure_command_with_base(root, None, mode, out_path)
}

pub(crate) fn check_repo_exposure_command_with_base(
    root: &str,
    base: Option<&str>,
    mode: &str,
    out_path: &str,
) -> String {
    let base_arg = base
        .map(|base| format!(" --base {}", shell_arg(base)))
        .unwrap_or_default();
    format!(
        "ripr check --root {}{} --mode {} --format repo-exposure-json > {}",
        shell_arg(root),
        base_arg,
        shell_arg(mode),
        shell_arg(out_path)
    )
}

pub(crate) fn agent_seam_packets_command(root: &str, mode: &str, out_path: &str) -> String {
    format!(
        "ripr check --root {} --mode {} --format agent-seam-packets-json > {}",
        shell_arg(root),
        shell_arg(mode),
        shell_arg(out_path)
    )
}

pub(crate) fn check_analysis_outcome_command(root: &str, mode: &str, out_path: &str) -> String {
    format!(
        "ripr check --root {} --mode {} --format json > {}",
        shell_arg(root),
        shell_arg(mode),
        shell_arg(out_path)
    )
}

pub(crate) fn agent_packet_command(root: &str, seam_id: &str, out_path: &str) -> String {
    format!(
        "ripr agent packet --root {} --seam-id {} --json > {}",
        shell_arg(root),
        shell_arg(seam_id),
        shell_arg(out_path)
    )
}

pub(crate) fn agent_brief_command(root: &str, seam_id: &str, out_path: &str) -> String {
    format!(
        "ripr agent brief --root {} --seam-id {} --json > {}",
        shell_arg(root),
        shell_arg(seam_id),
        shell_arg(out_path)
    )
}

pub(crate) fn agent_verify_command(
    root: &str,
    before_path: &str,
    after_path: &str,
    out_path: Option<&str>,
) -> String {
    let command = format!(
        "ripr agent verify --root {} --before {} --after {} --json",
        shell_arg(root),
        shell_arg(before_path),
        shell_arg(after_path)
    );
    append_redirect(command, out_path)
}

pub(crate) fn agent_receipt_command(
    root: &str,
    verify_json: &str,
    seam_id: &str,
    out_path: Option<&str>,
) -> String {
    let command = format!(
        "ripr agent receipt --root {} --verify-json {} --seam-id {} --json",
        shell_arg(root),
        shell_arg(verify_json),
        shell_arg(seam_id)
    );
    match out_path {
        Some(path) => format!("{command} --out {}", shell_arg(path)),
        None => command,
    }
}

pub(crate) fn agent_status_command(root: &str, out_path: Option<&str>) -> String {
    append_redirect(
        format!("ripr agent status --root {} --json", shell_arg(root)),
        out_path,
    )
}

pub(crate) fn agent_status_markdown_command(root: &str, out_path: Option<&str>) -> String {
    append_redirect(
        format!("ripr agent status --root {}", shell_arg(root)),
        out_path,
    )
}

pub(crate) fn agent_review_summary_command(root: &str, out_path: Option<&str>) -> String {
    append_redirect(
        format!(
            "ripr agent review-summary --root {} --json",
            shell_arg(root)
        ),
        out_path,
    )
}

pub(crate) fn agent_review_summary_markdown_command(root: &str, out_path: Option<&str>) -> String {
    append_redirect(
        format!("ripr agent review-summary --root {}", shell_arg(root)),
        out_path,
    )
}

pub(crate) fn outcome_command(
    before_path: &str,
    after_path: &str,
    out_path: Option<&str>,
) -> String {
    match out_path {
        Some(path) => {
            format!(
                "ripr outcome --before {} --after {} --format json --out {}",
                shell_arg(before_path),
                shell_arg(after_path),
                shell_arg(path)
            )
        }
        None => format!(
            "ripr outcome --before {} --after {}",
            shell_arg(before_path),
            shell_arg(after_path)
        ),
    }
}

pub(crate) fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        ".".to_string()
    } else {
        text
    }
}

pub(crate) fn workflow_artifact_path(out_dir: &Path, file_name: &str) -> String {
    let out_dir = display_path(out_dir);
    if out_dir == "." {
        file_name.to_string()
    } else {
        format!("{}/{}", out_dir.trim_end_matches('/'), file_name)
    }
}

pub(crate) fn shell_path(path: &Path) -> String {
    shell_arg(&display_path(path))
}

/// Render `value` as a single bash argv token for an advisory command string.
///
/// These strings are published for a human or an agent to copy into a shell, so
/// every value must survive the copy as exactly one argument with exactly its
/// original bytes (#2347).
///
/// Values made only of unambiguous characters pass through unquoted. Everything
/// else is single-quoted, with an embedded `'` rendered as `'\''` (close, escaped
/// quote, reopen). Single quotes are the only bash quoting form with no interior
/// interpretation at all: `$`, backtick, `!`, `\`, newline, and redirect
/// characters are all literal inside them. That makes one rule total over every
/// possible value, which double quotes cannot be:
///
/// - inside double quotes bash still expands `$VAR`, `$(cmd)`, and `` `cmd` ``;
/// - `\!` inside double quotes is literally backslash-bang in a non-interactive
///   shell, so escaping `!` that way corrupts the value instead of protecting it;
/// - a bare `\` cannot be left unquoted, because bash consumes it as an escape.
///
/// The empty string must be quoted as `''`, or the argument disappears from the
/// command line entirely.
pub(crate) fn shell_arg(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn append_redirect(command: String, out_path: Option<&str>) -> String {
    match out_path {
        Some(path) => format!("{command} > {}", shell_arg(path)),
        None => command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_commands_match_existing_status_templates() {
        assert_eq!(
            agent_start_command(".", "seam-a", "target/ripr/workflow"),
            "ripr agent start --root . --seam-id seam-a --out target/ripr/workflow"
        );
        assert_eq!(
            check_repo_exposure_command(".", "draft", WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT),
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
        );
        assert_eq!(
            check_repo_exposure_command(".", "draft", WORKFLOW_AFTER_SNAPSHOT_ARTIFACT),
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json"
        );
        assert_eq!(
            check_analysis_outcome_command(".", "draft", WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT),
            "ripr check --root . --mode draft --format json > target/ripr/workflow/analysis-outcome.json"
        );
        assert_eq!(
            agent_packet_command(".", "seam-a", WORKFLOW_AGENT_PACKET_ARTIFACT),
            "ripr agent packet --root . --seam-id seam-a --json > target/ripr/workflow/agent-packet.json"
        );
        assert_eq!(
            agent_brief_command(".", "seam-a", WORKFLOW_AGENT_BRIEF_ARTIFACT),
            "ripr agent brief --root . --seam-id seam-a --json > target/ripr/workflow/agent-brief.json"
        );
        assert_eq!(
            agent_verify_command(
                ".",
                WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
                WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
                Some(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            ),
            "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > target/ripr/workflow/agent-verify.json"
        );
        assert_eq!(
            agent_receipt_command(
                ".",
                WORKFLOW_AGENT_VERIFY_ARTIFACT,
                "seam-a",
                Some(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            ),
            "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id seam-a --json --out target/ripr/reports/agent-receipt.json"
        );
        assert_eq!(
            agent_status_command(".", Some(WORKFLOW_AGENT_STATUS_ARTIFACT)),
            "ripr agent status --root . --json > target/ripr/workflow/agent-status.json"
        );
        assert_eq!(
            agent_status_markdown_command(".", Some(WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT)),
            "ripr agent status --root . > target/ripr/workflow/agent-status.md"
        );
        assert_eq!(
            agent_review_summary_command(".", Some(WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT)),
            "ripr agent review-summary --root . --json > target/ripr/workflow/agent-review-summary.json"
        );
        assert_eq!(
            agent_review_summary_markdown_command(
                ".",
                Some(WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT),
            ),
            "ripr agent review-summary --root . > target/ripr/workflow/agent-review-summary.md"
        );
    }

    #[test]
    fn editor_commands_match_existing_lsp_templates() {
        assert_eq!(
            agent_packet_command(".", "seam-a", EDITOR_AGENT_PACKET_ARTIFACT),
            "ripr agent packet --root . --seam-id seam-a --json > target/ripr/agent/agent-packet.json"
        );
        assert_eq!(
            check_repo_exposure_command(".", "ready", PILOT_AFTER_SNAPSHOT_ARTIFACT),
            "ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json"
        );
        assert_eq!(
            agent_verify_command(
                ".",
                PILOT_BEFORE_SNAPSHOT_ARTIFACT,
                PILOT_AFTER_SNAPSHOT_ARTIFACT,
                Some(EDITOR_AGENT_VERIFY_ARTIFACT),
            ),
            "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json"
        );
    }

    #[test]
    fn command_args_quote_spaces_without_touching_plain_tokens() {
        assert_eq!(shell_arg("repo root"), "'repo root'");
        assert_eq!(shell_arg("target/ripr/workflow"), "target/ripr/workflow");
        assert_eq!(
            workflow_artifact_path(Path::new("target/ripr/workflow"), "workflow.json"),
            "target/ripr/workflow/workflow.json"
        );
        assert_eq!(
            agent_seam_packets_command(".", "draft mode", "target/ripr/workflow/packets.json"),
            "ripr check --root . --mode 'draft mode' --format agent-seam-packets-json > target/ripr/workflow/packets.json"
        );
        assert_eq!(
            agent_start_command("repo root", "seam a", "target/ripr/work flow"),
            "ripr agent start --root 'repo root' --seam-id 'seam a' --out 'target/ripr/work flow'"
        );
        assert_eq!(
            check_repo_exposure_command("repo root", "draft", "target/ripr/work flow/before.json"),
            "ripr check --root 'repo root' --mode draft --format repo-exposure-json > 'target/ripr/work flow/before.json'"
        );
        assert_eq!(
            check_repo_exposure_command_with_base(
                "repo root",
                Some("origin/main with space"),
                "draft",
                "target/ripr/work flow/before.json",
            ),
            "ripr check --root 'repo root' --base 'origin/main with space' --mode draft --format repo-exposure-json > 'target/ripr/work flow/before.json'"
        );
        assert_eq!(
            agent_verify_command(
                "repo root",
                "target/ripr/work flow/before.json",
                "target/ripr/work flow/after.json",
                Some("target/ripr/work flow/verify.json"),
            ),
            "ripr agent verify --root 'repo root' --before 'target/ripr/work flow/before.json' --after 'target/ripr/work flow/after.json' --json > 'target/ripr/work flow/verify.json'"
        );
        assert_eq!(
            agent_receipt_command(
                "repo root",
                "target/ripr/work flow/verify.json",
                "seam a",
                Some("target/ripr/work flow/receipt.json"),
            ),
            "ripr agent receipt --root 'repo root' --verify-json 'target/ripr/work flow/verify.json' --seam-id 'seam a' --json --out 'target/ripr/work flow/receipt.json'"
        );
    }

    /// Values that must round-trip through bash as exactly one argument with
    /// exactly these bytes. Each is a way the previous double-quote encoder
    /// could leak (#2347).
    fn hostile_values() -> Vec<(&'static str, &'static str)> {
        vec![
            ("empty", ""),
            ("space", "repo root"),
            ("backslash", "a\\b"),
            ("backslash separated path", "repo\\crates\\ripr\\src"),
            ("double quote", "a\"b"),
            ("single quote", "it's"),
            ("adjacent quotes", "''"),
            ("dollar var", "$HOME"),
            ("command substitution", "$(whoami)"),
            ("backtick", "`whoami`"),
            ("bang", "with!bang"),
            ("newline", "line one\nline two"),
            ("redirect gap id", "gap:pr:amount>=threshold"),
            ("redirect and space", "gap > /tmp/ripr-clobbered"),
            ("semicolon", "a; rm -rf /"),
            ("pipe and glob", "a | b *"),
            ("ampersand", "a && b"),
            ("tilde", "~/notes"),
            ("leading dash", "--not-a-flag"),
        ]
    }

    #[test]
    fn shell_arg_passes_unambiguous_tokens_unquoted() {
        assert_eq!(shell_arg("origin/main"), "origin/main");
        assert_eq!(shell_arg("target/ripr/workflow"), "target/ripr/workflow");
        assert_eq!(shell_arg("seam:a-b_c.d:1"), "seam:a-b_c.d:1");
    }

    #[test]
    fn shell_arg_quotes_every_hostile_value() {
        // A bare backslash must never pass through unquoted: bash would consume
        // it as an escape. The empty string must become '' or the argument
        // vanishes from the command line.
        assert_eq!(shell_arg(""), "''");
        assert_eq!(shell_arg("a\\b"), "'a\\b'");
        assert_eq!(shell_arg("$(whoami)"), "'$(whoami)'");
        assert_eq!(shell_arg("it's"), r"'it'\''s'");
        assert_eq!(
            shell_arg("gap > /tmp/ripr-clobbered"),
            "'gap > /tmp/ripr-clobbered'"
        );
    }

    /// Prove the encoding against a real bash, not against an assumed shape.
    ///
    /// Each value is encoded, embedded in a command line, and executed; bash
    /// prints one argument per line and the test compares the bytes it received
    /// with the bytes that went in. This is the check that would have caught the
    /// `\!`-inside-double-quotes and bare-backslash defects, both of which look
    /// correct as string assertions.
    #[test]
    fn shell_arg_round_trips_through_real_bash() -> Result<(), String> {
        let dir = RoundTripDir::new("argv")?;
        let Some(bash) = bash_for_round_trip(&dir) else {
            // Bash is not guaranteed on every developer machine. CI runs Linux,
            // where this always executes.
            return Ok(());
        };
        for (label, value) in hostile_values() {
            // `printf '%s\n'` prints each argv entry on its own line, so the
            // output is a faithful transcript of what bash actually parsed.
            //
            // The script is written to a file rather than passed with `-c`.
            // On Windows the `-c` route sends the text through Rust's argv
            // joining and then MSYS's re-parsing, which rewrites the quoting
            // before bash ever sees it — that path tests the host, not the
            // encoder. A file reaches bash byte for byte, which is also how
            // these advisory strings are really used.
            // `set --` binds the encoded token to the positional parameters, so
            // `$#` reports how many arguments bash actually parsed. `$1` is then
            // printed with no trailing newline, giving the exact bytes.
            let script = format!(
                "set -- {}\nprintf '%s\\n' \"$#\"\nprintf '%s' \"$1\"\n",
                shell_arg(value)
            );
            let script_path = dir.join("round-trip.sh");
            std::fs::write(&script_path, script.as_bytes())
                .map_err(|err| format!("{label}: write script: {err}"))?;
            let output = std::process::Command::new(&bash)
                .arg(bash_path(&script_path))
                .output()
                .map_err(|err| format!("{label}: failed to run bash: {err}"))?;
            if !output.status.success() {
                return Err(format!(
                    "{label}: bash exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            // The argument count is printed first and checked separately. It has
            // to be: a value split across two argv entries prints exactly the
            // same bytes as one argv entry containing a newline, so comparing
            // the bytes alone cannot tell a token split from the
            // `line one\nline two` case.
            let (count, received) = stdout
                .split_once('\n')
                .ok_or_else(|| format!("{label}: bash printed no argument count"))?;
            if count.trim_end_matches('\r') != "1" {
                return Err(format!(
                    "{label}: encoded value produced {count} arguments, expected exactly 1 \
                     (script {script:?})"
                ));
            }
            if received != value {
                return Err(format!(
                    "{label}: bash received {received:?} for script {script:?}, expected {value:?}"
                ));
            }
        }
        Ok(())
    }

    /// A redirect-shaped gap id must not be able to truncate a file.
    ///
    /// This is the concrete harm from #2347: the advisory command already ends
    /// in `> out.json`, so an unquoted id containing `>` supplies a second
    /// redirect, writing to a file the operator never named and changing the id
    /// the command actually receives.
    #[test]
    fn redirect_shaped_gap_id_cannot_open_a_second_redirect() -> Result<(), String> {
        let dir = RoundTripDir::new("redirect")?;
        let Some(bash) = bash_for_round_trip(&dir) else {
            return Ok(());
        };
        let victim = dir.join("must-not-be-written.txt");
        std::fs::write(&victim, b"original contents")
            .map_err(|err| format!("seed victim file: {err}"))?;

        // Use a bash-shaped path for the redirect target so the hostile value is
        // a plausible redirect on the shell under test.
        let victim_display = victim.display().to_string().replace('\\', "/");
        let hostile_id = format!("gap:pr:1 > {victim_display}");
        let script = format!(
            "set -- {}\nprintf '%s\\n' \"$#\"\nprintf '%s' \"$1\"\n",
            shell_arg(&hostile_id)
        );
        let script_path = dir.join("redirect.sh");
        std::fs::write(&script_path, script.as_bytes())
            .map_err(|err| format!("write script: {err}"))?;
        let output = std::process::Command::new(&bash)
            .arg(bash_path(&script_path))
            .output()
            .map_err(|err| format!("failed to run bash: {err}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (count, received) = stdout
            .split_once('\n')
            .map(|(count, rest)| (count.trim_end_matches('\r').to_string(), rest.to_string()))
            .unwrap_or_else(|| (String::new(), stdout.to_string()));
        let contents =
            std::fs::read_to_string(&victim).map_err(|err| format!("read victim file: {err}"))?;

        if count != "1" {
            return Err(format!(
                "redirect-shaped id produced {count} arguments, expected exactly 1"
            ));
        }
        if contents != "original contents" {
            return Err(format!(
                "redirect-shaped id truncated an unintended file; contents now {contents:?}"
            ));
        }
        if received != hostile_id {
            return Err(format!(
                "redirect-shaped id was altered in transit: got {received:?}, expected {hostile_id:?}"
            ));
        }
        Ok(())
    }

    /// Render a path the way the shell under test expects to receive it.
    ///
    /// A Windows path handed to Git Bash loses its backslashes to MSYS argument
    /// processing, so the directory separators are eaten and the remaining
    /// segments run together into one meaningless name. Forward slashes survive
    /// on both platforms.
    fn bash_path(path: &std::path::Path) -> String {
        path.display().to_string().replace('\\', "/")
    }

    /// Owns the scratch directory for one round-trip test.
    ///
    /// These tests return `Err` from several places and can panic inside the
    /// loop, so cleanup has to run on scope exit rather than at the end of the
    /// happy path — otherwise every run leaves another `ripr-shell-arg-*`
    /// directory behind in the system temp dir.
    struct RoundTripDir {
        path: std::path::PathBuf,
    }

    impl RoundTripDir {
        fn new(label: &str) -> Result<Self, String> {
            let path =
                std::env::temp_dir().join(format!("ripr-shell-arg-{}-{label}", std::process::id()));
            std::fs::create_dir_all(&path).map_err(|err| format!("create temp dir: {err}"))?;
            Ok(Self { path })
        }

        fn join(&self, name: &str) -> std::path::PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for RoundTripDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Find a bash that can actually execute a script at the path this test
    /// writes to.
    ///
    /// Probing with `bash -c "exit 0"` is not enough on Windows: `bash.exe` on
    /// `PATH` is frequently WSL bash, which runs fine but cannot open a
    /// drive-letter host path, so every case would fail for a reason that has
    /// nothing to do with the encoder. Git Bash is preferred and the candidate
    /// is confirmed by running a probe script from the real directory.
    fn bash_for_round_trip(dir: &RoundTripDir) -> Option<std::path::PathBuf> {
        // Git Bash is preferred over a bare `bash.exe`, which on Windows is
        // frequently WSL. The Git install prefix is read from the environment
        // rather than hard-coded, so no machine-local path is baked in.
        let mut candidates: Vec<String> = Vec::new();
        if cfg!(windows) {
            for prefix_var in ["ProgramFiles", "ProgramFiles(x86)"] {
                if let Ok(prefix) = std::env::var(prefix_var) {
                    candidates.push(format!("{prefix}/Git/bin/bash.exe"));
                }
            }
            candidates.push("bash.exe".to_string());
        } else {
            candidates.push("/bin/bash".to_string());
            candidates.push("bash".to_string());
        }
        let probe_path = dir.join("probe.sh");
        std::fs::write(&probe_path, b"printf 'ok\\n'\n").ok()?;
        let probe_arg = bash_path(&probe_path);
        candidates
            .iter()
            .map(std::path::PathBuf::from)
            .find(|candidate| {
                std::process::Command::new(candidate)
                    .arg(&probe_arg)
                    .output()
                    .is_ok_and(|output| {
                        output.status.success()
                            && String::from_utf8_lossy(&output.stdout).trim() == "ok"
                    })
            })
    }
}
