//! Fixed ignored native worker; no outer capture or terminal owner is supplied here.
//!
//! Only a genuine finite native invocation with the authenticated startup header
//! can reach analysis. Witness bytes are DATA, never evidence or cleanup grants.
//! Outer/controller custody and the final uncertainty endpoint remain deferred.
#![cfg(all(test, target_os = "linux", feature = "lang-rust"))]

use super::complete_execution::with_libtest_worker;
use super::complete_image::ObservedSelfImage;
use super::complete_input::{NATIVE_ANALYSIS_PUBLICATION_REFUSAL, prepare};
use crate::analysis::capture_complete_rust_policy;
use crate::process_owner::ObservedProcessIdentity;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::Instant;

pub(super) const WORKER_TEST_NAME: &str =
    "app::pr_evidence::complete_native_test::native_analysis_worker";
const AS_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const FS_BYTES: u64 = 256 * 1024 * 1024;
const HEADER_CAP: usize = 64 * 1024;
const WITNESS_PREFIX: &str = "RIPR_NATIVE_ANALYSIS_WITNESS ";
const WITNESS_SCHEMA: &str = "ripr.native_analysis_test.v1";
const OWNER_LINES: usize = 7038;
const PREDICATE_LINE: usize = 7034;

fn checkpoint(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("native analysis test original deadline expired".into())
    } else {
        Ok(())
    }
}

fn supported() -> Result<(), String> {
    if cfg!(target_arch = "x86_64") {
        Ok(())
    } else {
        Err("native analysis test requires reviewed Linux x86_64 frozen semantics".into())
    }
}

fn decimal(value: &str) -> Result<u64, String> {
    if value.is_empty()
        || value.len() > 20
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("native analysis test scalar is not canonical positive decimal".into());
    }
    value
        .parse::<u64>()
        .map_err(|error| format!("native analysis test scalar: {error}"))
}

fn argv_prefix(
    actual: &mut impl Iterator<Item = std::ffi::OsString>,
    name: &str,
) -> Result<(), String> {
    if name != WORKER_TEST_NAME {
        return Err("native analysis test entry name is not fixed".into());
    }
    if actual.next().is_none() {
        return Err("native analysis test lacks actual argv0".into());
    }
    for expected in [
        "--ignored",
        "--exact",
        name,
        "--nocapture",
        "--test-threads=1",
    ] {
        if actual.next().as_deref() != Some(std::ffi::OsStr::new(expected)) {
            return Err("native analysis test requires its exact actual libtest entry".into());
        }
    }
    Ok(())
}

pub(super) fn authenticate_actual_libtest_argv(name: &str, args: &[String]) -> Result<(), String> {
    if args.len() != 4 {
        return Err("native analysis test requires four scalar filters".into());
    }
    let mut actual = std::env::args_os();
    argv_prefix(&mut actual, name)?;
    for scalar in args {
        decimal(scalar)?;
        if actual.next().as_deref() != Some(std::ffi::OsStr::new(scalar)) {
            return Err("native analysis test scalar differs from actual invocation".into());
        }
    }
    if actual.next().is_some() {
        return Err("native analysis test actual invocation has extra arguments".into());
    }
    Ok(())
}

fn actual_scalars(name: &str) -> Result<Vec<String>, String> {
    let mut actual = std::env::args_os();
    argv_prefix(&mut actual, name)?;
    let mut scalars = Vec::with_capacity(4);
    for _ in 0..4 {
        let scalar = actual
            .next()
            .ok_or("native analysis test lacks a scalar filter")?
            .into_string()
            .map_err(|error| format!("native analysis test non-UTF8 scalar: {error:?}"))?;
        decimal(&scalar)?;
        scalars.push(scalar);
    }
    if actual.next().is_some() {
        return Err("native analysis test actual invocation has extra arguments".into());
    }
    authenticate_actual_libtest_argv(name, &scalars)?;
    Ok(scalars)
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct ProcessData {
    pid: u32,
    parent: u32,
    group: u32,
    start: u64,
}

impl ProcessData {
    fn observed(actual: &ObservedProcessIdentity) -> Self {
        Self {
            pid: actual.pid(),
            parent: actual.parent(),
            group: actual.group(),
            start: actual.start(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Witness {
    schema_version: String,
    role: String,
    process: ProcessData,
    parent: ProcessData,
    worker: ProcessData,
    image_sha256: String,
    image_bytes: u64,
    build_identity: String,
    base: String,
    head: String,
    head_tree: String,
    changed_lines: usize,
    removed_lines: usize,
    predicate_line: usize,
    findings: usize,
    ordinary_limit: usize,
    postflight: bool,
}

struct BoundedBytes(Vec<u8>);
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .0
            .len()
            .checked_add(bytes.len())
            .filter(|next| *next <= HEADER_CAP)
            .ok_or_else(|| io::Error::other("native analysis test serialization cap exceeded"))?;
        self.0
            .try_reserve_exact(next - self.0.len())
            .map_err(io::Error::other)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bounded_json(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut sink = BoundedBytes(Vec::new());
    serde_json::to_writer(&mut sink, value)
        .map_err(|error| format!("native analysis test bounded serialization: {error}"))?;
    Ok(sink.0)
}

fn emit_witness(witness: &Witness, deadline: Instant) -> Result<(), String> {
    checkpoint(deadline)?;
    let bytes = bounded_json(witness)?;
    if bytes
        .len()
        .checked_add(WITNESS_PREFIX.len() + 2)
        .filter(|size| *size <= HEADER_CAP)
        .is_none()
    {
        return Err("native analysis test witness cap exceeded".into());
    }
    let mut output = std::io::stdout().lock();
    for part in [
        b"\n".as_slice(),
        WITNESS_PREFIX.as_bytes(),
        bytes.as_slice(),
        b"\n",
    ] {
        let mut offset = 0;
        while offset < part.len() {
            checkpoint(deadline)?;
            let result = output.write(&part[offset..]);
            checkpoint(deadline)?;
            match result {
                Ok(0) => return Err("native analysis test witness write made no progress".into()),
                Ok(count) if count <= part.len() - offset => offset += count,
                Ok(_) => return Err("native analysis test witness write count invalid".into()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(format!("native analysis test witness write: {error}")),
            }
        }
    }
    output
        .flush()
        .map_err(|error| format!("native analysis test witness flush: {error}"))?;
    checkpoint(deadline)
}

pub(super) fn witness(bytes: &[u8], role: &str) -> Result<Witness, String> {
    if bytes.len() > HEADER_CAP {
        return Err("native analysis test captured stdout exceeds witness cap".into());
    }
    let mut found = None;
    for line in bytes.split(|byte| *byte == b'\n') {
        if let Some(record) = line.strip_prefix(WITNESS_PREFIX.as_bytes()) {
            if found.is_some() {
                return Err("native analysis test duplicate witness".into());
            }
            let value: Witness = serde_json::from_slice(record)
                .map_err(|error| format!("native analysis test closed witness: {error}"))?;
            if value.schema_version != WITNESS_SCHEMA
                || value.role != role
                || !value.postflight
                || value.changed_lines != OWNER_LINES
                || value.removed_lines != 0
                || value.predicate_line != PREDICATE_LINE
                || value.findings == 0
                || value.ordinary_limit >= OWNER_LINES
                || value.image_bytes == 0
                || value.image_bytes > FS_BYTES
                || value.build_identity != crate::build_identity::cache_identity()
                || value.build_identity.contains("+process:")
                || !value.image_sha256.starts_with("sha256:")
                || value.image_sha256.len() != 71
            {
                return Err("native analysis test witness assertions differ".into());
            }
            found = Some(value);
        }
    }
    found.ok_or("native analysis test callback/postflight witness missing".into())
}

#[test]
#[ignore = "requires a genuine finite native parent and authenticated startup header"]
fn native_analysis_worker() -> Result<(), String> {
    supported()?;
    let scalars = actual_scalars(WORKER_TEST_NAME)?;
    with_libtest_worker(
        &scalars,
        |invocation, startup, request, options, surface, check, profile| {
            let deadline = startup.deadline();
            let mut image = ObservedSelfImage::observe(AS_BYTES, FS_BYTES, deadline)?;
            let process = ProcessData::observed(invocation.observed_worker());
            let parent = ProcessData::observed(invocation.observed_parent());
            let limit = capture_complete_rust_policy()?.changed_rust_line_limit();
            if limit >= OWNER_LINES {
                return Err("native analysis worker ordinary guard is not discriminated".into());
            }
            let fresh = prepare(
                invocation, startup, request, &options, surface, check, &profile,
            )?;
            let guard = fresh.observe_ordinary_guard()?;
            if !guard.contains("diff_scope_oversized: 7038 changed Rust lines across 1 Rust files")
                || !guard.contains(&format!("limit ({limit})"))
            {
                return Err(format!(
                    "native analysis ordinary guard observation differs: {guard}"
                ));
            }
            let analyzed = fresh.execute()?;
            let mut reached = None;
            let refusal = analyzed.inspect_then_refuse_publication(|output, data| {
                let range = format!(
                    "{}...{}",
                    data.subject().base_commit.as_str(),
                    data.subject().head_commit.as_str(),
                );
                let numstat =
                    crate::git::with_complete_capture_restriction(deadline, HEADER_CAP, || {
                        crate::git::run_git_output_with_deadline(
                            &data.subject().root,
                            &["diff", "--no-renames", "--numstat", &range],
                            None,
                        )
                    })
                    .map_err(|error| format!("native analysis numstat oracle: {error}"))?;
                if !numstat.status.success() || numstat.stdout != b"7038\t0\tsrc/lib.rs\n" {
                    return Err("native analysis actual whole-subject numstat differs".into());
                }
                drop(numstat);
                let summary = data.coverage().summary();
                if summary.added_lines != OWNER_LINES
                    || summary.removed_lines != 0
                    || summary.changed_files != 1
                    || data.changed_paths() != ["src/lib.rs"]
                    || output.partial_scope.is_some()
                    || !output.language_runs.is_empty()
                    || output.findings.is_empty()
                    || output.base.is_some()
                {
                    return Err("native analysis sealed output/raw denominator differs".into());
                }
                let parsed = crate::analysis::diff::parse_unified_diff_bounded_with_metadata(
                    data.canonical_diff(),
                )?;
                if parsed.changed_files.len() != 1
                    || parsed.changed_files[0].added_lines.len() != OWNER_LINES
                    || !parsed.changed_files[0].removed_lines.is_empty()
                {
                    return Err(
                        "native analysis actual canonical parser denominator differs".into(),
                    );
                }
                let outcome = output
                    .analysis_outcome
                    .as_ref()
                    .ok_or("native analysis outcome missing")?;
                if !outcome.kind.is_complete()
                    || outcome.counts.changed_line_count != OWNER_LINES as u64
                    || outcome.counts.finding_count == 0
                    || outcome.identity.base_revision.is_some()
                {
                    return Err("native analysis actual outcome is incomplete".into());
                }
                if !output.findings.iter().any(|finding| {
                    finding.probe.location.line == PREDICATE_LINE
                        && finding.probe.location.file == data.authority_root().join("src/lib.rs")
                        && finding
                            .probe
                            .owner
                            .as_ref()
                            .is_some_and(|owner| owner.0.ends_with("::beyond_boundary"))
                        && finding.probe.expression.contains("value > 5400")
                }) {
                    return Err(
                        "native analysis actual predicate finding at line 7034 missing".into(),
                    );
                }
                reached = Some(Witness {
                    schema_version: WITNESS_SCHEMA.into(),
                    role: "worker".into(),
                    process: process.clone(),
                    parent: parent.clone(),
                    worker: process.clone(),
                    image_sha256: image.sha256().into(),
                    image_bytes: image.byte_len(),
                    build_identity: data.build_identity().into(),
                    base: data.subject().base_commit.as_str().into(),
                    head: data.subject().head_commit.as_str().into(),
                    head_tree: data.subject().head_tree.as_str().into(),
                    changed_lines: summary.added_lines,
                    removed_lines: summary.removed_lines,
                    predicate_line: PREDICATE_LINE,
                    findings: output.findings.len(),
                    ordinary_limit: limit,
                    postflight: true,
                });
                Ok(())
            });
            match refusal {
                Err(error) if error == NATIVE_ANALYSIS_PUBLICATION_REFUSAL => {}
                Err(error) => return Err(format!("native analysis postflight failed: {error}")),
                Ok(()) => {
                    return Err("native analysis control unexpectedly granted publication".into());
                }
            }
            // A DATA witness is emitted only after the inspector AND genuine
            // consuming postflight/source finalization reached the exact refusal.
            image.recheck()?;
            let report = reached.ok_or("native analysis inspector was not reached")?;
            emit_witness(&report, deadline)
        },
    )
}

#[test]
fn native_analysis_scalar_grammar_refuses_aliases_and_missing_witness() -> Result<(), String> {
    for invalid in ["", "0", "01", "-1", "1 ", "18446744073709551616"] {
        match decimal(invalid) {
            Err(error) if error.contains("native analysis test scalar") => {}
            Err(error) => return Err(format!("native analysis scalar category differed: {error}")),
            Ok(_) => return Err("native analysis scalar grammar accepted an alias".into()),
        }
    }
    assert_eq!(decimal("65536")?, HEADER_CAP as u64);
    match witness(b"running 0 tests\ntest result: ok\n", "worker") {
        Err(error) if error.contains("callback/postflight witness missing") => Ok(()),
        Err(error) => Err(format!(
            "native analysis zero-test oracle differed: {error}"
        )),
        Ok(_) => Err("native analysis zero-test success was accepted".into()),
    }
}
