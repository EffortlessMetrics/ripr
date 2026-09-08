/// Shell disclosure shown before generated command fences (#2628).
///
/// The command strings are bash source (`agent::loop_commands::shell_arg`), so
/// each fenced `bash` block is paired with a PowerShell translation derived by
/// [`powershell_command`]. Naming both shells — and the cmd.exe boundary — in
/// the prose keeps the packet honest on Windows; the wording mirrors the landed
/// `agent_workflow` disclosure so every generated-command surface states the
/// same contract. Shared here — beside the translation it describes — so the
/// fenced command surfaces do not fork one disclosure per module.
pub(crate) const COMMAND_SHELL_DISCLOSURE: &str = "Each command includes Bash and PowerShell variants. The Bash form uses POSIX single-quote quoting and `>` redirection; the PowerShell form uses PowerShell's doubled-quote equivalent and UTF-8 `Out-File` redirection. cmd.exe is not supported. On Windows, use either Git Bash or PowerShell. WSL bash is not a drop-in substitute: paths keep their Windows drive-letter prefix, which WSL resolves as a relative path.\n\n";

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
/// `: <command>`.
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
/// - bash `>` redirection becomes a .NET write with BOM-free UTF-8, so Windows
///   PowerShell 5.1 does not produce a UTF-16 or BOM-prefixed file that JSON
///   consumers would reject. The target is rendered as a PowerShell string
///   literal — method-call arguments parse in expression mode, where a bare
///   path is a parse error (PR #3617 review) — and the redirect is detected
///   only outside single- and double-quoted regions, so a quoted `>` inside
///   an argument cannot hijack it. A quote of the other kind is literal data.
/// - The artifact write is guarded: the invocation's output is captured, the
///   write happens only `if ($LASTEXITCODE -eq 0)`, and a nonzero status
///   throws `"ripr exited with code $LASTEXITCODE"`. Without the guard, a
///   nonzero `ripr` exit still published the artifact and exited 0, so a
///   failed step looked complete (PR #3625 review, codex P1). The failure
///   surfaces as `throw`, not `exit`: `throw` aborts a pasted or scripted
///   block — so in a composed fence a failed snapshot stops the sequence
///   before its outcome command — while leaving an interactive session open
///   where `exit` would close it (PR #3625 follow-up review).
///
/// cmd.exe has no translation: it has no quoting form that keeps an argv token
/// literal, so a generated command is deliberately not offered for it. This
/// lives beside the markdown render helpers because every generated-command
/// surface renders both shell variants from this one implementation.
///
/// Disclosed limitation: the tests pin these forms as strings, and the CI
/// environment that runs them cannot assume a pwsh runtime oracle, so the
/// guard and the translations carry no executed regression on that lane.
pub(crate) fn powershell_command(command: &str) -> Option<String> {
    if is_compound_bash_command(command) {
        return None;
    }
    let command = command.replace("'\\''", "''");
    if let Some(index) = powershell_redirect_offset(&command) {
        let invocation = command[..index].trim_end();
        let output = powershell_literal(command[index + 1..].trim());
        return Some(format!(
            "$ripr = (({invocation}) | Out-String); if ($LASTEXITCODE -eq 0) {{ [System.IO.File]::WriteAllText({output}, $ripr, [System.Text.UTF8Encoding]::new($false)) }} else {{ throw \"ripr exited with code $LASTEXITCODE\" }}"
        ));
    }
    Some(command)
}

/// Find the generated ` > ` operator after apostrophe translation. This is
/// only quote-aware boundary selection; unsupported escapes and compound
/// shell forms are rejected before this helper runs. Offsets remain UTF-8
/// byte indices, including when quoted arguments contain non-ASCII text.
fn powershell_redirect_offset(command: &str) -> Option<usize> {
    let mut chars = command.char_indices().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
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
        } else if ch == '>'
            && command[..index].ends_with(' ')
            && command[index + ch.len_utf8()..].starts_with(' ')
        {
            return Some(index);
        }
    }
    None
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
                '$' if next == Some('(') => return true,
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
                '&' => return true,
                '|' => return true,
                '<' => return true,
                '`' => return true,
                '$' if next == Some('(') => return true,
                _ => {}
            }
            index += 1;
        }
    }
    false
}

/// Render one value as a PowerShell single-quoted string literal.
///
/// A value that is already a single-quoted literal passes through: the bash
/// form quotes every argument that needs quoting, and the `'\''` rewrite in
/// [`powershell_command`] has already made such an interior PowerShell-valid.
/// Anything else is wrapped, doubling any embedded `'` — without the wrapping,
/// `WriteAllText(target/ripr/out.json, ...)` would not parse at the copy site.
fn powershell_literal(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_text_escapes_backslashes() {
        assert_eq!(markdown_text("a\\b"), "a\\\\b");
        assert_eq!(markdown_text("no backslash"), "no backslash");
    }

    #[test]
    fn render_string_section_lists_values_or_none() {
        let mut out = String::new();
        render_string_section(&mut out, "Example", &[]);
        assert_eq!(out, "\n## Example\n\n- none\n");

        let mut out = String::new();
        render_string_section(&mut out, "Example", &["a\\b".to_string()]);
        assert_eq!(out, "\n## Example\n\n- a\\\\b\n");
    }

    #[test]
    fn powershell_command_handles_unredirected_quoted_and_unicode_commands() {
        assert_eq!(
            powershell_command("ripr check --root 'a > b'"),
            Some("ripr check --root 'a > b'".to_string())
        );
        assert_eq!(
            powershell_command("ripr check --root 'café' > 'résumé.json'"),
            Some("$ripr = ((ripr check --root 'café') | Out-String); if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText('résumé.json', $ripr, [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }

    #[test]
    fn powershell_command_preserves_quoted_redirect_tokens_without_a_write() {
        for command in [
            "cargo test \"a > b\"",
            "cargo test \"owner's > case\"",
            "cargo test 'a \" > b'",
            "cargo test \"résumé > café\"",
        ] {
            assert_eq!(powershell_command(command).as_deref(), Some(command));
        }
    }

    #[test]
    fn powershell_command_finds_real_redirect_after_double_quoted_argument() {
        assert_eq!(
            powershell_command("ripr check --root \"café > owner's repo\" > 'résumé.json'"),
            Some("$ripr = ((ripr check --root \"café > owner's repo\") | Out-String); if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText('résumé.json', $ripr, [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }

    #[test]
    fn powershell_command_keeps_double_quote_literal_inside_single_quotes() {
        assert_eq!(
            powershell_command("cargo test 'a \" > b' > evidence.txt"),
            Some("$ripr = ((cargo test 'a \" > b') | Out-String); if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText('evidence.txt', $ripr, [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }

    /// #3625 review (CWE-78): bash treats `\$` inside double quotes as
    /// literal data, but PowerShell evaluates `$(...)` as a subexpression —
    /// the same pasted text changes meaning across shells, so a
    /// double-quoted backslash under-emits to bash-only.
    #[test]
    fn powershell_command_rejects_double_quoted_backslash_escapes() {
        assert_eq!(
            powershell_command("echo \"\\$(Write-Output injected)\""),
            None
        );
        assert_eq!(powershell_command("echo \"a\\b\""), None);
    }

    /// The PowerShell single-quote round-trip: bash's close-escape-reopen idiom
    /// (`'\''`) is rewritten to PowerShell's doubled quote (`''`), and
    /// PowerShell reads `it''s` back as the original bytes `it's`. An embedded
    /// quote left as `'\''` would be a syntax error at the copy site.
    #[test]
    fn powershell_command_round_trips_embedded_quotes_through_doubling() {
        let bash = "ripr receipt write --gap 'it'\\''s'";
        assert_eq!(
            powershell_command(bash),
            Some("ripr receipt write --gap 'it''s'".to_string())
        );
        // A quoted `>` inside an argument must not be mistaken for a redirect.
        assert_eq!(
            powershell_command("ripr receipt write --gap 'gap > file'"),
            Some("ripr receipt write --gap 'gap > file'".to_string())
        );
    }

    /// PowerShell parses method-call arguments in expression mode, where a
    /// bare path like `target/ripr/out.json` is a parse error before anything
    /// runs (PR #3617 review, gemini HIGH + codex P1): the redirect target
    /// must arrive as a quoted literal even when the bash form left it
    /// unquoted. This is the default pilot path shape.
    #[test]
    fn powershell_command_quotes_the_default_unquoted_redirect_target() {
        assert_eq!(
            powershell_command(
                "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json"
            ),
            Some("$ripr = ((ripr check --root . --mode draft --format repo-exposure-json) | Out-String); if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText('target/ripr/pilot/after.repo-exposure.json', $ripr, [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }

    /// An embedded quote in an unwrapped target must survive the literal: the
    /// wrapper doubles it. A target the bash form already single-quoted passes
    /// through with its interior `''` doubling intact.
    #[test]
    fn powershell_command_redirect_target_escapes_embedded_quotes() {
        assert_eq!(
            powershell_command("ripr check --root . > it's.json"),
            Some("$ripr = ((ripr check --root .) | Out-String); if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText('it''s.json', $ripr, [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
        assert_eq!(
            powershell_command("ripr check --root . > 'it'\\''s.json'"),
            Some("$ripr = ((ripr check --root .) | Out-String); if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText('it''s.json', $ripr, [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
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
        assert!(
            line.contains(
                "if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText('target/ripr/workflow/agent-packet.json', $ripr, [System.Text.UTF8Encoding]::new($false)) }"
            ),
            "write must be guarded by the success branch:\n{line}"
        );
        assert!(
            line.ends_with("} else { throw \"ripr exited with code $LASTEXITCODE\" }"),
            "failure must abort the block with throw, preserving the session:\n{line}"
        );
        assert!(
            !line.contains("exit $LASTEXITCODE"),
            "exit would terminate an interactive session:\n{line}"
        );
        assert!(
            !line.contains("WriteAllText")
                || line.contains("if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText("),
            "no unguarded WriteAllText may appear:\n{line}"
        );
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
    fn powershell_command_rejects_compound_commands() {
        assert_eq!(powershell_command("cmd1 && cmd2"), None);
        assert_eq!(powershell_command("cmd1 || cmd2"), None);
        assert_eq!(powershell_command("cmd1 & cmd2"), None);
        assert_eq!(powershell_command("cargo test | tee evidence.txt"), None);
        assert_eq!(powershell_command("cmd1; cmd2"), None);
        assert_eq!(powershell_command("cmd1 <<EOF"), None);
        assert_eq!(powershell_command("ripr check --diff < input.json"), None);
        assert_eq!(powershell_command("cmd1 <input.json"), None);
        assert_eq!(powershell_command(r"echo a\;b"), None);
        assert_eq!(powershell_command("cmd1 $(whoami)"), None);
        assert_eq!(powershell_command("cmd1 `whoami`"), None);
        assert_eq!(
            powershell_command("ripr receipt write --gap 'a;b'"),
            Some("ripr receipt write --gap 'a;b'".to_string())
        );
        assert_eq!(
            powershell_command("cargo test \"a && b\""),
            Some("cargo test \"a && b\"".to_string())
        );
        // The `'\''` idiom keeps translating: its `\'` is quoting, not a
        // compound escape.
        assert_eq!(
            powershell_command("ripr receipt write --gap 'it'\\''s'"),
            Some("ripr receipt write --gap 'it''s'".to_string())
        );
    }

    /// A bare line separator is not an argv character: it can delimit a
    /// second command, including after the first command's redirect target.
    #[test]
    fn powershell_command_rejects_unquoted_line_separators() {
        for separator in ["\n", "\r\n", "\r"] {
            for command in [
                format!("cargo test{separator}ripr check"),
                format!("cargo test{separator}ripr check > after.json"),
                format!("ripr check > after.json{separator}cargo test"),
                format!("cargo test \"owner's case\"{separator}ripr check"),
                format!("ripr check --root 'café'{separator}cargo test"),
            ] {
                assert_eq!(
                    powershell_command(&command),
                    None,
                    "must withhold a compound translation: {command:?}"
                );
            }
        }
    }

    /// Newlines inside either supported quote form are literal argument data,
    /// not command boundaries; rejecting every multiline string is too broad.
    #[test]
    fn powershell_command_preserves_quoted_line_separators() {
        for separator in ["\n", "\r\n", "\r"] {
            for command in [
                format!("cargo test 'first{separator}second'"),
                format!("cargo test \"first{separator}second\""),
                format!("cargo test \"owner's{separator}case\""),
                format!("cargo test 'a \"{separator}case'"),
            ] {
                let rendered = powershell_command(&command);
                assert_eq!(rendered.as_deref(), Some(command.as_str()));
            }
        }
    }

    /// Literal multiline data must not hide the real redirect that follows it.
    #[test]
    fn powershell_command_keeps_redirect_after_quoted_newline() {
        assert_eq!(
            powershell_command("ripr check --root 'café\nrepo' > 'résumé.json'"),
            Some("$ripr = ((ripr check --root 'café\nrepo') | Out-String); if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText('résumé.json', $ripr, [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }
}
