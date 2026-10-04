//! Process resource-cost observability (#5213).
//!
//! The opt-in latency trace family ([`LATENCY_TRACE_ENV`]) reports wall-clock
//! time only, so `ripr` could not falsify its own efficiency claims: "fast",
//! "bounded", "minimal working set" had no CPU or memory evidence behind
//! them. This module adds the missing observation and nothing else. It changes
//! no cost, no classification, no output document, and no exit code. (The
//! diagnostic report that consumes this receipt does change shape; see
//! `docs/OUTPUT_SCHEMA.md`.)
//!
//! ## What is observed
//!
//! One [`ResourceCostObservation`] of **this** `ripr` process, taken once
//! after the command has finished so the peak covers rendering and
//! serialization instead of stopping at the analysis boundary. It carries:
//!
//! * CPU time attributed to this process, split user/system, with the raw
//!   host value, the unit the host counted it in, and the rate used to reach
//!   milliseconds.
//! * Peak resident set size of this process, in bytes.
//! * The host identity (`host_os`, `host_arch`) and the observer scope, so a
//!   reader can tell these numbers belong to the analyzer and not to the
//!   harness that launched it.
//!
//! ## Honesty contract
//!
//! Every number is either observed or explicitly unavailable with a named
//! reason ([`ResourceMeasurement`], [`CpuCost`]). The two states are distinct
//! serialized shapes, so an unavailable observation cannot parse, render, or
//! aggregate as `0`, and nothing is silently omitted. A receipt that cannot be
//! serialized falls back to a hand-built body naming
//! [`ResourceUnavailableReason::ReceiptSerializationFailed`] for every number,
//! so it still parses and still reports nothing rather than disappearing.
//!
//! ## Platform support
//!
//! * **Linux** - observed. CPU time from `/proc/self/stat` `utime`+`stime`
//!   counted in `USER_HZ` clock ticks; peak resident set from
//!   `/proc/self/status` `VmHWM`. Both are plain file reads.
//! * **Windows** - observed. CPU time from
//!   `winsafe::HPROCESS::GetProcessTimes` (kernel time as system, user time as
//!   user, in 100-nanosecond intervals); peak resident set from
//!   `winsafe::HPROCESS::GetProcessMemoryInfo().PeakWorkingSetSize`, which the
//!   Windows API reports in bytes. Both are safe `fn` wrappers: the `unsafe`
//!   is inside `winsafe`, not here, which is what this crate's
//!   `unsafe_code = "forbid"` posture requires.
//! * **Every other target** - explicitly unavailable as
//!   [`ResourceUnavailableReason::PlatformNotSupported`]. No safe
//!   dependency-free per-process source is wired for those targets, and this
//!   crate does not add an FFI surface to manufacture one.
//!
//! Where a source exists but the read fails, the receipt says so with
//! [`ResourceUnavailableReason::SourceUnreadable`] (a file read) or
//! [`ResourceUnavailableReason::SourceQueryFailed`] (a platform query). It
//! never reports a capability as absent when it is present.
//!
//! ## Cost of the observation
//!
//! One bounded source read per metric, once per run, and one short JSON line
//! on stderr. With [`LATENCY_TRACE_ENV`] unset nothing is read and nothing is
//! printed, so default stdout JSON and default stderr are unchanged.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The opt-in stderr trace switch this module owns. Presence enables
/// tracing; the value is never read, so `RIPR_REPO_EXPOSURE_LATENCY_TRACE=0`
/// still enables it - the same contract the phase trace already had.
pub(crate) const LATENCY_TRACE_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TRACE";

/// Prefix of the single-line resource-cost receipt on stderr.
pub(crate) const RESOURCE_COST_RECEIPT_PREFIX: &str = "ripr_resource_cost_receipt ";

/// Schema version of the emitted resource-cost receipt.
pub(crate) const RESOURCE_COST_SCHEMA_VERSION: &str = "0.1";

/// Attribution string on every receipt. The numbers are read from inside the
/// analyzed `ripr` process; no harness measures them on this process's behalf.
pub(crate) const RESOURCE_COST_OBSERVER_SCOPE: &str = "ripr_process_self";

/// Whether the opt-in trace family is enabled for this process.
pub(crate) fn latency_trace_enabled() -> bool {
    std::env::var_os(LATENCY_TRACE_ENV).is_some()
}

/// Emit one wall-clock phase line. The single owner of that line shape for
/// every analysis phase, so a new phase cannot invent a parallel spelling.
pub(crate) fn trace_latency_phase(phase: &str, status: &str, duration: Duration) {
    if latency_trace_enabled() {
        eprintln!("{}", latency_trace_line(phase, status, duration));
    }
}

/// The wall-clock phase line, named by callers that only render it. Crate
/// visible so a consumer's test can assert the exact wire shape without
/// reading stderr.
pub(crate) fn latency_trace_line(phase: &str, status: &str, duration: Duration) -> String {
    format!(
        "ripr_repo_exposure_latency phase={phase} status={status} duration_ms={}",
        duration.as_millis()
    )
}

/// Why a resource number is not a measurement.
///
/// Each variant names what actually went wrong, so a reader can tell an absent
/// observation from a zero one and can tell which defect to chase. No variant
/// claims a capability is missing when it is present: the read-failure reasons
/// say the attempt happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResourceUnavailableReason {
    /// This build wires no safe, dependency-free per-process source for the
    /// host platform, and this crate does not add an FFI surface to
    /// manufacture one.
    PlatformNotSupported,
    /// The platform's per-process counter was queried and the query failed.
    /// The capability is present; the read did not succeed.
    SourceQueryFailed,
    /// The documented source file could not be read at all.
    SourceUnreadable,
    /// The documented source file was read but the expected field was absent.
    SourceFieldMissing,
    /// The expected field was present but could not be interpreted as its
    /// documented unit, or the interpreted value does not fit this crate's
    /// reporting range. Reporting a saturated substitute would present a
    /// number that was never measured.
    SourceValueMalformed,
    /// The receipt itself could not be serialized. Every number is
    /// unavailable for this reason rather than the receipt disappearing.
    ReceiptSerializationFailed,
}

/// The unit a host counts per-process CPU time in, and the documented number
/// of those units per second.
///
/// The rate is a property of the named source, not a measurement of this
/// process, and it travels in the receipt so a reader can recompute the
/// millisecond fields from the raw source fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CpuSourceUnit {
    /// Linux `/proc/self/stat` `utime` and `stime`, counted in `USER_HZ`
    /// clock ticks. `USER_HZ` is 100 on Linux (one tick is 10 ms), so 100
    /// source intervals per second.
    LinuxUserHzClockTicks,
    /// Windows `GetProcessTimes`, counted in 100-nanosecond intervals, so
    /// 10 000 000 intervals per second.
    WindowsHundredNanoseconds,
}

impl CpuSourceUnit {
    /// Source intervals per second for this unit.
    fn source_unit_per_second(self) -> u64 {
        match self {
            Self::LinuxUserHzClockTicks => 100,
            Self::WindowsHundredNanoseconds => 10_000_000,
        }
    }
}

/// CPU time attributed to this process, or the named reason it could not be.
///
/// User and kernel time come from one host source, so they are observed
/// together or not at all: a receipt that carried one without the other would
/// present a partial observation as a complete one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
// `deny_unknown_fields` so a receipt that smuggles a number into the
// `unavailable` arm is refused rather than silently dropped, matching the
// xtask consumer's own strictness on the same wire shape.
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CpuCost {
    /// Measured on this host, with the raw value and the rate kept so the
    /// millisecond fields are auditable rather than trusted.
    Observed {
        source_unit: CpuSourceUnit,
        source_unit_per_second: u64,
        user_source: u64,
        system_source: u64,
        user_ms: u64,
        system_ms: u64,
    },
    /// No usable per-process CPU reading here, with the reason named.
    Unavailable { reason: ResourceUnavailableReason },
}

impl CpuCost {
    /// Build an observed CPU cost, or refuse it when the conversion does not
    /// fit this crate's reporting range.
    ///
    /// Refusing beats saturating: `u64::MAX` inside an `Observed` arm would be
    /// a number this process never spent, presented as if it had.
    fn from_source(
        unit: CpuSourceUnit,
        user_source: u64,
        system_source: u64,
    ) -> Result<Self, ResourceUnavailableReason> {
        let source_unit_per_second = unit.source_unit_per_second();
        Ok(Self::Observed {
            source_unit: unit,
            source_unit_per_second,
            user_source,
            system_source,
            // Truncating the remainder keeps a sub-millisecond observation
            // visible as zero rather than rounding a cost up past what was
            // actually used.
            user_ms: source_to_ms(user_source, source_unit_per_second)?,
            system_ms: source_to_ms(system_source, source_unit_per_second)?,
        })
    }
}

fn source_to_ms(
    source: u64,
    source_unit_per_second: u64,
) -> Result<u64, ResourceUnavailableReason> {
    u128::from(source)
        .saturating_mul(1000)
        .checked_div(u128::from(source_unit_per_second.max(1)))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ResourceUnavailableReason::SourceValueMalformed)
}

/// One resource number: measured, or explicitly unavailable.
///
/// The two states are separate serialized shapes, so an unavailable
/// observation cannot round-trip, render, or aggregate as `0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
// `deny_unknown_fields` for the same reason as [`CpuCost`]: an unknown key
// beside a value is a producer defect that must not be read as agreement.
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ResourceMeasurement {
    /// Measured on this process by the host's documented safe source.
    Observed { value: u64 },
    /// Not measured here, with the reason named.
    Unavailable { reason: ResourceUnavailableReason },
}

/// Field 14 (`utime`) and field 15 (`stime`), 1-indexed within the whole
/// `/proc/self/stat` line.
///
/// `comm` (field 2) is parenthesized and may itself contain spaces and
/// parentheses, so the remaining fields are counted from the last `)`. After
/// it, `state` is field 3, which puts `utime` at offset 11 and `stime` at
/// offset 12 - 13 fields in total.
#[cfg(any(target_os = "linux", test))]
const LINUX_STAT_CPU_FIELD_COUNT: usize = 13;

#[cfg(any(target_os = "linux", test))]
const LINUX_STAT_USER_TICK_OFFSET: usize = LINUX_STAT_CPU_FIELD_COUNT - 2;

/// Parse `utime` and `stime` clock ticks out of a `/proc/self/stat` body.
///
/// A pure function over the file text, so the Linux parse path is covered by
/// tests on any host, not only on the host that produces the file.
#[cfg(any(target_os = "linux", test))]
fn parse_cpu_source_ticks(text: &str) -> Result<(u64, u64), ResourceUnavailableReason> {
    let Some(closing_paren) = text.rfind(')') else {
        return Err(ResourceUnavailableReason::SourceValueMalformed);
    };
    let after_comm = text
        .get(closing_paren.saturating_add(1)..)
        .unwrap_or_default();
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    if fields.len() < LINUX_STAT_CPU_FIELD_COUNT {
        return Err(ResourceUnavailableReason::SourceFieldMissing);
    }
    match (
        fields[LINUX_STAT_USER_TICK_OFFSET].parse::<u64>(),
        fields[LINUX_STAT_USER_TICK_OFFSET + 1].parse::<u64>(),
    ) {
        (Ok(user), Ok(system)) => Ok((user, system)),
        _ => Err(ResourceUnavailableReason::SourceValueMalformed),
    }
}

#[cfg(any(target_os = "linux", test))]
const LINUX_STATUS_PEAK_RSS_FIELD: &str = "VmHWM:";

#[cfg(any(target_os = "linux", test))]
const LINUX_STATUS_VALUE_UNIT: &str = "kB";

#[cfg(any(target_os = "linux", test))]
const BYTES_PER_KIBIBYTE: u64 = 1024;

/// Parse peak resident set bytes out of a `/proc/self/status` body.
///
/// `VmHWM` is the kernel's own high-water mark for this process's resident
/// set, reported in kibibytes. A pure function over the file text, so the
/// Linux parse path is covered by tests on any host.
#[cfg(any(target_os = "linux", test))]
fn parse_peak_resident_source_bytes(text: &str) -> Result<u64, ResourceUnavailableReason> {
    let Some(line) = text
        .lines()
        .find(|line| line.starts_with(LINUX_STATUS_PEAK_RSS_FIELD))
    else {
        return Err(ResourceUnavailableReason::SourceFieldMissing);
    };
    let rest = line
        .get(LINUX_STATUS_PEAK_RSS_FIELD.len()..)
        .unwrap_or_default();
    let mut parts = rest.split_whitespace();
    let (Some(amount), Some(unit), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(ResourceUnavailableReason::SourceValueMalformed);
    };
    if unit != LINUX_STATUS_VALUE_UNIT {
        return Err(ResourceUnavailableReason::SourceValueMalformed);
    }
    let kibibytes = amount
        .parse::<u64>()
        .map_err(|_not_a_count| ResourceUnavailableReason::SourceValueMalformed)?;
    kibibytes
        .checked_mul(BYTES_PER_KIBIBYTE)
        .ok_or(ResourceUnavailableReason::SourceValueMalformed)
}

/// This host's per-process CPU source: the unit, then user and kernel values
/// in that unit.
#[cfg(target_os = "linux")]
fn cpu_source() -> Result<(CpuSourceUnit, u64, u64), ResourceUnavailableReason> {
    let text = std::fs::read_to_string("/proc/self/stat")
        .map_err(|_unreadable| ResourceUnavailableReason::SourceUnreadable)?;
    let (user_source, system_source) = parse_cpu_source_ticks(&text)?;
    Ok((
        CpuSourceUnit::LinuxUserHzClockTicks,
        user_source,
        system_source,
    ))
}

/// This host's per-process peak resident set, in bytes.
#[cfg(target_os = "linux")]
fn peak_resident_source() -> Result<u64, ResourceUnavailableReason> {
    let text = std::fs::read_to_string("/proc/self/status")
        .map_err(|_unreadable| ResourceUnavailableReason::SourceUnreadable)?;
    parse_peak_resident_source_bytes(&text)
}

/// This host's per-process CPU source, from the safe `winsafe` wrappers.
///
/// `GetProcessTimes` returns `(creation, exit, kernel, user)`. Kernel time is
/// the system figure and user time is the user figure; both are 100-nanosecond
/// intervals. `HPROCESS::GetCurrentProcess()` returns the pseudo-handle for
/// this process and must not be closed.
#[cfg(target_os = "windows")]
fn cpu_source() -> Result<(CpuSourceUnit, u64, u64), ResourceUnavailableReason> {
    let (_creation, _exit, kernel, user) = winsafe::HPROCESS::GetCurrentProcess()
        .GetProcessTimes()
        .map_err(|_query_failed| ResourceUnavailableReason::SourceQueryFailed)?;
    Ok((
        CpuSourceUnit::WindowsHundredNanoseconds,
        u64::from(user),
        u64::from(kernel),
    ))
}

/// This host's per-process peak resident set, in bytes.
///
/// `PeakWorkingSetSize` is already in bytes; no kibibyte conversion applies on
/// this path.
#[cfg(target_os = "windows")]
fn peak_resident_source() -> Result<u64, ResourceUnavailableReason> {
    let counters = winsafe::HPROCESS::GetCurrentProcess()
        .GetProcessMemoryInfo()
        .map_err(|_query_failed| ResourceUnavailableReason::SourceQueryFailed)?;
    Ok(counters.PeakWorkingSetSize as u64)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn cpu_source() -> Result<(CpuSourceUnit, u64, u64), ResourceUnavailableReason> {
    Err(ResourceUnavailableReason::PlatformNotSupported)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn peak_resident_source() -> Result<u64, ResourceUnavailableReason> {
    Err(ResourceUnavailableReason::PlatformNotSupported)
}

/// CPU time of this process, or the named reason it could not be read.
pub(crate) fn observe_cpu_cost() -> CpuCost {
    let (unit, user_source, system_source) = match cpu_source() {
        Ok(source) => source,
        Err(reason) => return CpuCost::Unavailable { reason },
    };
    match CpuCost::from_source(unit, user_source, system_source) {
        Ok(observed) => observed,
        Err(reason) => CpuCost::Unavailable { reason },
    }
}

/// Peak resident set size of this process in bytes, or the named reason it
/// could not be read.
pub(crate) fn observe_peak_resident_bytes() -> ResourceMeasurement {
    match peak_resident_source() {
        Ok(value) => ResourceMeasurement::Observed { value },
        Err(reason) => ResourceMeasurement::Unavailable { reason },
    }
}

/// One resource-cost observation of this process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceCostObservation {
    pub(crate) schema_version: String,
    /// Always [`RESOURCE_COST_OBSERVER_SCOPE`]. Carried as data so a consumer
    /// that never sees the documentation still knows whose cost these are.
    pub(crate) observer: String,
    pub(crate) observer_pid: u32,
    pub(crate) host_os: String,
    pub(crate) host_arch: String,
    pub(crate) cpu: CpuCost,
    pub(crate) peak_resident_bytes: ResourceMeasurement,
}

impl ResourceCostObservation {
    /// Observe this process now.
    pub(crate) fn observe() -> Self {
        Self {
            schema_version: RESOURCE_COST_SCHEMA_VERSION.to_string(),
            observer: RESOURCE_COST_OBSERVER_SCOPE.to_string(),
            observer_pid: std::process::id(),
            host_os: std::env::consts::OS.to_string(),
            host_arch: std::env::consts::ARCH.to_string(),
            cpu: observe_cpu_cost(),
            peak_resident_bytes: observe_peak_resident_bytes(),
        }
    }

    /// Serialize the receipt body: the text after [`RESOURCE_COST_RECEIPT_PREFIX`].
    fn to_receipt_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|error| error.to_string())
    }
}

/// Parse a receipt body back into an observation.
///
/// The producer never reads its own receipt, so this exists to pin the wire
/// contract: the same derive pair that writes the line must read it back
/// unchanged, which is what makes the round trip a real discriminator rather
/// than a self-confirming serialize check. The xtask consumer parses the same
/// wire shape with its own independent mirror type, so a drift on either side
/// of the contract is caught rather than silently accepted.
#[cfg(test)]
fn parse_resource_cost_receipt(body: &str) -> Result<ResourceCostObservation, String> {
    let observation: ResourceCostObservation = serde_json::from_str(body)
        .map_err(|error| format!("parse resource-cost receipt: {error}"))?;
    if observation.schema_version != RESOURCE_COST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported resource-cost schema_version `{}`",
            observation.schema_version
        ));
    }
    Ok(observation)
}

/// Emit this run's resource cost on stderr when the trace family is enabled.
///
/// With [`LATENCY_TRACE_ENV`] unset this reads nothing and prints nothing, so
/// stdout JSON and default stderr stay byte-identical.
pub(crate) fn emit_run_resource_cost() {
    if !latency_trace_enabled() {
        return;
    }
    let observation = ResourceCostObservation::observe();
    match observation.to_receipt_json() {
        Ok(body) => eprintln!("{RESOURCE_COST_RECEIPT_PREFIX}{body}"),
        // The observation cannot be serialized. Emit a hand-built body that
        // names why, so the line still parses and still reports no number
        // rather than disappearing.
        Err(_) => eprintln!(
            "{RESOURCE_COST_RECEIPT_PREFIX}{}",
            receipt_serialization_failed_body(
                std::process::id(),
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        ),
    }
}

/// The receipt body emitted when the observation itself cannot be serialized.
///
/// Hand-built rather than re-serialized, so this path cannot itself fail: the
/// deepest fallback must not depend on the serializer that already failed.
/// Every number is [`ResourceUnavailableReason::ReceiptSerializationFailed`].
///
/// `host_os` and `host_arch` are `std::env::consts` values: compile-time
/// identifiers, so they need no JSON string escaping here.
fn receipt_serialization_failed_body(observer_pid: u32, host_os: &str, host_arch: &str) -> String {
    let unavailable = r#""state":"unavailable","reason":"receipt_serialization_failed""#;
    // Concatenated rather than joined with a `\` line continuation, which
    // `ra_ap_syntax` - the parser `check-public-api` uses to walk this crate's
    // module tree - does not accept inside a string literal.
    [
        format!(r#"{{"schema_version":"{RESOURCE_COST_SCHEMA_VERSION}","#),
        format!(r#""observer":"{RESOURCE_COST_OBSERVER_SCOPE}","#),
        format!(r#""observer_pid":{observer_pid},"#),
        format!(r#""host_os":"{host_os}","host_arch":"{host_arch}","#),
        format!(r#""cpu":{{{unavailable}}},"#),
        format!(r#""peak_resident_bytes":{{{unavailable}}}}}"#),
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::{
        BYTES_PER_KIBIBYTE, CpuCost, CpuSourceUnit, RESOURCE_COST_OBSERVER_SCOPE,
        RESOURCE_COST_SCHEMA_VERSION, ResourceCostObservation, ResourceMeasurement,
        ResourceUnavailableReason, latency_trace_line, observe_cpu_cost,
        observe_peak_resident_bytes, parse_cpu_source_ticks, parse_peak_resident_source_bytes,
        parse_resource_cost_receipt, receipt_serialization_failed_body, source_to_ms,
    };
    use std::time::{Duration, Instant};

    fn observation(host_os: &str) -> ResourceCostObservation {
        ResourceCostObservation {
            schema_version: RESOURCE_COST_SCHEMA_VERSION.to_string(),
            observer: RESOURCE_COST_OBSERVER_SCOPE.to_string(),
            observer_pid: 4242,
            host_os: host_os.to_string(),
            host_arch: "x86_64".to_string(),
            cpu: CpuCost::from_source(CpuSourceUnit::LinuxUserHzClockTicks, 1234, 56).unwrap_or(
                CpuCost::Unavailable {
                    reason: ResourceUnavailableReason::SourceValueMalformed,
                },
            ),
            peak_resident_bytes: ResourceMeasurement::Observed {
                value: 2048 * BYTES_PER_KIBIBYTE,
            },
        }
    }

    fn unavailable(host_os: &str) -> ResourceCostObservation {
        let reason = ResourceUnavailableReason::PlatformNotSupported;
        ResourceCostObservation {
            schema_version: RESOURCE_COST_SCHEMA_VERSION.to_string(),
            observer: RESOURCE_COST_OBSERVER_SCOPE.to_string(),
            observer_pid: 4242,
            host_os: host_os.to_string(),
            host_arch: "x86_64".to_string(),
            cpu: CpuCost::Unavailable { reason },
            peak_resident_bytes: ResourceMeasurement::Unavailable { reason },
        }
    }

    #[test]
    fn latency_trace_line_keeps_the_existing_phase_spelling() {
        assert_eq!(
            latency_trace_line("cache_load", "hit", Duration::from_millis(7)),
            "ripr_repo_exposure_latency phase=cache_load status=hit duration_ms=7"
        );
        assert_eq!(
            latency_trace_line(
                "file_fact_cache",
                "start_files_42_production_7",
                Duration::ZERO
            ),
            "ripr_repo_exposure_latency phase=file_fact_cache \
             status=start_files_42_production_7 duration_ms=0"
        );
    }

    #[test]
    fn observed_and_unavailable_receipts_round_trip_and_stay_distinct() -> Result<(), String> {
        // The raw source values must survive the wire so the millisecond
        // fields stay auditable instead of being trusted.
        let observed = observation("linux");
        let CpuCost::Observed {
            source_unit,
            source_unit_per_second,
            user_source,
            system_source,
            user_ms,
            system_ms,
        } = observed.cpu
        else {
            return Err("observed CPU must serialize as observed".to_string());
        };
        assert_eq!(source_unit, CpuSourceUnit::LinuxUserHzClockTicks);
        assert_eq!(source_unit_per_second, 100);
        assert_eq!((user_source, system_source), (1234, 56));
        assert_eq!((user_ms, system_ms), (12340, 560));

        let observed_body = observation("linux").to_receipt_json()?;
        assert_eq!(parse_resource_cost_receipt(&observed_body)?, observed);
        assert!(
            observed_body.contains(r#""state":"observed""#),
            "{observed_body}"
        );
        assert!(
            observed_body.contains(r#""observer":"ripr_process_self""#),
            "{observed_body}"
        );

        // An unavailable receipt round-trips into the unavailable state, is
        // not equal to a measurement, and carries no numeric field.
        let missing = unavailable("macos");
        let missing_body = missing.to_receipt_json()?;
        let round_tripped = parse_resource_cost_receipt(&missing_body)?;
        assert_eq!(round_tripped, missing);
        assert_ne!(round_tripped, observed);
        assert!(matches!(
            round_tripped.peak_resident_bytes,
            ResourceMeasurement::Unavailable { .. }
        ));
        let missing_value: serde_json::Value =
            serde_json::from_str(&missing_body).map_err(|error| error.to_string())?;
        for field in ["cpu", "peak_resident_bytes"] {
            let entry = missing_value
                .get(field)
                .ok_or_else(|| format!("{field} must be present: {missing_body}"))?;
            let object = entry
                .as_object()
                .ok_or_else(|| format!("{field} must be an object: {missing_body}"))?;
            assert_eq!(
                object.get("state").and_then(serde_json::Value::as_str),
                Some("unavailable"),
                "{field}: {missing_body}"
            );
            assert!(!object.contains_key("value"), "{field}: {missing_body}");
        }
        assert!(!missing_body.contains(r#""value""#), "{missing_body}");
        assert!(
            missing_body.contains(r#""reason":"platform_not_supported""#),
            "{missing_body}"
        );

        // A measured zero is still an observation and stays distinct.
        let zeroed = ResourceCostObservation {
            cpu: CpuCost::from_source(CpuSourceUnit::LinuxUserHzClockTicks, 0, 0)
                .map_err(|reason| format!("zero CPU cost must convert: {reason:?}"))?,
            peak_resident_bytes: ResourceMeasurement::Observed { value: 0 },
            ..observation("linux")
        };
        let zeroed_body = zeroed.to_receipt_json()?;
        assert_eq!(parse_resource_cost_receipt(&zeroed_body)?, zeroed);
        assert_ne!(zeroed, unavailable("macos"));
        assert_ne!(
            parse_resource_cost_receipt(&zeroed_body)?,
            parse_resource_cost_receipt(&missing_body)?
        );
        assert!(zeroed_body.contains(r#""value":0"#), "{zeroed_body}");
        Ok(())
    }

    #[test]
    fn receipt_rejects_an_unknown_schema_version() {
        let body = observation("linux")
            .to_receipt_json()
            .unwrap_or_default()
            .replace(RESOURCE_COST_SCHEMA_VERSION, "9.9");
        let error = parse_resource_cost_receipt(&body).err().unwrap_or_default();
        assert!(
            error.contains("unsupported resource-cost schema_version"),
            "unexpected error: {error}"
        );
    }

    /// A number smuggled into the *unavailable* CPU arm must be refused, not
    /// silently dropped. Without `deny_unknown_fields` on `CpuCost` this parses
    /// as `Unavailable` and the zero vanishes from view.
    #[test]
    fn receipt_parser_rejects_a_number_in_the_unavailable_cpu_arm() {
        let body = serde_json::json!({
            "schema_version": RESOURCE_COST_SCHEMA_VERSION,
            "observer": RESOURCE_COST_OBSERVER_SCOPE,
            "observer_pid": 4242,
            "host_os": "macos",
            "host_arch": "x86_64",
            "cpu": {
                "state": "unavailable",
                "reason": "platform_not_supported",
                "user_ms": 0,
            },
            "peak_resident_bytes": {"state": "unavailable", "reason": "platform_not_supported"},
        })
        .to_string();
        let error = parse_resource_cost_receipt(&body).err().unwrap_or_default();
        assert!(
            error.contains("parse resource-cost receipt"),
            "a number in the unavailable CPU arm must be refused: {body} -> {error}"
        );
        // The same shape without the smuggled number must parse, so the
        // rejection above is caused by that number and nothing else.
        let honest = serde_json::json!({
            "schema_version": RESOURCE_COST_SCHEMA_VERSION,
            "observer": RESOURCE_COST_OBSERVER_SCOPE,
            "observer_pid": 4242,
            "host_os": "macos",
            "host_arch": "x86_64",
            "cpu": {"state": "unavailable", "reason": "platform_not_supported"},
            "peak_resident_bytes": {"state": "unavailable", "reason": "platform_not_supported"},
        })
        .to_string();
        assert!(
            parse_resource_cost_receipt(&honest).is_ok(),
            "the honest shape must still parse: {honest}"
        );
    }

    /// An unknown key beside an observed value is a producer defect, not a
    /// field to read as agreement.
    #[test]
    fn receipt_parser_rejects_an_unknown_field_on_the_observed_measurement() {
        let body = serde_json::json!({
            "schema_version": RESOURCE_COST_SCHEMA_VERSION,
            "observer": RESOURCE_COST_OBSERVER_SCOPE,
            "observer_pid": 4242,
            "host_os": "linux",
            "host_arch": "x86_64",
            "cpu": {
                "state": "unavailable",
                "reason": "platform_not_supported",
            },
            "peak_resident_bytes": {"state": "observed", "value": 41, "bogus": 123},
        })
        .to_string();
        let error = parse_resource_cost_receipt(&body).err().unwrap_or_default();
        assert!(
            error.contains("parse resource-cost receipt"),
            "an unknown field beside a value must be refused: {body} -> {error}"
        );
    }

    /// The serialization-failure body must parse and must name no number. It
    /// is hand-built, so this is reachable without forcing a serializer
    /// failure.
    #[test]
    fn serialization_failed_body_is_parseable_and_carries_no_number() -> Result<(), String> {
        let body = receipt_serialization_failed_body(4242, "windows", "x86_64");
        let parsed = parse_resource_cost_receipt(&body)?;
        assert_eq!(parsed.observer_pid, 4242);
        assert_eq!(parsed.host_os, "windows");
        assert_eq!(parsed.host_arch, "x86_64");
        let expected = ResourceUnavailableReason::ReceiptSerializationFailed;
        assert_eq!(parsed.cpu, CpuCost::Unavailable { reason: expected });
        assert_eq!(
            parsed.peak_resident_bytes,
            ResourceMeasurement::Unavailable { reason: expected }
        );
        assert!(!body.contains(r#""value""#), "{body}");
        assert!(!body.is_empty());
        Ok(())
    }

    #[test]
    fn cpu_milliseconds_follow_the_named_source_divisor() -> Result<(), String> {
        // 10 000_000 hundred-nanosecond intervals is one second, so 1 000 ms.
        // 5 000 is half a millisecond, which truncation reports as 0 rather
        // than rounding the cost up past what was used.
        let windows =
            CpuCost::from_source(CpuSourceUnit::WindowsHundredNanoseconds, 10_000_000, 5_000)
                .map_err(|reason| format!("Windows CPU cost must convert: {reason:?}"))?;
        let CpuCost::Observed {
            source_unit_per_second,
            user_ms,
            system_ms,
            ..
        } = windows
        else {
            return Err("expected an observed CPU cost".to_string());
        };
        assert_eq!(source_unit_per_second, 10_000_000);
        assert_eq!((user_ms, system_ms), (1_000, 0));
        Ok(())
    }

    /// An unrepresentable conversion must be refused, not saturated into an
    /// `Observed` arm carrying a number this process never spent.
    #[test]
    fn unrepresentable_cpu_conversion_is_refused_rather_than_saturated() {
        // A one-interval-per-second source is the degenerate rate at which
        // `u64::MAX` overflows the millisecond range. It is not a real unit;
        // it isolates the overflow branch that a wrong rate would enter.
        assert_eq!(
            source_to_ms(u64::MAX, 1),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
        // 100 ticks at `USER_HZ` 100 is one second, so 1 000 ms.
        assert_eq!(source_to_ms(100, 100), Ok(1_000));
        // A real unit at an implausible tick count is also refused rather
        // than reported as a saturated measurement.
        assert_eq!(
            CpuCost::from_source(CpuSourceUnit::LinuxUserHzClockTicks, u64::MAX, 0),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
        // Both halves must survive the refusal: a receipt that reported user
        // time and dropped system time would be a partial observation.
        assert_eq!(
            CpuCost::from_source(CpuSourceUnit::LinuxUserHzClockTicks, 0, u64::MAX),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
    }

    /// A `/proc/self/stat` line with the given `comm` and the given
    /// `utime`/`stime` fields, spelled with the real field count so a change
    /// to the offset arithmetic shows up here rather than on a live host.
    fn stat_line(comm: &str, utime: &str, stime: &str) -> String {
        // Fields 3..13 after `comm`: state plus the ten fields before utime.
        format!("4242 ({comm}) S 1 1 1 0 0 0 0 0 0 0 {utime} {stime} 0 0")
    }

    #[test]
    fn cpu_source_parses_ticks_past_a_spacey_comm_field() -> Result<(), String> {
        // Field 2 is `comm`; a process named `my (weird) proc` proves the
        // count is taken from the last `)` rather than from whitespace.
        assert_eq!(
            parse_cpu_source_ticks(&stat_line("my (weird) proc", "4200", "137")),
            Ok((4200, 137))
        );
        assert_eq!(
            parse_cpu_source_ticks("no parens here"),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
        assert_eq!(
            parse_cpu_source_ticks("1 (short) S 1 2"),
            Err(ResourceUnavailableReason::SourceFieldMissing)
        );
        assert_eq!(
            parse_cpu_source_ticks(&stat_line("x", "notanumber", "137")),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
        // A negative tick count is not a clock tick count.
        assert_eq!(
            parse_cpu_source_ticks(&stat_line("x", "-1", "137")),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
        Ok(())
    }

    #[test]
    fn peak_resident_parses_kibibytes() -> Result<(), String> {
        let status =
            "Name:\tripr\nVmPeak:\t  204800 kB\nVmHWM:\t   40960 kB\nVmRSS:\t   10240 kB\n";
        assert_eq!(
            parse_peak_resident_source_bytes(status),
            Ok(40960 * BYTES_PER_KIBIBYTE)
        );
        // `VmRSS` is the current set, not the peak; reading it here would
        // understate the cost, so only `VmHWM` counts.
        assert_eq!(
            parse_peak_resident_source_bytes("Name:\tripr\nVmRSS:\t1 kB\n"),
            Err(ResourceUnavailableReason::SourceFieldMissing)
        );
        assert_eq!(
            parse_peak_resident_source_bytes("VmHWM:\n"),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
        assert_eq!(
            parse_peak_resident_source_bytes("VmHWM:\t40960 mB\n"),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
        // A value that cannot be represented in bytes must not wrap to zero.
        assert_eq!(
            parse_peak_resident_source_bytes(&format!("VmHWM:\t{}\tkB\n", u64::MAX)),
            Err(ResourceUnavailableReason::SourceValueMalformed)
        );
        Ok(())
    }

    #[test]
    fn every_host_names_its_wired_source_unit() -> Result<(), String> {
        // Linux and Windows are the wired hosts. Their unit is chosen by the
        // source implementation itself, so pin it per host here rather than
        // through a table no production path would consult.
        let expected_unit = match std::env::consts::OS {
            "linux" => Some(CpuSourceUnit::LinuxUserHzClockTicks),
            "windows" => Some(CpuSourceUnit::WindowsHundredNanoseconds),
            _ => None,
        };
        let Some(expected_unit) = expected_unit else {
            return Err(format!(
                "{} has no wired per-process source",
                std::env::consts::OS
            ));
        };
        let CpuCost::Observed { source_unit, .. } = observe_cpu_cost() else {
            return Err(format!(
                "{} must observe its own CPU time",
                std::env::consts::OS
            ));
        };
        assert_eq!(source_unit, expected_unit);
        Ok(())
    }

    /// Spend a measurable amount of CPU in this process.
    ///
    /// Windows accounts process CPU at kernel tick boundaries, so a test that
    /// observes immediately may read zero on a machine that has done no work yet.
    /// Burning first makes "a wired host reports non-zero CPU" deterministic
    /// rather than load-dependent.
    fn burn_cpu_briefly() {
        let deadline = Instant::now() + Duration::from_millis(30);
        let mut accumulator = 0u64;
        while Instant::now() < deadline {
            accumulator = accumulator
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
        }
        assert!(
            accumulator > 0,
            "the CPU burn loop must not be optimized away"
        );
    }

    /// The live observation must agree with the platform it ran on. A wired
    /// host must really produce numbers here.
    #[test]
    fn live_observation_matches_this_host() -> Result<(), String> {
        burn_cpu_briefly();
        let observation = ResourceCostObservation::observe();
        assert_eq!(observation.schema_version, RESOURCE_COST_SCHEMA_VERSION);
        assert_eq!(observation.observer, RESOURCE_COST_OBSERVER_SCOPE);
        assert_eq!(observation.observer_pid, std::process::id());
        assert_eq!(observation.host_os, std::env::consts::OS);

        let CpuCost::Observed {
            user_source,
            system_source,
            user_ms,
            ..
        } = observation.cpu
        else {
            return Err(format!(
                "{} must observe its own CPU time: {:?}",
                std::env::consts::OS,
                observation.cpu
            ));
        };
        let ResourceMeasurement::Observed { value } = observation.peak_resident_bytes else {
            return Err(format!(
                "{} must observe its own peak resident set: {:?}",
                std::env::consts::OS,
                observation.peak_resident_bytes
            ));
        };
        assert!(value > 0, "peak resident set must be a real byte count");
        // A process that just burned CPU must report non-zero CPU time in the
        // host's own source units. A hardcoded zero would pass the state
        // checks above; this rejects it.
        assert!(
            user_source + system_source > 0,
            "{} reported zero CPU source units after a deliberate CPU burn: {user_source} user, {system_source} system, {user_ms} ms user",
            std::env::consts::OS
        );
        Ok(())
    }

    #[test]
    fn observation_entry_points_are_callable_without_the_trace_switch() {
        // The observation is reachable without the trace switch so a test can
        // prove the capability without printing anything; the subprocess proof
        // that tracing off prints nothing lives in
        // `tests/resource_cost_trace.rs`, where the environment is isolated.
        let _ = observe_cpu_cost();
        let _ = observe_peak_resident_bytes();
    }
}
