//! Fixed native worker and cooperating controller, with an owned bootstrap test.
//!
//! Only genuine finite native invocations and closed authenticated startup data
//! can reach analysis. Each caller retains its actual process owners through
//! failure and ordinary unwind. Witness bytes remain DATA; publication is refused.
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

#[derive(Clone, Copy)]
enum NativeTestEntry {
    Worker,
    Controller,
}

impl NativeTestEntry {
    fn name(self) -> &'static str {
        match self {
            Self::Worker => WORKER_TEST_NAME,
            Self::Controller => CONTROLLER_TEST_NAME,
        }
    }
}

fn entry_prefix(
    actual: &mut impl Iterator<Item = std::ffi::OsString>,
    entry: NativeTestEntry,
) -> Result<(), String> {
    if actual.next().is_none() {
        return Err("native analysis test lacks actual argv0".into());
    }
    for expected in [
        "--ignored",
        "--exact",
        entry.name(),
        "--nocapture",
        "--test-threads=1",
    ] {
        if actual.next().as_deref() != Some(std::ffi::OsStr::new(expected)) {
            return Err("native analysis test requires its exact actual libtest entry".into());
        }
    }
    Ok(())
}

fn argv_prefix(
    actual: &mut impl Iterator<Item = std::ffi::OsString>,
    name: &str,
) -> Result<(), String> {
    if name != WORKER_TEST_NAME {
        return Err("native analysis test entry name is not fixed".into());
    }
    entry_prefix(actual, NativeTestEntry::Worker)
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

fn actual_entry_scalars(entry: NativeTestEntry) -> Result<Vec<String>, String> {
    let mut actual = std::env::args_os();
    entry_prefix(&mut actual, entry)?;
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
    Ok(scalars)
}

fn actual_scalars(name: &str) -> Result<Vec<String>, String> {
    if name != WORKER_TEST_NAME {
        return Err("native analysis test entry name is not fixed".into());
    }
    let scalars = actual_entry_scalars(NativeTestEntry::Worker)?;
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


#[test]
fn native_controller_entry_cannot_authorize_worker_invocation() -> Result<(), String> {
    let scalars = [AS_BYTES, FS_BYTES, 1, HEADER_CAP as u64]
        .map(|value| value.to_string());
    match authenticate_actual_libtest_argv(CONTROLLER_TEST_NAME, &scalars) {
        Err(error) if error == "native analysis test entry name is not fixed" => {}
        Err(error) => {
            return Err(format!("native controller alias reached worker argv decoding: {error}"));
        }
        Ok(()) => return Err("native controller entry granted worker authentication".into()),
    }
    match actual_scalars(CONTROLLER_TEST_NAME) {
        Err(error) if error == "native analysis test entry name is not fixed" => Ok(()),
        Err(error) => Err(format!("native controller alias reached worker scalar decoding: {error}")),
        Ok(_) => Err("native controller entry granted worker scalar authentication".into()),
    }
}

// Same-process caller custody remains alive over each bounded physical step.
// Unknown identity or membership retains the original owners and first refusal.
use super::complete_contract::{CompleteVerificationLimits, ProducerSurface};
use super::complete_native::StageRootBinding;
use super::complete_profile::{PROFILE_NAME, profile_for_request};
use super::complete_request::{
    CommittedRequestBinding, CompleteRequest, CompleteSubject, POLICY_PATH, RequestedRoute,
    select_request_with_deadline,
};
use super::{PrEvidenceOptions, parse_options};
use crate::analysis::CompleteRustPolicySnapshot;
use crate::analysis::committed_source::staged::DirectoryIdentity;
use crate::process_owner::{
    CompleteByteCapture, CompleteCaptureBudget, CompleteCaptureReceipt,
    CompleteEnclosingCustodian, PhysicalStep,
    CompleteEnclosingPhysicalClosure, CompleteTerminalCustodian, NativeLimitTuple, ParentStage,
    StageBudget, StageDirectoryBinding, StageRoleBudget,
};
#[cfg(all(
    not(feature = "lang-typescript"),
    not(feature = "lang-python"),
    not(feature = "lang-perl")
))]
use crate::process_owner::{CompleteControllerTerminal, CompleteControllerTransport};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const FIXTURE_BYTES_MAX: usize = 1024 * 1024;
const CAPTURE_PHASE_MAX: Duration = Duration::from_mins(2);
const DISCRIMINATOR_SLEEP: Duration = Duration::from_secs(30);
const FIXED_STAGE: &str = "/var/tmp/ripr-complete-retained";

fn adapter_lock<T>(value: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    value
        .lock()
        .map_err(|error| format!("native adapter held owner lock: {error}"))
}

fn adapter_self(deadline: Instant) -> Result<ObservedProcessIdentity, String> {
    checkpoint(deadline)?;
    let observed = ObservedProcessIdentity::read(std::process::id())?;
    checkpoint(deadline)?;
    if observed.pid() != observed.group()
        || observed.start() == 0
        || matches!(observed.state(), 'Z' | 'X' | 'x')
    {
        return Err("native adapter requires an actual live finite group leader".into());
    }
    Ok(observed)
}

fn adapter_native(deadline: Instant) -> Result<(), String> {
    checkpoint(deadline)?;
    let actual = NativeLimitTuple::observe(deadline)?;
    if actual.address_space_bytes() != AS_BYTES || actual.file_size_bytes() != FS_BYTES {
        return Err("native adapter requires its actual finite native profile".into());
    }
    checkpoint(deadline)
}

fn same_process(actual: &ObservedProcessIdentity, expected: &ObservedProcessIdentity) -> bool {
    actual.pid() == expected.pid()
        && actual.parent() == expected.parent()
        && actual.group() == expected.group()
        && actual.start() == expected.start()
}

fn fixture_git(root: &Path, args: &[&str], deadline: Instant) -> Result<Vec<u8>, String> {
    checkpoint(deadline)?;
    let output = crate::git::with_complete_capture_restriction(deadline, HEADER_CAP, || {
        crate::git::run_git_output_with_deadline(root, args, None)
    })
    .map_err(|error| format!("native adapter fixture Git: {error}"))?;
    checkpoint(deadline)?;
    if !output.status.success() {
        return Err(format!(
            "native adapter fixture Git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output.stdout)
}

fn fixture_oid(root: &Path, deadline: Instant) -> Result<String, String> {
    let bytes = fixture_git(root, &["rev-parse", "--verify", "HEAD"], deadline)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("native adapter fixture OID UTF-8: {error}"))?;
    let literal = text
        .strip_suffix('\n')
        .ok_or("native adapter fixture OID has no original LF")?;
    let oid = crate::domain::GitObjectId::parse(literal)
        .map_err(|error| error.to_string())?;
    if oid.as_str() != literal {
        return Err("native adapter fixture OID spelling changed".into());
    }
    Ok(literal.into())
}

fn fixture_source(deadline: Instant) -> Result<Vec<u8>, String> {
    checkpoint(deadline)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(FIXTURE_BYTES_MAX)
        .map_err(|error| format!("native adapter owner reservation: {error}"))?;
    for line in 1..=OWNER_LINES {
        checkpoint(deadline)?;
        let text = match line {
            1 => "pub fn beyond_boundary(value: i32) -> bool {\n",
            PREDICATE_LINE => "    if value > 5400 {\n",
            7035 => "        return true;\n",
            7036 => "    }\n",
            7037 => "    false\n",
            7038 => "}\n",
            _ => "    // finite native owner context\n",
        };
        let next = bytes
            .len()
            .checked_add(text.len())
            .filter(|next| *next <= FIXTURE_BYTES_MAX)
            .ok_or("native adapter fixture source exceeds its bound")?;
        bytes.extend_from_slice(text.as_bytes());
        if bytes.len() != next {
            return Err("native adapter fixture source length changed".into());
        }
    }
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("native adapter literal owner UTF-8: {error}"))?;
    if source.lines().count() != OWNER_LINES
        || source.lines().nth(PREDICATE_LINE - 1) != Some("    if value > 5400 {")
    {
        return Err("native adapter literal owner coordinates differ".into());
    }
    checkpoint(deadline)?;
    Ok(bytes)
}

fn fixture_write(file: &mut File, bytes: &[u8], deadline: Instant) -> Result<(), String> {
    if bytes.len() > FIXTURE_BYTES_MAX {
        return Err("native adapter fixture write exceeds its bound".into());
    }
    for chunk in bytes.chunks(64 * 1024) {
        let mut offset = 0;
        while offset < chunk.len() {
            checkpoint(deadline)?;
            let written = file.write(&chunk[offset..]);
            checkpoint(deadline)?;
            match written {
                Ok(0) => return Err("native adapter fixture write made no progress".into()),
                Ok(count) if count <= chunk.len() - offset => offset += count,
                Ok(_) => return Err("native adapter fixture write count is invalid".into()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(format!("native adapter fixture write: {error}")),
            }
        }
    }
    file.sync_all()
        .map_err(|error| format!("native adapter fixture file sync: {error}"))?;
    checkpoint(deadline)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FixtureIdentity {
    device: u64,
    inode: u64,
    bytes: u64,
}

impl FixtureIdentity {
    fn file(file: &File, deadline: Instant) -> Result<Self, String> {
        checkpoint(deadline)?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("native adapter fixture metadata: {error}"))?;
        checkpoint(deadline)?;
        if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > FIXTURE_BYTES_MAX as u64 {
            return Err("native adapter fixture file identity is inadmissible".into());
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            bytes: metadata.len(),
        })
    }
}

struct FixtureFile {
    relative: &'static str,
    file: Mutex<File>,
    identity: FixtureIdentity,
    sha256: String,
}

impl FixtureFile {
    fn verify(&self, root: &Path, deadline: Instant) -> Result<(), String> {
        let mut file = adapter_lock(&self.file)?;
        if FixtureIdentity::file(&file, deadline)? != self.identity {
            return Err("native adapter retained fixture file changed".into());
        }
        checkpoint(deadline)?;
        let named = fs::symlink_metadata(root.join(self.relative))
            .map_err(|error| format!("native adapter fixture name metadata: {error}"))?;
        checkpoint(deadline)?;
        if !named.is_file()
            || named.nlink() != 1
            || named.dev() != self.identity.device
            || named.ino() != self.identity.inode
            || named.len() != self.identity.bytes
        {
            return Err("native adapter fixture name no longer binds its held file".into());
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("native adapter fixture hash seek: {error}"))?;
        let mut digest = Sha256::new();
        let mut bytes = 0_u64;
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            checkpoint(deadline)?;
            let read = file.read(&mut chunk);
            checkpoint(deadline)?;
            let count = match read {
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(format!("native adapter fixture hash read: {error}")),
            };
            if count == 0 {
                break;
            }
            bytes = bytes
                .checked_add(count as u64)
                .filter(|bytes| *bytes <= self.identity.bytes)
                .ok_or("native adapter fixture hash length exceeds its original file")?;
            digest.update(&chunk[..count]);
        }
        if bytes != self.identity.bytes
            || format!("{:x}", digest.finalize()) != self.sha256
            || FixtureIdentity::file(&file, deadline)? != self.identity
        {
            return Err("native adapter fixture full EOF bytes differ".into());
        }
        checkpoint(deadline)?;
        let after = fs::symlink_metadata(root.join(self.relative))
            .map_err(|error| format!("native adapter fixture final name metadata: {error}"))?;
        checkpoint(deadline)?;
        if !after.is_file() || after.nlink() != 1
            || after.dev() != self.identity.device || after.ino() != self.identity.inode
            || after.len() != self.identity.bytes
        {
            return Err("native adapter fixture final name no longer binds its held file".into());
        }
        Ok(())
    }
}

/// Closing these descriptors never deletes the committed fixture.
struct CommittedFixture {
    root: PathBuf,
    directory: File,
    device: u64,
    inode: u64,
    files: [FixtureFile; 4],
}

impl CommittedFixture {
    fn create(deadline: Instant) -> Result<(Self, String), String> {
        checkpoint(deadline)?;
        // Keep immediately, including every later failure path.
        let claimed = tempfile::Builder::new()
            .prefix("ripr-native-analysis-retained-")
            .tempdir()
            .map_err(|error| format!("native adapter fixture claim: {error}"))?
            .keep();
        checkpoint(deadline)?;
        let root = fs::canonicalize(&claimed)
            .map_err(|error| format!("native adapter fixture canonical root: {error}"))?;
        checkpoint(deadline)?;
        if root.to_str().is_none_or(|text| text.len() > 4096) {
            return Err("native adapter fixture root is not bounded UTF-8".into());
        }
        let directory = File::open(&root)
            .map_err(|error| format!("native adapter fixture directory descriptor: {error}"))?;
        let metadata = directory
            .metadata()
            .map_err(|error| format!("native adapter fixture root metadata: {error}"))?;
        checkpoint(deadline)?;
        if !metadata.is_dir() {
            return Err("native adapter fixture root is not a directory".into());
        }
        for relative in ["src", ".ripr"] {
            checkpoint(deadline)?;
            fs::create_dir(root.join(relative))
                .map_err(|error| format!("native adapter fixture directory: {error}"))?;
            checkpoint(deadline)?;
        }
        let literals = [
            ("Cargo.toml", b"[package]\nname = \"native_analysis_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n".as_slice()),
            ("ripr.toml", b"[languages]\nenabled = [\"rust\"]\n".as_slice()),
            (POLICY_PATH, b"{\"schema_version\":\"ripr.complete_request.v1\",\"request\":\"complete\",\"profile\":\"whole-head-v1\"}".as_slice()),
            ("src/lib.rs", b"".as_slice()),
        ];
        let mut opened: [Option<File>; 4] = std::array::from_fn(|_| None);
        for (index, (relative, bytes)) in literals.iter().enumerate() {
            checkpoint(deadline)?;
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(root.join(relative))
                .map_err(|error| format!("native adapter fixture create: {error}"))?;
            checkpoint(deadline)?;
            fixture_write(&mut file, bytes, deadline)?;
            opened[index] = Some(file);
        }
        fixture_git(&root, &["-c", "init.templateDir=", "init", "--quiet", "-b", "request"], deadline)?;
        fixture_git(&root, &["config", "--local", "user.name", "RIPR native analysis fixture"], deadline)?;
        fixture_git(&root, &["config", "--local", "user.email", "native@example.invalid"], deadline)?;
        fixture_git(&root, &["config", "--local", "commit.gpgsign", "false"], deadline)?;
        fixture_git(&root, &["add", "--all"], deadline)?;
        fixture_git(&root, &["commit", "--quiet", "-m", "native analysis base"], deadline)?;
        let base = fixture_oid(&root, deadline)?;
        let source = fixture_source(deadline)?;
        fixture_write(
            opened[3].as_mut().ok_or("native adapter owner descriptor missing")?,
            &source,
            deadline,
        )?;
        let source_hash = format!("{:x}", Sha256::digest(&source));
        drop(source);
        fixture_git(&root, &["add", "--", "src/lib.rs"], deadline)?;
        fixture_git(&root, &["commit", "--quiet", "-m", "one 7038 line owner"], deadline)?;
        let range = format!("{base}...HEAD");
        if fixture_git(&root, &["diff", "--no-renames", "--numstat", &range], deadline)?
            != b"7038\t0\tsrc/lib.rs\n"
        {
            return Err("native adapter full fixture numstat differs from 7038/0".into());
        }
        let mut identities: [Option<FixtureFile>; 4] = std::array::from_fn(|_| None);
        for (index, (relative, bytes)) in literals.iter().enumerate() {
            let file = opened[index].take().ok_or("native adapter fixture descriptor missing")?;
            identities[index] = Some(FixtureFile {
                relative,
                identity: FixtureIdentity::file(&file, deadline)?,
                sha256: if index == 3 {
                    source_hash.clone()
                } else {
                    format!("{:x}", Sha256::digest(bytes))
                },
                file: Mutex::new(file),
            });
        }
        let fixture = Self {
            root,
            directory,
            device: metadata.dev(),
            inode: metadata.ino(),
            files: [
                identities[0].take().ok_or("native adapter Cargo identity missing")?,
                identities[1].take().ok_or("native adapter config identity missing")?,
                identities[2].take().ok_or("native adapter request identity missing")?,
                identities[3].take().ok_or("native adapter owner identity missing")?,
            ],
        };
        fixture.verify(deadline)?;
        Ok((fixture, base))
    }

    fn verify(&self, deadline: Instant) -> Result<(), String> {
        checkpoint(deadline)?;
        let held = self.directory.metadata()
            .map_err(|error| format!("native adapter fixture held directory: {error}"))?;
        let named = fs::symlink_metadata(&self.root)
            .map_err(|error| format!("native adapter fixture directory name: {error}"))?;
        checkpoint(deadline)?;
        if !held.is_dir() || !named.is_dir()
            || held.dev() != self.device || named.dev() != self.device
            || held.ino() != self.inode || named.ino() != self.inode
        {
            return Err("native adapter fixture directory identity changed".into());
        }
        for file in &self.files {
            file.verify(&self.root, deadline)?;
        }
        checkpoint(deadline)
    }
}

fn adapter_stage_budget() -> StageBudget {
    let source = StageRoleBudget {
        max_files: 32,
        max_directories: 32,
        max_name_bytes: 16 * 1024,
        max_depth: 7,
        max_logical_bytes: 2 * 1024 * 1024,
        max_allocated_bytes: 8 * 1024 * 1024,
    };
    let empty = StageRoleBudget {
        max_files: 0,
        max_directories: 1,
        ..source
    };
    StageBudget {
        max_files: 32,
        max_directories: 36,
        max_name_bytes: 32 * 1024,
        max_depth: 8,
        max_logical_bytes: 2 * 1024 * 1024,
        max_allocated_bytes: 8 * 1024 * 1024,
        source,
        spool: empty,
        artifacts: empty,
        inventory_timeout: Duration::from_secs(5),
    }
}

fn adapter_directory(binding: &StageDirectoryBinding) -> DirectoryIdentity {
    DirectoryIdentity {
        path: binding.path.clone(),
        dev: binding.dev,
        ino: binding.ino,
    }
}

#[derive(Serialize)]
struct AdapterParent {
    pid: u32,
    start: u64,
    group: u32,
    address_space_bytes: u64,
    file_bytes: u64,
}

#[derive(Serialize)]
struct AdapterSubject<'a> {
    requested_root: &'a str,
    requested_base: &'a str,
    requested_head: &'a str,
    invocation_repository: &'a str,
    logical_root: &'a str,
    work_tree: &'a str,
    base_commit: &'a str,
    head_commit: &'a str,
    base_tree: &'a str,
    head_tree: &'a str,
    origin_commit: &'a str,
    origin_tree: &'a str,
}

#[derive(Serialize)]
struct AdapterHeader<'a> {
    schema_version: &'a str,
    profile: &'a CompleteVerificationLimits,
    stage: &'a StageRootBinding,
    generation_nonce: &'a str,
    parent: AdapterParent,
    build_identity: &'a str,
    surface: ProducerSurface,
    producer_args: &'a [String],
    committed_request: &'a CommittedRequestBinding,
    subject_commitment: AdapterSubject<'a>,
}

fn adapter_path(path: &Path) -> Result<&str, String> {
    path.to_str()
        .filter(|path| path.len() <= 4096)
        .ok_or_else(|| "native adapter original path exceeds bounded UTF-8".into())
}

fn adapter_sum(values: &[u64], maximum: u64) -> Result<u64, String> {
    values.iter().try_fold(0_u64, |sum, value| {
        sum.checked_add(*value)
            .filter(|sum| *sum <= maximum)
            .ok_or_else(|| "native adapter simultaneous logical reservation exceeds profile".into())
    })
}

struct NativeResources {
    image: Mutex<ObservedSelfImage>,
    stage: ParentStage,
    fixture: CommittedFixture,
    controller: ObservedProcessIdentity,
    profile: CompleteVerificationLimits,
    policy: CompleteRustPolicySnapshot,
    options: PrEvidenceOptions,
    subject: CompleteSubject,
    request: Mutex<Option<CompleteRequest>>,
    committed_request: CommittedRequestBinding,
    producer_args: Vec<String>,
    binding: StageRootBinding,
    generation_nonce: String,
    build: String,
    enclosing_deadline: Instant,
}

impl NativeResources {
    fn verify(&self, empty: bool) -> Result<(), String> {
        let u = self.enclosing_deadline;
        adapter_native(u)?;
        if !same_process(&adapter_self(u)?, &self.controller)
            || capture_complete_rust_policy()? != self.policy
            || crate::build_identity::cache_identity() != self.build
        {
            return Err("native adapter parent/policy/build changed".into());
        }
        self.fixture.verify(u)?;
        let actual = self.stage.worker_binding()?;
        if actual.nonce != self.binding.stage_nonce
            || adapter_directory(&actual.stage) != self.binding.stage
            || adapter_directory(&actual.source) != self.binding.source
            || adapter_directory(&actual.spool) != self.binding.spool
            || adapter_directory(&actual.artifacts) != self.binding.artifacts
        {
            return Err("native adapter actual parent stage binding changed".into());
        }
        let inventory = self.stage.audit_closed_inventory_with_deadline(u)?;
        if empty && (inventory.aggregate.files != 0 || inventory.aggregate.directories != 4) {
            return Err("native adapter pre-analysis stage is not exactly empty".into());
        }
        // This bounded observed inventory is not membership or quiescence authority.
        adapter_lock(&self.image)?.recheck()?;
        checkpoint(u)
    }

    fn header(&self) -> Result<Vec<u8>, String> {
        checkpoint(self.enclosing_deadline)?;
        let subject = &self.subject;
        let header = AdapterHeader {
            schema_version: "ripr.whole_worker_startup.v1",
            profile: &self.profile,
            stage: &self.binding,
            generation_nonce: &self.generation_nonce,
            parent: AdapterParent {
                pid: self.controller.pid(),
                start: self.controller.start(),
                group: self.controller.group(),
                address_space_bytes: AS_BYTES,
                file_bytes: FS_BYTES,
            },
            build_identity: &self.build,
            surface: ProducerSurface::Installed,
            producer_args: &self.producer_args,
            committed_request: &self.committed_request,
            subject_commitment: AdapterSubject {
                requested_root: &self.options.root,
                requested_base: &self.options.base,
                requested_head: &self.options.head,
                invocation_repository: adapter_path(&subject.invocation_repository)?,
                logical_root: adapter_path(&subject.root)?,
                work_tree: adapter_path(&subject.work_tree)?,
                base_commit: subject.base_commit.as_str(),
                head_commit: subject.head_commit.as_str(),
                base_tree: subject.base_tree.as_str(),
                head_tree: subject.head_tree.as_str(),
                origin_commit: subject.origin_commit.as_str(),
                origin_tree: subject.origin_tree.as_str(),
            },
        };
        let bytes = bounded_json(&header)?;
        checkpoint(self.enclosing_deadline)?;
        Ok(bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativePhase {
    Discard,
    Unwind,
    Analyze,
}

impl NativePhase {
    fn index(self) -> usize {
        match self {
            Self::Discard => 0,
            Self::Unwind => 1,
            Self::Analyze => 2,
        }
    }
}

struct NativePhaseLease {
    resources: Arc<NativeResources>,
    phase: NativePhase,
    original_deadline: Instant,
    enclosing_deadline: Instant,
}

pub(super) struct PreparedNative {
    resources: Arc<NativeResources>,
    next: usize,
}

pub(super) struct NativePhaseSlot {
    lease: Arc<NativePhaseLease>,
    enclosing: Option<CompleteEnclosingCustodian<NativePhaseLease>>,
    execution: PathBuf,
    args: Vec<String>,
    input: Option<Vec<u8>>,
    started: Instant,
    execution_deadline: Option<Instant>,
    timeout: Duration,
    refusal: Option<String>,
    unexpected_unwind: Option<Box<dyn std::any::Any + Send>>,
    attempted: bool,
}

pub(super) struct NativePhaseSlots {
    slots: [NativePhaseSlot; 3],
}

impl NativePhaseSlots {
    pub(super) fn phase(&mut self, phase: NativePhase) -> &mut NativePhaseSlot {
        &mut self.slots[phase.index()]
    }
}

fn adapter_slot(
    resources: &Arc<NativeResources>,
    phase: NativePhase,
    original: Instant,
    started: Instant,
    execution_deadline: Option<Instant>,
    identifier: &Path,
    header: &[u8],
) -> Result<NativePhaseSlot, String> {
    let u = resources.enclosing_deadline;
    let lease = Arc::new(NativePhaseLease {
        resources: Arc::clone(resources),
        phase,
        original_deadline: original,
        enclosing_deadline: u,
    });
    let mut inner = Some(CompleteTerminalCustodian::new(original, Arc::clone(&lease))?);
    let enclosing = CompleteEnclosingCustodian::admit(&mut inner, u)
        .map_err(|error| format!("native adapter enclosing admission: {error}"))?;
    let (execution, args, input, timeout) = if phase == NativePhase::Analyze {
        let e = execution_deadline.ok_or("native adapter Analyze cutoff missing")?;
        let remaining = e.checked_duration_since(Instant::now())
            .ok_or("native adapter Analyze original cutoff expired")?;
        let millis = u64::try_from(remaining.as_millis())
            .map_err(|error| format!("native adapter child scalar duration: {error}"))?;
        if millis == 0 || millis > resources.profile.deadline_ms {
            return Err("native adapter child original scalar deadline inadmissible".into());
        }
        let args = vec![
            format!("--as={AS_BYTES}:{AS_BYTES}"),
            format!("--fsize={FS_BYTES}:{FS_BYTES}"),
            "--core=0:0".into(),
            "--".into(),
            adapter_path(identifier)?.into(),
            "--ignored".into(),
            "--exact".into(),
            WORKER_TEST_NAME.into(),
            "--nocapture".into(),
            "--test-threads=1".into(),
            AS_BYTES.to_string(),
            FS_BYTES.to_string(),
            millis.to_string(),
            HEADER_CAP.to_string(),
        ];
        (
            PathBuf::from("/usr/bin/prlimit"),
            args,
            Some(header.to_vec()),
            CAPTURE_PHASE_MAX,
        )
    } else {
        (
            PathBuf::from("/usr/bin/sleep"),
            vec![DISCRIMINATOR_SLEEP.as_secs().to_string()],
            None,
            DISCRIMINATOR_SLEEP,
        )
    };
    checkpoint(u)?;
    Ok(NativePhaseSlot {
        lease,
        enclosing: Some(enclosing),
        execution,
        args,
        input,
        started,
        execution_deadline,
        timeout,
        refusal: None,
        unexpected_unwind: None,
        attempted: false,
    })
}

/// Preparation performs actual observations, but launches no capture phase.
/// The caller owns both returned values for every later outcome.
pub(super) fn prepare_native(
    entered: Instant,
    enclosing_deadline: Instant,
) -> Result<(PreparedNative, NativePhaseSlots), String> {
    supported()?;
    let admitted = enclosing_deadline.checked_duration_since(entered)
        .filter(|duration| !duration.is_zero() && *duration <= Duration::from_mins(5))
        .ok_or("native adapter enclosing admission exceeds original named ceiling")?;
    if entered > Instant::now() || admitted.is_zero() {
        return Err("native adapter original entry instant is invalid".into());
    }
    adapter_native(enclosing_deadline)?;
    let controller = adapter_self(enclosing_deadline)?;
    let image = ObservedSelfImage::observe(AS_BYTES, FS_BYTES, enclosing_deadline)?;
    let policy = capture_complete_rust_policy()?;
    if policy.changed_rust_line_limit() >= OWNER_LINES {
        return Err("native adapter fixture does not discriminate its actual ordinary guard".into());
    }
    let profile = profile_for_request(PROFILE_NAME, &policy)?;
    // Startup and decoded startup, phase headers, sentinel-bounded streams,
    // image hash scratch, witnesses, fixed arguments and first diagnostics are
    // simultaneous bounded logical payloads. Native AS authority is observed
    // separately; this reservation does not claim exact allocator or RSS usage.
    adapter_sum(
        &[
            10 * HEADER_CAP as u64,
            2,
            8 * CONTROLLER_DIAGNOSTIC_CAP as u64,
            FIXTURE_BYTES_MAX as u64,
            profile.max_binding_bytes,
            std::mem::size_of::<NativeController>() as u64,
            3 * std::mem::size_of::<NativeClosedFailure>() as u64,
        ],
        profile.max_buffered_bytes,
    )?;
    let (fixture, base) = CommittedFixture::create(enclosing_deadline)?;
    let producer_args = vec![
        "--root".into(), ".".into(), "--base".into(), base, "--head".into(), "HEAD".into(),
    ];
    let options = parse_options(&producer_args)?;
    let mut request = match select_request_with_deadline(
        &fixture.root, &options.head, enclosing_deadline,
    )? {
        RequestedRoute::Complete(request) => *request,
        RequestedRoute::Ordinary => return Err("native adapter committed request absent".into()),
    };
    let subject = request.resolve_whole_subject(&options)?;
    if subject.invocation_repository.as_os_str() != subject.root.as_os_str()
        || subject.root.as_os_str() != subject.work_tree.as_os_str()
        || subject.root.as_os_str() != fixture.root.as_os_str()
    {
        return Err("native adapter requires original whole fixture invocation".into());
    }
    let build = crate::build_identity::cache_identity();
    if build.contains("+process:") || build.len() > 4096 || build.contains('\0') {
        return Err("native adapter stable compiled build identity unavailable".into());
    }
    let seed = format!("native-adapter:{}:{}:{:?}", controller.pid(), controller.start(), entered);
    let first = format!("{:x}", Sha256::digest(seed.as_bytes()));
    let second = format!("{:x}", Sha256::digest(first.as_bytes()));
    let stage_nonce = format!("{first}{second}");
    let generation_nonce = first[..32].to_string();
    let stage = ParentStage::claim_retained(
        &stage_nonce, adapter_stage_budget(), enclosing_deadline, AS_BYTES, FS_BYTES,
    )?;
    let actual = stage.worker_binding()?;
    let binding = StageRootBinding {
        stage_nonce: actual.nonce,
        stage: adapter_directory(&actual.stage),
        source: adapter_directory(&actual.source),
        spool: adapter_directory(&actual.spool),
        artifacts: adapter_directory(&actual.artifacts),
    };
    if binding.stage.path != FIXED_STAGE || binding.stage_nonce != stage_nonce {
        return Err("native adapter parent fixed stage identity differs".into());
    }
    let committed_request = request.binding().clone();
    let resources = Arc::new(NativeResources {
        image: Mutex::new(image),
        stage,
        fixture,
        controller,
        profile,
        policy,
        options,
        subject,
        request: Mutex::new(Some(request)),
        committed_request,
        producer_args,
        binding,
        generation_nonce,
        build: build.into(),
        enclosing_deadline,
    });
    resources.verify(true)?;
    // All three destinations and their unique leases are admitted before the
    // first captured phase. Each original T is immutable and independent, under SAME U.
    let identifier = adapter_lock(&resources.image)?.execution_identifier()?;
    let header = resources.header()?;
    let started = Instant::now();
    let discard_t = started.checked_add(Duration::from_secs(1))
        .ok_or("native adapter discriminator original clock overflow")?;
    let unwind_t = started.checked_add(Duration::from_secs(12))
        .ok_or("native adapter unwind original clock overflow")?;
    let analyze_t = enclosing_deadline.checked_sub(Duration::from_secs(1))
        .ok_or("native adapter Analyze original clock underflow")?;
    let latest_e = analyze_t.checked_sub(Duration::from_secs(7))
        .ok_or("native adapter Analyze original reserve underflow")?;
    let e = started.checked_add(CAPTURE_PHASE_MAX)
        .ok_or("native adapter Analyze execution overflow")?.min(latest_e);
    if e <= unwind_t.checked_add(Duration::from_secs(5))
        .ok_or("native adapter discriminator reserve overflow")?
    {
        return Err("native adapter original enclosing budget cannot admit all phases".into());
    }
    let slots = [
        adapter_slot(&resources, NativePhase::Discard, discard_t, started, None, &identifier, &header)?,
        adapter_slot(&resources, NativePhase::Unwind, unwind_t, started, None, &identifier, &header)?,
        adapter_slot(&resources, NativePhase::Analyze, analyze_t, started, Some(e), &identifier, &header)?,
    ];
    Ok((
        PreparedNative { resources, next: 0 },
        NativePhaseSlots { slots },
    ))
}

enum ActualNativeClosure {
    Combined {
        receipt: CompleteCaptureReceipt,
        status: std::process::ExitStatus,
    },
    Physical(Box<CompleteEnclosingPhysicalClosure<NativePhaseLease>>),
}

pub(super) struct NativeClosedFailure {
    closure: ActualNativeClosure,
    diagnostic: Option<String>,
    discriminator: Option<ObservedProcessIdentity>,
}

impl NativeClosedFailure {
    pub(super) fn message(&self) -> &str {
        match &self.closure {
            ActualNativeClosure::Physical(physical) => physical.message(),
            ActualNativeClosure::Combined { receipt, status } => {
                if receipt.settled_status() != Some(*status) {
                    "native adapter actual closed transport status differs"
                } else {
                    self.diagnostic.as_deref()
                        .unwrap_or("native adapter closed transport failed analysis validation")
                }
            },
        }
    }

    pub(super) fn secondary(&self) -> Option<&str> {
        self.diagnostic.as_deref()
    }

    pub(super) fn discriminator(&self) -> Option<&ObservedProcessIdentity> {
        self.discriminator.as_ref()
    }
}

pub(super) struct NativeAccepted {
    receipt: CompleteCaptureReceipt,
    witness: Witness,
}

impl NativeAccepted {
    pub(super) fn witness(&self) -> &Witness {
        &self.witness
    }

    pub(super) fn receipt(&self) -> &CompleteCaptureReceipt {
        &self.receipt
    }
}

/// Retained is a borrow of the SAME caller slot, never an owned diagnostic that
/// can replace the actual child/helper/image/stage/fixture custody.
pub(super) enum NativePhaseOutcome<'a> {
    Accepted(Box<NativeAccepted>),
    PhysicallyClosedFailure(Box<NativeClosedFailure>),
    Retained(&'a mut NativePhaseSlot),
}

impl NativePhaseSlot {
    fn remember_refusal(&mut self, error: String) {
        if self.refusal.is_none() {
            self.refusal = Some(error);
        }
    }

    pub(super) fn retained_message(&self) -> &str {
        self.enclosing.as_ref()
            .and_then(CompleteEnclosingCustodian::inner)
            .and_then(CompleteTerminalCustodian::failure)
            .map_or_else(|| self.refusal.as_deref().unwrap_or("native adapter phase is retained without a physical close observation"), |error| error.message())
    }

    fn binds(&self, prepared: &PreparedNative) -> Result<(), String> {
        checkpoint(self.lease.original_deadline)?;
        let enclosing = self.enclosing.as_ref()
            .ok_or("native adapter original phase holder absent")?;
        if !Arc::ptr_eq(&self.lease.resources, &prepared.resources)
            || self.lease.phase.index() != prepared.next
            || !enclosing.matches_lease(&self.lease)
            || enclosing.original_deadline() != self.lease.original_deadline
            || enclosing.enclosing_deadline() != self.lease.enclosing_deadline
            || self.lease.enclosing_deadline != prepared.resources.enclosing_deadline
        {
            return Err("native adapter phase/resources/original clocks differ".into());
        }
        prepared.resources.verify(true)?;
        checkpoint(self.lease.original_deadline)
    }
}

fn native_receipt_report(
    output: crate::process_owner::CompleteCapturedBytes,
    slot: &NativePhaseSlot,
    prepared: &PreparedNative,
) -> Result<(CompleteCaptureReceipt, Witness), Box<NativeClosedFailure>> {
    let (status, stdout, stderr, _duration, timed_out, receipt) = output.into_parts();
    let validate = (|| {
        checkpoint(slot.lease.original_deadline)?;
        if !status.success()
            || timed_out
            || receipt.settled_status() != Some(status)
            || !receipt.matches_lease(&slot.lease)
        {
            return Err("native adapter transport status or actual combined lease differs".into());
        }
        if slot.lease.phase != NativePhase::Analyze {
            return Err("native adapter deadline discriminator unexpectedly completed".into());
        }
        let report = witness(&stdout, "worker")?;
        let actual_worker = receipt.observed_worker();
        let actual_parent = receipt.observed_parent();
        if actual_worker.pid() != actual_worker.group()
            || actual_worker.parent() != prepared.resources.controller.pid()
            || !same_process(&actual_parent, &prepared.resources.controller)
            || report.process != ProcessData::observed(&actual_worker)
            || report.worker != ProcessData::observed(&actual_worker)
            || report.parent != ProcessData::observed(&actual_parent)
        {
            return Err("native adapter worker witness differs from actual receipt lineage".into());
        }
        let image = adapter_lock(&prepared.resources.image)?;
        if report.image_sha256 != image.sha256()
            || report.image_bytes != image.byte_len()
            || report.build_identity != prepared.resources.build
            || report.base != prepared.resources.subject.base_commit.as_str()
            || report.head != prepared.resources.subject.head_commit.as_str()
            || report.head_tree != prepared.resources.subject.head_tree.as_str()
            || report.ordinary_limit != prepared.resources.policy.changed_rust_line_limit()
            || report.changed_lines != OWNER_LINES
            || report.removed_lines != 0
            || report.predicate_line != PREDICATE_LINE
            || report.findings == 0
            || !report.postflight
        {
            return Err("native adapter witness differs from actual image/build/whole subject".into());
        }
        drop(image);
        drop(stdout);
        drop(stderr);
        prepared.resources.verify(false)?;
        let request = adapter_lock(&prepared.resources.request)?.take()
            .ok_or("native adapter original parent request already consumed")?;
        request.validate_whole_current(
            &prepared.resources.subject,
            prepared.resources.enclosing_deadline,
        )?;
        prepared.resources.verify(false)?;
        checkpoint(slot.lease.original_deadline)?;
        Ok(report)
    })();
    match validate {
        Ok(report) => Ok((receipt, report)),
        Err(error) => Err(Box::new(NativeClosedFailure {
            closure: ActualNativeClosure::Combined { receipt, status },
            diagnostic: Some(error),
            discriminator: None,
        })),
    }
}

fn original_deadline_discriminator(
    slot: &NativePhaseSlot,
    prepared: &PreparedNative,
) -> Result<ObservedProcessIdentity, String> {
    let owner = slot.enclosing.as_ref()
        .ok_or("native adapter actual enclosing holder missing after report")?;
    let error = owner.inner().and_then(CompleteTerminalCustodian::failure)
        .ok_or("native adapter actual capture failure missing")?;
    if Instant::now() < slot.lease.original_deadline
        || !error.matches_lease(&slot.lease)
        || !error.message().contains("deadline")
        || error.is_timeout_only()
    {
        return Err("native adapter did not cross its original live capture deadline".into());
    }
    let worker = owner.observed_retained_worker()?;
    if worker.pid() != worker.group()
        || worker.parent() != prepared.resources.controller.pid()
        || worker.start() == 0
        || matches!(worker.state(), 'Z' | 'X' | 'x')
    {
        return Err("native adapter deadline refusal lacks an actual live owned worker".into());
    }
    // The observed retained PID excludes a pre-spawn/zero-test discriminator.
    if !owner.matches_lease(&slot.lease)
        || owner.original_deadline() != slot.lease.original_deadline
        || owner.enclosing_deadline() != prepared.resources.enclosing_deadline
    {
        return Err("native adapter discarded/unwound report changed actual custody".into());
    }
    prepared.resources.verify(true)?;
    Ok(worker)
}

struct NativeReportUnwind;

enum PhaseCapture {
    Closed(crate::process_owner::CompleteCapturedBytes),
    Failed,
}

/// Callable only with externally owned prepared resources and phase slots.
/// No returning libtest/Termination endpoint over Retained is supplied.
pub(super) fn run_native_phase<'a>(
    prepared: &mut PreparedNative,
    slot: &'a mut NativePhaseSlot,
) -> NativePhaseOutcome<'a> {
    if slot.attempted {
        slot.remember_refusal("native adapter phase was already attempted; same custody retained".into());
        return NativePhaseOutcome::Retained(slot);
    }
    slot.attempted = true;
    if let Err(error) = slot.binds(prepared) {
        slot.remember_refusal(error);
        return NativePhaseOutcome::Retained(slot);
    }
    let phase = slot.lease.phase;
    let mut captured = None;
    let mut expected_unwind = false;
    // Allocate this tiny test payload before capture, not in a failure path.
    let unwind_payload: Box<dyn std::any::Any + Send> = Box::new(NativeReportUnwind);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let owner = match slot.enclosing.as_mut() {
            Some(owner) => owner,
            None => return Err("native adapter original holder absent"),
        };
        owner.with_inner(|inner| {
            let budget = CompleteCaptureBudget::new(
                slot.timeout,
                slot.input.as_ref().map_or(0, Vec::len),
                HEADER_CAP,
                HEADER_CAP,
            );
            let result = if let Some(e) = slot.execution_deadline {
                CompleteByteCapture::capture_with_terminal_custody_until(
                    (&slot.execution, &slot.args),
                    (&prepared.resources.fixture.root, slot.input.as_deref()),
                    &[],
                    (budget, "native prepared Analyze"),
                    slot.started,
                    e,
                    inner,
                )
            } else {
                CompleteByteCapture::capture_with_terminal_custody(
                    (&slot.execution, &slot.args),
                    (&prepared.resources.fixture.root, None),
                    &[],
                    (budget, "native prepared original-T discriminator"),
                    inner,
                )
            };
            match result {
                Ok(output) => PhaseCapture::Closed(output),
                Err(_report) if phase == NativePhase::Unwind => {
                    std::panic::resume_unwind(unwind_payload);
                }
                Err(_report) => PhaseCapture::Failed,
            }
        })
    }));
    match caught {
        Ok(Ok(result)) => captured = Some(result),
        Ok(Err(boundary)) => slot.remember_refusal(boundary.into()),
        Err(payload) => {
            expected_unwind = phase == NativePhase::Unwind
                && payload.downcast_ref::<NativeReportUnwind>().is_some();
            if !expected_unwind {
                slot.unexpected_unwind = Some(payload);
                slot.remember_refusal("native adapter unexpected unwind remains with its phase owner".into());
                return NativePhaseOutcome::Retained(slot);
            }
        }
    }
    if let Some(PhaseCapture::Closed(output)) = captured {
        return match native_receipt_report(output, slot, prepared) {
            Ok((receipt, report)) => {
                prepared.next = 3;
                NativePhaseOutcome::Accepted(Box::new(NativeAccepted { receipt, witness: report }))
            }
            Err(closed) => NativePhaseOutcome::PhysicallyClosedFailure(closed),
        };
    }
    let discriminator = if phase != NativePhase::Analyze {
        if phase == NativePhase::Unwind && !expected_unwind {
            Err("native adapter expected report unwind was not observed".into())
        } else {
            original_deadline_discriminator(slot, prepared)
        }
    } else {
        Err("native adapter Analyze transport failed".into())
    };
    // End the borrowed disposal report before moving the Option. No ? or
    // Result<String> path can drop an unresolved actual owner here.
    let (disposed, disposition_refusal) = match slot.enclosing.as_mut() {
        Some(owner) => match owner.try_dispose_failure() {
            Ok(disposed) => (Some(disposed), None),
            Err(retained) => (None, Some(retained.message().to_string())),
        },
        None => (None, Some("native adapter enclosing owner absent".into())),
    };
    if let Some(refusal) = disposition_refusal {
        slot.remember_refusal(refusal);
    }
    let controlled = match (&discriminator, &disposed) {
        (Ok(worker), Some(disposed))
            if disposed.matches_lease(&slot.lease)
                && disposed.original_deadline() == slot.lease.original_deadline
                && disposed.enclosing_deadline() == slot.lease.enclosing_deadline
                && same_process(disposed.observed_worker(), worker)
                && same_process(disposed.observed_parent(), &prepared.resources.controller)
                && !disposed.status().success() =>
        {
            prepared.resources.verify(true)
        }
        _ => Err("native adapter original-T physical discriminator was not authenticated".into()),
    };
    let physical = match CompleteEnclosingCustodian::take_physically_closed(&mut slot.enclosing) {
        Ok(physical) => physical,
        Err(retained) => {
            slot.remember_refusal(retained.into());
            return NativePhaseOutcome::Retained(slot);
        },
    };
    if !physical.enclosing().matches_lease(&slot.lease)
        || physical.enclosing().original_deadline() != slot.lease.original_deadline
        || physical.enclosing().enclosing_deadline() != slot.lease.enclosing_deadline
    {
        return NativePhaseOutcome::PhysicallyClosedFailure(Box::new(NativeClosedFailure {
            closure: ActualNativeClosure::Physical(Box::new(physical)),
            diagnostic: Some("native adapter physically closed lease identity differs".into()),
            discriminator: None,
        }));
    }
    let (actual_discriminator, secondary) = match (discriminator, controlled) {
        (Ok(worker), Ok(())) => {
            prepared.next += 1;
            (Some(worker), None)
        }
        (Err(error), _) | (_, Err(error)) => (None, Some(error)),
    };
    NativePhaseOutcome::PhysicallyClosedFailure(Box::new(NativeClosedFailure {
        closure: ActualNativeClosure::Physical(Box::new(physical)),
        diagnostic: secondary,
        discriminator: actual_discriminator,
    }))
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct BootstrapImageData {
    device: u64,
    inode: u64,
    links: u64,
    bytes: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl BootstrapImageData {
    fn observe(metadata: &std::fs::Metadata) -> Result<Self, String> {
        if !metadata.is_file() || metadata.nlink() == 0
            || metadata.len() == 0 || metadata.len() > FS_BYTES
        {
            return Err(format!(
                "native bootstrap kernel image refuses regular={}/links={}/bytes={}; byte bound={FS_BYTES}",
                metadata.is_file(), metadata.nlink(), metadata.len(),
            ));
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            links: metadata.nlink(),
            bytes: metadata.len(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }

    fn kernel(deadline: Instant) -> Result<Self, String> {
        checkpoint(deadline)?;
        let metadata = fs::metadata("/proc/self/exe")
            .map_err(|error| format!("native bootstrap kernel image metadata: {error}"))?;
        checkpoint(deadline)?;
        Self::observe(&metadata)
    }
}

/// One actual bootstrap executable descriptor. This is DATA custody only:
/// the unlimited caller is not represented as a finite native worker.
#[cfg(all(
    not(feature = "lang-typescript"),
    not(feature = "lang-python"),
    not(feature = "lang-perl")
))]
struct BootstrapHeldImage {
    file: File,
    data: BootstrapImageData,
    owner: ObservedProcessIdentity,
}

#[cfg(all(
    not(feature = "lang-typescript"),
    not(feature = "lang-python"),
    not(feature = "lang-perl")
))]
impl BootstrapHeldImage {
    fn observe(deadline: Instant) -> Result<Self, String> {
        checkpoint(deadline)?;
        let owner = ObservedProcessIdentity::read(std::process::id())?;
        if owner.pid() != std::process::id() || owner.start() == 0
            || matches!(owner.state(), 'Z' | 'X' | 'x')
        {
            return Err("native bootstrap executable owner is not actual live SELF".into());
        }
        let file = File::open("/proc/self/exe")
            .map_err(|error| format!("native bootstrap actual kernel image open: {error}"))?;
        checkpoint(deadline)?;
        let data = BootstrapImageData::observe(
            &file.metadata().map_err(|error| error.to_string())?,
        )?;
        let held = Self { file, data, owner };
        held.recheck(deadline)?;
        Ok(held)
    }

    fn recheck(&self, deadline: Instant) -> Result<(), String> {
        checkpoint(deadline)?;
        let actual = ObservedProcessIdentity::read(std::process::id())?;
        if !same_process(&actual, &self.owner) || matches!(actual.state(), 'Z' | 'X' | 'x') {
            return Err("native bootstrap executable owner changed".into());
        }
        let held = BootstrapImageData::observe(
            &self.file.metadata().map_err(|error| error.to_string())?,
        )?;
        if held != self.data || BootstrapImageData::kernel(deadline)? != self.data {
            return Err("native bootstrap held/kernel image identity changed".into());
        }
        checkpoint(deadline)
    }

    fn identifier(&self, deadline: Instant) -> Result<PathBuf, String> {
        self.recheck(deadline)?;
        use std::os::fd::AsRawFd;
        Ok(PathBuf::from(format!(
            "/proc/{}/fd/{}", self.owner.pid(), self.file.as_raw_fd(),
        )))
    }
}

const CONTROLLER_TEST_NAME: &str =
    "app::pr_evidence::complete_native_test::native_controller_worker";
const CONTROLLER_SCHEMA: &str = "ripr.native_controller_startup.v1";
const CONTROLLER_CEILING: Duration = Duration::from_mins(5);
const CONTROLLER_DIAGNOSTIC_CAP: usize = 4096;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ControllerStartup {
    schema_version: String,
    nonce: String,
    parent: ProcessData,
    image: BootstrapImageData,
    build_identity: String,
}

fn controller_nonce(nonce: &str) -> bool {
    nonce.len() == 32
        && nonce.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn controller_role(nonce: &str) -> String {
    format!("controller:{nonce}")
}

#[cfg(all(
    not(feature = "lang-typescript"),
    not(feature = "lang-python"),
    not(feature = "lang-perl")
))]
struct BootstrapResources {
    image: BootstrapHeldImage,
    nonce: String,
    build: &'static str,
}

#[cfg(all(
    not(feature = "lang-typescript"),
    not(feature = "lang-python"),
    not(feature = "lang-perl")
))]
impl BootstrapResources {
    fn observe(entered: Instant, deadline: Instant) -> Result<Self, String> {
        let image = BootstrapHeldImage::observe(deadline)?;
        let build = crate::build_identity::cache_identity();
        if build.contains("+process:") || build.len() > 4096 || build.contains('\0') {
            return Err("native bootstrap requires actual stable compiled build identity".into());
        }
        let seed = format!("native-controller:{}:{}:{entered:?}", image.owner.pid(), image.owner.start());
        let hash = format!("{:x}", Sha256::digest(seed.as_bytes()));
        Ok(Self { image, nonce: hash[..32].into(), build })
    }

    fn header(&self, deadline: Instant) -> Result<Vec<u8>, String> {
        self.image.recheck(deadline)?;
        bounded_json(&ControllerStartup {
            schema_version: CONTROLLER_SCHEMA.into(),
            nonce: self.nonce.clone(),
            parent: ProcessData::observed(&self.image.owner),
            image: self.image.data.clone(),
            build_identity: self.build.into(),
        })
    }
}

fn authenticate_controller_startup(
    entered: Instant,
) -> Result<(ControllerStartup, ObservedProcessIdentity, Instant, ObservedSelfImage), String> {
    let scalars = actual_entry_scalars(NativeTestEntry::Controller)?;
    let address_space = decimal(&scalars[0])?;
    let file_size = decimal(&scalars[1])?;
    let remaining = decimal(&scalars[2])?;
    let cap = decimal(&scalars[3])?;
    if address_space != AS_BYTES || file_size != FS_BYTES || cap != HEADER_CAP as u64
        || remaining > CONTROLLER_CEILING.as_millis() as u64
    {
        return Err("native controller actual scalar profile differs".into());
    }
    let deadline = entered.checked_add(Duration::from_millis(remaining))
        .ok_or("native controller original scalar deadline overflow")?;
    adapter_native(deadline)?;
    let process = adapter_self(deadline)?;
    let parent = ObservedProcessIdentity::read(process.parent())?;
    if parent.start() == 0 || matches!(parent.state(), 'Z' | 'X' | 'x') {
        return Err("native controller actual parent is not live".into());
    }
    checkpoint(deadline)?;
    // Exactly the existing UTF-8/cap/error/EOF primitive. No alternate reader
    // or worker header decoder can grant this private controller entry.
    let bytes = crate::bounded_input::read_reader_to_string_with_limit(
        std::io::stdin().lock(), cap,
    ).map_err(|error| format!("native controller startup read: {error}"))?;
    checkpoint(deadline)?;
    let startup: ControllerStartup = serde_json::from_str(&bytes)
        .map_err(|error| format!("native controller closed startup: {error}"))?;
    if startup.schema_version != CONTROLLER_SCHEMA || !controller_nonce(&startup.nonce)
        || startup.parent != ProcessData::observed(&parent)
        || startup.build_identity != crate::build_identity::cache_identity()
        || startup.build_identity.contains("+process:")
        || startup.image != BootstrapImageData::kernel(deadline)?
    {
        return Err("native controller startup differs from actual parent/image/build".into());
    }
    drop(bytes);
    // Genuine finite SELF observation/hash is never supplied by the header.
    let mut image = ObservedSelfImage::observe(AS_BYTES, FS_BYTES, deadline)?;
    if image.byte_len() != startup.image.bytes
        || BootstrapImageData::kernel(deadline)? != startup.image
    {
        return Err("native controller actual retained image differs from startup".into());
    }
    image.recheck()?;
    if BootstrapImageData::kernel(deadline)? != startup.image
        || !same_process(&ObservedProcessIdentity::read(parent.pid())?, &parent)
        || !same_process(&ObservedProcessIdentity::read(process.pid())?, &process)
    {
        return Err("native controller parent/image/process changed after native admission".into());
    }
    checkpoint(deadline)?;
    Ok((startup, process, deadline, image))
}

fn bounded_controller_diagnostic(message: &str) -> String {
    let mut end = message.len().min(CONTROLLER_DIAGNOSTIC_CAP);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_owned()
}

struct NativeController {
    prepared: PreparedNative,
    slots: NativePhaseSlots,
    closed: [bool; 3],
    failures: [Option<Box<NativeClosedFailure>>; 3],
    first_error: Option<String>,
    accepted: Option<Box<NativeAccepted>>,
    closeout_unwind: Option<Box<dyn std::any::Any + Send>>,
}

impl NativeController {
    fn new(prepared: PreparedNative, slots: NativePhaseSlots) -> Self {
        Self {
            prepared, slots, closed: [false; 3], failures: [None, None, None],
            first_error: None, accepted: None, closeout_unwind: None,
        }
    }

    fn refuse(&mut self, message: &str) {
        if self.first_error.is_none() {
            self.first_error = Some(bounded_controller_diagnostic(message));
        }
    }

    fn run_admitted(&mut self) {
        for phase in [NativePhase::Discard, NativePhase::Unwind, NativePhase::Analyze] {
            let index = phase.index();
            let outcome = run_native_phase(&mut self.prepared, self.slots.phase(phase));
            match outcome {
                NativePhaseOutcome::Accepted(accepted) => {
                    self.closed[index] = true;
                    self.accepted = Some(accepted);
                    if phase != NativePhase::Analyze {
                        self.refuse("native controller accepted a negative phase");
                    }
                }
                NativePhaseOutcome::PhysicallyClosedFailure(failure) => {
                    self.closed[index] = true;
                    if phase == NativePhase::Analyze || failure.secondary().is_some()
                        || failure.discriminator().is_none()
                    {
                        self.refuse(failure.message());
                    }
                    self.failures[index] = Some(failure);
                }
                NativePhaseOutcome::Retained(slot) => {
                    let message = bounded_controller_diagnostic(slot.retained_message());
                    self.refuse(&message);
                }
            }
            if self.first_error.is_some() { break; }
        }
        if self.first_error.is_none() && self.accepted.is_none() {
            self.refuse("native controller genuine accepted analysis is absent");
        }
    }

    fn physical_step(&mut self, index: usize) -> bool {
        let step = match self.slots.slots[index].enclosing.as_mut() {
            Some(owner) => owner.continue_physical_closeout(),
            None => PhysicalStep::Retained(
                "native controller attempted slot lost its actual custodian".into(),
            ),
        };
        match step {
            PhysicalStep::Closed => {
                match CompleteEnclosingCustodian::take_physically_closed(
                    &mut self.slots.slots[index].enclosing,
                ) {
                    Ok(physical) => {
                        self.failures[index] = Some(Box::new(NativeClosedFailure {
                            closure: ActualNativeClosure::Physical(Box::new(physical)),
                            diagnostic: self.first_error.clone(),
                            discriminator: None,
                        }));
                        self.closed[index] = true;
                        true
                    }
                    Err(error) => {
                        self.refuse(error);
                        false
                    }
                }
            }
            PhysicalStep::NoProcess => {
                // The custodian's actual no-spawn provenance permits returning
                // a refusal only. It supplies no physical or accepted receipt.
                self.refuse("native controller failed phase has no process receipt");
                self.closed[index] = true;
                true
            }
            PhysicalStep::Pending => false,
            PhysicalStep::Retained(message) => {
                self.refuse(&message);
                false
            }
        }
    }

    /// Never returns over attempted live custody. Bounded individual steps and
    /// existing backoff do not create a fresh eligibility or physical deadline.
    fn finish_to_terminal(&mut self) {
        loop {
            let mut terminal = true;
            for index in 0..3 {
                if self.closed[index] || !self.slots.slots[index].attempted {
                    continue;
                }
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.physical_step(index)
                }));
                match caught {
                    Ok(closed) => terminal &= closed,
                    Err(payload) => {
                        if self.closeout_unwind.is_none() {
                            self.closeout_unwind = Some(payload);
                        }
                        self.refuse("native controller closeout unwind retains original custody");
                        terminal = false;
                    }
                }
            }
            if terminal {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn take_unwind(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
        if let Some(payload) = self.closeout_unwind.take() {
            return Some(payload);
        }
        for slot in &mut self.slots.slots {
            if let Some(payload) = slot.unexpected_unwind.take() { return Some(payload); }
        }
        None
    }
}

#[test]
#[ignore = "requires a genuine owned continuing bootstrap and closed startup"]
fn native_controller_worker() -> Result<(), String> {
    let entered = Instant::now();
    supported()?;
    let (startup, process, deadline, mut authenticated_image) =
        authenticate_controller_startup(entered)?;
    let (prepared, slots) = prepare_native(entered, deadline)?;
    // All three actual owners and the resource Arc precede fallible execution.
    let mut controller = NativeController::new(prepared, slots);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        controller.run_admitted();
    }));
    if caught.is_err() {
        controller.refuse("native controller ordinary unwind requires physical closeout");
    }
    controller.finish_to_terminal();
    match caught {
        Err(payload) => std::panic::resume_unwind(payload),
        Ok(()) => {
            if let Some(payload) = controller.take_unwind() {
                std::panic::resume_unwind(payload);
            }
        }
    }
    if let Some(error) = controller.first_error.as_ref() { return Err(error.clone()); }
    let accepted = controller.accepted.take()
        .ok_or("native controller genuine completed analysis missing")?;
    if accepted.receipt().settled_status().is_none_or(|status| !status.success())
        || accepted.witness().worker != ProcessData::observed(&accepted.receipt().observed_worker())
        || !same_process(&accepted.receipt().observed_parent(), &process)
    {
        return Err("native controller actual accepted worker lineage differs".into());
    }
    let NativeAccepted { receipt, mut witness } = *accepted;
    checkpoint(deadline)?;
    authenticated_image.recheck()?;
    controller.prepared.resources.verify(false)?;
    adapter_native(deadline)?;
    if BootstrapImageData::kernel(deadline)? != startup.image
        || !same_process(&ObservedProcessIdentity::read(process.pid())?, &process)
        || startup.parent != ProcessData::observed(
            &ObservedProcessIdentity::read(process.parent())?,
        )
    {
        return Err("native controller final image/parent/process changed".into());
    }
    witness.role = controller_role(&startup.nonce);
    witness.process = ProcessData::observed(&process);
    witness.parent = startup.parent;
    // Actual W receipt is retained through reporting, never cloned/serialized.
    emit_witness(&witness, deadline)?;
    checkpoint(deadline)?;
    drop(receipt);
    Ok(())
}

#[cfg(all(
    not(feature = "lang-typescript"),
    not(feature = "lang-python"),
    not(feature = "lang-perl")
))]
fn finish_controller_transport(
    owner: &mut CompleteControllerTransport<BootstrapResources>,
    first_error: &mut Option<String>,
    unwind: &mut Option<Box<dyn std::any::Any + Send>>,
) {
    loop {
        let step = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.step_to_terminal()
        }));
        match step {
            Ok(PhysicalStep::Closed | PhysicalStep::NoProcess) => return,
            Ok(PhysicalStep::Pending) => {}
            Ok(PhysicalStep::Retained(message)) => {
                if first_error.is_none() {
                    *first_error = Some(bounded_controller_diagnostic(&message));
                }
            }
            Err(payload) => {
                if unwind.is_none() { *unwind = Some(payload); }
                if first_error.is_none() {
                    *first_error = Some("native bootstrap unwind requires controller closeout".into());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(all(
    not(feature = "lang-typescript"),
    not(feature = "lang-python"),
    not(feature = "lang-perl")
))]
#[test]
fn native_bootstrap_complete_owner_and_failure_recovery() -> Result<(), String> {
    supported()?;
    let started = Instant::now();
    let u = started.checked_add(CONTROLLER_CEILING)
        .ok_or("native bootstrap original enclosing clock overflow")?;
    let t = u.checked_sub(Duration::from_secs(1))
        .ok_or("native bootstrap original capture clock underflow")?;
    let e = t.checked_sub(Duration::from_secs(7))
        .ok_or("native bootstrap original execution clock underflow")?;
    let policy = capture_complete_rust_policy()?;
    let profile = profile_for_request(PROFILE_NAME, &policy)?;
    // Pre-admit startup copies, bounded outputs, hash/pump scratch, arguments,
    // diagnostic and fixed owner storage before descriptor/header allocation.
    adapter_sum(
        &[
            6 * HEADER_CAP as u64,
            2,
            3 * CONTROLLER_DIAGNOSTIC_CAP as u64,
            std::mem::size_of::<BootstrapResources>() as u64,
            std::mem::size_of::<CompleteControllerTransport<BootstrapResources>>() as u64,
        ],
        profile.max_buffered_bytes,
    )?;
    let resources = Arc::new(BootstrapResources::observe(started, e)?);
    if policy.changed_rust_line_limit() >= OWNER_LINES {
        return Err("native bootstrap actual ordinary guard is not discriminated".into());
    }
    let header = resources.header(e)?;
    let identifier = resources.image.identifier(e)?;
    let millis = u64::try_from(
        e.checked_duration_since(Instant::now())
            .ok_or("native bootstrap original execution window expired")?
            .as_millis(),
    ).map_err(|error| format!("native bootstrap original scalar conversion: {error}"))?;
    if millis == 0 || millis > CONTROLLER_CEILING.as_millis() as u64 {
        return Err("native bootstrap original scalar window is invalid".into());
    }
    let args = vec![
        format!("--as={AS_BYTES}:{AS_BYTES}"),
        format!("--fsize={FS_BYTES}:{FS_BYTES}"),
        "--core=0:0".into(), "--".into(), adapter_path(&identifier)?.into(),
        "--ignored".into(), "--exact".into(), CONTROLLER_TEST_NAME.into(),
        "--nocapture".into(), "--test-threads=1".into(),
        AS_BYTES.to_string(), FS_BYTES.to_string(), millis.to_string(), HEADER_CAP.to_string(),
    ];
    let source = std::env::current_dir()
        .map_err(|error| format!("native bootstrap actual invocation directory: {error}"))?;
    let budget = CompleteCaptureBudget::new(
        CONTROLLER_CEILING, header.len(), HEADER_CAP, HEADER_CAP,
    );
    // Admission allocates no child. The actual owner exists before launch and
    // remains outside both fallible work and every caught ordinary unwind.
    let mut owner = CompleteControllerTransport::admit(
        (Path::new("/usr/bin/prlimit"), &args),
        (&source, Some(&header)), &[], budget, (started, e, t, u), Arc::clone(&resources),
    )?;
    let mut first_error = None;
    let mut unwind = None;
    let launch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        owner.launch()
    }));
    match launch {
        Ok(Ok(())) => {}
        Ok(Err(error)) => first_error = Some(bounded_controller_diagnostic(&error)),
        Err(payload) => unwind = Some(payload),
    }
    finish_controller_transport(&mut owner, &mut first_error, &mut unwind);
    if let Some(payload) = unwind { std::panic::resume_unwind(payload); }
    let terminal = owner.take_terminal()?;
    if let Some(error) = first_error { return Err(error); }
    let output = match terminal {
        CompleteControllerTerminal::Accepted(output) => output,
        CompleteControllerTerminal::Failed(failure) => {
            if !failure.matches_lease(&resources) {
                return Err("native bootstrap failed terminal lease differs".into());
            }
            return Err(format!("native bootstrap closed controller failed: {}", failure.message()));
        }
    };
    let (status, stdout, stderr, _duration, timed_out, receipt) = output.into_parts();
    if !status.success() || timed_out || receipt.settled_status() != Some(status)
        || !receipt.matches_lease(&resources)
    {
        return Err("native bootstrap controller lacks actual timely combined receipt".into());
    }
    let report = witness(&stdout, &controller_role(&resources.nonce))?;
    let actual_controller = receipt.observed_worker();
    let actual_parent = receipt.observed_parent();
    if report.process != ProcessData::observed(&actual_controller)
        || report.parent != ProcessData::observed(&actual_parent)
        || actual_controller.pid() != actual_controller.group()
        || actual_controller.parent() != resources.image.owner.pid()
        || !same_process(&actual_parent, &resources.image.owner)
        || report.worker.parent != actual_controller.pid()
        || report.worker.pid != report.worker.group
        || report.worker.start == 0
        || report.image_bytes != resources.image.data.bytes
        || report.build_identity != resources.build
        || report.ordinary_limit != policy.changed_rust_line_limit()
    {
        return Err("native bootstrap actual controller/witness/image/worker joins differ".into());
    }
    drop(stdout);
    drop(stderr);
    resources.image.recheck(t)?;
    if capture_complete_rust_policy()? != policy {
        return Err("native bootstrap ordinary policy changed during native execution".into());
    }
    checkpoint(t)?;
    // This proves the returning native test route only. No JSON/witness or C
    // group receipt is converted to W settlement or product publication.
    drop(receipt);
    Ok(())
}
