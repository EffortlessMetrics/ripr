//! Opt-in visibility for the existing source-owned fixture only.
//! Diagnostics cannot earn coverage, alter deadlines or terminate processes.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) const CHILD_PREFIX: &str = "ripr_covered_by_phase ";
pub(crate) const PARENT_PREFIX: &str = "ripr_covered_by_parent ";
const CLASS_CAP: usize = 8 * 1024;
const RESERVED: usize = 256;
const STAT_CAP: u64 = 4096;
const SPAWN_CAP: usize = 64;
const PROC_CAP: usize = 4096;
const MEMBER_CAP: usize = 16;

pub(crate) fn requested() -> bool {
    std::env::var("RIPR_SOURCE_PROMOTION_PHASE_DIAGNOSTICS").is_ok_and(|value| value == "1")
}
fn scope(request: bool, owned: bool, validation: bool) -> bool {
    request && owned && validation
}
#[derive(Default)]
struct Budget {
    used: [usize; 2],
    exhausted: [bool; 2],
}
impl Budget {
    fn encode(&mut self, prefix: &str, value: &Value, class: usize) -> Option<String> {
        let text = serde_json::to_string(value).ok()?;
        let line = format!("{prefix}{text}\n");
        if self.used[class].saturating_add(line.len()) <= CLASS_CAP - RESERVED {
            self.used[class] += line.len();
            return Some(line);
        }
        if self.exhausted[class] {
            return None;
        }
        self.exhausted[class] = true;
        let line =
            format!("{prefix}{{\"event\":\"diagnostic_budget_exhausted\",\"class\":{class}}}\n");
        if self.used[class].saturating_add(line.len()) <= CLASS_CAP {
            self.used[class] += line.len();
            Some(line)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub(crate) struct PlannedPhase {
    pub(crate) selector: usize,
    pub(crate) line: usize,
    pub(crate) kind: &'static str,
    pub(crate) argv: usize,
}
#[derive(Clone)]
pub(crate) struct DiagnosticPlan {
    host: String,
    argv: Vec<Vec<String>>,
    tokens: Vec<String>,
    indices: Vec<Vec<usize>>,
    steps: Vec<PlannedPhase>,
}
impl DiagnosticPlan {
    pub(crate) fn new(
        host: &str,
        argv: Vec<Vec<String>>,
        steps: Vec<PlannedPhase>,
    ) -> Option<Self> {
        if !matches!(host, "unix" | "windows")
            || steps.is_empty()
            || steps.len() > 39
            || argv.is_empty()
            || argv.len() > 64
        {
            return None;
        }
        let mut tokens = Vec::new();
        let mut indices = Vec::new();
        for args in &argv {
            if args.is_empty()
                || args.len() > 64
                || args.first().map(String::as_str) != Some("test")
            {
                return None;
            }
            let mut row = Vec::new();
            for arg in args {
                if arg.len() > 256 {
                    return None;
                }
                let index = if let Some(index) = tokens.iter().position(|token| token == arg) {
                    index
                } else {
                    if tokens.len() == 64 {
                        return None;
                    }
                    tokens.push(arg.clone());
                    tokens.len() - 1
                };
                row.push(index);
            }
            indices.push(row);
        }
        if steps.iter().any(|step| {
            step.selector == 0
                || step.line == 0
                || step.argv >= argv.len()
                || !matches!(step.kind, "b" | "l" | "d")
        }) {
            return None;
        }
        Some(Self {
            host: host.into(),
            argv,
            tokens,
            indices,
            steps,
        })
    }
    fn matches_phase(
        &self,
        index: usize,
        ordinal: usize,
        line: Option<usize>,
        description: &str,
        args: &[String],
    ) -> bool {
        self.steps.get(index).is_some_and(|step| {
            step.selector == ordinal
                && Some(step.line) == line
                && Some(step.kind) == phase_kind(description)
                && self
                    .argv
                    .get(step.argv)
                    .is_some_and(|expected| expected == args)
        })
    }
    fn context(
        &self,
        pid: u32,
        source: Option<&str>,
        cwd: Option<&str>,
        target: Option<&str>,
    ) -> Value {
        json!([
            "c",
            2,
            pid,
            self.host,
            source,
            cwd,
            target,
            "cargo",
            self.tokens,
            self.indices
        ])
    }
}
fn phase_kind(description: &str) -> Option<&'static str> {
    match description {
        "test-valued covered_by build" => Some("b"),
        "test-valued covered_by enumeration" => Some("l"),
        "test-valued covered_by doc enumeration" => Some("d"),
        _ => None,
    }
}
fn phase_description(kind: &str) -> Option<&'static str> {
    match kind {
        "b" => Some("test-valued covered_by build"),
        "l" => Some("test-valued covered_by enumeration"),
        "d" => Some("test-valued covered_by doc enumeration"),
        _ => None,
    }
}
struct Shared {
    budget: Budget,
    next: usize,
    live: bool,
}
#[derive(Clone)]
pub(crate) struct Observation {
    started: Instant,
    shared: Arc<Mutex<Shared>>,
    plan: Arc<DiagnosticPlan>,
}
#[derive(Clone)]
pub(crate) struct PhaseToken {
    started: Instant,
    shared: Arc<Mutex<Shared>>,
    sequence: usize,
}
#[derive(Clone)]
pub(crate) struct CaptureObservation {
    phase: PhaseToken,
    pid: u32,
    sampled: Arc<Mutex<bool>>,
}
fn emit_compact(shared: &mut Shared, value: &Value, class: usize) -> bool {
    if let Some(line) = shared.budget.encode(CHILD_PREFIX, value, class) {
        let _diagnostic_write = std::io::stderr().lock().write_all(line.as_bytes());
    }
    !shared.budget.exhausted[class]
}
impl Observation {
    pub(crate) fn for_checker(plan: impl FnOnce() -> Option<DiagnosticPlan>) -> Option<Self> {
        let owned =
            std::env::var("RIPR_SOURCE_PROMOTION_PHASE_DIAGNOSTICS_OWNED").is_ok_and(|v| v == "1");
        let validation = std::env::var("RIPR_SOURCE_PROMOTION_VALIDATION").is_ok_and(|v| v == "1");
        if !scope(requested(), owned, validation) {
            return None;
        }
        let plan = Arc::new(plan()?);
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(str::to_string));
        let source = std::env::var("RIPR_SOURCE_PROMOTION_TRUSTED_CHECKER_SHA").ok();
        let target = std::env::var("CARGO_TARGET_DIR").ok();
        let context = plan.context(
            std::process::id(),
            source.as_deref(),
            cwd.as_deref(),
            target.as_deref(),
        );
        if serde_json::to_string(&context).ok()?.len() + CHILD_PREFIX.len() + 1 > 4096 {
            return None;
        }
        let mut shared = Shared {
            budget: Budget::default(),
            next: 0,
            live: true,
        };
        if !emit_compact(&mut shared, &context, 0) {
            return None;
        }
        Some(Self {
            started: Instant::now(),
            shared: Arc::new(Mutex::new(shared)),
            plan,
        })
    }
    pub(crate) fn phase(
        &self,
        ordinal: usize,
        line: Option<usize>,
        description: &str,
        args: &[String],
    ) -> Option<PhaseToken> {
        let Ok(mut shared) = self.shared.lock() else {
            return None;
        };
        let index = shared.next;
        let matched = self
            .plan
            .matches_phase(index, ordinal, line, description, args);
        if !shared.live || !matched {
            if shared.live {
                let _emitted = emit_compact(&mut shared, &json!(["invalid_plan"]), 0);
            }
            shared.live = false;
            return None;
        }
        let step = self.plan.steps.get(index)?;
        let sequence = index + 1;
        let value = json!([
            step.kind,
            sequence,
            ordinal,
            line,
            step.argv,
            self.started.elapsed().as_millis().to_string()
        ]);
        if !emit_compact(&mut shared, &value, 0) {
            shared.live = false;
            return None;
        }
        shared.next = sequence;
        Some(PhaseToken {
            started: self.started,
            shared: Arc::clone(&self.shared),
            sequence,
        })
    }
    pub(crate) fn finished(&self, success: bool) {
        let Ok(mut shared) = self.shared.lock() else {
            return;
        };
        if shared.live {
            let _emitted = emit_compact(
                &mut shared,
                &json!(["f", self.started.elapsed().as_millis().to_string(), success]),
                0,
            );
        }
    }
    #[cfg(test)]
    pub(crate) fn for_test(pid: u32) -> CaptureObservation {
        CaptureObservation {
            phase: PhaseToken {
                started: Instant::now(),
                shared: Arc::new(Mutex::new(Shared {
                    budget: Budget::default(),
                    next: 0,
                    live: true,
                })),
                sequence: 1,
            },
            pid,
            sampled: Arc::new(Mutex::new(false)),
        }
    }
}
impl PhaseToken {
    pub(crate) fn spawn(&self, pid: u32) -> CaptureObservation {
        let observed = read_stat(pid);
        if let Ok(mut shared) = self.shared.lock() {
            let _emitted = emit_compact(
                &mut shared,
                &json!([
                    "s",
                    self.sequence,
                    pid,
                    observed.as_ref().map(|v| v.pgid),
                    observed.as_ref().map(|v| v.start)
                ]),
                0,
            );
        }
        CaptureObservation::new(self.clone(), pid)
    }
}
const SAMPLE_CAP: usize = 196;
fn sample_value(sequence: usize, pid: u32, elapsed: &str, bytes: &[u8]) -> Option<Value> {
    if bytes.is_empty() {
        return None;
    }
    let bounded = &bytes[..bytes.len().min(64)];
    let mut text = String::from_utf8_lossy(bounded).into_owned();
    loop {
        let value = json!([
            "t",
            sequence,
            pid,
            elapsed,
            text,
            bytes.len(),
            bounded.len() < bytes.len() || text.len() < String::from_utf8_lossy(bounded).len()
        ]);
        if serde_json::to_string(&value).ok()?.len() + CHILD_PREFIX.len() < SAMPLE_CAP {
            return Some(value);
        }
        text.pop()?;
    }
}
impl CaptureObservation {
    fn new(phase: PhaseToken, pid: u32) -> Self {
        Self {
            phase,
            pid,
            sampled: Arc::new(Mutex::new(false)),
        }
    }
    fn sample_once(&self, bytes: &[u8], elapsed: &str) -> Option<Value> {
        if bytes.is_empty() {
            return None;
        }
        let Ok(mut sampled) = self.sampled.lock() else {
            return None;
        };
        if *sampled {
            return None;
        }
        *sampled = true;
        sample_value(self.phase.sequence, self.pid, elapsed, bytes)
    }
    pub(crate) fn tail(&self, bytes: &[u8]) {
        let Some(value) =
            self.sample_once(bytes, &self.phase.started.elapsed().as_millis().to_string())
        else {
            return;
        };
        if let Ok(mut shared) = self.phase.shared.lock() {
            let _emitted = emit_compact(&mut shared, &value, 1);
        }
    }
}
pub(crate) struct ParentContext {
    source: String,
    cwd: String,
    target: Option<String>,
    plan: DiagnosticPlan,
}
impl ParentContext {
    pub(crate) fn for_owned_root(root: &std::path::Path, source: &str) -> Option<Self> {
        let canonical = root.canonicalize().ok()?;
        let cwd = canonical.to_str()?.to_string();
        let path = canonical.join("policy/non-rust-allowlist.toml");
        let commands = crate::read_file_policy_test_commands(path.to_str()?).ok()?;
        let host = crate::FilePolicyHost::current().ok()?;
        let plan = super::file_policy::diagnostic_plan(&commands, host)?;
        Some(Self {
            source: source.into(),
            cwd,
            target: std::env::var("CARGO_TARGET_DIR").ok(),
            plan,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    pid: u32,
    pgid: u32,
    start: u64,
    state: char,
}
fn parse_stat(expected: u32, text: &str) -> Option<Identity> {
    let (first, _) = text.split_once(" (")?;
    let pid = first.parse::<u32>().ok()?;
    if pid != expected {
        return None;
    }
    let close = text.rfind(')')?;
    let fields = text
        .get(close + 1..)?
        .split_whitespace()
        .collect::<Vec<_>>();
    let state = fields.first()?.chars().next()?;
    let pgid = fields.get(2)?.parse::<u32>().ok()?;
    let start = fields.get(19)?.parse::<u64>().ok()?;
    if pid == 0 || pgid == 0 || start == 0 {
        return None;
    }
    Some(Identity {
        pid,
        pgid,
        start,
        state,
    })
}
fn read_stat(pid: u32) -> Option<Identity> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let file = File::open(format!("/proc/{pid}/stat")).ok()?;
    let mut bytes = Vec::with_capacity(STAT_CAP as usize + 1);
    file.take(STAT_CAP + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > STAT_CAP as usize {
        return None;
    }
    parse_stat(pid, std::str::from_utf8(&bytes).ok()?)
}
fn spawn_rows(text: &str, checker_pid: Option<u32>) -> (Vec<Identity>, bool) {
    let mut rows = Vec::new();
    let mut complete = true;
    for line in text.lines() {
        let Some(record) = line.strip_prefix(CHILD_PREFIX) else {
            continue;
        };
        if record.len() > 4096 {
            complete = false;
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(record) else {
            complete = false;
            continue;
        };
        if value["event"] == "diagnostic_budget_exhausted" && value["class"] == 0 {
            complete = false;
            continue;
        }
        if value["event"] != "spawn" {
            continue;
        }
        if value["checker_pid"].as_u64() != checker_pid.map(u64::from) {
            complete = false;
            continue;
        }
        let number = |key: &str| value[key].as_u64();
        let parsed = number("pid")
            .and_then(|pid| u32::try_from(pid).ok())
            .zip(number("pgid").and_then(|pgid| u32::try_from(pgid).ok()))
            .zip(number("start_ticks"));
        let Some(((pid, pgid), start)) = parsed else {
            complete = false;
            continue;
        };
        if pid == 0 || pgid == 0 || start == 0 || rows.len() == SPAWN_CAP {
            complete = false;
            continue;
        }
        rows.push(Identity {
            pid,
            pgid,
            start,
            state: '?',
        });
    }
    (rows, complete)
}
// One cooperative bounded scan, not an atomic process snapshot or cleanup proof.
fn group_scan(groups: &BTreeSet<u32>) -> (BTreeMap<u32, Vec<u32>>, bool) {
    let mut members = BTreeMap::<u32, Vec<u32>>::new();
    if !cfg!(target_os = "linux") || groups.is_empty() {
        return (members, false);
    }
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return (members, false);
    };
    let started = Instant::now();
    let mut complete = true;
    for (index, entry) in entries.enumerate() {
        if index >= PROC_CAP || started.elapsed() > Duration::from_millis(100) {
            complete = false;
            break;
        }
        let Ok(entry) = entry else {
            complete = false;
            continue;
        };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(identity) = read_stat(pid) else {
            // A vanished/unreadable entry prevents a complete-snapshot claim.
            complete = false;
            continue;
        };
        if matches!(identity.state, 'Z' | 'X') || !groups.contains(&identity.pgid) {
            continue;
        }
        let group = members.entry(identity.pgid).or_default();
        if group.len() == MEMBER_CAP {
            complete = false;
            continue;
        }
        group.push(identity.pid);
    }
    (members, complete)
}
fn identity_matches(record: &Identity, current: Option<&Identity>) -> bool {
    current.is_some_and(|actual| {
        actual.pgid == record.pgid && actual.start == record.start && actual.pid == record.pid
    })
}
const PHASE_CAP: usize = 1536;

fn nullable_text(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .is_some_and(|field| field.is_null() || field.is_string())
}
fn phase_shape(value: &Value, checker_pid: Option<u32>) -> bool {
    let Some(expected) = checker_pid.filter(|pid| *pid != 0) else {
        return false;
    };
    value["checker_pid"].as_u64() == Some(u64::from(expected))
        && value["selector"]
            .as_u64()
            .is_some_and(|ordinal| ordinal > 0)
        && value
            .get("policy_line")
            .is_some_and(|line| line.is_null() || line.as_u64().is_some_and(|number| number > 0))
        && matches!(value["host"].as_str(), Some("unix" | "windows"))
        && matches!(
            value["phase"].as_str(),
            Some(
                "test-valued covered_by build"
                    | "test-valued covered_by enumeration"
                    | "test-valued covered_by doc enumeration"
            )
        )
        && value["program"] == "cargo"
        && value["argv"].as_array().is_some_and(|args| {
            args.first().and_then(Value::as_str) == Some("test")
                && args.iter().all(Value::is_string)
        })
        && value["elapsed_ms"].as_u64().is_some()
        && ["cwd", "target", "source"]
            .iter()
            .all(|key| nullable_text(value, key))
}
fn phase_summaries(
    text: &str,
    checker_pid: Option<u32>,
    capture_truncated: bool,
) -> (Option<Value>, Option<Value>, bool) {
    let mut first = None;
    let mut last = None;
    let mut complete = !capture_truncated;
    for line in text.lines() {
        let Some(record) = line.strip_prefix(CHILD_PREFIX) else {
            continue;
        };
        if record.len() > 4096 {
            complete = false;
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(record) else {
            complete = false;
            continue;
        };
        if value["event"] == "diagnostic_budget_exhausted" && value["class"] == 0 {
            complete = false;
            continue;
        }
        if value["event"] != "phase" {
            continue;
        }
        if record.len() > PHASE_CAP || !phase_shape(&value, checker_pid) {
            complete = false;
            continue;
        }
        if first.is_none() {
            first = Some(value.clone())
        }
        last = Some(value);
    }
    complete = complete && first.is_some();
    (first, last, complete)
}
fn phase_summary(text: &str, checker_pid: Option<u32>, capture_truncated: bool) -> Value {
    let (first_phase, last_phase, phases_complete) =
        phase_summaries(text, checker_pid, capture_truncated);
    json!({"event":"phase_summary","checker_pid":checker_pid,
        "first_phase":first_phase,"last_phase":last_phase,"phases_complete":phases_complete,
        "phase_status":if phases_complete {"OBSERVED"} else {"INCOMPLETE_OR_UNKNOWN"},
        "per_phase_encoded_byte_cap":PHASE_CAP,
        "scope":"SOURCE_PREFIXED_DIAGNOSTIC_RECORDS_NOT_AUTHENTICATED_EVIDENCE"})
}

struct RecordedSpawn {
    sequence: usize,
    pid: u32,
    identity: Option<Identity>,
}
struct Decoded {
    phases: Vec<Value>,
    spawns: Vec<RecordedSpawn>,
    complete: bool,
    coverage_return: Option<bool>,
}
fn decode_compact(
    text: &str,
    pid: Option<u32>,
    truncated: bool,
    expected: Option<&ParentContext>,
) -> Decoded {
    let mut out = Decoded {
        phases: Vec::new(),
        spawns: Vec::new(),
        complete: !truncated,
        coverage_return: None,
    };
    let Some((pid, expected)) = pid.filter(|pid| *pid != 0).zip(expected) else {
        out.complete = false;
        return out;
    };
    let wanted = expected.plan.context(
        pid,
        Some(&expected.source),
        Some(&expected.cwd),
        expected.target.as_deref(),
    );
    let mut context = false;
    let mut ended = false;
    let mut seen_samples = BTreeSet::new();
    let mut pending = None;
    for line in text.lines() {
        let Some(raw) = line.strip_prefix(CHILD_PREFIX) else {
            continue;
        };
        if raw.len() > 4096 {
            out.complete = false;
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(raw) else {
            out.complete = false;
            continue;
        };
        if value["event"] == "diagnostic_budget_exhausted" {
            out.complete = false;
            continue;
        }
        let Some(row) = value.as_array() else {
            out.complete = false;
            continue;
        };
        let Some(tag) = row.first().and_then(Value::as_str) else {
            out.complete = false;
            continue;
        };
        if tag == "c" {
            if context || ended || value != wanted {
                out.complete = false;
                pending = None
            } else {
                context = true
            }
            continue;
        }
        if !context {
            out.complete = false;
            continue;
        }
        let number = |index: usize| {
            row.get(index)
                .and_then(Value::as_u64)
                .and_then(|v| usize::try_from(v).ok())
        };
        if matches!(tag, "b" | "l" | "d") {
            if pending.is_some() {
                out.complete = false
            }
            pending = None;
            let parsed = number(1).zip(number(2)).zip(number(3)).zip(number(4));
            let Some((((seq, selector), line), argid)) = parsed else {
                out.complete = false;
                continue;
            };
            let Some(step) = expected.plan.steps.get(seq.saturating_sub(1)) else {
                out.complete = false;
                continue;
            };
            let elapsed = row
                .get(5)
                .and_then(Value::as_str)
                .and_then(|v| v.parse::<u128>().ok());
            if ended
                || row.len() != 6
                || seq == 0
                || seq != out.phases.len() + 1
                || seq > 39
                || step.selector != selector
                || step.line != line
                || step.kind != tag
                || step.argv != argid
                || elapsed.is_none()
            {
                out.complete = false;
                continue;
            }
            let elapsed = elapsed.and_then(|v| u64::try_from(v).ok());
            let phase = json!({"event":"phase","checker_pid":pid,"phase_sequence":seq,
                "selector":selector,"policy_line":line,"host":expected.plan.host,
                "phase":phase_description(tag),"program":"cargo","argv":expected.plan.argv.get(argid),
                "cwd":expected.cwd,"target":expected.target,"source":expected.source,"elapsed_ms":elapsed});
            let typed = phase_shape(&phase, Some(pid))
                && serde_json::to_string(&phase).is_ok_and(|v| v.len() <= PHASE_CAP);
            if !typed {
                out.complete = false
            }
            // Preserve sequence positions and actual own PID; never emit an invalid typed phase.
            out.phases.push(if typed { phase } else { Value::Null });
            pending = Some(seq);
        } else if tag == "s" {
            let Some(seq) = number(1) else {
                out.complete = false;
                continue;
            };
            let cargo = row
                .get(2)
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v != 0);
            if ended
                || row.len() != 5
                || pending != Some(seq)
                || out.spawns.iter().any(|v| v.sequence == seq)
                || cargo.is_none()
            {
                out.complete = false;
                continue;
            }
            pending = None;
            let Some(cargo) = cargo else {
                out.complete = false;
                continue;
            };
            let group = row
                .get(3)
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v != 0);
            let start = row.get(4).and_then(Value::as_u64).filter(|v| *v != 0);
            let both_null =
                row.get(3).is_some_and(Value::is_null) && row.get(4).is_some_and(Value::is_null);
            let identity = if let Some((group, start)) = group.zip(start) {
                let legacy = format!(
                    "{CHILD_PREFIX}{}\n",
                    json!({"event":"spawn","checker_pid":pid,
                    "pid":cargo,"pgid":group,"start_ticks":start})
                );
                let (rows, complete) = spawn_rows(&legacy, Some(pid));
                if !complete {
                    out.complete = false
                }
                rows.into_iter().next()
            } else {
                if !both_null {
                    out.complete = false
                }
                None
            };
            out.spawns.push(RecordedSpawn {
                sequence: seq,
                pid: cargo,
                identity,
            });
        } else if tag == "t" {
            let seq = number(1);
            let cargo = row
                .get(2)
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let association = seq.zip(cargo);
            let valid = association.is_some_and(|(seq, cargo)| {
                out.spawns
                    .iter()
                    .any(|v| v.sequence == seq && v.pid == cargo)
            });
            if ended
                || row.len() != 7
                || !valid
                || raw.len() + CHILD_PREFIX.len() + 1 > SAMPLE_CAP
                || row
                    .get(3)
                    .and_then(Value::as_str)
                    .is_none_or(|v| v.parse::<u128>().is_err())
                || !row.get(4).is_some_and(Value::is_string)
                || !row
                    .get(5)
                    .and_then(Value::as_u64)
                    .is_some_and(|n| n > 0 && n <= 4096)
                || !row.get(6).is_some_and(Value::is_boolean)
                || !association.is_some_and(|key| seen_samples.insert(key))
            {
                out.complete = false
            }
        } else if tag == "f" {
            let valid_time = row
                .get(1)
                .and_then(Value::as_str)
                .is_some_and(|v| v.parse::<u128>().is_ok());
            let success = row.get(2).and_then(Value::as_bool);
            if ended
                || row.len() != 3
                || !valid_time
                || success.is_none()
                || success == Some(true)
                    && (out.phases.len() != expected.plan.steps.len()
                        || out.spawns.len() != out.phases.len())
            {
                out.complete = false;
                continue;
            }
            ended = true;
            out.coverage_return = success;
        } else {
            out.complete = false;
            pending = None
        }
    }
    if !context
        || !ended
        || out.phases.is_empty()
        || pending.is_some()
        || out.spawns.len() != out.phases.len()
    {
        out.complete = false
    }
    out
}
fn decoded_phase_text(decoded: &Decoded) -> String {
    let mut text = String::new();
    for value in &decoded.phases {
        text.push_str(&format!("{CHILD_PREFIX}{value}\n"))
    }
    text
}
fn parent_line(budget: &mut Budget, value: &Value) -> Option<String> {
    let text = serde_json::to_string(value).ok()?;
    if text.len().checked_add(PARENT_PREFIX.len() + 1)? > 4096 {
        return budget.encode(
            PARENT_PREFIX,
            &json!({"event":"diagnostic_budget_exhausted","class":0,"reason":"parent_line_cap"}),
            0,
        );
    }
    budget.encode(PARENT_PREFIX, value, 0)
}

pub(crate) fn observe_parent(
    text: &str,
    checker_pid: Option<u32>,
    capture_truncated: bool,
    expected: Option<&ParentContext>,
) {
    let decoded = decode_compact(text, checker_pid, capture_truncated, expected);
    let groups = decoded
        .spawns
        .iter()
        .filter_map(|v| v.identity.as_ref().map(|i| i.pgid))
        .collect::<BTreeSet<_>>();
    let (members, scan_complete) = group_scan(&groups);
    let mut budget = Budget::default();
    let mut emit = |value| {
        if let Some(line) = parent_line(&mut budget, &value) {
            let _diagnostic_write = std::io::stderr().lock().write_all(line.as_bytes());
        }
    };
    emit(
        json!({"event":"post_capture","checker_pid":checker_pid,"spawn_rows":decoded.spawns.len(),
        "records_complete":decoded.complete,"scan_complete":scan_complete,"proc_entry_cap":PROC_CAP,
        "stat_byte_cap":STAT_CAP,"scan_cooperative_ms":100,"member_cap_per_group":MEMBER_CAP,
        "coverage_return":decoded.coverage_return,
        "coverage_completion":if decoded.coverage_return.is_some() {"RETURN_OBSERVED"}else{"UNKNOWN"},
        "scope":"OBSERVATION_ONLY_NO_KILL_NO_DESCENDANT_CLEANUP_PROOF"}),
    );
    emit(phase_summary(
        &decoded_phase_text(&decoded),
        checker_pid,
        !decoded.complete,
    ));
    // Last actual own spawn is first; never attach a previous PID to a pending later phase.
    for (index, row) in decoded.spawns.iter().rev().enumerate() {
        let current = read_stat(row.pid);
        let same = row
            .identity
            .as_ref()
            .is_some_and(|record| identity_matches(record, current.as_ref()));
        let phase = if index == 0 {
            decoded
                .phases
                .get(row.sequence.saturating_sub(1))
                .filter(|phase| phase_shape(phase, checker_pid))
        } else {
            None
        };
        emit(
            json!({"event":"process_liveness","checker_pid":checker_pid,"phase_sequence":row.sequence,
            "recorded_phase":phase,"pid":row.pid,"recorded_pgid":row.identity.as_ref().map(|v|v.pgid),
            "recorded_start_ticks":row.identity.as_ref().map(|v|v.start),
            "same_identity":same,"state":current.as_ref().map(|v|v.state),
            "ownership":if same {"MATCHED_CURRENT_PID"}else{"UNKNOWN"},
            "group_members_observed":row.identity.as_ref().and_then(|v|members.get(&v.pgid)),
            "group_observation_complete":decoded.complete&&scan_complete&&same,
            "descendants_terminated":"NOT_ESTABLISHED"}),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Budget, CHILD_PREFIX, CLASS_CAP, PARENT_PREFIX, PHASE_CAP, identity_matches, parse_stat,
        phase_summaries, phase_summary, scope, spawn_rows,
    };
    use serde_json::json;
    const FROZEN_ARGS: &str = r#"[["test","-p","xtask","repository_language_policy_admits_real_mixed_language_producer","--no-run"],["test","-p","xtask","repository_language_policy_admits_real_mixed_language_producer","--","--list","--format","terse"],["test","-p","xtask","implementation_slices_validate_and_coexist","--no-run"],["test","-p","xtask","implementation_slices_validate_and_coexist","--","--list","--format","terse"],["test","-p","xtask","committed_spec_review_receipts_validate","--no-run"],["test","-p","xtask","committed_spec_review_receipts_validate","--","--list","--format","terse"],["test","-p","xtask","--locked","--offline","rust_judged_panel::rolling_observation","--no-run"],["test","-p","xtask","--locked","--offline","rust_judged_panel::rolling_observation","--","--list","--format","terse"],["test","-p","xtask","--locked","--offline","rust_judged_panel::calibration","--no-run"],["test","-p","xtask","--locked","--offline","rust_judged_panel::calibration","--","--list","--format","terse"],["test","-p","xtask","--locked","--offline","rust_analysis_feedback","--no-run"],["test","-p","xtask","--locked","--offline","rust_analysis_feedback","--","--list","--format","terse"],["test","-p","xtask","rust_judged_panel::subject","--no-run"],["test","-p","xtask","rust_judged_panel::subject","--","--list","--format","terse"],["test","-p","xtask","--locked","--offline","rust_judged_panel::packet::tests","--no-run"],["test","-p","xtask","--locked","--offline","rust_judged_panel::packet::tests","--","--list","--format","terse"],["test","-p","xtask","--bin","xtask","dx_scoreboard","--no-run"],["test","-p","xtask","--bin","xtask","dx_scoreboard","--","--list","--format","terse"],["test","-p","xtask","--bin","xtask","pilot_ranking","--no-run"],["test","-p","xtask","--bin","xtask","pilot_ranking","--","--list","--format","terse"],["test","-p","ripr","--test","causal_delta_fixture","--no-run"],["test","-p","ripr","--test","causal_delta_fixture","--","--list","--format","terse"],["test","-p","xtask","--bin","xtask","source_promotion_workflow","--no-run"],["test","-p","xtask","--bin","xtask","source_promotion_workflow","--","--list","--format","terse"],["test","-p","xtask","source_promotion_control","--no-run"],["test","-p","xtask","source_promotion_control","--","--list","--format","terse"],["test","-p","xtask","--locked","--offline","portable_consumer","--no-run"],["test","-p","xtask","--locked","--offline","portable_consumer","--","--list","--format","terse"],["test","-p","ripr","--locked","--offline","--test","portable_consumer_packet","--no-run"],["test","-p","ripr","--locked","--offline","--test","portable_consumer_packet","--","--list","--format","terse"],["test","-p","xtask","public_proof","--no-run"],["test","-p","xtask","public_proof","--","--list","--format","terse"]]"#;
    const FROZEN_STEPS: &str = r#"[[1,1,107,0,"b"],[2,1,107,1,"l"],[3,2,348,2,"b"],[4,2,348,3,"l"],[5,3,357,4,"b"],[6,3,357,5,"l"],[7,4,411,6,"b"],[8,4,411,7,"l"],[9,5,411,8,"b"],[10,5,411,9,"l"],[11,6,411,10,"b"],[12,6,411,11,"l"],[13,7,420,12,"b"],[14,7,420,13,"l"],[15,8,429,14,"b"],[16,8,429,15,"l"],[17,9,661,16,"b"],[18,9,661,17,"l"],[19,10,697,18,"b"],[20,10,697,19,"l"],[21,11,706,19,"l"],[22,12,733,20,"b"],[23,12,733,21,"l"],[24,13,742,22,"b"],[25,13,742,23,"l"],[26,14,751,24,"b"],[27,14,751,25,"l"],[28,15,764,26,"b"],[29,15,764,27,"l"],[30,16,764,28,"b"],[31,16,764,29,"l"],[32,17,893,30,"b"],[33,17,893,31,"l"],[34,18,903,31,"l"],[35,19,913,31,"l"],[36,20,923,31,"l"],[37,21,932,31,"l"],[38,22,941,31,"l"],[39,23,950,31,"l"]]"#;
    const FROZEN_SOURCE: &str = "01b50d7b8f02a07fab684a078406594d5d138534";
    // Synthetic paths retain the captured wire lengths for the byte/cap controls.
    const FROZEN_CWD: &str = "/workspace/runner/ripr/ripr/target/ripr-resolved-tree-validation-ed229bd68c24e3184a888da7/tree";
    const FROZEN_TARGET: &str = "/workspace/runner/ripr/ripr/target";

    #[test]
    fn compact_source_plan_wire_decoder_and_capture_fairness_are_bounded() -> Result<(), String> {
        let argv: Vec<Vec<String>> =
            serde_json::from_str(FROZEN_ARGS).map_err(|error| error.to_string())?;
        let rows: Vec<(usize, usize, usize, usize, String)> =
            serde_json::from_str(FROZEN_STEPS).map_err(|error| error.to_string())?;
        let mut steps = Vec::new();
        for (sequence, selector, line, argv, kind) in rows {
            if sequence != steps.len() + 1 {
                return Err("frozen phase sequence drift".into());
            }
            let kind = match kind.as_str() {
                "b" => "b",
                "l" => "l",
                "d" => "d",
                _ => return Err("unknown frozen kind".into()),
            };
            steps.push(super::PlannedPhase {
                selector,
                line,
                kind,
                argv,
            });
        }
        let plan = super::DiagnosticPlan::new("unix", argv, steps)
            .ok_or("frozen diagnostic plan refused")?;
        if plan.steps.len() != 39 || plan.argv.len() != 32 || plan.tokens.len() != 29 {
            return Err("frozen denominator drift".into());
        }
        if !plan.matches_phase(
            0,
            1,
            Some(107),
            "test-valued covered_by build",
            &plan.argv[0],
        ) || plan.matches_phase(
            0,
            1,
            Some(107),
            "test-valued covered_by build",
            &["test".into(), "wrong".into()],
        ) {
            return Err("actual received argv equality lost".into());
        }
        let expected = super::ParentContext {
            source: FROZEN_SOURCE.into(),
            cwd: FROZEN_CWD.into(),
            target: Some(FROZEN_TARGET.into()),
            plan,
        };
        let mut wire = vec![expected.plan.context(
            u32::MAX,
            Some(&expected.source),
            Some(&expected.cwd),
            expected.target.as_deref(),
        )];
        for (index, step) in expected.plan.steps.iter().enumerate() {
            wire.push(json!([
                step.kind,
                index + 1,
                step.selector,
                step.line,
                step.argv,
                u128::MAX.to_string()
            ]));
            wire.push(json!(["s", index + 1, u32::MAX, u32::MAX, u64::MAX]));
        }
        wire.push(json!(["f", u128::MAX.to_string(), false]));
        let render = |rows: &[serde_json::Value]| {
            rows.iter()
                .map(|v| format!("{CHILD_PREFIX}{v}\n"))
                .collect::<String>()
        };
        let full = render(&wire);
        if full.len() != 7614
            || format!("{CHILD_PREFIX}{}\n", wire.last().ok_or("finish missing")?).len() != 76
        {
            return Err("actual false-finished full-width byte forecast drift".into());
        }
        let mut budget = Budget::default();
        let mut emitted = String::new();
        for value in &wire {
            emitted.push_str(
                &budget
                    .encode(CHILD_PREFIX, value, 0)
                    .ok_or("compact metadata unexpectedly exhausted")?,
            );
        }
        if emitted != full || budget.exhausted[0] || emitted.len() > CLASS_CAP - 256 {
            return Err("actual metadata encoder not bounded".into());
        }
        let unrepresentable = super::decode_compact(&full, Some(u32::MAX), false, Some(&expected));
        if unrepresentable.complete
            || unrepresentable.spawns.len() != 39
            || !unrepresentable
                .phases
                .iter()
                .all(serde_json::Value::is_null)
        {
            return Err(
                "u128 parent elapsed was silently narrowed or invalid raw phase retained".into(),
            );
        }
        for value in &mut wire {
            if matches!(value[0].as_str(), Some("b" | "l" | "d")) {
                value[5] = json!(128867.to_string())
            }
        }
        let genuine = render(&wire);
        let decoded = super::decode_compact(&genuine, Some(u32::MAX), false, Some(&expected));
        if !decoded.complete
            || decoded.phases.len() != 39
            || decoded.spawns.len() != 39
            || decoded.coverage_return != Some(false)
            || decoded.phases.last().and_then(|v| v["selector"].as_u64()) != Some(23)
        {
            return Err("genuine entire compact trace lost".into());
        }
        let last = decoded.spawns.last().ok_or("last own spawn missing")?;
        if last.sequence != 39 || last.pid != u32::MAX {
            return Err("last own sequence/PID lost".into());
        }
        for (pid, truncated, context) in [
            (Some(1), false, Some(&expected)),
            (Some(u32::MAX), true, Some(&expected)),
            (Some(u32::MAX), false, None),
        ] {
            if super::decode_compact(&genuine, pid, truncated, context).complete {
                return Err("missing parent authority/truncated/different checker admitted".into());
            }
        }
        for case in 0..10 {
            let mut wrong = wire.clone();
            match case {
                0 => {
                    wrong.remove(0);
                }
                1 => {
                    wrong.insert(1, wrong[0].clone());
                }
                2 => wrong[0][4] = json!("wrong source"),
                3 => wrong[0][5] = json!("/other/root"),
                4 => wrong[1][1] = json!(2),
                5 => wrong[1][4] = json!(999),
                6 => wrong[2][1] = json!(2),
                7 => {
                    wrong.insert(3, wrong[2].clone());
                }
                8 => wrong[0][9][0][0] = json!(999),
                _ => {
                    wrong.remove(2);
                }
            }
            if super::decode_compact(&render(&wrong), Some(u32::MAX), false, Some(&expected))
                .complete
            {
                return Err(format!(
                    "invalid compact context/sequence/dictionary admitted:{case}"
                ));
            }
        }
        let mut unknown = wire.clone();
        unknown[2][3] = json!(null);
        unknown[2][4] = json!(null);
        let retained =
            super::decode_compact(&render(&unknown), Some(u32::MAX), false, Some(&expected));
        let first = retained
            .spawns
            .first()
            .ok_or("nullable actual PID was lost")?;
        if first.pid != u32::MAX || first.sequence != 1 || first.identity.is_some() {
            return Err("nullable actual identity fabricated/lost".into());
        }
        unknown[2][3] = json!(1);
        if super::decode_compact(&render(&unknown), Some(u32::MAX), false, Some(&expected)).complete
        {
            return Err("mixed-null identity considered complete".into());
        }
        let mut missing_finish = wire.clone();
        missing_finish.pop();
        let unfinished = super::decode_compact(
            &render(&missing_finish),
            Some(u32::MAX),
            false,
            Some(&expected),
        );
        if unfinished.complete || unfinished.coverage_return.is_some() {
            return Err("missing finish inferred complete records or coverage completion".into());
        }
        let shared = std::sync::Arc::new(std::sync::Mutex::new(super::Shared {
            budget: Budget::default(),
            next: 0,
            live: true,
        }));
        let token = |sequence| super::PhaseToken {
            started: std::time::Instant::now(),
            shared: std::sync::Arc::clone(&shared),
            sequence,
        };
        let old = super::CaptureObservation::new(token(1), 7);
        let new = super::CaptureObservation::new(token(2), 7);
        let latest = new
            .sample_once(b"new phase", "1")
            .ok_or("new first sample missing")?;
        let delayed = old
            .sample_once(b"old delayed phase", "2")
            .ok_or("delayed old sample missing")?;
        if latest[1] != 2
            || delayed[1] != 1
            || latest[2] != 7
            || delayed[2] != 7
            || old.sample_once(b"duplicate", "3").is_some()
            || new.sample_once(b"duplicate", "3").is_some()
        {
            return Err("immutable per-capture sequence/PID fairness lost".into());
        }
        let mut sample_budget = Budget::default();
        let mut text_bytes = 0usize;
        let raw = "\\\"\n界".repeat(100);
        for sequence in 1..=39 {
            let capture = super::CaptureObservation::new(token(sequence), u32::MAX);
            if capture.sample_once(b"", "0").is_some() {
                return Err("empty input consumed a sample".into());
            }
            let value = capture
                .sample_once(raw.as_bytes(), &u128::MAX.to_string())
                .ok_or("fair late sample missing")?;
            let rendered = sample_budget
                .encode(CHILD_PREFIX, &value, 1)
                .ok_or("fair sample budget exhausted")?;
            if rendered.len() > 196 {
                return Err("framing/escaping exceeded per-capture slot".into());
            }
            text_bytes += rendered.len();
            if capture.sample_once(b"second read", "2").is_some() {
                return Err("capture used multiple slots".into());
            }
        }
        if text_bytes > 7644 || sample_budget.exhausted[1] {
            return Err("all39 fair slots exceeded original budget".into());
        }
        let mut with_samples = wire.clone();
        with_samples.insert(
            with_samples.len() - 1,
            json!(["t", 1, u32::MAX, "1", "delayed", 7, false]),
        );
        with_samples.insert(
            with_samples.len() - 1,
            json!(["t", 2, u32::MAX, "2", "new", 3, false]),
        );
        if !super::decode_compact(
            &render(&with_samples),
            Some(u32::MAX),
            false,
            Some(&expected),
        )
        .complete
        {
            return Err("genuine delayed/PID-reused sample associations lost".into());
        }
        let last_index = with_samples.len() - 2;
        with_samples[last_index][1] = json!(99);
        if super::decode_compact(
            &render(&with_samples),
            Some(u32::MAX),
            false,
            Some(&expected),
        )
        .complete
        {
            return Err("orphan sample sequence admitted".into());
        }

        let wide_plan = super::DiagnosticPlan::new(
            "unix",
            vec![
                std::iter::once("test".to_string())
                    .chain(std::iter::repeat_n("x".repeat(256), 14))
                    .collect(),
            ],
            vec![super::PlannedPhase {
                selector: 1,
                line: 107,
                kind: "b",
                argv: 0,
            }],
        )
        .ok_or("legal repeated-token plan was refused")?;
        let wide_context = super::ParentContext {
            source: FROZEN_SOURCE.into(),
            cwd: "/owned/tree".into(),
            target: None,
            plan: wide_plan,
        };
        let wide_rows = vec![
            wide_context.plan.context(
                u32::MAX,
                Some(&wide_context.source),
                Some(&wide_context.cwd),
                None,
            ),
            json!(["b", 1, 1, 107, 0, "1"]),
            json!(["s", 1, u32::MAX, u32::MAX, u64::MAX]),
            json!(["f", "1", false]),
        ];
        let wide_decoded = super::decode_compact(
            &render(&wide_rows),
            Some(u32::MAX),
            false,
            Some(&wide_context),
        );
        if wide_decoded.complete
            || wide_decoded.phases.len() != 1
            || !wide_decoded.phases[0].is_null()
            || wide_decoded.spawns.len() != 1
            || wide_decoded.spawns[0].sequence != 1
            || wide_decoded.spawns[0].pid != u32::MAX
        {
            return Err(
                "oversized reconstructed phase lost its positional own PID or escaped typed bounds"
                    .into(),
            );
        }
        let probe = json!({"event":"post_capture","padding":""});
        let overhead = serde_json::to_string(&probe)
            .map_err(|error| error.to_string())?
            .len()
            + PARENT_PREFIX.len()
            + 1;
        let exact = json!({"event":"post_capture","padding":"x".repeat(4096-overhead)});
        let oversized = json!({"event":"post_capture","padding":"x".repeat(4097-overhead)});
        let mut parent_budget = Budget::default();
        let exact_line = super::parent_line(&mut parent_budget, &exact)
            .ok_or("exact parent line bound was refused")?;
        if exact_line.len() != 4096 {
            return Err("parent framing was omitted from4096 bound".into());
        }
        let refused = super::parent_line(&mut parent_budget, &oversized)
            .ok_or("oversized parent omission marker lost")?;
        let refusal: serde_json::Value = serde_json::from_str(
            refused
                .strip_prefix(PARENT_PREFIX)
                .ok_or("parent omission marker prefix lost")?,
        )
        .map_err(|error| error.to_string())?;
        if refused.len() > 4096
            || refusal["event"] != "diagnostic_budget_exhausted"
            || refusal["reason"] != "parent_line_cap"
        {
            return Err("parent cap+1 line escaped or omitted diagnostic signal".into());
        }
        let exhausted = super::parent_line(&mut parent_budget, &exact)
            .ok_or("aggregate parent exhaustion marker lost")?;
        if exact_line.len() + refused.len() + exhausted.len() > CLASS_CAP
            || !parent_budget.exhausted[0]
        {
            return Err("actual parent encoding route exceeded its original aggregate cap".into());
        }
        Ok(())
    }

    fn stat(pid: u32, group: u32, start: u64) -> String {
        let mut fields = vec!["0".to_string(); 20];
        fields[0] = "S".to_string();
        fields[2] = group.to_string();
        fields[19] = start.to_string();
        format!("{pid} (name with ) spaces) {}", fields.join(" "))
    }
    #[test]
    fn exact_owned_optin_and_process_identity_are_required() -> Result<(), String> {
        if !scope(true, true, true) {
            return Err("owned optin lost".into());
        }
        for bits in [
            (false, true, true),
            (true, false, true),
            (true, true, false),
        ] {
            if scope(bits.0, bits.1, bits.2) {
                return Err("scope escaped".into());
            }
        }
        let text = stat(42, 77, 991);
        let Some(identity) = parse_stat(42, &text) else {
            return Err("actual stat fields lost".into());
        };
        if identity.pid != 42
            || identity.pgid != 77
            || identity.start != 991
            || parse_stat(43, &text).is_some()
            || parse_stat(42, "malformed").is_some()
        {
            return Err("identity discriminator lost".into());
        }
        let mut reused = identity.clone();
        reused.start += 1;
        if !identity_matches(&identity, Some(&identity))
            || identity_matches(&identity, Some(&reused))
            || identity_matches(&identity, None)
        {
            return Err("PID reuse or unavailable identity admitted".into());
        }
        reused = identity.clone();
        reused.pgid += 1;
        if identity_matches(&identity, Some(&reused)) {
            return Err("process-group reassignment admitted".into());
        }
        let phase_value = |ordinal, argument: &str| {
            json!({
                "event":"phase","checker_pid":9,"selector":ordinal,"policy_line":114,"host":"unix",
                "phase":"test-valued covered_by build","program":"cargo","argv":["test",argument],
                "cwd":"/owned/tree","target":"/owned/cache","source":null,"elapsed_ms":1
            })
        };
        let phase =
            |ordinal, argument: &str| format!("{CHILD_PREFIX}{}\n", phase_value(ordinal, argument));
        let phases = format!("{}{}", phase(1, "first"), phase(2, "last"));
        let (first, last, complete) = phase_summaries(&phases, Some(9), false);
        if !complete
            || first.as_ref().and_then(|value| value["argv"][1].as_str()) != Some("first")
            || last.as_ref().and_then(|value| value["argv"][1].as_str()) != Some("last")
        {
            return Err("actual first/last phase fields lost".into());
        }
        for (pid, truncated) in [(Some(10), false), (Some(9), true), (None, false)] {
            if phase_summaries(&phases, pid, truncated).2 {
                return Err("wrong checker or truncated phase summary admitted".into());
            }
        }
        let exhausted = format!(
            "{phases}{CHILD_PREFIX}{{\"event\":\"diagnostic_budget_exhausted\",\"class\":0}}\n"
        );
        if phase_summaries(&exhausted, Some(9), false).2 || phase_summaries("", Some(9), false).2 {
            return Err("exhausted or missing phases claimed complete".into());
        }
        let minimal = format!("{CHILD_PREFIX}{{\"event\":\"phase\",\"checker_pid\":9}}\n");
        let (first, last, complete) = phase_summaries(&minimal, Some(9), false);
        if complete || first.is_some() || last.is_some() {
            return Err("missing typed phase facts accepted".into());
        }
        for key in [
            "selector",
            "policy_line",
            "host",
            "phase",
            "program",
            "argv",
            "cwd",
            "target",
            "source",
            "elapsed_ms",
        ] {
            let mut malformed = phase_value(1, "first");
            malformed
                .as_object_mut()
                .ok_or("phase object missing")?
                .remove(key);
            if phase_summaries(&format!("{CHILD_PREFIX}{malformed}\n"), Some(9), false).2 {
                return Err(format!("missing phase field accepted: {key}"));
            }
        }
        for (key, wrong) in [
            ("selector", json!(0)),
            ("policy_line", json!("114")),
            ("host", json!("unknown")),
            ("phase", json!("unknown")),
            ("program", json!("other")),
            ("argv", json!(["test", 1])),
            ("cwd", json!(1)),
            ("target", json!(1)),
            ("source", json!(1)),
            ("elapsed_ms", json!("1")),
        ] {
            let mut malformed = phase_value(1, "first");
            malformed[key] = wrong;
            if phase_summaries(&format!("{CHILD_PREFIX}{malformed}\n"), Some(9), false).2 {
                return Err(format!("wrong typed phase field accepted: {key}"));
            }
        }
        let mut nullable = phase_value(1, "first");
        for key in ["policy_line", "cwd", "target", "source"] {
            nullable[key] = json!(null)
        }
        if !phase_summaries(&format!("{CHILD_PREFIX}{nullable}\n"), Some(9), false).2 {
            return Err("genuine nullable phase fields refused".into());
        }
        let argument = "\\\"\n界".repeat(80);
        let mut boundary = phase_value(1, &argument);
        let initial = serde_json::to_string(&boundary).map_err(|error| error.to_string())?;
        if initial.len() > PHASE_CAP {
            return Err("boundary fixture exceeded cap".into());
        }
        boundary["argv"][1] = json!(format!(
            "{argument}{}",
            "x".repeat(PHASE_CAP - initial.len())
        ));
        let encoded = serde_json::to_string(&boundary).map_err(|error| error.to_string())?;
        if encoded.len() != PHASE_CAP {
            return Err("phase exact boundary not exercised".into());
        }
        let mut second = boundary.clone();
        second["selector"] = json!(2);
        let boundary_rows = format!("{CHILD_PREFIX}{boundary}\n{CHILD_PREFIX}{second}\n");
        let actual_summary = phase_summary(&boundary_rows, Some(9), false);
        if actual_summary["phases_complete"] != true
            || actual_summary["first_phase"]["selector"] != 1
            || actual_summary["last_phase"]["selector"] != 2
        {
            return Err("near-boundary actual first/last summary lost".into());
        }
        let rendered = Budget::default()
            .encode(PARENT_PREFIX, &actual_summary, 0)
            .ok_or("actual summary encoder refused genuine near-boundary rows")?;
        if rendered.len() > 4096
            || !rendered.starts_with(PARENT_PREFIX)
            || !rendered.contains("phase_summary")
        {
            return Err("rendered summary cannot fit unchanged exporter line cap".into());
        }
        boundary["argv"][1] = json!(format!(
            "{}x",
            boundary["argv"][1].as_str().ok_or("argument missing")?
        ));
        if phase_summaries(&format!("{CHILD_PREFIX}{boundary}\n"), Some(9), false).2 {
            return Err("phase cap plus one accepted".into());
        }
        let record = format!(
            "{CHILD_PREFIX}{}\n",
            json!({"event":"spawn","checker_pid":9,"pid":42,"pgid":77,"start_ticks":991})
        );
        let (rows, complete) = spawn_rows(&record, Some(9));
        if rows.len() != 1 || !complete {
            return Err("genuine source spawn row lost".into());
        }
        let (rows, complete) = spawn_rows(&record, Some(10));
        if !rows.is_empty() || complete {
            return Err("different checker identity admitted".into());
        }
        Ok(())
    }
    #[test]
    fn escaped_rendered_budgets_and_child_text_never_become_spawn_records() -> Result<(), String> {
        let mut budget = Budget::default();
        let mut totals = [0usize; 2];
        for (class, total) in totals.iter_mut().enumerate() {
            for _ in 0..100 {
                if let Some(line) = budget.encode(
                    CHILD_PREFIX,
                    &json!({"event":"stderr_tail","tail":"\\\"\n界".repeat(128)}),
                    class,
                ) {
                    *total += line.len();
                    if *total > CLASS_CAP {
                        return Err("rendered budget exceeded".into());
                    }
                }
            }
            if !budget.exhausted[class] {
                return Err("escaping-heavy budget failed to exhaust".into());
            }
        }
        let child = format!(
            "{CHILD_PREFIX}{}\n",
            json!({"event":"stderr_tail","tail":format!("{CHILD_PREFIX}{{\"event\":\"spawn\",\"pid\":1}}")})
        );
        let (rows, complete) = spawn_rows(&child, Some(9));
        if !rows.is_empty() || !complete {
            return Err("child text reinterpreted as owner frame".into());
        }
        Ok(())
    }
}
