//! Every command ripr prints, pasted into every shell a user has, from the
//! wrong directory, against a repository whose names are hostile.
//!
//! Earlier work pinned single surfaces: first-pr preflight recovery (#5188),
//! the PowerShell translation (#5232), the agent card and repair routes
//! (#5247), and the first-action and proof commands (#5269). Each fix found the
//! next surface by hand. This harness removes the hand: it runs the main ripr
//! flows against one hostile fixture, lifts every printed command out of the
//! output (text, Markdown and JSON, including the artifacts ripr writes),
//! and pastes each into bash, sh, zsh, PowerShell 7 and Windows PowerShell as
//! the platform provides them.
//!
//! The `ripr` and `git` a shell finds are argv recorders, so the observation is
//! what the shell passed, not what an analysis did. A command fails the
//! harness when it
//!
//! - does not reach the program exactly once (a parse error, or a split that
//!   runs a tail as a second command),
//! - passes different argv in different shells,
//! - passes an argument carrying a fragment of a hostile name that is not the
//!   whole path or finding id (a split argument),
//! - names no `--root`, or one that resolves elsewhere than the fixture, when
//!   pasted from a foreign directory (the wrong repository), or
//! - leaves a canary file or any file in the foreign directory (an injected
//!   command ran, or a redirect landed beside the shell).
//!
//! What is a command: a line or Markdown code span that starts with `ripr <known
//! subcommand>` or `git <subcommand>` and has arguments, plus every JSON string
//! of that shape. A bare mention such as `ripr pilot` is prose, not a paste
//! target. A line labelled `(PowerShell)` is the PowerShell form of the command
//! before it; a command with no such line is pasted unchanged into PowerShell,
//! which is the contract `COMMAND_SHELL_DISCLOSURE` prints. Commands in
//! Markdown and plain text are pasted from a foreign directory; JSON-carried
//! commands are portable records meant to run at the repository root.
//!
//! Shells absent from a machine are skipped with a notice. `RIPR_PASTE_REQUIRE`
//! lists the shells whose absence fails; the paste workflow sets it per OS.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "common/fixture_git.rs"]
mod fixture_git;
#[path = "common/paste_shell.rs"]
mod paste_shell;

use fixture_git::fixture_git_ok;
use paste_shell::{Case, Outcome, Shell};

const MARK_HEAD: &str = "ZQ";
const MARK_TAIL: &str = "QZ";
const CANARIES: [&str; 3] = ["ROOTCANARY", "FILECANARY", MARK_TAIL];

/// Where a command is pasted from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Origin {
    /// Human-facing text: pasted from any directory.
    Foreign,
    /// A JSON record: pasted with the repository root as the working directory.
    Portable,
}

#[derive(Clone, Debug)]
struct Printed {
    source: String,
    origin: Origin,
    bash: String,
    powershell: Option<String>,
    /// Carried in a JSON string: pasted into POSIX shells only.
    bash_only: bool,
}

fn hostile_root_name() -> String {
    format!(
        "{MARK_HEAD} it's \u{2018}q\u{2019} `bt` $HOME \u{fc}n\u{ef} \u{65e5}\u{672c} ;touch ROOTCANARY {MARK_TAIL}"
    )
}

fn hostile_file_name() -> String {
    format!("{MARK_HEAD} it's;x y $(touch FILECANARY) {MARK_TAIL}.rs")
}

struct Fixture {
    base: PathBuf,
    root: PathBuf,
    /// Where pasted commands run from. It must stay empty.
    foreign: PathBuf,
    /// Where the flows that print the commands run from. Some flows write
    /// default outputs beside the working directory, which is not what this
    /// harness judges, so it is kept apart from `foreign`.
    collection: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if std::env::var_os("RIPR_PASTE_KEEP").is_some() {
            eprintln!("kept fixture at {}", self.base.display());
            return;
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn build_fixture() -> Result<Fixture, String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let base = std::env::temp_dir().join(format!("ripr-paste-{}-{nonce}", std::process::id()));
    let root = base.join(hostile_root_name());
    let foreign = base.join("foreign launch");
    let collection = base.join("collection launch");
    for dir in [
        root.join("src"),
        root.join("tests"),
        foreign.clone(),
        collection.clone(),
    ] {
        std::fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    }
    let fixture = Fixture {
        base,
        root,
        foreign,
        collection,
    };
    let root = &fixture.root;
    let file = hostile_file_name();
    let write = |relative: &str, text: &str| {
        std::fs::write(root.join(relative), text).map_err(|err| format!("write {relative}: {err}"))
    };
    write(
        "Cargo.toml",
        "[package]\nname = \"hostile\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write(
        "src/lib.rs",
        &format!("#[path = \"{file}\"]\nmod hostile_mod;\npub use hostile_mod::*;\n"),
    )?;
    // `agent repair --phase before` refuses a tree that does not ignore `target/`.
    write(".gitignore", "/target/\n")?;
    write(
        "tests/t.rs",
        "#[test]\nfn price_is_positive() { assert!(hostile::price(5) > 0); }\n",
    )?;
    let source = format!("src/{file}");
    write(
        &source,
        "pub fn price(a: i32) -> i32 {\n    if a > 100 { a - 10 } else { a }\n}\n",
    )?;
    let git =
        |args: &[&str]| fixture_git_ok(root, args).map_err(|err| format!("fixture git: {err}"));
    git(&["init", "-q"])?;
    git(&["config", "core.autocrlf", "false"])?;
    git(&["config", "user.email", "ripr@example.invalid"])?;
    git(&["config", "user.name", "RIPR Test"])?;
    git(&["add", "-A"])?;
    git(&["commit", "-q", "-m", "base"])?;
    write(
        &source,
        "pub fn price(a: i32) -> i32 {\n    if a >= 100 { a - 10 } else { a }\n}\n",
    )?;
    git(&["add", "-A"])?;
    git(&["commit", "-q", "-m", "change"])?;
    for dir in ["reports", "workflow", "pilot", "review", "receipts"] {
        let dir = root.join("target/ripr").join(dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    }
    Ok(fixture)
}

/// The program's stdout and stderr, kept apart so JSON stays parseable.
fn ripr_streams(fixture: &Fixture, args: &[&str]) -> Result<(String, String), String> {
    ripr_streams_in(&fixture.collection, args)
}

fn ripr_streams_in<S: AsRef<std::ffi::OsStr> + std::fmt::Debug>(
    cwd: &Path,
    args: &[S],
) -> Result<(String, String), String> {
    let output = Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(args)
        .current_dir(cwd)
        .env_remove("RIPR_CACHE_DIR")
        .output()
        .map_err(|err| format!("spawn ripr {args:?}: {err}"))?;
    // A refusal still prints commands, so the exit status is not the oracle.
    Ok((
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

fn ripr(fixture: &Fixture, args: &[&str]) -> Result<String, String> {
    let (stdout, stderr) = ripr_streams(fixture, args)?;
    Ok(format!("{stdout}\n{stderr}"))
}

/// Every string value in `value` with the key path leading to it.
fn json_strings<'a>(value: &'a Value, path: &mut Vec<String>, out: &mut Vec<(String, &'a str)>) {
    match value {
        Value::String(text) => out.push((path.join("."), text)),
        Value::Array(items) => {
            for item in items {
                json_strings(item, path, out);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                path.push(key.clone());
                json_strings(item, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

fn find_string(value: &Value, key: &str, accept: impl Fn(&str) -> bool + Copy) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(text)) = map.get(key)
                && accept(text)
            {
                return Some(text.clone());
            }
            map.values().find_map(|item| find_string(item, key, accept))
        }
        Value::Array(items) => items.iter().find_map(|item| find_string(item, key, accept)),
        _ => None,
    }
}

fn collect_strings(value: &Value, key: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(text)) = map.get(key) {
                out.insert(text.clone());
            }
            for item in map.values() {
                collect_strings(item, key, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_strings(item, key, out);
            }
        }
        _ => {}
    }
}

/// The top-level subcommand words `ripr help --all` lists.
fn subcommands(fixture: &Fixture) -> Result<BTreeSet<String>, String> {
    let help = ripr(fixture, &["help", "--all"])?;
    let words: BTreeSet<String> = help
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("ripr "))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter(|word| word.chars().all(|ch| ch.is_ascii_lowercase() || ch == '-'))
        .map(str::to_string)
        .collect();
    if words.len() < 20 || !words.contains("check") || !words.contains("agent") {
        return Err(format!(
            "`ripr help --all` listed too few commands: {words:?}"
        ));
    }
    Ok(words)
}

const GIT_SUBCOMMANDS: [&str; 10] = [
    "fetch",
    "diff",
    "rev-parse",
    "show",
    "log",
    "checkout",
    "add",
    "commit",
    "status",
    "switch",
];

/// Whether `text` is a command worth pasting rather than prose naming one.
fn is_paste_target(text: &str, bare_line: bool, subcommands: &BTreeSet<String>) -> bool {
    let mut words = text.split_whitespace();
    let (Some(program), Some(second)) = (words.next(), words.next()) else {
        return false;
    };
    // `git -C <root> rev-parse ...` puts global options before the verb, so
    // any whole-word verb counts; `ripr` must name its subcommand first.
    let known = match program {
        "ripr" => subcommands.contains(second) && words.next().is_some(),
        "git" => text
            .split_whitespace()
            .skip(1)
            .any(|word| GIT_SUBCOMMANDS.contains(&word)),
        _ => false,
    };
    if !known {
        return false;
    }
    !bare_line || text.contains(" --")
}

/// CommonMark inline code spans on one line: a run of N backticks closes at
/// the next run of exactly N, and one space pad on each side is not content.
fn code_spans(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut spans = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '`' {
            index += 1;
            continue;
        }
        let open_start = index;
        while index < chars.len() && chars[index] == '`' {
            index += 1;
        }
        let width = index - open_start;
        let mut cursor = index;
        let mut close = None;
        while cursor < chars.len() {
            if chars[cursor] != '`' {
                cursor += 1;
                continue;
            }
            let run_start = cursor;
            while cursor < chars.len() && chars[cursor] == '`' {
                cursor += 1;
            }
            if cursor - run_start == width {
                close = Some(run_start);
                break;
            }
        }
        let Some(close) = close else { continue };
        let inner: String = chars[index..close].iter().collect();
        let inner = match (inner.strip_prefix(' '), inner.strip_suffix(' ')) {
            (Some(_), Some(_)) if inner.trim().len() == inner.len().saturating_sub(2) => {
                inner[1..inner.len() - 1].to_string()
            }
            _ => inner,
        };
        spans.push(inner);
        index = close + width;
    }
    spans
}

/// Commands in plain text or Markdown, with each PowerShell form folded into
/// the command before it. A PowerShell form is a line labelled `(PowerShell)`
/// or the line of a fenced `powershell` block.
fn extract_text(
    source: &str,
    text: &str,
    origin: Origin,
    subcommands: &BTreeSet<String>,
) -> Vec<Printed> {
    let mut found: Vec<Printed> = Vec::new();
    let mut fence: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(info) = trimmed.strip_prefix("```") {
            fence = match fence {
                Some(_) => None,
                None => Some(info.trim().to_ascii_lowercase()),
            };
            continue;
        }
        if let Some(language) = &fence {
            let body = line.trim();
            if body.is_empty() {
                continue;
            }
            if matches!(language.as_str(), "powershell" | "pwsh" | "ps1") {
                if let Some(previous) = found.last_mut()
                    && previous.powershell.is_none()
                {
                    previous.powershell = Some(body.to_string());
                }
            } else if matches!(language.as_str(), "bash" | "sh" | "shell" | "")
                && is_paste_target(body, false, subcommands)
            {
                found.push(Printed {
                    source: source.to_string(),
                    origin,
                    bash: body.to_string(),
                    powershell: None,
                    bash_only: false,
                });
            }
            continue;
        }
        let powershell = line.contains("(PowerShell)");
        if powershell {
            // The PowerShell form need not start with `ripr`: the redirecting
            // form is a guarded write that calls it mid-expression.
            let rest = line
                .split_once("(PowerShell)")
                .map(|(_, rest)| rest.trim_start_matches([':', ' ']).trim())
                .unwrap_or_default();
            let form = Some(
                rest.strip_prefix('`')
                    .and_then(|inner| inner.strip_suffix('`'))
                    .unwrap_or(rest),
            );
            if let (Some(form), Some(previous)) = (form, found.last_mut())
                && !form.is_empty()
            {
                previous.powershell = Some(form.to_string());
            }
            continue;
        }
        // Card detail routes are portable by design: they carry no root and
        // are read in the repository the card came from.
        let line_origin = if trimmed.starts_with("detail [") {
            Origin::Portable
        } else {
            origin
        };
        let mut candidates: Vec<String> = Vec::new();
        // Plain-text output wraps a command in single backticks without
        // escaping the ones inside it, so a path holding a backtick closes a
        // CommonMark span early. With one command on the line the decoration is
        // not the command: take everything from the opening backtick to the
        // line's last one.
        let starts = line.matches("`ripr ").count() + line.matches("`git ").count();
        if starts == 1
            && let Some(at) = line.find("`ripr ").or_else(|| line.find("`git "))
            && let Some(end) = line.rfind('`')
            && end > at
        {
            let span = &line[at + 1..end];
            if is_paste_target(span, false, subcommands) {
                candidates.push(span.to_string());
            }
        }
        if candidates.is_empty() {
            candidates.extend(
                code_spans(line)
                    .into_iter()
                    .filter(|span| is_paste_target(span, false, subcommands)),
            );
        }
        if candidates.is_empty() {
            let start = ["ripr ", "git "]
                .iter()
                .filter_map(|program| {
                    trimmed.match_indices(program).map(|(at, _)| at).find(|at| {
                        *at == 0 || trimmed[..*at].ends_with(' ') || trimmed[..*at].ends_with(')')
                    })
                })
                .min();
            if let Some(start) = start {
                let candidate = trimmed[start..].trim_end().to_string();
                if is_paste_target(&candidate, true, subcommands) {
                    candidates.push(candidate);
                }
            }
        }
        for candidate in candidates {
            found.push(Printed {
                source: source.to_string(),
                origin: line_origin,
                bash: candidate,
                powershell: None,
                bash_only: false,
            });
        }
    }
    found
}

fn extract_json(source: &str, value: &Value, subcommands: &BTreeSet<String>) -> Vec<Printed> {
    let mut strings = Vec::new();
    json_strings(value, &mut Vec::new(), &mut strings);
    strings
        .into_iter()
        // The provenance record of how an artifact was produced, not a hint.
        .filter(|(path, _)| !path.ends_with("analysis.command"))
        .filter(|(_, text)| is_paste_target(text.trim(), false, subcommands))
        .map(|(_, text)| Printed {
            source: source.to_string(),
            origin: Origin::Portable,
            bash: text.trim().to_string(),
            powershell: None,
            // A JSON field holds one string and names no shell, so the record
            // is Bash by contract and has no PowerShell form to paste.
            bash_only: true,
        })
        .collect()
}

fn artifact_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "cache") {
                    continue;
                }
                walk(&path, out);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "json" || extension == "md")
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("target/ripr"), &mut out);
    out.sort();
    out
}

struct Sink<'a> {
    printed: Vec<Printed>,
    known_ids: BTreeSet<String>,
    per_source: BTreeMap<String, usize>,
    subcommands: &'a BTreeSet<String>,
}

impl Sink<'_> {
    /// File one flow's output: JSON is walked for command strings and finding
    /// ids, anything else is read as text or Markdown.
    fn ingest(&mut self, source: &str, text: &str, stderr: &str) {
        let found = match serde_json::from_str::<Value>(text.trim()) {
            Ok(value) => {
                collect_strings(&value, "id", &mut self.known_ids);
                extract_json(source, &value, self.subcommands)
            }
            Err(_) => extract_text(
                source,
                &format!("{text}\n{stderr}"),
                Origin::Foreign,
                self.subcommands,
            ),
        };
        *self.per_source.entry(source.to_string()).or_default() += found.len();
        self.printed.extend(found);
    }
}

struct Collected {
    printed: Vec<Printed>,
    known_ids: BTreeSet<String>,
    per_source: BTreeMap<String, usize>,
}

/// Flows a user or agent walks, with the fewest commands each must surface.
/// The minimum guards the extractor: a flow that stops printing commands, or an
/// extractor that stops seeing them, fails instead of passing on nothing.
fn collect(fixture: &Fixture, subcommands: &BTreeSet<String>) -> Result<Collected, String> {
    let root = fixture.root.to_string_lossy().into_owned();
    let root = root.as_str();
    let mut sink = Sink {
        printed: Vec::new(),
        known_ids: BTreeSet::new(),
        per_source: BTreeMap::new(),
        subcommands,
    };
    let (check_json, _) = ripr_streams(
        fixture,
        &[
            "check", "--root", root, "--base", "HEAD~1", "--format", "json",
        ],
    )?;
    let check_value: Value = serde_json::from_str(check_json.trim()).map_err(|err| {
        format!("`ripr check --format json` did not print JSON: {err}\n{check_json}")
    })?;
    let finding_id = find_string(&check_value, "id", |id| id.starts_with("probe:"))
        .ok_or_else(|| format!("the fixture produced no finding id\n{check_json}"))?;
    let (snapshot, _) = ripr_streams(
        fixture,
        &[
            "check",
            "--root",
            root,
            "--mode",
            "instant",
            "--format",
            "repo-exposure-json",
        ],
    )?;
    let snapshot_value: Value = serde_json::from_str(snapshot.trim())
        .map_err(|err| format!("repo-exposure-json did not print JSON: {err}"))?;
    let seam_id = find_string(&snapshot_value, "seam_id", |_| true)
        .ok_or_else(|| "the fixture produced no seam id".to_string())?;
    let head = git_output(fixture, &["rev-parse", "HEAD"])?;
    let base_sha = git_output(fixture, &["rev-parse", "HEAD~1"])?;

    // The same flows twice: once with the absolute root a script passes, and
    // once with the relative root a person types from the directory above it.
    // A relative root is where a printed command most easily binds the wrong
    // repository, because every default is then resolved against a directory
    // the printed command no longer shares.
    let relative_root = fixture
        .root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| "the fixture root has no name".to_string())?;
    let flows = |root: &str| -> Vec<(&'static str, usize, Vec<String>)> {
        let args = |parts: &[&str]| parts.iter().map(|part| part.to_string()).collect();
        vec![
            (
                "check",
                2,
                args(&["check", "--root", root, "--base", "HEAD~1"]),
            ),
            (
                "check-full",
                2,
                args(&[
                    "check",
                    "--root",
                    root,
                    "--base",
                    "HEAD~1",
                    "--format",
                    "human-full",
                ]),
            ),
            ("doctor", 2, args(&["doctor", "--root", root])),
            ("pilot", 1, args(&["pilot", "--root", root])),
            (
                "first-pr",
                2,
                args(&[
                    "first-pr", "--root", root, "--base", "HEAD~1", "--head", "HEAD",
                ]),
            ),
            (
                "first-pr-missing-head",
                1,
                args(&[
                    "first-pr",
                    "--root",
                    root,
                    "--base",
                    "HEAD~1",
                    "--head",
                    "no-such-head",
                ]),
            ),
            (
                "first-pr-missing-base",
                1,
                args(&[
                    "first-pr",
                    "--root",
                    root,
                    "--base",
                    "no-such-base",
                    "--head",
                    "HEAD",
                ]),
            ),
            (
                "explain",
                1,
                args(&["explain", "--root", root, "--base", "HEAD~1", &finding_id]),
            ),
            (
                "context",
                1,
                args(&[
                    "context",
                    "--root",
                    root,
                    "--base",
                    "HEAD~1",
                    "--at",
                    &finding_id,
                ]),
            ),
            (
                "agent-card",
                2,
                args(&["agent", "card", "--root", root, "--seam-id", &seam_id]),
            ),
            (
                "agent-packet",
                1,
                args(&[
                    "agent",
                    "packet",
                    "--root",
                    root,
                    "--seam-id",
                    &seam_id,
                    "--json",
                ]),
            ),
            (
                "agent-repair-before",
                1,
                args(&[
                    "agent",
                    "repair",
                    "--root",
                    root,
                    "--seam-id",
                    &seam_id,
                    "--phase",
                    "before",
                ]),
            ),
            (
                "agent-status",
                0,
                args(&["agent", "status", "--root", root]),
            ),
            (
                "review-comments",
                0,
                args(&[
                    "review-comments",
                    "--root",
                    root,
                    "--base",
                    &base_sha,
                    "--head",
                    &head,
                ]),
            ),
        ]
    };
    sink.ingest("check-json", &check_json, "");
    sink.ingest("repo-exposure-json", &snapshot, "");
    for (prefix, root_arg, cwd, only) in [
        ("", root.to_string(), &fixture.collection, None),
        (
            "relative-root:",
            relative_root,
            &fixture.base,
            Some(RELATIVE_ROOT_FLOWS),
        ),
    ] {
        // The artifacts the absolute-root flows wrote are read before the
        // relative-root flows add their own, whose roots would be doubled.
        if only.is_some() {
            ingest_artifacts(&mut sink, fixture)?;
        }
        let flows: Vec<_> = flows(&root_arg)
            .into_iter()
            .filter(|(label, _, _)| only.is_none_or(|labels: &[&str]| labels.contains(label)))
            .collect();
        for (label, _, args) in &flows {
            let (stdout, stderr) = ripr_streams_in(cwd, args)?;
            sink.ingest(&format!("{prefix}{label}"), &stdout, &stderr);
        }
        for (label, minimum, _) in &flows {
            let seen = sink
                .per_source
                .get(&format!("{prefix}{label}"))
                .copied()
                .unwrap_or(0);
            if seen < *minimum {
                return Err(format!(
                    "flow `{prefix}{label}` surfaced {seen} printed commands, expected at least {minimum}; the flow stopped printing them or the extractor no longer sees them"
                ));
            }
        }
    }
    sink.known_ids.insert(finding_id);
    Ok(Collected {
        printed: sink.printed,
        known_ids: sink.known_ids,
        per_source: sink.per_source,
    })
}

/// Flows also run with a relative `--root`, as typed from the directory above
/// the repository.
const RELATIVE_ROOT_FLOWS: &[&str] = &["check", "first-pr", "doctor", "pilot", "agent-card"];

fn ingest_artifacts(sink: &mut Sink, fixture: &Fixture) -> Result<(), String> {
    for path in artifact_files(&fixture.root) {
        let relative = path
            .strip_prefix(&fixture.root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("read artifact {}: {err}", path.display()))?;
        sink.ingest(&format!("artifact:{relative}"), &text, "");
    }
    Ok(())
}

fn git_output(fixture: &Fixture, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(&fixture.root)
        .output()
        .map_err(|err| format!("spawn git {args:?}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            paste_shell::describe(&output)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resolve `value` against `cwd` through the longest existing ancestor, so an
/// output path that does not exist yet still resolves.
fn resolve(cwd: &Path, value: &str) -> PathBuf {
    let joined = cwd.join(value);
    let mut existing = joined.as_path();
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            let mut out = canonical;
            for part in tail.iter().rev() {
                if *part == ".." {
                    out.pop();
                } else if *part != "." {
                    out.push(part);
                }
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name);
                existing = parent;
            }
            _ => return joined,
        }
    }
}

/// Rootless by design: cache commands take no `--root` and act on the working
/// directory's own cache.
const ROOTLESS: [&str; 2] = ["cache", "help"];

/// A printed command that is known not to paste correctly yet.
///
/// The ledger is strict in both directions. A problem on a listed command is
/// reported as a known gap instead of failing the run, and a row whose gap no
/// longer reproduces fails the run until the row is deleted, so the ledger
/// cannot outlive the fix it tracks or hide a different failure on the same
/// command.
struct KnownGap {
    /// Substring of the flow or artifact the command was printed by.
    source: &'static str,
    /// Prefix of the command as printed for Bash.
    command: &'static str,
    /// The shells that fail. A PowerShell gap is a command printed with no
    /// PowerShell form that PowerShell cannot parse; the row applies only
    /// while that is the case.
    shells: GapShells,
    /// Prefixes of the problems this gap explains. Any other problem on the
    /// same command is a new failure, so the ledger cannot hide it.
    explains: &'static [&'static str],
    reason: &'static str,
}

/// What a PowerShell parse of a Bash-only command with a hostile root looks
/// like: a parse error, a command that never ran, or a split argument.
const POWERSHELL_SPLIT: &[&str] = &[
    "shell error",
    "reached the program",
    "argument ",
    "--root ",
    "argv differs",
];
/// A drill-in that repeats a typed relative root names another repository.
const WRONG_ROOT: &[&str] = &["--root ", "argument "];

#[derive(Clone, Copy, PartialEq, Eq)]
enum GapShells {
    PowershellWithoutForm,
    Every,
}

const KNOWN_GAPS: &[KnownGap] = &[
    KnownGap {
        source: "start-here.md",
        command: "ripr first-pr --root",
        shells: GapShells::PowershellWithoutForm,
        explains: POWERSHELL_SPLIT,
        reason: "the missing-base recovery sentence embeds a Bash command with no PowerShell form",
    },
    KnownGap {
        source: "relative-root:check",
        command: "ripr explain --root",
        shells: GapShells::Every,
        explains: WRONG_ROOT,
        reason: "the `check` drill-in repeats a typed relative --root, so pasting from another directory targets another repository",
    },
    KnownGap {
        source: "relative-root:check",
        command: "ripr agent stub --root",
        shells: GapShells::Every,
        explains: WRONG_ROOT,
        reason: "the `check` drill-in repeats a typed relative --root, so pasting from another directory targets another repository",
    },
    KnownGap {
        source: "relative-root:check",
        command: "ripr context --root",
        shells: GapShells::Every,
        explains: WRONG_ROOT,
        reason: "the `check` drill-in repeats a typed relative --root, so pasting from another directory targets another repository",
    },
];

fn known_gap(printed: &Printed, shell: Shell) -> Option<usize> {
    KNOWN_GAPS.iter().position(|gap| {
        printed.source.contains(gap.source)
            && printed.bash.starts_with(gap.command)
            && match gap.shells {
                GapShells::PowershellWithoutForm => {
                    shell.is_powershell() && printed.powershell.is_none()
                }
                GapShells::Every => true,
            }
    })
}

fn judge_call(
    printed: &Printed,
    call: &paste_shell::Recorded,
    fixture: &Fixture,
    known_ids: &BTreeSet<String>,
    subcommands: &BTreeSet<String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let root = fixture
        .root
        .canonicalize()
        .unwrap_or_else(|_| fixture.root.clone());
    let cwd = Path::new(&call.cwd);
    if call.program == "ripr" {
        match call.args.first() {
            Some(sub) if subcommands.contains(sub) => {
                let root_value = call
                    .args
                    .iter()
                    .position(|arg| arg == "--root")
                    .and_then(|at| call.args.get(at + 1));
                match root_value {
                    Some(value) => {
                        if resolve(cwd, value) != root {
                            problems.push(format!(
                                "--root {value:?} resolves to {} from {}, not the fixture root",
                                resolve(cwd, value).display(),
                                cwd.display()
                            ));
                        }
                    }
                    // A portable record runs at the root, which is its binding.
                    None if printed.origin == Origin::Portable => {}
                    None if ROOTLESS.contains(&sub.as_str()) => {}
                    None => problems.push(format!(
                        "`ripr {sub}` has no --root, so it acts on {} ({:?} paste)",
                        cwd.display(),
                        printed.origin
                    )),
                }
            }
            other => problems.push(format!("first argument {other:?} is not a ripr subcommand")),
        }
    }
    if call.program == "git"
        && let Some(dir) = call
            .args
            .iter()
            .position(|arg| arg == "-C")
            .and_then(|at| call.args.get(at + 1))
        && resolve(cwd, dir) != root
    {
        problems.push(format!(
            "git -C {dir:?} resolves to {} from {}, not the fixture root",
            resolve(cwd, dir).display(),
            cwd.display()
        ));
    }
    // `ripr agent stub --at FILE:LINE` names a file relative to its own
    // --root (ripr joins the two), not to the shell's directory.
    let stub_location = (call.program == "ripr"
        && call.args.first().map(String::as_str) == Some("agent")
        && call.args.get(1).map(String::as_str) == Some("stub"))
    .then(|| {
        let at = call.args.iter().position(|arg| arg == "--at")?;
        let root_value = call
            .args
            .iter()
            .position(|arg| arg == "--root")
            .and_then(|index| call.args.get(index + 1))?;
        Some((at + 1, resolve(cwd, root_value)))
    })
    .flatten();
    for (index, arg) in call.args.iter().enumerate() {
        if !arg.contains(MARK_HEAD) && !arg.contains(MARK_TAIL) {
            continue;
        }
        if known_ids.contains(arg) {
            continue;
        }
        let path = match &stub_location {
            Some((at, stub_root)) if *at == index => {
                let file = arg
                    .rsplit_once(':')
                    .filter(|(_, line)| {
                        !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit())
                    })
                    .map_or(arg.as_str(), |(file, _)| file);
                resolve(stub_root, file)
            }
            _ => resolve(cwd, arg),
        };
        if path.starts_with(&root) {
            continue;
        }
        problems.push(format!(
            "argument {arg:?} carries part of a hostile name but is neither a path under the root nor a finding id (a split or mis-parsed argument)"
        ));
    }
    problems
}

fn required(shell: Shell) -> bool {
    std::env::var("RIPR_PASTE_REQUIRE")
        .is_ok_and(|list| list.split(',').any(|name| name.trim() == shell.name()))
}

fn canary_hits(fixture: &Fixture) -> Vec<String> {
    let mut hits = Vec::new();
    for (label, dir) in [
        ("foreign directory", &fixture.foreign),
        ("fixture base", &fixture.base),
        ("repository root", &fixture.root),
    ] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if CANARIES.contains(&name.as_str()) {
                hits.push(format!("{label} gained {name:?}"));
            }
            if label == "foreign directory" {
                hits.push(format!("foreign directory gained {name:?}"));
            }
        }
    }
    hits
}

#[test]
fn printed_commands_paste_unchanged_in_every_shell_from_a_foreign_directory() -> Result<(), String>
{
    let fixture = build_fixture()?;
    let subcommands = subcommands(&fixture)?;
    let collected = collect(&fixture, &subcommands)?;

    // One case per distinct printed form per flow. The PowerShell pairing is
    // part of the identity, so a site that prints a command with its
    // PowerShell form cannot hide another that prints it without one.
    let mut unique: BTreeMap<(String, Origin, String, Option<String>), Printed> = BTreeMap::new();
    for printed in &collected.printed {
        unique
            .entry((
                printed.source.clone(),
                printed.origin,
                printed.bash.clone(),
                printed.powershell.clone(),
            ))
            .or_insert_with(|| printed.clone());
    }
    let commands: Vec<Printed> = unique.into_values().collect();
    if commands.len() < 15 {
        return Err(format!(
            "only {} distinct printed commands were collected; per source: {:?}",
            commands.len(),
            collected.per_source
        ));
    }

    // Every collected command with its source and pairing, for the CI artifact
    // and for reading what a flow actually printed.
    if let Some(path) = std::env::var_os("RIPR_PASTE_REPORT") {
        let lines: Vec<String> = commands
            .iter()
            .map(|printed| {
                serde_json::json!({
                    "source": printed.source,
                    "origin": format!("{:?}", printed.origin),
                    "bash": printed.bash,
                    "powershell": printed.powershell,
                })
                .to_string()
            })
            .collect();
        std::fs::write(&path, lines.join("\n"))
            .map_err(|err| format!("write RIPR_PASTE_REPORT: {err}"))?;
    }

    let scratch = fixture.base.join("harness");
    let recorders = scratch.join("recorders");
    paste_shell::install_recorders(&recorders)?;

    let mut problems: Vec<String> = Vec::new();
    let mut known: Vec<String> = Vec::new();
    let mut gap_hits = vec![0usize; KNOWN_GAPS.len()];
    let mut baseline: BTreeMap<usize, (Shell, Vec<String>)> = BTreeMap::new();
    let mut ran: Vec<Shell> = Vec::new();
    for shell in Shell::ALL {
        let Some(executable) = paste_shell::locate(shell, &scratch.join("probe")) else {
            if shell.expected_on_this_platform() {
                if required(shell) {
                    return Err(format!(
                        "{} is required on this platform and was not found; the paste lane must not silently drop a shell",
                        shell.name()
                    ));
                }
                eprintln!("SKIPPED {}: not installed on this machine", shell.name());
            }
            continue;
        };
        ran.push(shell);
        let cases: Vec<Case> = commands
            .iter()
            .enumerate()
            .filter_map(|(id, printed)| {
                if shell.is_powershell() && printed.bash_only {
                    return None;
                }
                let command = if shell.is_powershell() {
                    printed
                        .powershell
                        .clone()
                        .unwrap_or_else(|| printed.bash.clone())
                } else {
                    printed.bash.clone()
                };
                Some(Case {
                    id,
                    command,
                    cwd: match printed.origin {
                        Origin::Foreign => fixture.foreign.clone(),
                        Origin::Portable => fixture.root.clone(),
                    },
                })
            })
            .collect();
        // PowerShell runs a batch in one process, so a canary cannot be traced
        // to its command. Known-gap cases get a batch of their own, and only
        // that batch may leave a canary behind.
        let (gap_cases, other_cases): (Vec<Case>, Vec<Case>) = if shell.is_powershell() {
            cases.into_iter().partition(|case| {
                // Only a command printed with no PowerShell form may run
                // its tail; a wrong-root gap must still leave no canary.
                known_gap(&commands[case.id], shell).is_some_and(|index| {
                    matches!(KNOWN_GAPS[index].shells, GapShells::PowershellWithoutForm)
                })
            })
        } else {
            (Vec::new(), cases)
        };
        for (cases, excused) in [(other_cases, false), (gap_cases, true)] {
            if cases.is_empty() {
                continue;
            }
            let outcomes: Vec<Outcome> =
                paste_shell::run_cases(shell, &executable, &scratch, &recorders, &cases)?;
            for (case, outcome) in cases.iter().zip(&outcomes) {
                let printed = &commands[case.id];
                let label = format!(
                    "[{}] {} <- {}\n    {}",
                    shell.name(),
                    match printed.origin {
                        Origin::Foreign => "foreign",
                        Origin::Portable => "portable",
                    },
                    printed.source,
                    case.command
                );
                let mut case_problems: Vec<String> = Vec::new();
                if let Some(error) = &outcome.shell_error {
                    case_problems.push(format!("shell error: {error}"));
                }
                if outcome.calls.len() != 1 {
                    case_problems.push(format!(
                        "reached the program {} times, expected exactly once: {:?}",
                        outcome.calls.len(),
                        outcome
                            .calls
                            .iter()
                            .map(|call| &call.args)
                            .collect::<Vec<_>>()
                    ));
                } else {
                    let call = &outcome.calls[0];
                    case_problems.extend(judge_call(
                        printed,
                        call,
                        &fixture,
                        &collected.known_ids,
                        &subcommands,
                    ));
                    match baseline.get(&case.id) {
                        Some((first, args)) if args != &call.args => case_problems.push(format!(
                            "argv differs from {}: {:?} vs {:?}",
                            first.name(),
                            call.args,
                            args
                        )),
                        Some(_) => {}
                        None => {
                            baseline.insert(case.id, (shell, call.args.clone()));
                        }
                    }
                }
                if case_problems.is_empty() {
                    continue;
                }
                match known_gap(printed, shell) {
                    Some(index) => {
                        let gap = &KNOWN_GAPS[index];
                        let (explained, unexplained): (Vec<&String>, Vec<&String>) =
                            case_problems.iter().partition(|problem| {
                                gap.explains
                                    .iter()
                                    .any(|prefix| problem.starts_with(prefix))
                            });
                        if !explained.is_empty() {
                            gap_hits[index] += 1;
                            known.push(format!(
                                "{label_line} ({})",
                                gap.reason,
                                label_line = label.lines().next().unwrap_or_default()
                            ));
                        }
                        if !unexplained.is_empty() {
                            let rest: Vec<&str> =
                                unexplained.iter().map(|problem| problem.as_str()).collect();
                            problems.push(format!(
                                "{label}\n    {}\n    (a different failure from the listed gap: {})",
                                rest.join("\n    "),
                                gap.reason
                            ));
                        }
                    }
                    None => problems.push(format!("{label}\n    {}", case_problems.join("\n    "))),
                }
            }
            for hit in canary_hits(&fixture) {
                // A Bash command with no PowerShell form that PowerShell splits at
                // a `;` in the path runs its tail, which is the injection the
                // known PowerShell gaps allow. Those cases run in their own
                // batch, and only that one effect is excused: any other file
                // (FILECANARY, the tail marker, a stray redirect target) is a
                // new failure even when the command also hits the listed gap.
                if !(excused && hit.ends_with("gained \"ROOTCANARY\"")) {
                    problems.push(format!("[{}] {hit}", shell.name()));
                }
                // Clear it so one injection is reported against the shell that ran
                // it, not every shell after.
                for dir in [&fixture.foreign, &fixture.base, &fixture.root] {
                    for name in CANARIES {
                        let _ = std::fs::remove_file(dir.join(name));
                    }
                    if dir == &fixture.foreign
                        && let Ok(entries) = std::fs::read_dir(dir)
                    {
                        for entry in entries.flatten() {
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }
    }
    for (gap, hits) in KNOWN_GAPS.iter().zip(&gap_hits) {
        let applies = match gap.shells {
            GapShells::PowershellWithoutForm => ran.iter().any(|shell| shell.is_powershell()),
            GapShells::Every => true,
        };
        if applies && *hits == 0 {
            problems.push(format!(
                "known gap no longer reproduces; delete its row: {} / `{}` ({})",
                gap.source, gap.command, gap.reason
            ));
        }
    }
    if ran.len() < 2 {
        return Err(format!(
            "only {ran:?} ran; a paste test needs a POSIX shell and a PowerShell on the same machine"
        ));
    }
    for line in &known {
        eprintln!("KNOWN GAP {line}");
    }
    eprintln!(
        "pasted {} distinct printed commands into {:?}",
        commands.len(),
        ran.iter().map(|shell| shell.name()).collect::<Vec<_>>()
    );
    if problems.is_empty() {
        Ok(())
    } else {
        // `Result::Err` is Debug-printed, which escapes every newline and quote
        // in the report. Print the report itself, then fail with its size.
        eprintln!("{}", problems.join("\n\n"));
        Err(format!(
            "{} paste problems across {} commands (report above)",
            problems.len(),
            commands.len()
        ))
    }
}
