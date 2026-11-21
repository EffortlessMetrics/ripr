//! Static npm dist-tag guard for in-tree GitHub Actions workflows
//! (EffortlessMetrics/ripr-swarm#9663, carried from the swarm
//! `check-release-targets` npm-launcher guard).
//!
//! The real npm publish workflow (`.github/workflows/publish-npm.yml`) lives
//! in this repository, so the swarm guard cannot see it. This module ports the
//! same refusal semantics: a workflow `npm publish` must pass exactly one
//! literal non-`latest` `--tag`, and an `npm dist-tag add`/`set` must carry a
//! literal non-`latest` tag. Variables, expressions, command substitution and
//! quoting tricks are rejected because they can expand to `latest` at run
//! time; `--dry-run` publishes are exempt. Without this guard an accidental
//! `latest` move from the publish workflow (or a later promotion step) would
//! not be noticed by any check.

/// Joins shell `\`-continued lines so a command split across physical lines is
/// evaluated as one logical command. Returns each logical line with the
/// one-based number of its first physical line.
fn logical_lines(physical: &[(usize, String)]) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut start = 0;
    for (number, raw) in physical {
        if current.is_empty() {
            start = *number;
        }
        let code = raw.trim_end_matches('\r');
        if let Some(stripped) = code.strip_suffix('\\')
            && !shell_scan(&format!("{current}{stripped}")).1
        {
            current.push_str(stripped);
            current.push(' ');
            continue;
        }
        current.push_str(code);
        lines.push((start, std::mem::take(&mut current)));
    }
    if !current.is_empty() {
        lines.push((start, current));
    }
    lines
}

fn is_block_scalar_header(value: &str) -> bool {
    // YAML allows a trailing comment after the indicator (`run: | # note`);
    // the comment is not part of the scalar style.
    let value = match value.find(" #") {
        Some(index) => &value[..index],
        None => value,
    };
    let value = value.trim();
    (value.starts_with('|') || value.starts_with('>'))
        && value[1..]
            .chars()
            .all(|c| c.is_ascii_digit() || c == '+' || c == '-')
}

/// Decodes one complete YAML quoted scalar from an inline `run:` value so
/// `run: "cd pkg && npm publish"` reaches the shell tokenizer as the text
/// GitHub Actions actually executes. The closing quote ends the scalar; any
/// remainder (a trailing YAML comment) is discarded. YAML double-quoted
/// escapes are decoded, including `\xNN`/`\uNNNN`/`\UXXXXXXXX`, so an escaped
/// `npm` cannot hide behind a different spelling. A value that does not start
/// with a quote is returned unchanged; an unterminated quote is returned
/// unchanged and stays fail-closed in the shell tokenizer.
fn strip_yaml_quotes(value: &str) -> String {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() || (bytes[0] != b'"' && bytes[0] != b'\'') {
        return trimmed.to_string();
    }
    let quote = bytes[0];
    let mut index = 1;
    while index < bytes.len() {
        if quote == b'"' && bytes[index] == b'\\' && index + 1 < bytes.len() {
            index += 2;
            continue;
        }
        if bytes[index] == quote {
            if quote == b'\'' && bytes.get(index + 1) == Some(&b'\'') {
                index += 2;
                continue;
            }
            let inner = &trimmed[1..index];
            return if quote == b'\'' {
                inner.replace("''", "'")
            } else {
                unescape_yaml_double_quoted(inner)
            };
        }
        index += 1;
    }
    trimmed.to_string()
}

fn unescape_yaml_double_quoted(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('0') => out.push('\0'),
            Some('a') => out.push('\u{7}'),
            Some('b') => out.push('\u{8}'),
            Some('t') | Some('\t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('v') => out.push('\u{b}'),
            Some('f') => out.push('\u{c}'),
            Some('r') => out.push('\r'),
            Some('e') => out.push('\u{1b}'),
            Some(' ') => out.push(' '),
            Some('"') => out.push('"'),
            Some('/') => out.push('/'),
            Some('\\') => out.push('\\'),
            Some('N') => out.push('\u{85}'),
            Some('_') => out.push('\u{a0}'),
            Some('L') => out.push('\u{2028}'),
            Some('P') => out.push('\u{2029}'),
            Some('x') => push_hex(&mut chars, 2, &mut out),
            Some('u') => push_hex(&mut chars, 4, &mut out),
            Some('U') => push_hex(&mut chars, 8, &mut out),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn push_hex(chars: &mut std::str::Chars<'_>, width: usize, out: &mut String) {
    let mut value: u32 = 0;
    for _ in 0..width {
        match chars.next().and_then(|c| c.to_digit(16)) {
            Some(digit) => value = value * 16 + digit,
            None => {
                out.push('\\');
                return;
            }
        }
    }
    if let Some(decoded) = char::from_u32(value) {
        out.push(decoded);
    }
}

/// Splits a YAML line into `(key, value)` when it is a mapping entry such as
/// `name: x`, `- run: npm publish` or `run: |`.
fn yaml_key_value(content: &str) -> Option<(&str, &str)> {
    let colon = content.find(':')?;
    let key = &content[..colon];
    let rest = &content[colon + 1..];
    let key_like = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    let value_boundary = rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t');
    (key_like && value_boundary).then(|| (key, rest.trim_start()))
}

/// Extracts only text the shell can run from workflow YAML: the value of a
/// `run:` key, the body of a `run:` block scalar, and plain non-key lines.
/// Other keys (`name:`, `description:`, `if:`, `env:` values, ...) and their
/// block-scalar bodies are skipped. Returns one-based physical line numbers.
///
/// YAML folding is respected conservatively: a `>`-style run body emits both
/// the folded group as one space-joined shell line and its physical lines, so
/// a publish split across folded lines is caught while a trailing comment in
/// one folded line cannot hide a later command. `|`-style bodies stay one
/// shell line per physical line, joined only by backslash continuation.
fn workflow_script_lines(text: &str) -> Vec<(usize, String)> {
    let mut scripts = Vec::new();
    // (key column of the block header line, body is shell script, folded style)
    let mut block: Option<(usize, bool, bool)> = None;
    // (first physical line, accumulated folded lines) for a `>`-style run body.
    let mut fold: Option<(usize, Vec<String>)> = None;
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let indent = raw.len() - raw.trim_start().len();
        let trimmed = raw.trim();
        if let Some((header_indent, is_script, folded)) = block {
            if trimmed.is_empty() || indent > header_indent {
                if is_script {
                    if folded {
                        if trimmed.is_empty() {
                            if let Some((start, lines)) = fold.take() {
                                scripts.push((start, lines.join(" ")));
                            }
                        } else {
                            fold.get_or_insert_with(|| (number, Vec::new()))
                                .1
                                .push(trimmed.to_string());
                            scripts.push((number, trimmed.to_string()));
                        }
                    } else {
                        scripts.push((number, raw.to_string()));
                    }
                }
                continue;
            }
            block = None;
            if let Some((start, lines)) = fold.take() {
                scripts.push((start, lines.join(" ")));
            }
        }
        let mut content = raw.trim_start();
        while let Some(rest) = content.strip_prefix("- ") {
            content = rest.trim_start();
        }
        // YAML ends a block scalar at the key's column, which for a `- key:`
        // list item is past the dash.
        let key_column = raw.len() - content.len();
        match yaml_key_value(content) {
            Some((key, value)) => {
                let is_run = key == "run";
                if is_block_scalar_header(value) {
                    block = Some((key_column, is_run, value.trim_start().starts_with('>')));
                } else if is_run {
                    scripts.push((number, strip_yaml_quotes(value)));
                }
            }
            None => scripts.push((number, content.to_string())),
        }
    }
    if let Some((start, lines)) = fold.take() {
        scripts.push((start, lines.join(" ")));
    }
    scripts
}

/// A dist-tag is acceptable only when it is a literal, valid, non-`latest`
/// word. Variables, expressions, command substitution and quoting tricks are
/// rejected because they can expand to `latest` at run time.
fn literal_non_latest_tag(token: &str) -> Result<(), String> {
    let value = token.trim_matches(|c| c == '"' || c == '\'');
    let valid = value
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !valid {
        return Err(format!(
            "tag `{token}` is not a literal dist-tag (variables and expressions can expand to `latest`)"
        ));
    }
    if value.eq_ignore_ascii_case("latest") {
        return Err("tag `latest` is forbidden".to_string());
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum ShellToken {
    Word(String),
    Separator,
}

/// Small shell-aware tokenizer: quotes group words (and are stripped), `#` at
/// the start of a word outside quotes begins a comment, and `;`, `&`, `|`, `(`
/// and `)` outside quotes end a command even when glued to a word. A quoted
/// word that itself begins with `npm ` is re-tokenized so `run: 'npm publish'`
/// is still seen.
fn shell_tokens(line: &str) -> Vec<ShellToken> {
    shell_scan(line).0
}

/// Tokenizes `line` and reports whether an unquoted comment began on it, using
/// the one quote/comment rule shared with continuation joining.
fn shell_scan(line: &str) -> (Vec<ShellToken>, bool) {
    let mut commented = false;
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut quoted_word = false;
    let flush =
        |tokens: &mut Vec<ShellToken>, word: &mut String, in_word: &mut bool, quoted: &mut bool| {
            if *in_word {
                let text = std::mem::take(word);
                if *quoted && text.trim_start().starts_with("npm ") {
                    tokens.push(ShellToken::Separator);
                    tokens.extend(shell_tokens(&text));
                    tokens.push(ShellToken::Separator);
                } else {
                    tokens.push(ShellToken::Word(text));
                }
            }
            *in_word = false;
            *quoted = false;
        };
    for ch in line.chars() {
        if let Some(open) = quote {
            if ch == open {
                quote = None;
            } else {
                word.push(ch);
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                in_word = true;
                quoted_word = true;
            }
            '#' if !in_word => {
                commented = true;
                break;
            }
            ';' | '&' | '|' | '(' | ')' => {
                flush(&mut tokens, &mut word, &mut in_word, &mut quoted_word);
                if tokens.last() != Some(&ShellToken::Separator) {
                    tokens.push(ShellToken::Separator);
                }
            }
            c if c.is_whitespace() => {
                flush(&mut tokens, &mut word, &mut in_word, &mut quoted_word);
            }
            c => {
                word.push(c);
                in_word = true;
            }
        }
    }
    flush(&mut tokens, &mut word, &mut in_word, &mut quoted_word);
    (tokens, commented)
}

const VALUE_FLAGS: &[&str] = &[
    "--otp",
    "--registry",
    "--workspace",
    "-w",
    "--userconfig",
    "--access",
    "--provenance-file",
    "--cache",
];

/// Fails when a workflow publishes or tags npm without exactly one literal
/// non-`latest` tag. Backslash-continued lines are joined first, global npm
/// flags may precede the subcommand, and `--dry-run` publishes are exempt. The
/// npm publish workflow stages prereleases under `next`; this keeps every
/// in-tree workflow that way and refuses an accidental `latest` move.
/// Out of scope: `pnpm`/`yarn publish`, `NPM_CONFIG_TAG`, and composite or
/// reusable actions.
pub(crate) fn validate_workflow_npm_tags(path: &str, text: &str, violations: &mut Vec<String>) {
    for (number, line) in logical_lines(&workflow_script_lines(text)) {
        let mut command: Vec<String> = Vec::new();
        let mut commands: Vec<Vec<String>> = Vec::new();
        for token in shell_tokens(&line) {
            match token {
                ShellToken::Word(word) => command.push(word),
                ShellToken::Separator => commands.push(std::mem::take(&mut command)),
            }
        }
        commands.push(command);
        for command in commands {
            let Some(npm) = command.iter().position(|t| t == "npm") else {
                continue;
            };
            let args = command[npm + 1..]
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let Some(sub) = subcommand_index(&args) else {
                continue;
            };
            match args[sub] {
                "publish" => check_publish_args(path, number, &args, violations),
                "dist-tag" | "dist-tags" => {
                    check_dist_tag_args(path, number, &args[sub + 1..], violations)
                }
                _ => {}
            }
        }
    }
}

/// Index of the npm subcommand of interest, skipping the values of
/// value-taking global flags so `npm --workspace x publish` is not misread.
fn subcommand_index(args: &[&str]) -> Option<usize> {
    let mut position = 0;
    while position < args.len() {
        let arg = args[position];
        if VALUE_FLAGS.contains(&arg) || arg == "--tag" {
            position += 2;
            continue;
        }
        if matches!(
            arg,
            "publish" | "dist-tag" | "dist-tags" | "run" | "run-script" | "exec" | "x"
        ) {
            return Some(position);
        }
        position += 1;
    }
    None
}

fn check_publish_args(path: &str, number: usize, args: &[&str], violations: &mut Vec<String>) {
    if args
        .iter()
        .any(|a| *a == "--dry-run" || a.starts_with("--dry-run=") && !a.ends_with("=false"))
    {
        return;
    }
    let mut tags = Vec::new();
    let mut position = 0;
    while position < args.len() {
        let arg = args[position];
        if arg == "--tag" {
            tags.push(args.get(position + 1).copied().unwrap_or_default());
            position += 1;
        } else if let Some(value) = arg.strip_prefix("--tag=") {
            tags.push(value);
        }
        position += 1;
    }
    match tags.as_slice() {
        [] => violations.push(format!(
            "{path}:{number}: `npm publish` must pass an explicit non-`latest` --tag"
        )),
        [tag] => {
            if let Err(reason) = literal_non_latest_tag(tag) {
                violations.push(format!("{path}:{number}: `npm publish` {reason}"));
            }
        }
        _ => violations.push(format!(
            "{path}:{number}: `npm publish` must pass exactly one --tag; repeated or mixed --tag options are ambiguous"
        )),
    }
}

fn check_dist_tag_args(path: &str, number: usize, args: &[&str], violations: &mut Vec<String>) {
    let mut positionals = Vec::new();
    let mut position = 0;
    while position < args.len() {
        let arg = args[position];
        if VALUE_FLAGS.contains(&arg) {
            position += 1;
        } else if !arg.starts_with('-') {
            positionals.push(arg);
        }
        position += 1;
    }
    if let ["add" | "set", package, rest @ ..] = positionals.as_slice() {
        if rest.is_empty() {
            violations.push(format!(
                "{path}:{number}: `npm dist-tag` without a tag argument defaults to `latest`"
            ));
            return;
        }
        // Every trailing positional must be a literal non-`latest` tag. An
        // unknown value-taking flag's bare-word value would otherwise shift
        // the tag read and hide a `latest` move.
        for candidate in rest {
            if let Err(reason) = literal_non_latest_tag(candidate) {
                violations.push(format!(
                    "{path}:{number}: `npm dist-tag` on `{package}` {reason}"
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_workflow_npm_tags;

    fn workflow_violations(text: &str) -> Vec<String> {
        let mut violations = Vec::new();
        validate_workflow_npm_tags("wf.yml", text, &mut violations);
        violations
    }

    fn has_violation(violations: &[String], needle: &str) -> bool {
        violations.iter().any(|v| v.contains(needle))
    }

    #[test]
    fn workflow_npm_publish_and_dist_tag_require_explicit_non_latest_tag() {
        let check = workflow_violations;
        assert!(check("      - run: npm publish --tag next\n").is_empty());
        assert!(check("      - run: npm publish --tag=next --access public\n").is_empty());
        assert!(check("      - run: npm publish --tag \"next\"\n").is_empty());
        assert!(check("      # npm publish\n      - run: npm pack --dry-run\n").is_empty());
        assert!(has_violation(
            &check("      - run: npm publish\n"),
            "explicit non-`latest`"
        ));
        assert!(has_violation(
            &check("      - run: npm publish --tag latest\n"),
            "forbidden"
        ));
        assert!(has_violation(
            &check("      - run: npm dist-tag add pkg@1.0.0-rc.1 latest\n"),
            "forbidden"
        ));
        assert!(check("      - run: npm dist-tag add pkg@1.0.0-rc.1 next\n").is_empty());
    }

    #[test]
    fn workflow_multiline_and_chained_npm_publish_is_not_skipped() {
        let multiline = "      - run: |\n          npm \\\n            publish --access public\n";
        assert!(has_violation(
            &workflow_violations(multiline),
            "explicit non-`latest`"
        ));
        let flag_split = "      - run: |\n          npm publish \\\n            --tag latest\n";
        assert!(has_violation(&workflow_violations(flag_split), "forbidden"));
        let tagged = "      - run: |\n          npm publish \\\n            --tag next\n";
        assert!(workflow_violations(tagged).is_empty());
        let chained = "      - run: npm pack && npm publish\n";
        assert!(has_violation(
            &workflow_violations(chained),
            "explicit non-`latest`"
        ));
        let leading_flag = "      - run: npm --silent publish\n";
        assert!(has_violation(
            &workflow_violations(leading_flag),
            "explicit non-`latest`"
        ));
    }

    #[test]
    fn workflow_interpolated_or_repeated_tag_options_are_rejected() {
        for text in [
            "      - run: npm publish --tag \"$DIST_TAG\"\n",
            "      - run: npm publish --tag ${{ inputs.tag }}\n",
            "      - run: npm publish --tag $(echo latest)\n",
            "      - run: npm publish --tag=`echo latest`\n",
            "      - run: npm dist-tag add pkg@1.0.0 $TAG\n",
        ] {
            assert!(
                has_violation(&workflow_violations(text), "not a literal dist-tag"),
                "{text}"
            );
        }
        for text in [
            "      - run: npm publish --tag next --tag latest\n",
            "      - run: npm publish --tag next --tag=latest\n",
            "      - run: npm publish --tag=next --tag=next\n",
        ] {
            assert!(
                has_violation(&workflow_violations(text), "exactly one --tag"),
                "{text}"
            );
        }
    }

    #[test]
    fn workflow_global_flags_dry_run_comments_and_glued_separators() {
        for text in [
            "      - run: npm -w pkg publish\n",
            "      - run: npm --workspace x publish\n",
            "      - run: npm --otp 123 publish\n",
            "      - run: npm --tag latest publish\n",
            "      - run: echo \"a #b\"; npm publish\n",
            "      - run: foo;npm publish\n",
            "      - run: (npm publish)\n",
            "      - run: 'npm publish'\n",
            "      - run: npm publish --tag next;npm publish\n",
            "      - run: npm dist-tag add pkg@1.0.0-rc.1\n",
            "      - run: npm dist-tags add pkg@1.0.0-rc.1\n",
            "      - run: npm dist-tag add --otp 123 pkg@1.0.0 latest\n",
        ] {
            assert!(!workflow_violations(text).is_empty(), "{text}");
        }
        for text in [
            "      - run: npm publish --dry-run\n",
            "# npm publish\n",
            "      - run: echo done # npm publish\n",
            "      - run: npm --otp 123 publish --tag next;\n",
            "      - run: npm -w pkg publish --tag=next\n",
            "      - run: npm run publish\n",
            "      - run: npm --tag next publish\n",
            "      - run: npm dist-tag add --otp 123 pkg@1.0.0 next\n",
            "      - run: npm dist-tag ls pkg\n",
        ] {
            assert!(
                workflow_violations(text).is_empty(),
                "{text}: {:?}",
                workflow_violations(text)
            );
        }
    }

    #[test]
    fn workflow_scan_only_reads_script_text_not_yaml_metadata() {
        let metadata = "steps:\n  - name: Verify npm publish gating\n    description: runs npm publish checks\n    if: contains(github.event.head_commit.message, 'npm publish')\n    env:\n      NOTE: npm publish\n    run: echo ok\n  - name: note\n    with:\n      body: |\n        npm publish should be gated\n";
        assert!(
            workflow_violations(metadata).is_empty(),
            "{:?}",
            workflow_violations(metadata)
        );
    }

    #[test]
    fn workflow_folded_scalar_publish_is_refused_and_literal_split_is_not() {
        // `>` folds the body into one shell line: `npm publish --tag latest` runs.
        let folded = "      - run: >\n          npm\n          publish --tag latest\n";
        assert!(has_violation(&workflow_violations(folded), "forbidden"));
        // Folding also joins a tagless publish across the fold.
        let tagless = "      - run: >-\n          npm\n          publish --access public\n";
        assert!(has_violation(
            &workflow_violations(tagless),
            "explicit non-`latest`"
        ));
        // A folded group whose earlier line comments the tail is still caught
        // because each physical line is also scanned.
        let swallowed =
            "      - run: >\n          echo hi # note\n          npm publish --tag latest\n";
        assert!(has_violation(&workflow_violations(swallowed), "forbidden"));
        // A literal block really runs one command per line; a bare `npm`
        // publishes nothing and `publish` is not a command.
        let literal = "      - run: |\n          npm\n          publish --tag latest\n";
        assert!(workflow_violations(literal).is_empty());
        // Backslash-continued literal bodies remain joined and caught.
        let continued = "      - run: |\n          npm \\\n            publish --tag latest\n";
        assert!(has_violation(&workflow_violations(continued), "forbidden"));
    }

    #[test]
    fn workflow_quoted_run_values_are_decoded_before_shell_scan() {
        let double = "      - run: \"cd pkg && npm publish\"\n";
        assert!(has_violation(
            &workflow_violations(double),
            "explicit non-`latest`"
        ));
        let single = "      - run: 'cd pkg && npm publish --tag latest'\n";
        assert!(has_violation(&workflow_violations(single), "forbidden"));
        let tagged = "      - run: \"npm publish --tag next\"\n";
        assert!(workflow_violations(tagged).is_empty());
        // A trailing YAML comment after the closing quote does not prevent
        // scalar decoding.
        let commented = "      - run: \"cd pkg && npm publish --tag latest\" # release\n";
        assert!(has_violation(&workflow_violations(commented), "forbidden"));
        // YAML double-quoted escapes are decoded, so an escaped `npm` cannot
        // hide behind a different spelling.
        let escaped = "      - run: \"cd pkg && \\x6epm publish --tag latest\"\n";
        assert!(has_violation(&workflow_violations(escaped), "forbidden"));
    }

    #[test]
    fn workflow_dist_tag_value_flag_shift_is_not_exploitable() {
        // `--cache` takes a value; its value must not be read as the tag.
        let shifted = "      - run: npm dist-tag add pkg@1.0.0 --cache cache latest\n";
        assert!(has_violation(&workflow_violations(shifted), "forbidden"));
        let prefixed = "      - run: npm dist-tag add --cache cache pkg@1.0.0 next\n";
        assert!(workflow_violations(prefixed).is_empty());
        // Any unrecognized trailing positional after the package is held to
        // the same literal non-`latest` rule.
        let extra = "      - run: npm dist-tag add pkg@1.0.0 next https://registry.example\n";
        assert!(has_violation(
            &workflow_violations(extra),
            "not a literal dist-tag"
        ));
    }

    #[test]
    fn workflow_block_header_trailing_comment_still_scans_body() {
        // `run: | # note` is a valid block scalar header; the body must be
        // scanned even when a body line looks like a YAML mapping entry.
        let commented = "      - run: | # install\n          foo: npm publish --tag latest\n";
        assert!(has_violation(&workflow_violations(commented), "forbidden"));
        assert!(workflow_violations("      - run: | # install\n          echo ok\n").is_empty());
    }

    /// The real publish workflow's exact staging command is the guard's
    /// subject. The checked-in workflow must be guard-clean (good path), and
    /// the same exact line mutated to `latest` — or with the explicit tag
    /// dropped, which npm defaults to `latest` — must be refused (bad path).
    #[test]
    fn real_publish_npm_workflow_passes_and_its_latest_mutations_are_refused() {
        const WORKFLOW: &str = include_str!("../../../.github/workflows/publish-npm.yml");
        assert!(
            workflow_violations(WORKFLOW).is_empty(),
            "checked-in publish-npm.yml must pass the dist-tag guard: {:?}",
            workflow_violations(WORKFLOW)
        );
        let stage_lines: Vec<&str> = WORKFLOW
            .lines()
            .filter(|line| line.contains("npm stage publish"))
            .collect();
        assert_eq!(
            stage_lines.len(),
            1,
            "publish-npm.yml must contain exactly one npm stage publish command"
        );
        let stage_line = stage_lines[0];
        assert!(stage_line.contains("--tag=next"), "{stage_line}");
        let latest_mutation = stage_line.replace("--tag=next", "--tag=latest");
        assert_ne!(stage_line, latest_mutation);
        let mutated = WORKFLOW.replace(stage_line, &latest_mutation);
        let violations = workflow_violations(&mutated);
        assert!(
            has_violation(&violations, "`npm publish` tag `latest` is forbidden"),
            "{violations:?}"
        );
        let dropped_mutation = stage_line.replace(" --tag=next", "");
        assert_ne!(stage_line, dropped_mutation);
        let dropped = WORKFLOW.replace(stage_line, &dropped_mutation);
        let violations = workflow_violations(&dropped);
        assert!(
            has_violation(&violations, "explicit non-`latest` --tag"),
            "{violations:?}"
        );
    }
}
