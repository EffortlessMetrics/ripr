//! Read-only capability observation. This module cannot authorize calibration.
//! No cgroup, mount, namespace, limit, process, or workflow setting is changed.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

const REPORT_MAX_BYTES: usize = 64 * 1024;
const PROC_CGROUP_MAX_BYTES: usize = 4 * 1024;
const MOUNTINFO_MAX_BYTES: usize = 64 * 1024;
const CONTROL_MAX_BYTES: usize = 4 * 1024;
const REQUIRED_MEMORY_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const REQUIRED_STORAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
struct Observation {
    state: &'static str,
    bytes: usize,
    sha256: Option<String>,
    text: Option<String>,
    error_kind: Option<String>,
}

impl Observation {
    fn unavailable(state: &'static str, kind: &str) -> Self {
        Self {
            state,
            bytes: 0,
            sha256: None,
            text: None,
            error_kind: Some(kind.to_string()),
        }
    }

    fn descriptor(&self, path: &str) -> Value {
        json!({
            "path": path,
            "state": self.state,
            "bytes": self.bytes,
            "sha256": self.sha256,
            "error_kind": self.error_kind,
        })
    }
}

fn read_limited(reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let ceiling = limit
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| "readiness read ceiling overflow".to_string())?;
    let mut bytes = Vec::new();
    reader
        .take(ceiling)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("readiness read failed: {:?}", err.kind()))?;
    if bytes.len() > limit {
        return Err("readiness observation exceeded byte ceiling".to_string());
    }
    Ok(bytes)
}

fn observe_file(path: &str, limit: usize) -> Observation {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(err) => {
            let state = match err.kind() {
                std::io::ErrorKind::NotFound => "MISSING",
                std::io::ErrorKind::PermissionDenied => "UNREADABLE",
                _ => "READ_ERROR",
            };
            return Observation::unavailable(state, &format!("{:?}", err.kind()));
        }
    };
    let bytes = match read_limited(file, limit) {
        Ok(bytes) => bytes,
        Err(error) => {
            let state = if error == "readiness observation exceeded byte ceiling" {
                "OVERSIZED_OBSERVATION"
            } else {
                "UNVERIFIED_READ"
            };
            return Observation::unavailable(state, &error);
        }
    };
    let length = bytes.len();
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    match String::from_utf8(bytes) {
        Ok(text) => Observation {
            state: "OBSERVED",
            bytes: length,
            sha256: Some(sha256),
            text: Some(text),
            error_kind: None,
        },
        Err(_) => Observation::unavailable("MALFORMED_UTF8", "non-UTF8 observation"),
    }
}

fn valid_posix_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 1024
        && !path
            .chars()
            .any(|character| character.is_control() || character == '\\')
        && (path == "/"
            || path.strip_prefix('/').is_some_and(|rest| {
                rest.split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
            }))
}

fn unified_membership(text: &str) -> Result<String, String> {
    let mut membership = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("0::") {
            if membership.is_some() {
                return Err("ambiguous cgroup v2 membership".to_string());
            }
            if !valid_posix_path(path) {
                return Err("unsupported or unsafe cgroup v2 membership path".to_string());
            }
            membership = Some(path.to_string());
        }
    }
    membership.ok_or_else(|| "cgroup v2 membership absent".to_string())
}

fn decode_mount_field(field: &str) -> Result<String, String> {
    if field.len() > 1024 {
        return Err("mount field exceeded byte ceiling".to_string());
    }
    let mut decoded = Vec::with_capacity(field.len());
    let mut position = 0;
    while position < field.len() {
        if field.as_bytes()[position] == b'\\' {
            let code = field
                .as_bytes()
                .get(position + 1..position + 4)
                .ok_or_else(|| "incomplete mountinfo escape".to_string())?;
            let byte = match code {
                b"040" => b' ',
                b"011" => b'\t',
                b"012" => b'\n',
                b"134" => b'\\',
                _ => return Err("unsupported mountinfo escape".to_string()),
            };
            decoded.push(byte);
            position += 4;
        } else {
            decoded.push(field.as_bytes()[position]);
            position += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_utf8_error| "non-UTF8 mount field".to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CgroupMount {
    root: String,
    membership: String,
    mountpoint: String,
    directory: String,
}

fn resolve_cgroup_mount(mountinfo: &str, membership: &str) -> Result<CgroupMount, String> {
    if !valid_posix_path(membership) {
        return Err("unsupported cgroup membership".to_string());
    }
    let mut selected: Option<CgroupMount> = None;
    let mut ambiguous = false;
    for line in mountinfo.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            return Err("malformed mountinfo row".to_string());
        };
        let mut filesystem_fields = right.split_whitespace();
        if filesystem_fields.next() != Some("cgroup2") {
            continue;
        }
        filesystem_fields
            .next()
            .ok_or_else(|| "cgroup2 source absent".to_string())?;
        filesystem_fields
            .next()
            .ok_or_else(|| "cgroup2 super options absent".to_string())?;
        let mut fields = left.split_whitespace();
        let root = fields
            .nth(3)
            .ok_or_else(|| "mountinfo root absent".to_string())?;
        let point = fields
            .next()
            .ok_or_else(|| "mountinfo mountpoint absent".to_string())?;
        fields
            .next()
            .ok_or_else(|| "mountinfo mount options absent".to_string())?;
        let root = decode_mount_field(root)?;
        let point = decode_mount_field(point)?;
        if !valid_posix_path(&root) || !valid_posix_path(&point) {
            return Err("unsupported cgroup mount path".to_string());
        }
        let relative = if membership == root {
            Some("")
        } else if root == "/" {
            membership.strip_prefix('/')
        } else {
            membership.strip_prefix(&format!("{root}/"))
        };
        let Some(relative) = relative else {
            continue;
        };
        let directory = if relative.is_empty() {
            point.clone()
        } else {
            format!("{point}/{relative}")
        };
        let candidate = CgroupMount {
            root,
            membership: membership.to_string(),
            mountpoint: point,
            directory,
        };
        match &selected {
            Some(previous) if previous.root.len() > candidate.root.len() => {}
            Some(previous) if previous.root.len() == candidate.root.len() => ambiguous = true,
            _ => {
                selected = Some(candidate);
                ambiguous = false;
            }
        }
    }
    if ambiguous {
        return Err("ambiguous cgroup v2 mount mapping".to_string());
    }
    selected.ok_or_else(|| "matching cgroup v2 mount absent".to_string())
}

fn scalar(observation: &Observation) -> Option<&str> {
    observation
        .text
        .as_deref()
        .map(str::trim)
        .filter(|value| value.len() <= 128 && !value.contains('\n'))
}

fn numeric_limit(observation: &Observation, expected: u64) -> &'static str {
    match scalar(observation) {
        Some("max") => "UNLIMITED_OBSERVED",
        Some(value) => match value.parse::<u64>() {
            Ok(actual) if actual == expected => "EXACT_VALUE_OBSERVED_NOT_EXCLUSIVE",
            Ok(_) => "DIFFERENT_VALUE_OBSERVED",
            Err(_) => "MALFORMED_VALUE",
        },
        None => "UNVERIFIED",
    }
}

fn build_observation(os: &str, read: impl Fn(&str, usize) -> Observation) -> Value {
    let membership = read("/proc/self/cgroup", PROC_CGROUP_MAX_BYTES);
    let mounts = read("/proc/self/mountinfo", MOUNTINFO_MAX_BYTES);
    let kernel = read("/proc/sys/kernel/osrelease", 256);
    let mapping = match (membership.text.as_deref(), mounts.text.as_deref()) {
        (Some(member_text), Some(mount_text)) if os == "linux" => {
            unified_membership(member_text).and_then(|path| resolve_cgroup_mount(mount_text, &path))
        }
        _ => Err("Linux cgroup observations unavailable".to_string()),
    };
    let mut controls = Vec::new();
    let mut limit_states = json!({
        "memory_max": "UNVERIFIED",
        "swap_max": "UNVERIFIED",
        "oom_group": "UNVERIFIED",
    });
    let cgroup = match mapping {
        Ok(mapping) => {
            for name in [
                "cgroup.type",
                "cgroup.controllers",
                "cgroup.subtree_control",
                "cgroup.procs",
                "cgroup.events",
                "memory.max",
                "memory.current",
                "memory.peak",
                "memory.swap.max",
                "memory.oom.group",
                "memory.events",
            ] {
                let path = format!("{}/{name}", mapping.directory);
                let observed = read(&path, CONTROL_MAX_BYTES);
                let value = match name {
                    "cgroup.procs" => None,
                    _ => scalar(&observed),
                };
                if name == "memory.max" {
                    limit_states["memory_max"] =
                        json!(numeric_limit(&observed, REQUIRED_MEMORY_BYTES));
                } else if name == "memory.swap.max" {
                    limit_states["swap_max"] = json!(numeric_limit(&observed, 0));
                } else if name == "memory.oom.group" {
                    limit_states["oom_group"] = json!(numeric_limit(&observed, 1));
                }
                controls.push(json!({
                    "observation": observed.descriptor(&path),
                    "single_line_value": value,
                    "bounded_control_text": if name == "cgroup.procs" { None } else { observed.text.as_deref() },
                    "pid_rows_observed": if name == "cgroup.procs" { observed.text.as_deref().map(|text| text.lines().count()) } else { None },
                    "membership_observation_is_racy": name == "cgroup.procs",
                }));
            }
            json!({
                "state": "PATH_MAPPING_OBSERVED",
                "membership_path": mapping.membership,
                "mount_root": mapping.root,
                "mountpoint": mapping.mountpoint,
                "directory": mapping.directory,
                "ownership": "UNVERIFIED_CURRENT_OR_ANCESTOR_SCOPE",
                "concurrent_occupants_and_headroom": "UNVERIFIED",
                "control_write_authority": "UNVERIFIED_READ_ONLY",
                "cgroup_kill_effectiveness": "NOT_TESTED",
            })
        }
        Err(error) => json!({"state": "UNVERIFIED", "reason": error}),
    };
    json!({
        "schema_version": "0.1",
        "status": "NOT_READY",
        "full_trial": "NOT_RUN",
        "observation_mode": "READ_ONLY_NO_PROCESS_LAUNCH",
        "platform": os,
        "requested": {
            "family_memory_bytes": REQUIRED_MEMORY_BYTES,
            "all_invocation_added_storage_bytes": REQUIRED_STORAGE_BYTES,
            "retained_output_ceiling_bytes": 128 * 1024 * 1024,
        },
        "observations": {
            "membership": membership.descriptor("/proc/self/cgroup"),
            "mountinfo": mounts.descriptor("/proc/self/mountinfo"),
            "kernel": {
                "observation": kernel.descriptor("/proc/sys/kernel/osrelease"),
                "release": scalar(&kernel),
            },
            "cgroup": cgroup,
            "controls": controls,
            "numeric_limit_states": limit_states,
        },
        "source_provider_availability": {
            "before_exec_exclusive_family_provider": "NOT_IMPLEMENTED",
            "sealed_aggregate_writable_storage_provider": "NOT_IMPLEMENTED",
            "immediate_stream_overflow_family_abort": "NOT_IMPLEMENTED",
            "delegation_and_effective_control_write_authority": "UNVERIFIED_READ_ONLY"
        },
        "required_proofs": [
            {"owner": "run/process_owner", "proof": "before_exec_owned_domain_assignment_and_descendant_nonescape", "state": "NOT_ESTABLISHED"},
            {"owner": "run/process_owner", "proof": "family_kill_populated_zero_and_primary_reap", "state": "NOT_ESTABLISHED"},
            {"owner": "run", "proof": "aggregate_writable_storage_including_checkout_home_tmp_cache_output_open_unlinked_files_and_external_escape_refusal", "state": "NOT_ESTABLISHED"},
            {"owner": "run", "proof": "bounded_stream_overflow_before_buffer_or_file_growth_and_owned_family_abort", "state": "NOT_ESTABLISHED"},
            {"owner": "run", "proof": "scaled_memory_disk_output_and_setsid_controls", "state": "NOT_RUN"},
        ],
        "limits": {
            "report_bytes": REPORT_MAX_BYTES,
            "proc_cgroup_read_bytes": PROC_CGROUP_MAX_BYTES,
            "mountinfo_read_bytes": MOUNTINFO_MAX_BYTES,
            "per_control_read_bytes": CONTROL_MAX_BYTES,
            "settings_written": 0,
            "processes_launched": 0,
            "source_size_budgets_changed": 0,
        },
        "memory_semantics": "Kernel memory.max can transiently overshoot; observations do not prove exclusive family enforcement.",
        "storage_semantics": "Mount options or free space alone cannot prove all write paths are confined to one aggregate capped pool.",
    })
}

struct BoundedJson {
    storage: Vec<u8>,
    position: usize,
}

impl BoundedJson {
    fn new() -> Self {
        Self {
            storage: vec![0; REPORT_MAX_BYTES],
            position: 0,
        }
    }
}

impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .position
            .checked_add(bytes.len())
            .filter(|end| *end <= self.storage.len())
            .ok_or_else(|| std::io::Error::other("readiness report exceeded byte ceiling"))?;
        self.storage[self.position..end].copy_from_slice(bytes);
        self.position = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn encode_report(value: &Value) -> Result<String, String> {
    let mut output = BoundedJson::new();
    serde_json::to_writer_pretty(&mut output, value)
        .map_err(|err| format!("serialize bounded readiness report: {err}"))?;
    output
        .write_all(b"\n")
        .map_err(|err| format!("finish bounded readiness report: {err}"))?;
    output.storage.truncate(output.position);
    String::from_utf8(output.storage)
        .map_err(|_utf8_error| "readiness serializer emitted non-UTF8".to_string())
}

pub(crate) fn hard_enforcement_readiness_report() -> Result<String, String> {
    let read = |path: &str, limit: usize| {
        if std::env::consts::OS == "linux" {
            observe_file(path, limit)
        } else {
            Observation::unavailable("UNAVAILABLE_PLATFORM", "Linux observation required")
        }
    };
    let mut report = build_observation(std::env::consts::OS, read);
    report["compiled_observer_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(include_bytes!("hard_enforcement_readiness.rs"))
    ));
    let mut runner = serde_json::Map::new();
    for key in [
        "GITHUB_SHA",
        "GITHUB_RUN_ID",
        "GITHUB_RUN_ATTEMPT",
        "GITHUB_JOB",
        "RUNNER_OS",
        "RUNNER_ARCH",
        "ImageOS",
        "ImageVersion",
    ] {
        let value = match std::env::var(key) {
            Ok(value) if value.len() <= 128 => json!({"state": "ENV_DECLARED", "value": value}),
            Ok(_) => json!({"state": "OVERSIZED_ENV_REFUSED"}),
            Err(_) => json!({"state": "UNAVAILABLE"}),
        };
        runner.insert(key.to_string(), value);
    }
    report["runner_context"] = Value::Object(runner);
    report["source_identity_semantics"] = json!(
        "Compiled observer digest is exact. Environment SHA is declared context, not independently verified Git checkout identity."
    );
    encode_report(&report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn observed(text: &str) -> Observation {
        Observation {
            state: "OBSERVED",
            bytes: text.len(),
            sha256: Some(format!("{:x}", Sha256::digest(text.as_bytes()))),
            text: Some(text.to_string()),
            error_kind: None,
        }
    }

    #[test]
    fn exact_limits_do_not_authorize_a_family_or_trial() {
        let report = build_observation("linux", |path, _| match path {
            "/proc/self/cgroup" => observed("0::/runner/job\n"),
            "/proc/self/mountinfo" => {
                observed("31 20 0:27 / /sys/fs/cgroup rw - cgroup2 cgroup rw\n")
            }
            "/sys/fs/cgroup/runner/job/memory.max" => observed("8589934592\n"),
            "/sys/fs/cgroup/runner/job/memory.swap.max" => observed("0\n"),
            "/sys/fs/cgroup/runner/job/memory.oom.group" => observed("1\n"),
            _ => Observation::unavailable("MISSING", "NotFound"),
        });
        assert_eq!(report["status"], "NOT_READY");
        assert_eq!(report["full_trial"], "NOT_RUN");
        assert_eq!(
            report["observations"]["numeric_limit_states"]["memory_max"],
            "EXACT_VALUE_OBSERVED_NOT_EXCLUSIVE"
        );
        assert_eq!(
            report["observations"]["cgroup"]["control_write_authority"],
            "UNVERIFIED_READ_ONLY"
        );
        assert_eq!(report["required_proofs"][0]["state"], "NOT_ESTABLISHED");
    }

    #[test]
    fn absent_v2_and_unreadable_observations_stay_unverified() {
        let report = build_observation("linux", |_, _| {
            Observation::unavailable("UNREADABLE", "PermissionDenied")
        });
        assert_eq!(report["status"], "NOT_READY");
        assert_eq!(report["observations"]["cgroup"]["state"], "UNVERIFIED");
        assert_eq!(report["observations"]["membership"]["state"], "UNREADABLE");
        assert_eq!(report["observations"]["controls"], json!([]));
    }

    #[test]
    fn mount_root_translation_preserves_component_boundaries() -> Result<(), String> {
        let mapping = resolve_cgroup_mount(
            "31 20 0:27 /runner /sys/fs/cgroup rw - cgroup2 cgroup rw\n",
            "/runner/job",
        )?;
        assert_eq!(mapping.directory, "/sys/fs/cgroup/job");
        match resolve_cgroup_mount(
            "31 20 0:27 /runner /sys/fs/cgroup rw - cgroup2 cgroup rw\n",
            "/runner-other/job",
        ) {
            Err(error) => assert_eq!(error, "matching cgroup v2 mount absent"),
            Ok(_) => return Err("prefix collision was accepted".to_string()),
        }
        Ok(())
    }

    #[test]
    fn ambiguous_mapping_and_parent_components_are_refused() -> Result<(), String> {
        for (mounts, member, expected) in [
            (
                "31 20 0:27 / /a rw - cgroup2 cgroup rw\n32 20 0:27 / /b rw - cgroup2 cgroup rw\n",
                "/job",
                "ambiguous cgroup v2 mount mapping",
            ),
            (
                "31 20 0:27 / /a rw - cgroup2 cgroup rw\n",
                "/job/../escape",
                "unsupported cgroup membership",
            ),
        ] {
            match resolve_cgroup_mount(mounts, member) {
                Err(error) => assert_eq!(error, expected),
                Ok(_) => return Err("unsupported cgroup mapping was accepted".to_string()),
            }
        }
        match unified_membership("0::/a\n0::/b\n") {
            Err(error) => assert_eq!(error, "ambiguous cgroup v2 membership"),
            Ok(_) => return Err("ambiguous membership was accepted".to_string()),
        }
        Ok(())
    }

    #[test]
    fn unsupported_escape_and_malformed_row_are_refused() -> Result<(), String> {
        assert_eq!(decode_mount_field(r"/runner\040job")?, "/runner job");
        match decode_mount_field(r"/runner\377job") {
            Err(error) => assert_eq!(error, "unsupported mountinfo escape"),
            Ok(_) => return Err("unsupported mount escape accepted".to_string()),
        }
        match resolve_cgroup_mount("missing-fields\n", "/job") {
            Err(error) => assert_eq!(error, "malformed mountinfo row"),
            Ok(_) => return Err("malformed mount row accepted".to_string()),
        }
        Ok(())
    }

    #[test]
    fn read_and_report_caps_refuse_overflow() -> Result<(), String> {
        assert_eq!(read_limited(Cursor::new(b"abcd"), 4)?, b"abcd");
        match read_limited(Cursor::new(b"abcde"), 4) {
            Err(error) => assert_eq!(error, "readiness observation exceeded byte ceiling"),
            Ok(_) => return Err("oversized readiness read accepted".to_string()),
        }
        let mut output = BoundedJson::new();
        output
            .write_all(&vec![b'x'; REPORT_MAX_BYTES])
            .map_err(|err| err.to_string())?;
        match output.write_all(b"x") {
            Err(error) => assert_eq!(error.to_string(), "readiness report exceeded byte ceiling"),
            Ok(()) => return Err("oversized readiness output accepted".to_string()),
        }
        assert_eq!(output.position, REPORT_MAX_BYTES);
        assert_eq!(output.storage.len(), REPORT_MAX_BYTES);
        Ok(())
    }

    #[test]
    fn actual_serializer_enforces_escaped_control_and_exact_json_bounds() -> Result<(), String> {
        let escaping_control = "\t".repeat(CONTROL_MAX_BYTES);
        let report = build_observation("linux", |path, _| match path {
            "/proc/self/cgroup" => observed("0::/job\n"),
            "/proc/self/mountinfo" => {
                observed("31 20 0:27 / /sys/fs/cgroup rw - cgroup2 cgroup rw\n")
            }
            "/proc/sys/kernel/osrelease" => observed("test-kernel\n"),
            _ => observed(&escaping_control),
        });
        match encode_report(&report) {
            Err(error) => assert!(error.starts_with("serialize bounded readiness report:")),
            Ok(_) => {
                return Err(
                    "real readiness serializer accepted oversized escaped controls".to_string(),
                );
            }
        }
        let exact_value = Value::String("x".repeat(REPORT_MAX_BYTES - 3));
        let exact = encode_report(&exact_value)?;
        assert_eq!(exact.len(), REPORT_MAX_BYTES);
        let decoded: Value = serde_json::from_str(&exact).map_err(|err| err.to_string())?;
        assert_eq!(decoded, exact_value);
        match encode_report(&Value::String("x".repeat(REPORT_MAX_BYTES - 2))) {
            Err(error) => assert!(error.starts_with("finish bounded readiness report:")),
            Ok(_) => return Err("real readiness serializer accepted cap-plus-newline".to_string()),
        }
        Ok(())
    }

    #[test]
    fn truncated_cgroup2_mount_fields_are_refused() -> Result<(), String> {
        for (row, expected) in [
            ("31 20 0:27 / /a rw - cgroup2\n", "cgroup2 source absent"),
            (
                "31 20 0:27 / /a rw - cgroup2 cgroup\n",
                "cgroup2 super options absent",
            ),
            (
                "31 20 0:27 / /a - cgroup2 cgroup rw\n",
                "mountinfo mount options absent",
            ),
        ] {
            match resolve_cgroup_mount(row, "/job") {
                Err(error) => assert_eq!(error, expected),
                Ok(_) => return Err("truncated cgroup2 mount row accepted".to_string()),
            }
        }
        Ok(())
    }

    #[test]
    fn unlimited_and_malformed_limits_cannot_be_exact() {
        assert_eq!(
            numeric_limit(&observed("max\n"), REQUIRED_MEMORY_BYTES),
            "UNLIMITED_OBSERVED"
        );
        assert_eq!(
            numeric_limit(&observed("8589934593\n"), REQUIRED_MEMORY_BYTES),
            "DIFFERENT_VALUE_OBSERVED"
        );
        assert_eq!(
            numeric_limit(&observed("not-a-number\n"), REQUIRED_MEMORY_BYTES),
            "MALFORMED_VALUE"
        );
        assert_eq!(numeric_limit(&observed("1\n2\n"), 1), "UNVERIFIED");
    }

    #[test]
    fn bounded_report_retains_missing_proofs() -> Result<(), String> {
        let report = build_observation("windows", |_, _| {
            Observation::unavailable("UNAVAILABLE_PLATFORM", "Linux observation required")
        });
        let text = encode_report(&report)?;
        assert!(text.len() <= REPORT_MAX_BYTES);
        let parsed: Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
        assert_eq!(parsed["status"], "NOT_READY");
        assert_eq!(parsed["full_trial"], "NOT_RUN");
        assert_eq!(parsed["limits"]["settings_written"], 0);
        assert_eq!(parsed["limits"]["processes_launched"], 0);
        assert_eq!(parsed["required_proofs"][2]["state"], "NOT_ESTABLISHED");
        Ok(())
    }
}
