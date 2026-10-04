//! The value semantics the generated workflow's summary step used to get
//! from `jq -r ... 2>/dev/null || echo FALLBACK` and its shell helpers.
//!
//! The step summary read each artifact field through a jq query whose
//! failure printed a fallback word. Readers of existing summaries learned
//! those words (`unknown`, `0`, `none`), so this module keeps them: a
//! missing or malformed artifact, or a field whose type breaks the query,
//! still renders the same fallback rather than a new error shape. Only the
//! jq behaviours those queries depend on are modelled here.

use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::fs;
use std::path::Path;

/// A query that failed the way a jq type error does; the caller renders its
/// fallback word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Fail;

pub(super) type Q = Result<Value, Fail>;

/// One artifact as jq read it: the JSON values it parsed, in order, and
/// whether it stopped on unreadable or malformed input.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Doc {
    pub(super) values: Vec<Value>,
    pub(super) broken: bool,
}

impl Doc {
    pub(super) fn read(path: &Path) -> Self {
        match fs::read(path) {
            Ok(bytes) => Self::parse(&bytes),
            Err(_) => Self {
                values: Vec::new(),
                broken: true,
            },
        }
    }

    pub(super) fn parse(bytes: &[u8]) -> Self {
        let mut values = Vec::new();
        let mut broken = false;
        for value in serde_json::Deserializer::from_slice(bytes).into_iter::<Value>() {
            match value {
                Ok(value) => values.push(value),
                Err(_) => {
                    broken = true;
                    break;
                }
            }
        }
        Self { values, broken }
    }

    /// `$(jq -r QUERY file 2>/dev/null || echo FAIL)`, captured. jq runs
    /// the query on every value and exits nonzero when the last one failed
    /// or the input was malformed; then `FAIL` follows whatever it printed.
    /// An empty `fail` is `|| true`, which prints nothing.
    pub(super) fn text(&self, query: impl Fn(&Value) -> Q, fail: &str) -> String {
        let mut printed = String::new();
        let mut failed = false;
        for value in &self.values {
            match query(value) {
                Ok(result) => {
                    printed.push_str(&raw(&result));
                    printed.push('\n');
                    failed = false;
                }
                Err(Fail) => failed = true,
            }
        }
        if (failed || self.broken) && !fail.is_empty() {
            printed.push_str(fail);
        }
        captured(&printed)
    }

    /// `.a.b // .c // "default"`, failing to `fail`.
    pub(super) fn field(&self, paths: &[&str], default: &str, fail: &str) -> String {
        self.text(|value| alt(value, paths, Value::from(default)), fail)
    }

    /// `.a.b // empty`: empty when every path is null or false.
    pub(super) fn optional(&self, paths: &[&str]) -> String {
        self.text(|value| alt(value, paths, Value::from("")), "")
    }

    /// `(.path // [] | length)`, failing to `0`.
    pub(super) fn length(&self, path: &str) -> String {
        self.text(
            |value| length(&alt(value, &[path], Value::Array(Vec::new()))?),
            "0",
        )
    }
}

/// Bash command substitution drops every trailing newline.
pub(super) fn captured(text: &str) -> String {
    text.trim_end_matches('\n').to_string()
}

/// The workflow's `markdown_inline`: one line, with backticks escaped so
/// the value cannot close the surrounding code span.
pub(super) fn inline(text: &str) -> String {
    text.replace(['\r', '\n'], " ").replace('`', "\\`")
}

/// `jq -r` output for one value.
pub(super) fn raw(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_default()
        }
        other => other.to_string(),
    }
}

/// String interpolation and `tostring`: strings raw, anything else as
/// compact JSON.
pub(super) fn interpolated(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// `.key` on one value: null passes through, a non-object is a type error.
pub(super) fn index<'a>(value: &'a Value, key: &str) -> Result<&'a Value, Fail> {
    match value {
        Value::Null => Ok(&Value::Null),
        Value::Object(map) => Ok(map.get(key).unwrap_or(&Value::Null)),
        _ => Err(Fail),
    }
}

/// `.a.b.c` for a dotted path.
pub(super) fn at<'a>(value: &'a Value, path: &str) -> Result<&'a Value, Fail> {
    path.split('.').try_fold(value, index)
}

/// jq truthiness: everything except `null` and `false`.
pub(super) fn truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

/// `.p1 // .p2 // default`. An error on the left of `//` is not
/// suppressed in jq, so it fails the whole query.
pub(super) fn alt(value: &Value, paths: &[&str], default: Value) -> Q {
    for path in paths {
        let found = at(value, path)?;
        if truthy(found) {
            return Ok(found.clone());
        }
    }
    Ok(default)
}

/// jq `length`.
pub(super) fn length(value: &Value) -> Q {
    match value {
        Value::Null => Ok(Value::from(0)),
        Value::Bool(_) => Err(Fail),
        Value::Number(number) => Ok(number
            .as_i64()
            .map(|n| Value::from(n.unsigned_abs()))
            .or_else(|| number.as_u64().map(Value::from))
            .or_else(|| number.as_f64().map(|n| Value::from(n.abs())))
            .unwrap_or(Value::Null)),
        Value::String(text) => Ok(Value::from(text.chars().count())),
        Value::Array(items) => Ok(Value::from(items.len())),
        Value::Object(map) => Ok(Value::from(map.len())),
    }
}

/// jq `+`: null is the identity, strings concatenate, numbers add.
pub(super) fn add(left: Value, right: Value) -> Q {
    match (left, right) {
        (Value::Null, other) | (other, Value::Null) => Ok(other),
        (Value::String(mut left), Value::String(right)) => {
            left.push_str(&right);
            Ok(Value::String(left))
        }
        (Value::Number(left), Value::Number(right)) => Ok(left
            .as_i64()
            .zip(right.as_i64())
            .and_then(|(left, right)| left.checked_add(right))
            .map(Value::from)
            .unwrap_or_else(|| {
                Value::from(left.as_f64().unwrap_or_default() + right.as_f64().unwrap_or_default())
            })),
        (Value::Array(mut left), Value::Array(right)) => {
            left.extend(right);
            Ok(Value::Array(left))
        }
        (Value::Object(mut left), Value::Object(right)) => {
            left.extend(right);
            Ok(Value::Object(left))
        }
        _ => Err(Fail),
    }
}

/// `.[]?`: an array's items or an object's values; nothing otherwise.
pub(super) fn each(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(map) => map.values().collect(),
        _ => Vec::new(),
    }
}

/// `.key[]?`: the `?` covers the iteration, not the index.
pub(super) fn each_at<'a>(value: &'a Value, key: &str) -> Result<Vec<&'a Value>, Fail> {
    index(value, key).map(each)
}

/// `"(.path // "unknown") + (if .line then ":" + (.line|tostring) else "" end)"`,
/// the location shape several at-a-glance lines share.
pub(super) fn location(value: &Value, path_key: &str, unknown: &str) -> Q {
    let path = alt(value, &[path_key], Value::from(unknown))?;
    let line = index(value, "line")?;
    let suffix = if truthy(line) {
        Value::from(format!(":{}", interpolated(line)))
    } else {
        Value::from("")
    };
    add(path, suffix)
}

/// jq `join(sep)` over an array.
pub(super) fn join(items: &[Value], separator: &str) -> Result<String, Fail> {
    let mut joined = String::new();
    for (position, item) in items.iter().enumerate() {
        if position > 0 {
            joined.push_str(separator);
        }
        match item {
            Value::Null => {}
            Value::String(text) => joined.push_str(text),
            Value::Bool(_) | Value::Number(_) => joined.push_str(&item.to_string()),
            Value::Array(_) | Value::Object(_) => return Err(Fail),
        }
    }
    Ok(joined)
}

/// `if length == 0 then "none" else join(", ") end`.
pub(super) fn join_or_none(items: &[Value]) -> Q {
    if items.is_empty() {
        Ok(Value::from("none"))
    } else {
        join(items, ", ").map(Value::from)
    }
}

/// jq `sort`/`unique` ordering: null < false < true < numbers < strings <
/// arrays < objects.
pub(super) fn compare(left: &Value, right: &Value) -> Ordering {
    fn rank(value: &Value) -> u8 {
        match value {
            Value::Null => 0,
            Value::Bool(false) => 1,
            Value::Bool(true) => 2,
            Value::Number(_) => 3,
            Value::String(_) => 4,
            Value::Array(_) => 5,
            Value::Object(_) => 6,
        }
    }
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left
            .as_f64()
            .unwrap_or_default()
            .total_cmp(&right.as_f64().unwrap_or_default()),
        (Value::String(left), Value::String(right)) => left.cmp(right),
        (Value::Array(left), Value::Array(right)) => left
            .iter()
            .zip(right)
            .map(|(left, right)| compare(left, right))
            .find(|ordering| ordering.is_ne())
            .unwrap_or_else(|| left.len().cmp(&right.len())),
        (Value::Object(_), Value::Object(_)) => left.to_string().cmp(&right.to_string()),
        _ => rank(left).cmp(&rank(right)),
    }
}

/// `sort | group_by(.) | map("\(.[0])=\(length)") | if length == 0 then
/// "none" else join(", ") end`.
pub(super) fn tally(mut values: Vec<Value>) -> Value {
    values.sort_by(compare);
    let mut groups: Vec<(Value, usize)> = Vec::new();
    for value in values {
        match groups.last_mut() {
            Some((last, count)) if compare(last, &value).is_eq() => *count += 1,
            _ => groups.push((value, 1)),
        }
    }
    if groups.is_empty() {
        return Value::from("none");
    }
    Value::from(
        groups
            .iter()
            .map(|(value, count)| format!("{}={count}", interpolated(value)))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// `unique`.
pub(super) fn unique(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by(compare);
    values.dedup_by(|right, left| compare(left, right).is_eq());
    values
}

/// `.. | objects`, in document order.
pub(super) fn objects<'a>(value: &'a Value, found: &mut Vec<&'a Map<String, Value>>) {
    match value {
        Value::Object(map) => {
            found.push(map);
            for child in map.values() {
                objects(child, found);
            }
        }
        Value::Array(items) => {
            for child in items {
                objects(child, found);
            }
        }
        _ => {}
    }
}

/// The workflow's `repo_relative`: the checkout path, where it is a whole
/// path token, becomes the repository root `.`. The physical root is
/// rewritten first, then the logical one, line by line.
#[derive(Debug, Clone, Default)]
pub(super) struct RepoRelative {
    pub(super) physical: Vec<u8>,
    pub(super) logical: Vec<u8>,
}

impl RepoRelative {
    /// `printf '%s\n' "$value" | repo_relative`, captured.
    pub(super) fn value(&self, text: &str) -> String {
        let lines = text
            .split('\n')
            .map(|line| String::from_utf8_lossy(&self.line(line.as_bytes())).into_owned())
            .collect::<Vec<_>>();
        captured(&lines.join("\n"))
    }

    /// `repo_relative < file`: every input line, newline-terminated.
    pub(super) fn file(&self, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len());
        let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        if bytes.is_empty() {
            return out;
        }
        for line in body.split(|byte| *byte == b'\n') {
            out.extend(self.line(line));
            out.push(b'\n');
        }
        out
    }

    fn line(&self, line: &[u8]) -> Vec<u8> {
        rewrite(&rewrite(line, &self.physical), &self.logical)
    }
}

fn rewrite(text: &[u8], root: &[u8]) -> Vec<u8> {
    if root.is_empty() {
        return text.to_vec();
    }
    let mut out = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = find(rest, root) {
        let (pre, tail) = rest.split_at(start);
        let after_root = tail.get(root.len()..).unwrap_or_default();
        let before_ok = pre.last().is_none_or(|byte| b" '\"`=(".contains(byte));
        let after_ok = after_root
            .first()
            .is_none_or(|byte| b"/ '\"`):".contains(byte));
        out.extend_from_slice(pre);
        if before_ok && after_ok {
            out.push(b'.');
        } else {
            out.extend_from_slice(root);
        }
        rest = after_root;
    }
    out.extend_from_slice(rest);
    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
