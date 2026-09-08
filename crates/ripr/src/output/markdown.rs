/// Shell disclosure shown before generated command fences (#2628).
///
/// The command strings are bash source (`agent::loop_commands::shell_arg`), so
/// each fenced `bash` block is paired with a PowerShell translation derived by
/// [`powershell_command`]. Naming both shells — and the cmd.exe boundary — in
/// the prose keeps the packet honest on Windows; the wording mirrors the landed
/// `agent_workflow` disclosure so every generated-command surface states the
/// same contract. Shared here — beside the translation it describes — so the
/// fenced command surfaces do not fork one disclosure per module.
pub(crate) const COMMAND_SHELL_DISCLOSURE: &str = "Each command includes Bash and PowerShell 7+ variants. The Bash form uses POSIX single-quote quoting and `>` redirection; the PowerShell form uses PowerShell's doubled-quote equivalent and staged native byte-preserving redirection. cmd.exe and Windows PowerShell 5.1 are not supported. On Windows, use Git Bash or PowerShell 7+. WSL bash is not a drop-in substitute: paths keep their Windows drive-letter prefix, which WSL resolves as a relative path.\n\n";

pub(crate) fn render_string_section(out: &mut String, title: &str, values: &[String]) {
    out.push_str(&format!("\n## {title}\n\n"));
    if values.is_empty() {
        out.push_str("- none\n");
    } else {
        for value in values {
            out.push_str(&format!("- {}\n", markdown_text(value)));
        }
    }
}

pub(crate) fn markdown_text(value: &str) -> String {
    value.replace('\\', "\\\\")
}

/// One-line disclosure emitted in place of a PowerShell variant when the bash
/// command is compound and no honest translation exists (#2628). Standalone
/// emitters append the sentence period; emitters that name the command append
/// `: `<command>`.
pub(crate) const POWERSHELL_UNAVAILABLE_DISCLOSURE: &str =
    "PowerShell form unavailable for compound commands";

/// Translate a bash-rendered advisory command into its PowerShell form.
///
/// The bash string stays authoritative (`agent::loop_commands::shell_arg`
/// renders it); this derives a copy-pasteable PowerShell equivalent so a
/// Windows reader is not left with bash source that cmd.exe reads as literal
/// quotes and PowerShell rejects at the `'\''` escape (#2628). Translations
/// applied:
///
/// - Compound bash commands (`&&`, `||`, `;`, bare line separators, heredocs,
///   input redirection, command substitution outside quoted regions) return [`None`]:
///   re-tokenizing them as PowerShell would be a second shell parser, so the
///   caller under-emits — bash form plus
///   [`POWERSHELL_UNAVAILABLE_DISCLOSURE`] — instead of shipping a line that
///   is invalid or semantically different (PR #3625 review, devin BUG).
/// - PowerShell single quotes are also literal inside, but its doubling idiom
///   differs: bash closes, escapes, and reopens (`'\''`) where PowerShell
///   doubles in place (`''`), so every occurrence is rewritten.
/// - bash `>` redirection becomes native PowerShell process redirection. The
///   child writes directly to a random sibling staging file, preserving its
///   stdout bytes rather than routing them through `Out-String`, `Out-File`, or
///   `WriteAllText`; only a successful process moves that file to the target.
///   Existing target files and unrelated staging files are preserved. Directory
///   targets are rejected. The target is rendered as a PowerShell string
///   literal and the redirect is detected only outside quoted regions.
/// - Redirected and unredirected invocations are guarded. A nonzero native
///   status throws before a generated command sequence can continue, so a
///   failed step cannot look complete or permit a later success artifact.
///
/// cmd.exe has no translation: it has no quoting form that keeps an argv token
/// literal, so a generated command is deliberately not offered for it. This
/// lives beside the markdown render helpers because every generated-command
/// surface renders both shell variants from this one implementation.
///
/// The Windows test compiles and runs a tiny Rust native executable under
/// `pwsh`, checking both argv and exact stdout bytes. Other platforms retain
/// deterministic structural tests and do not claim native PowerShell proof.
pub(crate) fn powershell_command(command: &str) -> Option<String> {
    if is_compound_bash_command(command) {
        return None;
    }
    let redirect = powershell_redirect_offset(command).ok()?;
    if let Some(index) = redirect {
        let invocation = command[..index].trim_end();
        let output = powershell_literal(&command[index + 1..].trim().replace("'\\''", "''"));
        let argv = powershell_argv(invocation)?;
        let file = argv.first()?.clone();
        let args = argv
            .iter()
            .skip(1)
            .map(|arg| powershell_literal(arg))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(format!(
            "$target = {output}; if (Test-Path -LiteralPath $target -PathType Container) {{ throw \"output path is a directory: $target\" }}; $staging = Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($target))) ('.ripr-' + [IO.Path]::GetRandomFileName() + '.tmp'); try {{ $process = Start-Process -FilePath {} -ArgumentList @({args}) -RedirectStandardOutput $staging -NoNewWindow -Wait -PassThru; if ($process.ExitCode -ne 0) {{ throw \"ripr exited with code $($process.ExitCode)\" }}; Move-Item -LiteralPath $staging -Destination $target -Force -ErrorAction Stop }} finally {{ if (Test-Path -LiteralPath $staging -PathType Leaf) {{ Remove-Item -LiteralPath $staging -Force -ErrorAction SilentlyContinue }} }}",
            powershell_literal(&file)
        ));
    }
    let command = command.replace("'\\''", "''");
    Some(format!(
        "& {command}; if ($LASTEXITCODE -ne 0) {{ throw \"native command exited with code $($LASTEXITCODE)\" }}"
    ))
}

/// Find the generated ` > ` operator after apostrophe translation. This is
/// only quote-aware boundary selection; unsupported escapes and compound
/// shell forms are rejected before this helper runs. Offsets remain UTF-8
/// byte indices, including when quoted arguments contain non-ASCII text.
fn powershell_redirect_offset(command: &str) -> Result<Option<usize>, ()> {
    let mut chars = command.char_indices().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut redirect = None;
    while let Some((index, ch)) = chars.next() {
        if in_single_quote {
            if ch == '\'' {
                if chars.peek().is_some_and(|(_, next)| *next == '\'') {
                    chars.next();
                } else {
                    in_single_quote = false;
                }
            }
        } else if in_double_quote {
            if ch == '"' {
                in_double_quote = false;
            }
        } else if ch == '\'' {
            in_single_quote = true;
        } else if ch == '"' {
            in_double_quote = true;
        } else if ch == '>' {
            if !command[..index].ends_with(' ')
                || !command[index + ch.len_utf8()..].starts_with(' ')
                || redirect.is_some()
            {
                return Err(());
            }
            redirect = Some(index);
        }
    }
    if redirect.is_some_and(|index| command[index + 1..].trim().is_empty()) {
        return Err(());
    }
    Ok(redirect)
}

/// Decide whether a bash command is compound: forms whose PowerShell
/// translation would need a second shell parser rather than a quoting
/// translation.
///
/// Detected outside single-quoted regions: `;`, `&` and `&&`, `|` and `||`,
/// heredoc `<<`, input redirection `<` — a parse error in PowerShell, which
/// defines no `<` operator (PR #3625 follow-up review) — command substitution
/// `$(`, backtick, and any other backslash escape (PowerShell does not treat
/// backslash as an escape, so `\;` would execute `b` as a separate command,
/// PR #3625 review round 3, devin); inside double quotes, where bash still
/// expands them: `$(` and backtick. The one exception is `\'` outside quotes,
/// which is the close-escape-reopen idiom inside a `'\''`-escaped token and
/// must keep translating. When in doubt the caller under-emits: a false
/// "compound" costs one disclosure line, a false "simple" would publish an
/// invalid or semantically different PowerShell line. Unquoted LF, CRLF, and
/// bare CR are withheld before redirect parsing: a command list must not be
/// folded into one invocation or mistaken for part of an artifact path. Quoted
/// line separators are argument data and keep the normal translation path.
fn is_compound_bash_command(command: &str) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let mut index = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        if in_single_quote {
            if ch == '\'' {
                in_single_quote = false;
            }
            index += 1;
        } else if in_double_quote {
            match ch {
                '"' => in_double_quote = false,
                // Bash treats `\$` inside double quotes as literal data, but
                // PowerShell evaluates `$(...)` as a subexpression — the
                // same text changes meaning across shells, so any
                // double-quoted backslash under-emits (#3625 review).
                '\\' => return true,
                '`' => return true,
                '$' => return true,
                _ => {}
            }
            index += 1;
        } else {
            match ch {
                '\'' => in_single_quote = true,
                '"' => in_double_quote = true,
                '\\' => match chars.get(index + 1).copied() {
                    // `\'` outside quotes closes, escapes, and reopens a
                    // single-quoted region (`'it'\''s'`); it is quoting, not
                    // a compound form.
                    Some('\'') => index += 1,
                    // Any other escape (`\;`, `\&`, `\ `, `\\`...) changes
                    // how the shells tokenize the line.
                    Some(_) => return true,
                    None => index += 1,
                },
                ';' | '\n' | '\r' => return true,
                '>' => {
                    let previous = index.checked_sub(1).and_then(|offset| chars.get(offset));
                    if previous != Some(&' ') || next != Some(' ') {
                        return true;
                    }
                }
                '&' => return true,
                '|' => return true,
                '<' => return true,
                '`' => return true,
                '$' => return true,
                _ => {}
            }
            index += 1;
        }
    }
    in_single_quote || in_double_quote
}

/// Render one value as a PowerShell single-quoted string literal.
///
/// A value that is already a single-quoted literal passes through: the bash
/// form quotes every argument that needs quoting, and the `'\''` rewrite in
/// [`powershell_command`] has already made such an interior PowerShell-valid.
/// Anything else is wrapped, doubling any embedded `'` so PowerShell receives
/// the intended argv or redirection path.
fn powershell_literal(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "''"))
}

fn powershell_argv(command: &str) -> Option<Vec<String>> {
    let mut values = Vec::new();
    let mut value = String::new();
    let mut token_started = false;
    let mut chars = command.chars().peekable();
    let mut quote = None;
    while let Some(ch) = chars.next() {
        match quote {
            Some('\'') => match ch {
                '\'' => quote = None,
                _ => value.push(ch),
            },
            Some('"') => match ch {
                '"' => quote = None,
                '\\' | '`' | '$' => return None,
                _ => value.push(ch),
            },
            Some(_) => return None,
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    token_started = true;
                }
                ch if ch.is_whitespace() => {
                    if token_started {
                        values.push(std::mem::take(&mut value));
                        token_started = false;
                    }
                }
                '\\' => match chars.next() {
                    Some('\'') => {
                        token_started = true;
                        value.push('\'');
                    }
                    _ => return None,
                },
                _ => {
                    token_started = true;
                    value.push(ch);
                }
            },
        }
    }
    if quote.is_some() {
        return None;
    }
    if token_started {
        values.push(value);
    }
    (!values.is_empty()).then_some(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn require_contains(value: &str, needle: &str) -> Result<(), String> {
        if value.contains(needle) {
            Ok(())
        } else {
            Err(format!("missing `{needle}` in `{value}`"))
        }
    }

    macro_rules! check {
        ($condition:expr $(, $message:expr)*) => {
            if !$condition {
                return Err(format!("assertion failed: {}", stringify!($condition)));
            }
        };
    }

    macro_rules! check_eq {
        ($left:expr, $right:expr $(, $message:tt)*) => {
            if $left != $right {
                return Err(format!(
                    "assertion failed: {} != {}",
                    stringify!($left),
                    stringify!($right)
                ));
            }
        };
    }

    struct TempDirGuard(std::path::PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn markdown_text_escapes_backslashes() -> Result<(), String> {
        check_eq!(markdown_text("a\\b"), "a\\\\b");
        check_eq!(markdown_text("no backslash"), "no backslash");
        Ok(())
    }

    #[test]
    fn render_string_section_lists_values_or_none() -> Result<(), String> {
        let mut out = String::new();
        render_string_section(&mut out, "Example", &[]);
        check_eq!(out, "\n## Example\n\n- none\n");

        let mut out = String::new();
        render_string_section(&mut out, "Example", &["a\\b".to_string()]);
        check_eq!(out, "\n## Example\n\n- a\\\\b\n");
        check_eq!(out, "\n## Example\n\n- a\\\\b\n");
        Ok(())
    }

    #[test]
    fn powershell_command_handles_unredirected_quoted_and_unicode_commands() -> Result<(), String> {
        check_eq!(
            powershell_command("ripr check --root 'a > b'"),
            Some("& ripr check --root 'a > b'; if ($LASTEXITCODE -ne 0) { throw \"native command exited with code $($LASTEXITCODE)\" }".to_string())
        );
        let rendered = powershell_command("ripr check --root 'café' > 'résumé.json'")
            .ok_or_else(|| "redirect command was withheld".to_string())?;
        require_contains(&rendered, "Start-Process")?;
        require_contains(&rendered, "'café'")?;
        require_contains(&rendered, "'résumé.json'")
    }

    #[test]
    fn powershell_command_preserves_quoted_redirect_tokens_without_a_write() -> Result<(), String> {
        for command in [
            "cargo test \"a > b\"",
            "cargo test \"owner's > case\"",
            "cargo test 'a \" > b'",
            "cargo test \"résumé > café\"",
        ] {
            check_eq!(powershell_command(command).as_deref(), Some(format!("& {command}; if ($LASTEXITCODE -ne 0) {{ throw \"native command exited with code $($LASTEXITCODE)\" }}").as_str()));
        }
        Ok(())
    }

    #[test]
    fn powershell_command_finds_real_redirect_after_double_quoted_argument() -> Result<(), String> {
        let rendered =
            powershell_command("ripr check --root \"café > owner's repo\" > 'résumé.json'")
                .ok_or_else(|| "redirect command was withheld".to_string())?;
        require_contains(&rendered, "'café > owner''s repo'")?;
        require_contains(&rendered, "-RedirectStandardOutput $staging")?;
        require_contains(&rendered, "$target = 'résumé.json'")
    }

    #[test]
    fn powershell_command_keeps_double_quote_literal_inside_single_quotes() -> Result<(), String> {
        let rendered = powershell_command("cargo test 'a \" > b' > evidence.txt")
            .ok_or_else(|| "redirect command was withheld".to_string())?;
        require_contains(&rendered, "-FilePath 'cargo'")?;
        require_contains(&rendered, "'a \" > b'")?;
        require_contains(&rendered, "$target = 'evidence.txt'")
    }

    /// #3625 review (CWE-78): bash treats `\$` inside double quotes as
    /// literal data, but PowerShell evaluates `$(...)` as a subexpression —
    /// the same pasted text changes meaning across shells, so a
    /// double-quoted backslash under-emits to bash-only.
    #[test]
    fn powershell_command_rejects_double_quoted_backslash_escapes() -> Result<(), String> {
        check_eq!(
            powershell_command("echo \"\\$(Write-Output injected)\""),
            None
        );
        check_eq!(powershell_command("echo \"a\\b\""), None);
        Ok(())
    }

    /// The PowerShell single-quote round-trip: bash's close-escape-reopen idiom
    /// (`'\''`) is rewritten to PowerShell's doubled quote (`''`), and
    /// PowerShell reads `it''s` back as the original bytes `it's`. An embedded
    /// quote left as `'\''` would be a syntax error at the copy site.
    #[test]
    fn powershell_command_round_trips_embedded_quotes_through_doubling() -> Result<(), String> {
        let bash = "ripr receipt write --gap 'it'\\''s'";
        check_eq!(
            powershell_command(bash),
            Some("& ripr receipt write --gap 'it''s'; if ($LASTEXITCODE -ne 0) { throw \"native command exited with code $($LASTEXITCODE)\" }".to_string())
        );
        // A quoted `>` inside an argument must not be mistaken for a redirect.
        check_eq!(
            powershell_command("ripr receipt write --gap 'gap > file'"),
            Some("& ripr receipt write --gap 'gap > file'; if ($LASTEXITCODE -ne 0) { throw \"native command exited with code $($LASTEXITCODE)\" }".to_string())
        );
        Ok(())
    }

    /// PowerShell parses method-call arguments in expression mode, where a
    /// bare path like `target/ripr/out.json` is a parse error before anything
    /// runs (PR #3617 review, gemini HIGH + codex P1): the redirect target
    /// must arrive as a quoted literal even when the bash form left it
    /// unquoted. This is the default pilot path shape.
    #[test]
    fn powershell_command_quotes_the_default_unquoted_redirect_target() -> Result<(), String> {
        let rendered = powershell_command(
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json",
        ).ok_or_else(|| "redirect command was withheld".to_string())?;
        require_contains(
            &rendered,
            "-ArgumentList @('check', '--root', '.', '--mode', 'draft', '--format', 'repo-exposure-json')",
        )?;
        require_contains(
            &rendered,
            "$target = 'target/ripr/pilot/after.repo-exposure.json'",
        )
    }

    /// An embedded quote in an unwrapped target must survive the literal: the
    /// wrapper doubles it. A target the bash form already single-quoted passes
    /// through with its interior `''` doubling intact.
    #[test]
    fn powershell_command_redirect_target_escapes_embedded_quotes() -> Result<(), String> {
        check_eq!(powershell_command("ripr check --root . > it's.json"), None);
        let rendered = powershell_command("ripr check --root . > 'it'\\''s.json'")
            .ok_or_else(|| "redirect command was withheld".to_string())?;
        require_contains(&rendered, "$target = 'it''s.json'")
    }

    /// The artifact write must sit inside the success branch, and a nonzero
    /// invocation must abort without publishing it (PR #3625 review, codex
    /// P1): without the guard, a nonzero `ripr` exit still published the
    /// artifact and exited 0, so a failed step advanced as if it had
    /// completed. The failure surfaces as `throw`, not `exit` (PR #3625
    /// follow-up review): `throw` aborts a pasted or scripted block — in a
    /// composed fence a failed snapshot stops the sequence before its outcome
    /// command — while leaving an interactive session open, where `exit`
    /// would close the reader's shell. String-pinned only — see the disclosed
    /// limitation on [`powershell_command`] for why no pwsh runtime oracle
    /// backs this.
    #[test]
    fn powershell_command_guard_only_writes_the_artifact_on_success() -> Result<(), String> {
        let line = powershell_command(
            "ripr agent packet --root . --json > target/ripr/workflow/agent-packet.json",
        )
        .ok_or_else(|| "simple command must translate".to_string())?;
        // The write is textually inside the success branch, and the failure
        // branch throws with the invocation's exit status instead of exiting.
        check!(
            line.contains("Start-Process -FilePath 'ripr' -ArgumentList @('agent', 'packet', '--root', '.', '--json') -RedirectStandardOutput $staging"),
            "write must be guarded by the success branch:\n{line}"
        );
        check!(
            line.contains("$process.ExitCode -ne 0") && line.contains("throw"),
            "failure must remain visible:\n{line}"
        );
        check!(
            !line.contains("exit $LASTEXITCODE"),
            "exit would terminate an interactive session:\n{line}"
        );
        check!(
            !line.contains("Out-String") && !line.contains("WriteAllText"),
            "PowerShell must not normalize stdout through text conversion:\n{line}"
        );
        require_contains(
            &line,
            "Move-Item -LiteralPath $staging -Destination $target -Force -ErrorAction Stop",
        )?;
        Ok(())
    }

    /// Compound bash commands have no honest PowerShell translation: they must
    /// return [`None`] so the caller under-emits (bash form plus a disclosure)
    /// instead of shipping an invalid or semantically different line (PR
    /// #3625 review, devin BUG). A single `|` or `&` is as compound as its
    /// doubled form (PR #3625 review round 3, coderabbit), and input
    /// redirection `<` is included: PowerShell defines no `<` operator, so it
    /// would be a parse error at the copy site. Backslash escapes outside
    /// quotes under-emit too: PowerShell does not treat backslash as an
    /// escape, so `echo a\;b` would execute `b` as a separate command — only
    /// the `'\''` idiom's `\'` keeps translating. Quoted separators stay
    /// simple: `;` inside a single-quoted token is data, and `&&` inside a
    /// double-quoted token is data.
    #[test]
    fn powershell_command_rejects_compound_commands() -> Result<(), String> {
        check_eq!(powershell_command("cmd1 && cmd2"), None);
        check_eq!(powershell_command("cmd1 || cmd2"), None);
        check_eq!(powershell_command("cmd1 & cmd2"), None);
        check_eq!(powershell_command("cargo test | tee evidence.txt"), None);
        check_eq!(powershell_command("cmd1; cmd2"), None);
        check_eq!(powershell_command("cmd1 <<EOF"), None);
        check_eq!(powershell_command("ripr check --diff < input.json"), None);
        check_eq!(powershell_command("cmd1 <input.json"), None);
        check_eq!(powershell_command(r"echo a\;b"), None);
        check_eq!(powershell_command("cmd1 $(whoami)"), None);
        check_eq!(powershell_command("cmd1 `whoami`"), None);
        check_eq!(powershell_command("ripr check >output.json"), None);
        check_eq!(powershell_command("ripr check >> output.json"), None);
        check_eq!(
            powershell_command("ripr check > first.json > second.json"),
            None
        );
        check_eq!(powershell_command("$RIPR_ROOT > output.json"), None);
        check_eq!(powershell_command("ripr check>output.json"), None);
        check_eq!(
            powershell_command("ripr receipt write --gap 'a;b'"),
            Some("& ripr receipt write --gap 'a;b'; if ($LASTEXITCODE -ne 0) { throw \"native command exited with code $($LASTEXITCODE)\" }".to_string())
        );
        check_eq!(
            powershell_command("cargo test \"a && b\""),
            Some("& cargo test \"a && b\"; if ($LASTEXITCODE -ne 0) { throw \"native command exited with code $($LASTEXITCODE)\" }".to_string())
        );
        // The `'\''` idiom keeps translating: its `\'` is quoting, not a
        // compound escape.
        check_eq!(
            powershell_command("ripr receipt write --gap 'it'\\''s'"),
            Some("& ripr receipt write --gap 'it''s'; if ($LASTEXITCODE -ne 0) { throw \"native command exited with code $($LASTEXITCODE)\" }".to_string())
        );
        Ok(())
    }

    /// A bare line separator is not an argv character: it can delimit a
    /// second command, including after the first command's redirect target.
    #[test]
    fn powershell_command_rejects_unquoted_line_separators() -> Result<(), String> {
        for separator in ["\n", "\r\n", "\r"] {
            for command in [
                format!("cargo test{separator}ripr check"),
                format!("cargo test{separator}ripr check > after.json"),
                format!("ripr check > after.json{separator}cargo test"),
                format!("cargo test \"owner's case\"{separator}ripr check"),
                format!("ripr check --root 'café'{separator}cargo test"),
            ] {
                check_eq!(
                    powershell_command(&command),
                    None,
                    "must withhold a compound translation: {command:?}"
                );
            }
        }
        Ok(())
    }

    /// Newlines inside either supported quote form are literal argument data,
    /// not command boundaries; rejecting every multiline string is too broad.
    #[test]
    fn powershell_command_preserves_quoted_line_separators() -> Result<(), String> {
        for separator in ["\n", "\r\n", "\r"] {
            for command in [
                format!("cargo test 'first{separator}second'"),
                format!("cargo test \"first{separator}second\""),
                format!("cargo test \"owner's{separator}case\""),
                format!("cargo test 'a \"{separator}case'"),
            ] {
                let rendered = powershell_command(&command);
                let expected = format!(
                    "& {command}; if ($LASTEXITCODE -ne 0) {{ throw \"native command exited with code $($LASTEXITCODE)\" }}"
                );
                check_eq!(rendered.as_deref(), Some(expected.as_str()));
            }
        }
        Ok(())
    }

    /// Literal multiline data must not hide the real redirect that follows it.
    #[test]
    fn powershell_command_keeps_redirect_after_quoted_newline() -> Result<(), String> {
        let rendered = powershell_command("ripr check --root 'café\nrepo' > 'résumé.json'")
            .ok_or_else(|| "redirect command was withheld".to_string())?;
        require_contains(&rendered, "'café\nrepo'")?;
        require_contains(&rendered, "$target = 'résumé.json'")
    }

    /// Exercise the generated form against a real Windows native executable.
    /// A PowerShell function mock cannot establish argv marshalling or native
    /// exit/output behavior, so this compiles a tiny Rust fixture with the
    /// approved toolchain and checks both the exact payload bytes and the
    /// fixture's independently validated argv contract.
    #[cfg(windows)]
    #[test]
    fn powershell_command_native_fixture_preserves_argv_and_bytes() -> Result<(), String> {
        use std::fs;
        use std::process::Command;

        let root =
            std::env::temp_dir().join(format!("ripr-powershell-1672-{}", std::process::id()));
        let _guard = TempDirGuard(root.clone());
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let source = root.join("fixture.rs");
        let executable = root.join("fixture.exe");
        let artifact = root.join("artifact.bin");
        fs::write(
            &source,
            r##"
use std::{env, process};
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() == 1 && args.first().is_some_and(|arg| arg == "fail") {
        print!("partial failure");
        process::exit(7);
    }
    let expected = [
        "",
        "space value",
        "café",
        "it's",
        "\"",
        "$x",
        "backtick`",
        "C:\\workspace\\path",
        "'quoted'",
    ];
    if args.iter().map(String::as_str).collect::<Vec<_>>() != expected {
        eprintln!("unexpected argv: {args:?}");
        process::exit(17);
    }
    print!(r#"{{"ok":true}}"#);
}
"##,
        )
        .map_err(|error| error.to_string())?;
        let compile = Command::new("rustc")
            .args(["--edition=2024"])
            .arg(&source)
            .args(["-o"])
            .arg(&executable)
            .output()
            .map_err(|error| error.to_string())?;
        if !compile.status.success() {
            return Err(format!("fixture compilation failed: {:?}", compile));
        }

        let bash_quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
        let command = format!(
            "{} '' 'space value' 'café' 'it'\\''s' '\"' '$x' 'backtick`' 'C:\\workspace\\path' ''\\''quoted'\\''' > {}",
            bash_quote(&executable.to_string_lossy()),
            bash_quote(&artifact.to_string_lossy()),
        );
        let powershell = powershell_command(&command)
            .ok_or_else(|| format!("fixture command was withheld: {command}"))?;
        let run = Command::new("pwsh")
            .args(["-NoProfile", "-Command", &powershell])
            .output()
            .map_err(|error| error.to_string())?;
        if !run.status.success() {
            return Err(format!("native PowerShell proof failed: {:?}", run));
        }
        let payload = fs::read(&artifact).map_err(|error| error.to_string())?;
        if payload != br#"{"ok":true}"# {
            return Err(format!("native payload drifted: {payload:?}"));
        }
        let sentinel = root.join("artifact.bin.ripr-staging");
        fs::write(&sentinel, b"preexisting-user-file").map_err(|error| error.to_string())?;
        fs::write(&artifact, b"prior-valid").map_err(|error| error.to_string())?;
        let failure_command = format!(
            "{} 'fail' > {}",
            bash_quote(&executable.to_string_lossy()),
            bash_quote(&artifact.to_string_lossy())
        );
        let failure_powershell = powershell_command(&failure_command)
            .ok_or_else(|| format!("failure command was withheld: {failure_command}"))?;
        let failed = Command::new("pwsh")
            .args(["-NoProfile", "-Command", &failure_powershell])
            .output()
            .map_err(|error| error.to_string())?;
        if failed.status.success() {
            return Err("native failure unexpectedly succeeded".to_string());
        }
        let retained = fs::read(&artifact).map_err(|error| error.to_string())?;
        if retained != b"prior-valid" {
            return Err(format!(
                "failed command overwrote retained artifact: {retained:?}"
            ));
        }
        let sentinel_bytes = fs::read(&sentinel).map_err(|error| error.to_string())?;
        if sentinel_bytes != b"preexisting-user-file" {
            return Err(format!(
                "failed command removed unrelated staging file: {sentinel_bytes:?}"
            ));
        }
        fs::remove_file(&artifact).map_err(|error| error.to_string())?;
        fs::create_dir(&artifact).map_err(|error| error.to_string())?;
        let wrong_role = Command::new("pwsh")
            .args(["-NoProfile", "-Command", &powershell])
            .output()
            .map_err(|error| error.to_string())?;
        if wrong_role.status.success() {
            return Err("directory output role unexpectedly succeeded".to_string());
        }
        if !artifact.is_dir() {
            return Err("wrong output role was not preserved as a directory".to_string());
        }
        fs::remove_dir(&artifact).map_err(|error| error.to_string())?;
        Ok(())
    }
}
