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
pub(crate) struct Observation {
    started: Instant,
    budget: Arc<Mutex<Budget>>,
}
impl Observation {
    pub(crate) fn for_checker() -> Option<Self> {
        let owned = std::env::var("RIPR_SOURCE_PROMOTION_PHASE_DIAGNOSTICS_OWNED")
            .is_ok_and(|value| value == "1");
        let validation =
            std::env::var("RIPR_SOURCE_PROMOTION_VALIDATION").is_ok_and(|value| value == "1");
        scope(requested(), owned, validation).then(|| Self {
            started: Instant::now(),
            budget: Arc::new(Mutex::new(Budget::default())),
        })
    }
    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            started: Instant::now(),
            budget: Arc::new(Mutex::new(Budget::default())),
        }
    }
    pub(crate) fn emit(&self, mut value: Value, child_text: bool) {
        value["checker_pid"] = json!(std::process::id());
        value["elapsed_ms"] = json!(self.started.elapsed().as_millis());
        let Ok(mut budget) = self.budget.lock() else {
            return;
        };
        if let Some(line) = budget.encode(CHILD_PREFIX, &value, usize::from(child_text)) {
            let _diagnostic_write = std::io::stderr().lock().write_all(line.as_bytes());
        }
    }
    pub(crate) fn phase(&self, ordinal: usize, line: Option<usize>, phase: &str, args: &[String]) {
        self.emit(json!({
            "event":"phase","selector":ordinal,"policy_line":line,"host":std::env::consts::FAMILY,
            "phase":phase,"program":"cargo","argv":args,
            "cwd":std::env::current_dir().ok(),
            "target":std::env::var("CARGO_TARGET_DIR").ok(),
            "source":std::env::var("RIPR_SOURCE_PROMOTION_TRUSTED_CHECKER_SHA").ok()
        }), false);
    }
    pub(crate) fn spawn(&self, pid: u32) {
        let observed = read_stat(pid);
        self.emit(
            json!({
                "event":"spawn","program":"cargo","pid":pid,
                "pgid":observed.as_ref().map(|value|value.pgid),
                "start_ticks":observed.as_ref().map(|value|value.start),
                "identity":if observed.is_some() {"OBSERVED"} else {"UNKNOWN"}
            }),
            false,
        );
    }
    pub(crate) fn tail(&self, pid: u32, bytes: &[u8]) {
        let tail = &bytes[bytes.len().saturating_sub(512)..];
        self.emit(
            json!({
                "event":"stderr_tail","pid":pid,"tail":String::from_utf8_lossy(tail),
                "input_bytes":bytes.len(),"tail_truncated":bytes.len()>512
            }),
            true,
        );
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

pub(crate) fn observe_parent(text: &str, checker_pid: Option<u32>, capture_truncated: bool) {
    let (rows, records_complete) = spawn_rows(text, checker_pid);
    let records_complete = records_complete && !capture_truncated;
    let groups = rows.iter().map(|row| row.pgid).collect::<BTreeSet<_>>();
    let (members, scan_complete) = group_scan(&groups);
    let mut budget = Budget::default();
    let mut emit = |value| {
        if let Some(line) = budget.encode(PARENT_PREFIX, &value, 0) {
            let _diagnostic_write = std::io::stderr().lock().write_all(line.as_bytes());
        }
    };
    emit(json!({"event":"post_capture","checker_pid":checker_pid,
        "spawn_rows":rows.len(),"records_complete":records_complete,
        "scan_complete":scan_complete,"proc_entry_cap":PROC_CAP,"stat_byte_cap":STAT_CAP,
        "scan_cooperative_ms":100,"member_cap_per_group":MEMBER_CAP,
        "scope":"OBSERVATION_ONLY_NO_KILL_NO_DESCENDANT_CLEANUP_PROOF"}));
    // Same parent8KiB budget; actual phase visibility precedes potentially numerous liveness rows.
    emit(phase_summary(text, checker_pid, capture_truncated));
    for row in rows {
        let current = read_stat(row.pid);
        let same = identity_matches(&row, current.as_ref());
        emit(json!({"event":"process_liveness","checker_pid":checker_pid,
            "pid":row.pid,"recorded_pgid":row.pgid,"recorded_start_ticks":row.start,
            "same_identity":same,"state":current.as_ref().map(|actual|actual.state),
            "ownership":if same {"MATCHED_CURRENT_PID"} else {"UNKNOWN"},
            "group_members_observed":members.get(&row.pgid),
            "group_observation_complete":records_complete&&scan_complete&&same,
            "descendants_terminated":"NOT_ESTABLISHED"}));
    }
}
#[cfg(test)]
mod tests {
    use super::{
        Budget, CHILD_PREFIX, CLASS_CAP, PARENT_PREFIX, PHASE_CAP, identity_matches, parse_stat,
        phase_summaries, phase_summary, scope, spawn_rows,
    };
    use serde_json::json;
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
        for class in 0..2 {
            for _ in 0..100 {
                if let Some(line) = budget.encode(
                    CHILD_PREFIX,
                    &json!({"event":"stderr_tail","tail":"\\\"\n界".repeat(128)}),
                    class,
                ) {
                    totals[class] += line.len();
                    if totals[class] > CLASS_CAP {
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
