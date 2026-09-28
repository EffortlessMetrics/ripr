/// Shell disclosure shown before generated command fences (#2628).
///
/// The command strings are bash source (`agent::loop_commands::shell_arg`), so
/// a fenced `bash` block is paired with a PowerShell translation derived by
/// [`powershell_command`] when the translation differs ([`powershell_form`]). Naming both shells — and the cmd.exe boundary — in
/// the prose keeps the packet honest on Windows; the wording mirrors the landed
/// `agent_workflow` disclosure so every generated-command surface states the
/// same contract. Shared here — beside the translation it describes — so the
/// fenced command surfaces do not fork one disclosure per module.
pub(crate) const COMMAND_SHELL_DISCLOSURE: &str = "Commands are written for Bash, with POSIX single-quote quoting and `>` redirection. A PowerShell form follows a command only when PowerShell needs a different one: doubled-quote escaping, the `&` call operator before a quoted program path, or a guarded BOM-free UTF-8 write in place of `>`. A command with no PowerShell form after it runs unchanged in PowerShell, unless a line says its PowerShell form is unavailable. cmd.exe is not supported. On Windows, use either Git Bash or PowerShell. WSL bash is not a drop-in substitute: paths keep their Windows drive-letter prefix, which WSL resolves as a relative path.\n\n";

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

/// Render `text` as one CommonMark inline code span, delimiters included.
///
/// Untrusted text (paths, changed expressions, owner and test names) reaches
/// PR comments and step summaries through code spans. A backslash does not
/// escape a backtick inside a code span, so a lone backtick in the text used
/// to close a single-backtick span early and let `@mention` or raw HTML after
/// it render live. Here the delimiter is a backtick run one longer than the
/// longest run inside the text, so no run inside can close the span. Line
/// endings become spaces so the span stays on one line and a blank line
/// cannot end the paragraph. One space pads each side when the text starts or
/// ends with a backtick, or when it starts and ends with a space (CommonMark
/// strips exactly one such space pair). Empty text renders as `` ` ` ``.
///
/// Ordinary text without backticks renders as `` `text` ``, byte-identical to
/// the plain `format!("`{text}`")` it replaces. The result is for block and
/// list contexts; a GFM table cell additionally needs `|` escaped as `\|`.
pub(crate) fn code_span(text: &str) -> String {
    let text = one_line(text);
    if text.is_empty() {
        return "` `".to_string();
    }
    let mut longest_run = 0usize;
    let mut run = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            run += 1;
            longest_run = longest_run.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(longest_run + 1);
    let edge_backtick = text.starts_with('`') || text.ends_with('`');
    let stripped_space_pair =
        text.starts_with(' ') && text.ends_with(' ') && text.chars().any(|ch| ch != ' ');
    let pad = if edge_backtick || stripped_space_pair {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{text}{pad}{fence}")
}

/// Content of `text` when the whole of it is exactly one inline code span as
/// [`code_span`] renders it, else `None`. It inverts [`code_span`] for text
/// without line endings, so a renderer reading a section back from a posted
/// comment body recovers the original value, backticks included.
pub(crate) fn code_span_content(text: &str) -> Option<String> {
    let fence_len = text.chars().take_while(|ch| *ch == '`').count();
    if fence_len == 0 || text.len() < fence_len * 2 {
        return None;
    }
    // Backticks are one byte each, so byte offsets from the fence length are
    // exact; `get` refuses a split inside a multi-byte character.
    let fence = text.get(..fence_len)?;
    let inner = text.get(fence_len..text.len() - fence_len)?;
    let closing = text.get(text.len() - fence_len..)?;
    if closing != fence || inner.is_empty() || inner.ends_with('`') {
        return None;
    }
    // A run of exactly `fence_len` backticks inside would close the span early.
    let mut run = 0usize;
    for ch in inner.chars().chain(std::iter::once(' ')) {
        if ch == '`' {
            run += 1;
        } else {
            if run == fence_len {
                return None;
            }
            run = 0;
        }
    }
    let content = match inner
        .strip_prefix(' ')
        .and_then(|rest| rest.strip_suffix(' '))
    {
        Some(stripped) if inner.chars().any(|ch| ch != ' ') => stripped,
        _ => inner,
    };
    Some(content.to_string())
}

/// [`code_span`] for a GFM table cell: every `|` becomes `\|`, so a pipe in
/// the text cannot split the cell. GFM unescapes `\|` inside the cell before
/// it parses the code span, so the span still shows a bare `|`.
pub(crate) fn table_code_span(text: &str) -> String {
    code_span(text).replace('|', "\\|")
}

/// Untrusted prose for a Markdown block (a comment section or a paragraph):
/// neutralises `@mention` and raw HTML outside code spans. See [`neutralize`].
pub(crate) fn prose(text: &str) -> String {
    neutralize(text, false)
}

/// Untrusted prose on one line (a list item or a heading): line endings
/// become spaces so the text cannot start a new block, then [`prose`].
pub(crate) fn inline_prose(text: &str) -> String {
    neutralize(&one_line(text), false)
}

/// [`inline_prose`] that also renders `*`, `_`, `[` and `]` literally outside
/// code spans, for free text that must not turn into emphasis or a link.
pub(crate) fn inline_prose_literal(text: &str) -> String {
    neutralize(&one_line(text), true)
}

/// [`inline_prose`] for a GFM table cell: `|` also becomes `\|`.
pub(crate) fn table_cell_text(text: &str) -> String {
    inline_prose(text).replace('|', "\\|")
}

fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

/// Neutralise untrusted prose outside code spans (#4468).
///
/// `@` followed by a username character gains a word joiner (U+2060), so
/// GitHub renders the text but does not notify the named user or team, and
/// `<` becomes `&lt;`, so no raw HTML or autolink renders. A code span, as
/// CommonMark reads it, is copied unchanged: its content is already literal,
/// and ripr renders its own spans through [`code_span`]. A backslash escape
/// is copied as a pair, so an escaped backtick opens no span; `\@` is not a
/// pair, since the escaped `@` still renders as a mention. The result is
/// idempotent, so text read back from a posted body can be rendered again.
fn neutralize(text: &str, literal_markup: bool) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let mut out = String::with_capacity(text.len());
    let mut index = 0usize;
    while let Some(&ch) = chars.get(index) {
        match ch {
            '\\' => match chars.get(index + 1) {
                Some(&next) if next.is_ascii_punctuation() && next != '@' => {
                    out.push(ch);
                    out.push(next);
                    index += 2;
                }
                _ => {
                    out.push(ch);
                    index += 1;
                }
            },
            '`' => {
                let run = backtick_run(&chars, index);
                let end = closing_backtick_run(&chars, index + run, run).unwrap_or(index + run);
                out.extend(chars.get(index..end).unwrap_or_default());
                index = end;
            }
            '<' => {
                out.push_str("&lt;");
                index += 1;
            }
            '@' => {
                out.push('@');
                if chars
                    .get(index + 1)
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '-')
                {
                    out.push('\u{2060}');
                }
                index += 1;
            }
            '*' | '_' | '[' | ']' if literal_markup => {
                out.push('\\');
                out.push(ch);
                index += 1;
            }
            _ => {
                out.push(ch);
                index += 1;
            }
        }
    }
    out
}

fn backtick_run(chars: &[char], start: usize) -> usize {
    chars
        .iter()
        .skip(start)
        .take_while(|ch| **ch == '`')
        .count()
}

/// End (exclusive) of the first run of exactly `len` backticks at or after
/// `from`, which closes a code span opened by a run of `len`.
fn closing_backtick_run(chars: &[char], from: usize, len: usize) -> Option<usize> {
    let mut index = from;
    while index < chars.len() {
        let run = backtick_run(chars, index);
        if run == len {
            return Some(index + run);
        }
        index += run.max(1);
    }
    None
}

/// One-line disclosure emitted in place of a PowerShell variant when the bash
/// command is unsupported or compound and no honest translation exists
/// (#2628). Standalone emitters append the sentence period; emitters that
/// name the command append `: <command>`.
pub(crate) const POWERSHELL_UNAVAILABLE_DISCLOSURE: &str =
    "PowerShell form unavailable for unsupported or compound commands";

/// What a generated-command surface prints after a Bash command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PowershellForm {
    /// The command runs unchanged in PowerShell; print no second form. A
    /// byte-identical PowerShell block read as a second, different command
    /// (onboarding re-walk F60-12).
    SameAsBash,
    /// PowerShell needs this different form.
    Translated(String),
    /// No honest translation exists (a compound command); print
    /// [`POWERSHELL_UNAVAILABLE_DISCLOSURE`].
    Unavailable,
}

/// Classify [`powershell_command`]'s result for one Bash command. Every
/// generated-command surface prints a PowerShell form only for
/// `Translated`, so the choice is made here once.
pub(crate) fn powershell_form(command: &str) -> PowershellForm {
    match powershell_command(command) {
        None => PowershellForm::Unavailable,
        Some(line) if line == command => PowershellForm::SameAsBash,
        Some(line) => PowershellForm::Translated(line),
    }
}

/// Statement that makes PowerShell decode the captured producer stdout as
/// UTF-8. PowerShell decodes native stdout with `[Console]::OutputEncoding`,
/// which is the OEM code page (437, 850, ...) in Windows PowerShell 5.1 and
/// in pwsh without the UTF-8 system locale, so a non-ASCII byte such as the
/// `—` in check JSON was re-encoded as `ΓÇö` before the BOM-free write and
/// broke `agent verify` content commitments. The setter fails without an
/// attached console; the `catch` keeps the command running in that case,
/// where there is also no console code page to misdecode through.
const POWERSHELL_UTF8_STDOUT: &str =
    "try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; ";

/// Saves the session's console encoding before [`POWERSHELL_UTF8_STDOUT`]
/// so the capture can restore it: the setting is process-wide, and a pasted
/// line must not leave later native tools in a long-lived session decoding
/// their OEM output as UTF-8.
const POWERSHELL_SAVE_ENCODING: &str = "$riprEncoding = [Console]::OutputEncoding; ";

/// Restores the saved encoding in a `finally`, so a throwing capture
/// restores it too.
const POWERSHELL_RESTORE_ENCODING: &str =
    " finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }";

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
///   A second redirect withholds: anything after the first operator is the
///   artifact path, and `>` is not a valid Windows filename character.
/// - The artifact write is guarded: the invocation's output is captured, the
///   write happens only `if ($LASTEXITCODE -eq 0)`, and a nonzero status
///   throws `"ripr exited with code $LASTEXITCODE"`. Without the guard, a
///   nonzero `ripr` exit still published the artifact and exited 0, so a
///   failed step looked complete (PR #3625 review, codex P1). The failure
///   surfaces as `throw`, not `exit`: `throw` aborts a pasted or scripted
///   block — so in a composed fence a failed snapshot stops the sequence
///   before its outcome command — while leaving an interactive session open
///   where `exit` would close it (PR #3625 follow-up review).
/// - The captured text is normalized back to LF before the write:
///   `Out-String` reflows producer stdout to CRLF line endings, which
///   rewrote every byte of a pasted snapshot and broke `agent verify`
///   content commitments (issue #3966). Producers emit LF-only stdout, so
///   collapsing CRLF pairs restores the exact producer bytes the
///   commitment was computed over; the write then preserves them with
///   BOM-free UTF-8. A producer emitting raw CR bytes would need a
///   different transport — none exists on main.
/// - Capture decodes stdout as UTF-8 ([`POWERSHELL_UTF8_STDOUT`]), and the
///   write target resolves against the PowerShell location: .NET resolves a
///   relative path against the process directory, which `Set-Location` does
///   not move, so `cd repo` followed by the pasted line wrote the artifact
///   into the directory PowerShell started in.
///
/// cmd.exe has no translation: it has no quoting form that keeps an argv token
/// literal, so a generated command is deliberately not offered for it. This
/// lives beside the markdown render helpers because every generated-command
/// surface renders both shell variants from this one implementation.
///
/// Test depth: the string pins below run on every lane, and the
/// Windows-gated `powershell_translation_preserves_native_argv_and_artifact_bytes`
/// executes the translated lines under a real `pwsh` on the Windows lane,
/// where a missing `pwsh` fails closed instead of skipping.
pub(crate) fn powershell_command(command: &str) -> Option<String> {
    if is_compound_bash_command(command) {
        return None;
    }
    let command = command.replace("'\\''", "''");
    if let Some(index) = powershell_redirect_offset(&command) {
        let invocation = invoke_quoted_program(command[..index].trim_end());
        let target = command[index + 1..].trim();
        // A second redirect leaves `>` inside the artifact path, which has
        // no Windows translation (`>` is not a valid filename character
        // there), so the whole form withholds instead of emitting a
        // WriteAllText call that rejects its own path.
        if target.contains('>') {
            return None;
        }
        let output = powershell_literal(target);
        return Some(format!(
            "{POWERSHELL_SAVE_ENCODING}{POWERSHELL_UTF8_STDOUT}try {{ $ripr = (({invocation}) | Out-String) }}{POWERSHELL_RESTORE_ENCODING}; if ($LASTEXITCODE -eq 0) {{ [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath({output}), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) }} else {{ throw \"ripr exited with code $LASTEXITCODE\" }}"
        ));
    }
    Some(invoke_quoted_program(&command))
}

/// A quoted program path in command position is a string expression in
/// PowerShell, not an invocation: without the call operator the copied line
/// echoes the path and exits 0 without running anything (native proof,
/// #1672 — the recorder never ran, so no stdout marker and no argv record
/// appeared). The quote may follow leading whitespace, which the compound
/// check accepts; the operator still applies and the spacing is preserved.
/// Unquoted program names invoke directly and need no operator.
fn invoke_quoted_program(invocation: &str) -> String {
    if matches!(
        invocation.trim_start().chars().next(),
        Some('\'') | Some('"')
    ) {
        format!("& {invocation}")
    } else {
        invocation.to_string()
    }
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
/// must keep translating. Shell-expansion forms that PowerShell would read
/// differently are withheld too: glob characters (`*`, `?`, `[`, `]`), brace
/// expansion (`{`, `}`), subshell and grouping parentheses, `@` splatting,
/// `~` expansion, `#` comments, `$` expansion anywhere (including inside
/// double quotes), and any `>` that is not the spaced ` > ` redirect operator
/// (`2>err`, `>&2`, `>>`). Anything else outside the allowlist
/// (alphanumerics, whitespace, `.`, `/`, `_`, `-`, `:`) fails closed: an
/// unlisted character withholds the translation, so `--%`, `KEY=value`, and
/// other unsupported tokens never ship a line the shells would read
/// differently. A trailing unbalanced quote withholds as well: the bash form
/// is malformed there, so there is no honest line to translate. When in doubt
/// the caller under-emits: a false "compound" costs one disclosure line, a
/// false "simple" would publish an invalid or semantically different
/// PowerShell line. Unquoted LF, CRLF, and bare CR are withheld before
/// redirect parsing: a command list must not be folded into one invocation or
/// mistaken for part of an artifact path. Quoted line separators are argument
/// data and keep the normal translation path.
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
                // double-quoted backslash under-emits (#3625 review). Any
                // double-quoted `$` under-emits too: PowerShell would expand
                // `$VAR` and `$()` forms that bash may leave literal, so the
                // same pasted text can run differently across shells.
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
                    // `\'` outside quotes is quoting only as the middle of
                    // the close-escape-reopen idiom `'\''` (`'it'\''s'`),
                    // which `powershell_command` rewrites to `''`. A bare
                    // `a\'b` is one Bash argument but an unmatched quote in
                    // PowerShell, so it withholds.
                    Some('\'')
                        if index > 0
                            && chars[index - 1] == '\''
                            && chars.get(index + 2) == Some(&'\'') =>
                    {
                        index += 1;
                    }
                    // Any other escape (`\;`, `\&`, `\ `, `\\`...) changes
                    // how the shells tokenize the line.
                    Some(_) => return true,
                    None => index += 1,
                },
                ';' | '\n' | '\r' => return true,
                '>' => {
                    // Only the spaced ` > ` operator translates (detected by
                    // `powershell_redirect_offset`); any other `>` form
                    // (`2>err`, `>&2`, `>>`, `a>b`) tokenizes differently
                    // across shells.
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
                '#' if index == 0
                    || chars
                        .get(index - 1)
                        .is_some_and(|character| character.is_whitespace()) =>
                {
                    return true;
                }
                '*' | '?' | '[' | ']' => return true,
                '{' | '}' => return true,
                '(' | ')' => return true,
                '@' => return true,
                '~' if index == 0
                    || chars
                        .get(index - 1)
                        .is_some_and(|character| character.is_whitespace()) =>
                {
                    return true;
                }
                _ if ch.is_alphanumeric()
                    || ch.is_ascii_whitespace()
                    || matches!(ch, '.' | '/' | '_' | '-' | ':') => {}
                _ => return true,
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

    /// Every case must read back unchanged, and its span must hold no
    /// backtick run as long as its fence (else the span closes early).
    fn assert_code_span(text: &str, expected: &str) {
        let rendered = code_span(text);
        assert_eq!(rendered, expected, "code span for {text:?}");
        let fence_len = rendered.chars().take_while(|ch| *ch == '`').count();
        let inner = rendered
            .get(fence_len..rendered.len() - fence_len)
            .unwrap_or_default();
        let mut run = 0usize;
        for ch in inner.chars().chain(std::iter::once(' ')) {
            if ch == '`' {
                run += 1;
            } else {
                assert_ne!(run, fence_len, "inner run closes {rendered:?}");
                run = 0;
            }
        }
        let one_line = text.replace("\r\n", " ").replace(['\r', '\n'], " ");
        let expected_content = if one_line.is_empty() {
            " ".to_string()
        } else {
            one_line
        };
        assert_eq!(
            code_span_content(&rendered),
            Some(expected_content),
            "round trip for {text:?}"
        );
    }

    #[test]
    fn code_span_keeps_plain_text_byte_identical() {
        assert_code_span(
            "amount >= discount_threshold",
            "`amount >= discount_threshold`",
        );
        assert_code_span("src/lib.rs:12", "`src/lib.rs:12`");
        assert_code_span("a | b", "`a | b`");
    }

    #[test]
    fn code_span_single_backtick_cannot_close_the_span() {
        assert_code_span(
            "\"x` @octocat <img src=x onerror=alert(1)> | y\" (equality boundary)",
            "``\"x` @octocat <img src=x onerror=alert(1)> | y\" (equality boundary)``",
        );
    }

    #[test]
    fn code_span_double_backtick_run_gets_a_longer_fence() {
        assert_code_span("a``b @octocat", "```a``b @octocat```");
        assert_code_span("a`b``c```d", "````a`b``c```d````");
    }

    #[test]
    fn code_span_pads_text_that_starts_or_ends_with_a_backtick() {
        assert_code_span("`x", "`` `x ``");
        assert_code_span("x`", "`` x` ``");
        assert_code_span("`", "`` ` ``");
        assert_code_span("``x``", "``` ``x`` ```");
    }

    #[test]
    fn code_span_replaces_line_endings_with_spaces() {
        assert_code_span("a\nb\r\nc\rd", "`a b c d`");
        assert_code_span("x\n\n@octocat <img>", "`x  @octocat <img>`");
    }

    #[test]
    fn code_span_preserves_an_edge_space_pair_and_empty_text() {
        assert_code_span(" a ", "`  a  `");
        assert_code_span("", "` `");
        assert_eq!(code_span_content("plain"), None);
        assert_eq!(code_span_content("`a` and `b`"), None);
        assert_eq!(code_span_content("``a`"), None);
        assert_eq!(code_span_content("`é"), None);
    }

    #[test]
    fn table_code_span_escapes_pipes_after_choosing_the_fence() {
        assert_eq!(table_code_span("a | b"), "`a \\| b`");
        assert_eq!(table_code_span("x`|@y"), "``x`\\|@y``");
        assert_eq!(table_code_span("plain"), "`plain`");
    }

    #[test]
    fn prose_neutralises_mentions_and_raw_html() {
        assert_eq!(prose("ping @octocat now"), "ping @\u{2060}octocat now");
        assert_eq!(prose("@org/team"), "@\u{2060}org/team");
        assert_eq!(prose("<script>x</script>"), "&lt;script>x&lt;/script>");
        // `@` without a username character after it stays as written.
        assert_eq!(prose("a @ b, a@"), "a @ b, a@");
        assert_eq!(prose("line\n@octocat"), "line\n@\u{2060}octocat");
        // An escaped `@` still renders as a mention, so it is neutralised.
        assert_eq!(prose("\\@octocat"), "\\@\u{2060}octocat");
    }

    #[test]
    fn prose_leaves_text_inside_code_spans_untouched() {
        let span = code_span("a` @octocat <img>");
        let text = format!("Add an assertion for {span} and tell @octocat <b>");
        assert_eq!(
            prose(&text),
            format!("Add an assertion for {span} and tell @\u{2060}octocat &lt;b>")
        );
        // An unclosed backtick run is literal text, so what follows it is not
        // protected by it.
        assert_eq!(prose("a ` @octocat"), "a ` @\u{2060}octocat");
        assert_eq!(prose("``x` @octocat"), "``x` @\u{2060}octocat");
        // An escaped backtick opens no span.
        assert_eq!(prose("\\`@octocat`"), "\\`@\u{2060}octocat`");
    }

    #[test]
    fn prose_is_idempotent() {
        let once = prose("@octocat <img> `@x` \\@y");
        assert_eq!(prose(&once), once);
    }

    #[test]
    fn inline_and_table_prose_stay_on_one_line_and_in_one_cell() {
        assert_eq!(inline_prose("a\r\n# b\n@c"), "a # b @\u{2060}c");
        assert_eq!(table_cell_text("a | @b\n<i>"), "a \\| @\u{2060}b &lt;i>");
        assert_eq!(
            inline_prose_literal("*a* [l](u) `*k*` snake_case"),
            "\\*a\\* \\[l\\](u) `*k*` snake\\_case"
        );
    }

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
            Some("$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr check --root 'café') | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('résumé.json'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
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
            Some("$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr check --root \"café > owner's repo\") | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('résumé.json'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }

    #[test]
    fn powershell_command_keeps_double_quote_literal_inside_single_quotes() {
        assert_eq!(
            powershell_command("cargo test 'a \" > b' > evidence.txt"),
            Some("$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((cargo test 'a \" > b') | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('evidence.txt'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
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
        // A bare escaped apostrophe outside the idiom is one Bash argument
        // (`a'b`) but an unmatched quote in PowerShell: withhold it.
        assert_eq!(powershell_command("cargo test a\\'b"), None);
        assert_eq!(powershell_command("cargo test 'a'\\'b"), None);
        // A quoted `>` inside an argument must not be mistaken for a redirect.
        assert_eq!(
            powershell_command("ripr receipt write --gap 'gap > file'"),
            Some("ripr receipt write --gap 'gap > file'".to_string())
        );
    }

    /// A quoted program path in command position needs the call operator:
    /// PowerShell reads a bare quoted string as a string expression, so the
    /// translated line would echo the path and exit 0 without executing
    /// anything (native proof, #1672). Unquoted program names invoke
    /// directly and keep the line unchanged.
    #[test]
    fn powershell_command_invokes_quoted_program_paths_with_call_operator() {
        assert_eq!(
            powershell_command("'my tools\\recorder.exe' --gap 'it''s'"),
            Some("& 'my tools\\recorder.exe' --gap 'it''s'".to_string())
        );
        assert_eq!(
            powershell_command("'my tools\\recorder.exe' --gap > 'out\\after.json'"),
            Some("$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((& 'my tools\\recorder.exe' --gap) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('out\\after.json'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
        assert_eq!(
            powershell_command("cargo test --gap"),
            Some("cargo test --gap".to_string())
        );
        // Leading whitespace does not hide the quoted program: the operator
        // still applies and the spacing is preserved.
        assert_eq!(
            powershell_command("  'my tools\\recorder.exe' --gap"),
            Some("&   'my tools\\recorder.exe' --gap".to_string())
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
            Some("$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr check --root . --mode draft --format repo-exposure-json) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('target/ripr/pilot/after.repo-exposure.json'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }

    /// An embedded quote in an unwrapped target must survive the literal: the
    /// wrapper doubles it. A target the bash form already single-quoted passes
    /// through with its interior `''` doubling intact.
    #[test]
    fn powershell_command_redirect_target_escapes_embedded_quotes() {
        // An unbalanced quote withholds: the bash form is malformed there
        // (the `'` opens a quote that never closes), so there is no honest
        // line to translate — the caller discloses instead.
        assert_eq!(powershell_command("ripr check --root . > it's.json"), None);
        assert_eq!(
            powershell_command("ripr check --root . > 'it'\\''s.json'"),
            Some("$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr check --root .) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('it''s.json'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }

    /// Fail-closed shell-expansion guards: forms PowerShell would read
    /// differently never ship a translated line. Quoted forms stay data and
    /// keep translating.
    #[test]
    fn powershell_command_withholds_shell_expansion_forms() {
        for command in [
            "ripr check {a,b}",
            "ripr check (echo literal)",
            "ripr check @missing",
            "ripr check --% data",
            "ripr check KEY=value",
            "ripr check --root ~/report",
            "ripr check ~",
            "ripr check # diagnostic",
            "ripr check $HOME",
            "cargo test \"prefix $HOME suffix\"",
            "cargo test \"$HOME\"",
            "ripr check *.rs",
            "ripr check a?b",
            "ripr check 2>err.json",
            "ripr check a>b.json",
            "ripr check > first.json > second.json",
            "ripr check !important",
        ] {
            assert_eq!(
                powershell_command(command),
                None,
                "must withhold {command:?}"
            );
        }
        // Quoted expansion characters are argument data, not shell syntax.
        for command in [
            "cargo test '{a,b}'",
            "cargo test '(echo literal)'",
            "ripr receipt write --gap 'a;b'",
            "cargo test '$HOME'",
        ] {
            assert!(
                powershell_command(command).is_some(),
                "must translate quoted data {command:?}"
            );
        }
    }

    /// The artifact write must sit inside the success branch, and a nonzero
    /// invocation must abort without publishing it (PR #3625 review, codex
    /// P1): without the guard, a nonzero `ripr` exit still published the
    /// artifact and exited 0, so a failed step advanced as if it had
    /// completed. The failure surfaces as `throw`, not `exit` (PR #3625
    /// follow-up review): `throw` aborts a pasted or scripted block — in a
    /// composed fence a failed snapshot stops the sequence before its outcome
    /// command — while leaving an interactive session open, where `exit`
    /// would close the reader's shell. String-pinned here; the native
    /// execution of the guard is covered by
    /// `powershell_translation_preserves_native_argv_and_artifact_bytes`.
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
                "if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('target/ripr/workflow/agent-packet.json'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) }"
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

    /// The redirect write must restore LF bytes before writing (issue #3966):
    /// `Out-String` reflows captured producer stdout to CRLF, which rewrote
    /// every byte of a pasted snapshot and broke `agent verify` content
    /// commitments, while the bash `>` form preserves bytes. Producers emit
    /// LF-only stdout, so collapsing CRLF pairs restores the exact producer
    /// bytes. String-pinned here; the executed proof lives in the #3937
    /// native PowerShell packet (V1 row on the fixed template).
    #[test]
    fn powershell_command_redirect_write_restores_lf_bytes() -> Result<(), String> {
        let line = powershell_command("ripr check --root . --json > out.json")
            .ok_or_else(|| "simple redirect must translate".to_string())?;
        if !line.contains("$ripr.Replace(\"`r`n\", \"`n\")") {
            return Err(format!(
                "redirect write must collapse Out-String CRLF reflow back to LF:\n{line}"
            ));
        }
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
            Some("$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr check --root 'café\nrepo') | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('résumé.json'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }".to_string())
        );
    }

    /// Benign argv recorder for the native Windows proof (#1672): writes each
    /// received argument length-prefixed so the test compares bytes exactly,
    /// prints one stdout marker, and exits with the `RIPR_EXIT_CODE` status
    /// (default 0). No network, no credentials, no writes outside the
    /// `RIPR_ARGV_RECORD` path. Compiled at test time with the same `rustc`
    /// that runs the suite, into a disposable root.
    const NATIVE_PROOF_RECORDER: &str = r#"use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut out: Vec<u8> = Vec::new();
    for arg in env::args_os().skip(1) {
        let bytes = arg.as_encoded_bytes();
        out.extend_from_slice(bytes.len().to_string().as_bytes());
        out.push(b':');
        out.extend_from_slice(bytes);
        out.push(b'\n');
    }
    if let Ok(record) = env::var("RIPR_ARGV_RECORD") {
        if !record.is_empty() {
            fs::write(&record, &out).unwrap();
        }
    }
    println!("RECORDER_OK café —");
    let code: u8 = env::var("RIPR_EXIT_CODE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    ExitCode::from(code)
}
"#;

    /// Exact stdout of [`NATIVE_PROOF_RECORDER`]: UTF-8 with non-ASCII text,
    /// LF-terminated as `println!` writes it.
    #[cfg(windows)]
    const RECORDER_STDOUT: &[u8] = "RECORDER_OK café —\n".as_bytes();

    /// Removes a native-proof root when the case ends, pass or fail, so
    /// repeated runs do not accumulate compiled recorders under the temp dir.
    struct RemoveOnDrop(std::path::PathBuf);

    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Disposable root for one native-proof case. Unique per call (timestamp
    /// plus pid) so parallel tests never share it.
    fn native_proof_root(name: &str) -> Result<std::path::PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ripr-pwsh-proof-{name}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("failed to create proof root: {error}"))?;
        Ok(dir)
    }

    /// Compile the recorder into the disposable root. A missing or broken
    /// `rustc` fails the proof: without the fixture executable there is no
    /// native observation to assert.
    fn compile_native_proof_recorder(root: &std::path::Path) -> Result<std::path::PathBuf, String> {
        let source = root.join("recorder.rs");
        std::fs::write(&source, NATIVE_PROOF_RECORDER)
            .map_err(|error| format!("failed to stage recorder source: {error}"))?;
        let exe = root.join("recorder.exe");
        let output = std::process::Command::new("rustc")
            .arg("--edition=2021")
            .arg("-O")
            .arg(&source)
            .arg("-o")
            .arg(&exe)
            .output()
            .map_err(|error| format!("failed to spawn rustc for recorder: {error}"))?;
        if !output.status.success() || !exe.exists() {
            return Err(format!(
                "recorder did not compile: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(exe)
    }

    /// Resolve a `pwsh` instrument by actually running it. A missing
    /// instrument fails the proof — it never passes through a skip (#1672).
    fn resolve_pwsh(program: &str) -> Result<(), String> {
        match std::process::Command::new(program)
            .arg("--version")
            .output()
        {
            Ok(output) if output.status.success() => Ok(()),
            _ => Err(format!("{program} is not runnable")),
        }
    }

    /// The recorder source must stay compilable on every lane, including
    /// lanes without `pwsh`: this guards the fixture against rot where the
    /// native test itself cannot run.
    #[test]
    fn native_proof_recorder_source_compiles() -> Result<(), String> {
        let root = native_proof_root("compile")?;
        let _cleanup = RemoveOnDrop(root.clone());
        let exe = compile_native_proof_recorder(&root)?;
        assert!(exe.exists(), "recorder executable missing after compile");
        Ok(())
    }

    /// The instrument resolver must report an absent program instead of
    /// passing silently.
    #[test]
    fn native_proof_pwsh_resolver_reports_absent_instrument() {
        assert!(resolve_pwsh("ripr-nonexistent-pwsh-probe").is_err());
    }

    #[cfg(windows)]
    fn run_pwsh_line(
        line: &str,
        cwd: &std::path::Path,
        record: &std::path::Path,
        exit_code: &str,
    ) -> Result<std::process::Output, String> {
        std::process::Command::new("pwsh")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(line)
            .current_dir(cwd)
            .env("RIPR_ARGV_RECORD", record)
            .env("RIPR_EXIT_CODE", exit_code)
            .output()
            .map_err(|error| format!("failed to spawn pwsh: {error}"))
    }

    #[cfg(windows)]
    fn recorded_argv_matches(record: &std::path::Path, expected: &[&str]) -> Result<(), String> {
        let bytes = std::fs::read(record)
            .map_err(|error| format!("failed to read argv record: {error}"))?;
        let mut want: Vec<u8> = Vec::new();
        for arg in expected {
            want.extend_from_slice(arg.len().to_string().as_bytes());
            want.push(b':');
            want.extend_from_slice(arg.as_bytes());
            want.push(b'\n');
        }
        if bytes != want {
            return Err(format!(
                "native argv mismatch: observed {bytes:?}, wanted {want:?}"
            ));
        }
        Ok(())
    }

    /// Native Windows proof (#1672): the translated lines are executed by a
    /// real `pwsh`, not compared as strings. The bash input is built with the
    /// production [`crate::agent::loop_commands::shell_arg`] renderer, the
    /// translation comes from [`powershell_command`], and the oracle compares
    /// observed argv/artifact bytes against semantic inputs — never against
    /// translator-derived expectations.
    #[cfg(windows)]
    #[test]
    fn powershell_translation_preserves_native_argv_and_artifact_bytes() -> Result<(), String> {
        use crate::agent::loop_commands::shell_arg;

        let root = native_proof_root("argv")?;
        let _cleanup = RemoveOnDrop(root.clone());
        let recorder = compile_native_proof_recorder(&root)?;
        resolve_pwsh("pwsh").map_err(|reason| {
            format!(
                "pwsh is required for the native proof and was not found ({reason}): \
                 failing closed instead of skipping"
            )
        })?;
        let recorder_arg = shell_arg(
            recorder
                .to_str()
                .ok_or_else(|| "recorder path is not UTF-8".to_string())?,
        );
        // Positive controls: spaces, apostrophe, Unicode, empty, and
        // shell-special data plus one plain token.
        let args = ["--gap", "it's", "café", "", "--verify=x;y", "plain"];
        let mut bash = recorder_arg.clone();
        for arg in args {
            bash.push(' ');
            bash.push_str(&shell_arg(arg));
        }
        let line = powershell_command(&bash)
            .ok_or_else(|| format!("supported invocation must translate: {bash}"))?;
        let record = root.join("argv.record");
        let output = run_pwsh_line(&line, &root, &record, "0")?;
        if !output.status.success() {
            return Err(format!(
                "translated invocation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if !output
            .stdout
            .windows(b"RECORDER_OK".len())
            .any(|window| window == b"RECORDER_OK")
        {
            return Err("recorder stdout marker missing from pwsh output".to_string());
        }
        recorded_argv_matches(&record, &args)?;

        // Redirect control: the artifact carries the invocation's output
        // bytes as BOM-free UTF-8.
        let artifact = root.join("after.json");
        let redirect_bash = format!(
            "{bash} > {}",
            shell_arg(
                artifact
                    .to_str()
                    .ok_or_else(|| "artifact path is not UTF-8".to_string())?
            )
        );
        let redirect_line = powershell_command(&redirect_bash)
            .ok_or_else(|| format!("supported redirect must translate: {redirect_bash}"))?;
        let redirect_record = root.join("redirect.record");
        let redirect_output = run_pwsh_line(&redirect_line, &root, &redirect_record, "0")?;
        if !redirect_output.status.success() {
            return Err(format!(
                "translated redirect failed: {}",
                String::from_utf8_lossy(&redirect_output.stderr)
            ));
        }
        recorded_argv_matches(&redirect_record, &args)?;
        let artifact_bytes = std::fs::read(&artifact)
            .map_err(|error| format!("failed to read artifact: {error}"))?;
        if artifact_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return Err("artifact carries a UTF-8 BOM".to_string());
        }
        // Byte-exact: `Out-String` ends the captured line with CRLF, so this
        // fails if the `.Replace` LF normalization is dropped or altered.
        if artifact_bytes != RECORDER_STDOUT {
            return Err(format!(
                "artifact bytes {artifact_bytes:?} are not exactly the recorder's UTF-8 stdout"
            ));
        }

        // Encoding control: under the OEM console code page Windows
        // PowerShell 5.1 defaults to, the non-ASCII stdout bytes still reach
        // the artifact unchanged. The same line without the UTF-8 capture
        // statement must garble them, which proves this instrument can see
        // the misdecode rather than passing on a UTF-8 host.
        let oem = "[Console]::OutputEncoding = [System.Text.Encoding]::GetEncoding(437); ";
        // The trailing probe prints the session's encoding after the line,
        // so the capture must also have restored code page 437.
        let probe = "; [Console]::OutputEncoding.CodePage";
        let oem_output = run_pwsh_line(
            &format!("{oem}{redirect_line}{probe}"),
            &root,
            &redirect_record,
            "0",
        )?;
        if !oem_output.status.success() {
            return Err(format!(
                "translated redirect under code page 437 failed: {}",
                String::from_utf8_lossy(&oem_output.stderr)
            ));
        }
        if String::from_utf8_lossy(&oem_output.stdout).trim() != "437" {
            return Err(format!(
                "the capture left the session encoding changed: {:?}",
                String::from_utf8_lossy(&oem_output.stdout)
            ));
        }
        // A failing invocation throws inside the capture; `finally` must
        // still restore the session encoding.
        let failed_restore = run_pwsh_line(
            &format!("{oem}try {{ {redirect_line} }} catch {{}}{probe}"),
            &root,
            &redirect_record,
            "1",
        )?;
        if String::from_utf8_lossy(&failed_restore.stdout).trim() != "437" {
            return Err(format!(
                "a failing capture left the session encoding changed: {:?}",
                String::from_utf8_lossy(&failed_restore.stdout)
            ));
        }
        let oem_bytes = std::fs::read(&artifact)
            .map_err(|error| format!("failed to read code page 437 artifact: {error}"))?;
        if oem_bytes != RECORDER_STDOUT {
            return Err(format!(
                "code page 437 capture changed the artifact bytes to {oem_bytes:?}"
            ));
        }
        let unguarded = format!(
            "{oem}{}",
            redirect_line.replacen(POWERSHELL_UTF8_STDOUT, "", 1)
        );
        run_pwsh_line(&unguarded, &root, &redirect_record, "0")?;
        let garbled = std::fs::read(&artifact)
            .map_err(|error| format!("failed to read unguarded artifact: {error}"))?;
        if garbled == RECORDER_STDOUT {
            return Err(
                "control failed: without the UTF-8 capture statement code page 437 still \
                 produced exact bytes, so this host cannot observe the misdecode"
                    .to_string(),
            );
        }

        // Location control: a relative target lands under the PowerShell
        // location after `Set-Location`, not under the directory pwsh
        // started in, which `Set-Location` does not move.
        let elsewhere = root.join("started-here");
        std::fs::create_dir_all(&elsewhere)
            .map_err(|error| format!("failed to stage start directory: {error}"))?;
        let relative_line = powershell_command(&format!("{bash} > relative.json"))
            .ok_or_else(|| "relative redirect must translate".to_string())?;
        let root_literal = root
            .to_str()
            .ok_or_else(|| "native proof root is not UTF-8".to_string())?
            .replace('\'', "''");
        let located = format!("Set-Location -LiteralPath '{root_literal}'; {relative_line}");
        let located_output = run_pwsh_line(&located, &elsewhere, &redirect_record, "0")?;
        if !located_output.status.success() {
            return Err(format!(
                "relative redirect after Set-Location failed: {}",
                String::from_utf8_lossy(&located_output.stderr)
            ));
        }
        if !root.join("relative.json").is_file() || elsewhere.join("relative.json").exists() {
            return Err(
                "relative artifact target did not resolve against the PowerShell location"
                    .to_string(),
            );
        }

        // Failure control: a nonzero invocation throws without publishing
        // the artifact over the pre-existing bytes.
        std::fs::write(&artifact, b"SENTINEL")
            .map_err(|error| format!("failed to stage sentinel artifact: {error}"))?;
        let failure_record = root.join("failure.record");
        let failure_output = run_pwsh_line(&redirect_line, &root, &failure_record, "1")?;
        if failure_output.status.success() {
            return Err("failing invocation must not exit zero".to_string());
        }
        let preserved = std::fs::read(&artifact)
            .map_err(|error| format!("failed to re-read artifact: {error}"))?;
        if preserved != b"SENTINEL" {
            return Err("failing invocation published over the artifact".to_string());
        }

        // Withholding controls: unsupported forms never reach a subprocess.
        // There is nothing to execute, so the proof is that translation
        // withholds and the would-be artifact is never created.
        let ghost = root.join("ghost.record");
        for withheld in [
            format!("{bash} && {bash}"),
            format!("{bash} > {} > {}", shell_arg("a.json"), shell_arg("b.json")),
            format!("{bash} < {}", shell_arg("input.json")),
            format!("{bash} --gap 'unterminated"),
        ] {
            assert!(
                powershell_command(&withheld).is_none(),
                "must withhold: {withheld:?}"
            );
        }
        assert!(
            !ghost.exists(),
            "withheld forms must take no subprocess/output action"
        );
        Ok(())
    }
}
