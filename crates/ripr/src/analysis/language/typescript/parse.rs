//! Parsing utilities for the TypeScript preview adapter.
//!
//! Every oxc parse in this adapter runs through [`parse_on_worker`]:
//!
//! 1. A cheap single-pass nesting scan estimates the deepest expression
//!    nesting in the source before any parse is attempted (issue #4101).
//!    Sources past [`PARSE_NESTING_BUDGET`] are refused with a typed
//!    `expression_nesting_budget` static-limit reason instead of being
//!    parsed, so no input can abort the process and every refusal carries
//!    the same `unsupported_syntax` disclosure contract as a parser error.
//! 2. Accepted sources are parsed on a dedicated worker thread with a large
//!    explicit stack ([`PARSE_WORKER_STACK_BYTES`]), so the recursive oxc
//!    parser no longer depends on the caller's (often 1 MiB) stack. The
//!    budget stays far below the worker's own measured overflow depth, so
//!    inputs under the budget keep parsing — including deep but legitimate
//!    files that previously aborted the process.

use super::*;

/// Stack size for the dedicated parse worker thread.
///
/// The oxc 0.130 parser is recursive-descent; its recursion depth is bounded
/// only by the thread stack it runs on. The main thread's stack is small and
/// platform-dependent (1 MiB on Windows), where paren nesting around 400
/// already aborts the process (issue #4101). A dedicated worker with an
/// explicit 256 MiB stack lifts the practical overflow depth to the tens of
/// thousands under Windows debug builds (the worst frame sizes observed for
/// this fix; release builds recurse further per stack byte). The reservation
/// is address-space only: the worker touches pages as the parse actually
/// recurses, and at most one worker is alive at a time because the adapter
/// parses workspace files sequentially.
pub(crate) const PARSE_WORKER_STACK_BYTES: usize = 256 * 1024 * 1024;

/// Maximum estimated expression nesting depth accepted for a parse.
///
/// Justification (issue #4101):
///
/// - Abort floor: on the unguarded main-thread path, paren nesting of 350
///   parsed cleanly and 400 aborted on Windows debug (1 MiB stack); unary
///   `!` / `await` chains and ternary chains aborted around n≈2000 there.
///   The budget sits above every historically aborting input, so those
///   inputs are now refused with a typed disclosure instead of being parsed
///   on a small stack.
/// - Headroom: the 256 MiB worker still parses nesting far beyond the
///   budget (measured on Windows debug; see the `parse_depth_tests` module),
///   so the budget is a disclosed safety envelope far below the worker's
///   own overflow depth, not the worker's limit.
/// - Real-world code: expression nesting beyond a few dozen levels is
///   dominated by iterative binary/member chains, which do not count toward
///   the estimate; recursive paren/ternary/unary nesting at 2000+ levels is
///   generated/minified pathology, not reviewable source.
pub(crate) const PARSE_NESTING_BUDGET: usize = 2_000;

/// Typed refusal when the worker thread cannot be spawned.
const PARSE_WORKER_UNAVAILABLE: &str =
    "static limit parse_worker_unavailable: TypeScript parse worker thread could not start";

/// Typed refusal when the worker thread died (for example an oxc panic).
const PARSE_WORKER_FAILED: &str = "static limit parse_worker_failed: TypeScript parse worker thread failed; syntax facts unavailable";

pub(crate) fn parse_error_reason(file: &Path, source: &str) -> Option<String> {
    parse_on_worker(file, source, |file, source, allocator| {
        let ret = Parser::new(allocator, source, source_type_for(file)).parse();
        if ret.errors.is_empty() {
            None
        } else {
            // Include the first parser message so the limitation is actionable
            // ("1 parser error(s): Unexpected token") instead of a bare count.
            // oxc diagnostics implement Display as their message text.
            Some(format!(
                "{} parser error(s): {}",
                ret.errors.len(),
                ret.errors[0]
            ))
        }
    })
    .unwrap_or_else(Some)
}

pub(crate) fn parse_limit_for_file<'a>(
    file: &Path,
    limits: &'a [TypeScriptParseLimit],
) -> Option<&'a TypeScriptParseLimit> {
    let changed_file = normalized_path(file);
    limits
        .iter()
        .find(|limit| normalized_path(&limit.file) == changed_file)
}

pub(crate) fn unsupported_syntax_finding(
    file: &Path,
    line: usize,
    line_text: &str,
    limit: &TypeScriptParseLimit,
) -> Finding {
    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let unsup_probe_id = fingerprint_probe_id(
        "probe",
        &id_path,
        "typescript_preview_unsupported_syntax",
        "",
        &normalize_expression(line_text),
        1,
    );
    let probe = Probe {
        id: unsup_probe_id.clone(),
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: None,
        family: ProbeFamily::StaticUnknown,
        delta: DeltaKind::Unknown,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks: Vec::new(),
        required_oracles: Vec::new(),
    };
    let summary = format!(
        "TypeScript preview parser could not build syntax facts for `{}`: {}",
        normalized_path(file),
        limit.reason
    );
    let stage = StageEvidence::new(StageState::Unknown, Confidence::Low, &summary);
    let missing = format!(
        "Static limit `unsupported_syntax`: malformed TypeScript/JavaScript prevented syntax-first owner, test, and probe extraction for `{}`. Repair route: fix or isolate the unsupported syntax before relying on repair guidance.",
        normalized_path(file)
    );
    let why_not_actionable = format!(
        "static limit `unsupported_syntax` prevents bounded TypeScript repair guidance: {}",
        limit.reason
    );
    let repair_route =
        "fix or isolate the unsupported syntax before relying on repair guidance".to_string();
    let recommended = "TypeScript preview advisory: static limit `unsupported_syntax`; Repair route: fix or isolate the unsupported syntax before relying on repair guidance; no actionable repair packet is emitted.".to_string();
    // Resolved from the probe's own delta evidence (#3281) before the probe
    // moves into the finding.
    let source_currentness = crate::domain::SourceCurrentness::from_probe_delta(
        probe.before.as_deref(),
        probe.after.as_deref(),
    );

    Finding {
        id: probe.id.0.clone(),
        canonical_gap: None,
        probe,
        class: ExposureClass::StaticUnknown,
        ripr: RiprEvidence {
            reach: stage.clone(),
            infect: stage.clone(),
            propagate: stage.clone(),
            reveal: RevealEvidence {
                observe: stage.clone(),
                discriminate: stage,
            },
        },
        confidence: 0.2,
        evidence: vec![
            format!("static_limit unsupported_syntax: {}", limit.reason),
            "gap_state: static_limitation".to_string(),
            "actionability_category: unsupported_syntax".to_string(),
            format!("why_not_actionable: {why_not_actionable}"),
            format!("repair_route: {repair_route}"),
            "evidence_needed_to_promote: resolve the named static limit and re-run TypeScript preview evidence extraction".to_string(),
            typescript_raw_evidence_ref(
                file,
                line,
                None,
                &unsup_probe_id.0,
            ),
        ],
        missing: vec![
            missing,
            format!(
                "TypeScript preview actionability `static_limitation` / `unsupported_syntax`: {why_not_actionable}. Repair route: {repair_route}"
            ),
        ],
        flow_sinks: Vec::new(),
        activation: Default::default(),
        stop_reasons: vec![StopReason::StaticProbeUnknown],
        related_tests: Vec::new(),
        recommended_next_step: Some(recommended),
        language: Some(output_language_for(file)),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: None,
        static_limit_kind: Some(StaticLimitKind::UnsupportedSyntax),
        changed_sink: None,
        observed_sink: None,
        oracle_alignment: None,
        alignment_reason: None,
        source_currentness,
    }
}

/// Parse `source` for `file` behind the nesting budget and the large-stack
/// worker thread, then run `analyze` over the parsed program inside the
/// worker. Returns `Err` with a typed static-limit reason when the budget
/// refuses the source or the worker cannot run; `Ok` otherwise.
pub(crate) fn parse_on_worker<T, F>(file: &Path, source: &str, analyze: F) -> Result<T, String>
where
    F: FnOnce(&Path, &str, &Allocator) -> T + Send + 'static,
    T: Send + 'static,
{
    if let Some(estimated) = nesting_over_budget(source) {
        return Err(nesting_budget_refusal_reason(estimated));
    }
    let owned_file = file.to_path_buf();
    let owned_source = source.to_string();
    let spawn = std::thread::Builder::new()
        .name("ripr-ts-oxc-parse".to_string())
        .stack_size(PARSE_WORKER_STACK_BYTES)
        .spawn(move || {
            let allocator = Allocator::default();
            analyze(&owned_file, &owned_source, &allocator)
        });
    match spawn {
        Ok(handle) => handle
            .join()
            .map_err(|_join_error| PARSE_WORKER_FAILED.to_string()),
        Err(err) => Err(format!("{PARSE_WORKER_UNAVAILABLE}: {err}")),
    }
}

/// Typed `expression_nesting_budget` static-limit reason for a source whose
/// estimated nesting depth is over [`PARSE_NESTING_BUDGET`].
fn nesting_budget_refusal_reason(estimated: usize) -> String {
    format!(
        "static limit expression_nesting_budget: estimated expression nesting depth {estimated} exceeds the {PARSE_NESTING_BUDGET}-level parse budget; parse refused to avoid a parser stack overflow"
    )
}

/// `Some(estimated_depth)` when the cheap nesting scan estimates the source
/// over [`PARSE_NESTING_BUDGET`], else `None`.
fn nesting_over_budget(source: &str) -> Option<usize> {
    let estimated = max_expression_nesting_estimate(source);
    (estimated > PARSE_NESTING_BUDGET).then_some(estimated)
}

/// What the last significant identifier/number word was, so regex-vs-division
/// and prefix-operator decisions can see keywords instead of only bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LastWord {
    None,
    /// `await` / `typeof` / `void` / `delete` / `new`.
    PrefixOperator,
    /// Keywords that put a following `/` into expression position
    /// (`return`, `case`, `throw`, ...).
    ExpressionKeyword,
    Operand,
}

/// Estimate the deepest expression nesting in `source` with a single
/// left-to-right byte scan (issue #4101).
///
/// The scan is a lexical approximation, not a parse. It tracks three
/// contributors that all add recursive-descent frames in the oxc parser and
/// returns the maximum of their running sum:
///
/// - bracket depth: `(`, `[`, `{`;
/// - open ternary chain: `?` conditionals not yet closed by their `:`
///   (object-literal / type-annotation colons can pop a ternary early,
///   which under-counts; the bracket terms keep those sources covered);
/// - prefix-operator run: consecutive `!`/`~`, prefix `+`/`-`, and prefix
///   `await`/`typeof`/`void`/`delete`/`new` with no operand between them.
///
/// Long binary/member chains are iterative in the parser and contribute
/// nothing. Line/block comments, string and template literals, and regex
/// literals are skipped so their contents cannot inflate the estimate; the
/// regex-vs-division choice uses the standard expression-position heuristic,
/// and a regex that never terminates on its line falls back to code.
pub(crate) fn max_expression_nesting_estimate(source: &str) -> usize {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Code,
        Template,
        LineComment,
        BlockComment,
        Str(u8),
        Regex { in_class: bool },
    }

    fn is_word_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
    }

    let mut mode = Mode::Code;
    // Brace depth recorded when each template interpolation `${` opened; a
    // `}` that brings the depth back to the recorded value closes it.
    let mut interpolations: Vec<usize> = Vec::new();
    let mut depth: usize = 0;
    let mut ternary_open: usize = 0;
    let mut unary_run: usize = 0;
    let mut max_estimate: usize = 0;
    let mut last_word = LastWord::None;
    // Last significant byte inside code, for the position heuristics.
    // `None` means start of input.
    let mut prev: Option<u8> = None;

    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        match mode {
            Mode::LineComment => {
                if byte == b'\n' {
                    mode = Mode::Code;
                }
            }
            Mode::BlockComment => {
                if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    index += 1;
                    mode = Mode::Code;
                }
            }
            Mode::Str(quote) => {
                if byte == b'\\' {
                    index += 1;
                } else if byte == quote {
                    mode = Mode::Code;
                    last_word = LastWord::Operand;
                    prev = Some(quote);
                }
            }
            Mode::Template => match byte {
                b'\\' => index += 1,
                b'`' => {
                    mode = Mode::Code;
                    last_word = LastWord::Operand;
                    prev = Some(b'`');
                }
                b'$' if bytes.get(index + 1) == Some(&b'{') => {
                    index += 1;
                    interpolations.push(depth);
                    mode = Mode::Code;
                    last_word = LastWord::None;
                    prev = Some(b'{');
                }
                _ => {}
            },
            Mode::Regex { in_class } => match byte {
                b'\\' => index += 1,
                b'[' if !in_class => mode = Mode::Regex { in_class: true },
                b']' => mode = Mode::Regex { in_class: false },
                b'\n' => {
                    // A regex cannot span lines: the heuristic misjudged a
                    // division, so fall back to code scanning.
                    mode = Mode::Code;
                    last_word = LastWord::Operand;
                    prev = Some(b'\n');
                }
                b'/' if !in_class => {
                    mode = Mode::Code;
                    last_word = LastWord::Operand;
                    prev = Some(b'/');
                }
                _ => {}
            },
            Mode::Code => {
                if byte.is_ascii_whitespace() {
                    index += 1;
                    continue;
                }
                if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
                    mode = Mode::LineComment;
                    index += 2;
                    continue;
                }
                if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
                    mode = Mode::BlockComment;
                    index += 2;
                    continue;
                }
                if is_word_byte(byte) {
                    let mut end = index;
                    while end < bytes.len() && is_word_byte(bytes[end]) {
                        end += 1;
                    }
                    let word = &source[index..end];
                    last_word = if byte.is_ascii_digit() {
                        LastWord::Operand
                    } else if prev != Some(b'.')
                        && matches!(word, "await" | "typeof" | "void" | "delete" | "new")
                    {
                        LastWord::PrefixOperator
                    } else if matches!(
                        word,
                        "return"
                            | "case"
                            | "throw"
                            | "in"
                            | "of"
                            | "instanceof"
                            | "do"
                            | "else"
                            | "yield"
                    ) {
                        LastWord::ExpressionKeyword
                    } else {
                        LastWord::Operand
                    };
                    if last_word == LastWord::PrefixOperator && expression_position(prev, last_word)
                    {
                        unary_run = unary_run.saturating_add(1);
                    } else {
                        unary_run = 0;
                    }
                    prev = bytes.get(end.wrapping_sub(1)).copied();
                    index = end;
                    max_estimate = max_estimate
                        .max(depth.saturating_add(ternary_open).saturating_add(unary_run));
                    continue;
                }
                match byte {
                    b'\'' | b'"' => {
                        mode = Mode::Str(byte);
                        unary_run = 0;
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                    b'`' => {
                        mode = Mode::Template;
                        unary_run = 0;
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                    b'/' if expression_position(prev, last_word) => {
                        mode = Mode::Regex { in_class: false };
                        last_word = LastWord::None;
                        // Regex contents are skipped; the literal becomes
                        // an operand when it closes.
                    }
                    b'/' => {
                        unary_run = 0;
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                    b'(' | b'[' | b'{' => {
                        depth = depth.saturating_add(1);
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                    b')' | b']' => {
                        depth = depth.saturating_sub(1);
                        unary_run = 0;
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                    b'}' => {
                        if interpolations.last() == Some(&depth) {
                            interpolations.pop();
                            mode = Mode::Template;
                            // The interpolation continues template text: this
                            // brace is template syntax, was never counted,
                            // and closes no code bracket.
                        } else {
                            depth = depth.saturating_sub(1);
                            unary_run = 0;
                            prev = Some(byte);
                        }
                        last_word = LastWord::None;
                    }
                    b'?' => match bytes.get(index + 1) {
                        Some(b'.') | Some(b'?') | Some(b'=') => {
                            index += 1;
                            unary_run = 0;
                            last_word = LastWord::None;
                            prev = Some(b'?');
                        }
                        _ => {
                            ternary_open = ternary_open.saturating_add(1);
                            last_word = LastWord::None;
                            prev = Some(byte);
                        }
                    },
                    b':' => {
                        ternary_open = ternary_open.saturating_sub(1);
                        unary_run = 0;
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                    b'!' | b'~' => {
                        unary_run = unary_run.saturating_add(1);
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                    b'+' | b'-' => {
                        if expression_position(prev, last_word) {
                            unary_run = unary_run.saturating_add(1);
                        } else {
                            unary_run = 0;
                        }
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                    _ => {
                        unary_run = 0;
                        last_word = LastWord::None;
                        prev = Some(byte);
                    }
                }
                max_estimate =
                    max_estimate.max(depth.saturating_add(ternary_open).saturating_add(unary_run));
            }
        }
        index += 1;
    }
    max_estimate
}

/// Whether the position after `prev`/`last_word` starts an expression: a `/`
/// there begins a regex literal and a `+`/`-`/prefix keyword there is a
/// unary operator.
fn expression_position(prev: Option<u8>, last_word: LastWord) -> bool {
    match last_word {
        LastWord::PrefixOperator | LastWord::ExpressionKeyword => return true,
        LastWord::Operand => return false,
        LastWord::None => {}
    }
    match prev {
        // Start of input is an expression position.
        None => true,
        // After an operand or a closer, a `/` divides and `+`/`-` are binary.
        Some(b')') | Some(b']') | Some(b'}') => false,
        Some(b'"') | Some(b'\'') | Some(b'`') => false,
        Some(b'/') => false,
        Some(byte) if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' => false,
        // Operators, openers, and punctuation are expression positions.
        Some(_) => true,
    }
}

#[cfg(test)]
mod parse_depth_tests {
    use super::*;

    /// Wrap `inner` in `depth` balanced parentheses.
    fn nested(depth: usize, inner: &str) -> String {
        let mut source = String::new();
        source.extend(std::iter::repeat_n('(', depth));
        source.push_str(inner);
        source.extend(std::iter::repeat_n(')', depth));
        source
    }

    #[test]
    fn nesting_estimate_counts_recursive_expression_shapes() {
        assert_eq!(max_expression_nesting_estimate("((((a))))"), 4);
        assert_eq!(max_expression_nesting_estimate("[[[{a}]]]"), 4);
        // Nested ternaries: each `?` opens a chain link, each `:` closes one.
        assert_eq!(max_expression_nesting_estimate("a ? b ? c : d : e"), 2);
        // Prefix operator runs.
        assert_eq!(max_expression_nesting_estimate("!!!!value"), 4);
        assert_eq!(max_expression_nesting_estimate("- - -value"), 3);
        assert_eq!(
            max_expression_nesting_estimate("await await await value"),
            3
        );
        // Combined contributors sum while they nest together.
        assert_eq!(max_expression_nesting_estimate("!!(answer ? yes : no)"), 3);
    }

    #[test]
    fn nesting_estimate_ignores_lexical_shells() {
        // String, comment, template, and regex contents do not nest.
        assert_eq!(
            max_expression_nesting_estimate("const s = ')))((( (( ';"),
            0
        );
        assert_eq!(
            max_expression_nesting_estimate("/* ((( (( */ const a = 1; // ((( "),
            0
        );
        assert_eq!(
            max_expression_nesting_estimate("const t = `a ${f((g))} b )(` ;"),
            2
        );
        assert_eq!(max_expression_nesting_estimate("const r = /a(b)c/ ;"), 0);
        assert_eq!(max_expression_nesting_estimate("return /(((/;"), 0);
        // Division chains are iterative in the parser and count for nothing.
        assert_eq!(max_expression_nesting_estimate("const q = a / b / c;"), 0);
        // Binary/member chains are iterative in the parser and count for nothing.
        assert_eq!(
            max_expression_nesting_estimate("a.b + c.d + e.f + g.h + i.j + k.l;"),
            0
        );
    }

    #[test]
    fn nesting_budget_refuses_over_budget_source_with_typed_reason() {
        // Balanced at budget+1: the oxc worker would parse this cleanly, so
        // any non-`None` reason can only come from the pre-parse budget.
        let source = format!(
            "const over = {}v{};",
            "(".repeat(PARSE_NESTING_BUDGET + 1),
            ")".repeat(PARSE_NESTING_BUDGET + 1)
        );
        let reason = parse_error_reason(Path::new("src/over.ts"), &source);
        let reason = reason.unwrap_or_default();
        assert!(
            reason.contains("expression_nesting_budget"),
            "typed budget refusal expected, got: {reason}"
        );
        assert!(
            !reason.contains("parser error"),
            "budget refusal must not masquerade as a parser error: {reason}"
        );
    }

    #[test]
    fn nesting_budget_admits_deep_legitimate_source_on_worker_stack() {
        // Exactly at budget: admitted, and the large-stack worker parses it
        // cleanly (None) — the parse previously ran on the caller's stack,
        // where far shallower nesting already aborted (issue #4101).
        let source = nested(PARSE_NESTING_BUDGET, "value");
        assert_eq!(
            max_expression_nesting_estimate(&source),
            PARSE_NESTING_BUDGET
        );
        let reason = parse_error_reason(Path::new("src/deep.ts"), &source);
        assert_eq!(reason, None);
    }

    #[test]
    fn previously_aborting_depth_now_parses_and_classifies() {
        // Depth 400 aborted the unguarded Windows-debug main-thread parse
        // (issue #4101 red witness). It must now parse cleanly.
        let source = format!(
            "export function deep(v: number): number {{\n  const r = {};\n  return r;\n}}\n",
            nested(400, "v")
        );
        let reason = parse_error_reason(Path::new("src/deep.ts"), &source);
        assert_eq!(reason, None);
        let owners = extract_owners(Path::new("src/deep.ts"), &source);
        assert_eq!(owners.len(), 1);
        assert_eq!(
            owners.first().map(|owner| owner.name.as_str()),
            Some("deep")
        );
    }

    #[test]
    fn unsupported_syntax_finding_carries_budget_reason() {
        let reason = nesting_budget_refusal_reason(PARSE_NESTING_BUDGET + 1);
        let limit = TypeScriptParseLimit {
            file: PathBuf::from("src/over.ts"),
            reason: reason.clone(),
        };
        let finding =
            unsupported_syntax_finding(Path::new("src/over.ts"), 2, "  const over = ...;", &limit);
        assert!(matches!(finding.class, ExposureClass::StaticUnknown));
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::UnsupportedSyntax)
        );
        let joined = finding.evidence.join("\n");
        assert!(
            joined.contains(
                "static_limit unsupported_syntax: static limit expression_nesting_budget"
            ),
            "budget reason must ride the unsupported_syntax evidence line: {joined}"
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|line| line.contains(reason.as_str()))
        );
    }
}
