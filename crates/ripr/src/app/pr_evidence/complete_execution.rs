//! Inactive complete-execution resource experiment (#1627).
//! This runs the ordinary producer; it does not bypass its line/partial guards
//! and does not establish exhaustive canonical-history coverage.
use super::*;
use std::collections::BTreeMap;

pub(super) const WORKER_FLAG: &str = "--experimental-complete-execution-worker";
pub(super) const GENERATION_FIELD: &str = "experimental_complete_execution";
pub(super) const RECEIPT: &str = "target/ripr/pr/complete-execution.receipt.json";
#[cfg(test)]
const ADDRESS_SPACE_MAX: u64 = 2 * 1024 * 1024 * 1024;
#[cfg(test)]
use crate::process_owner::native_limits::require_limits;
pub(super) use crate::process_owner::native_limits::verify_limits;

pub(super) fn run_worker(args: &[String]) -> Result<(), String> {
    let address_space = args
        .first()
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| "experimental worker missing address-space bound".to_string())?;
    let file = args
        .get(1)
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| "experimental worker missing file bound".to_string())?;
    // Verify before repository/config/Git reads, analysis or full JSON conversion.
    verify_limits(address_space, file)?;
    let nonce = args
        .get(2)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or_else(|| "experimental worker missing or malformed generation nonce".to_string())?;
    let options = parse_options(&args[3..])?;
    if options.check || !options.base_explicit {
        return Err(
            "experimental worker requires an explicit producer base and cannot use --check"
                .to_string(),
        );
    }
    let repo = std::env::current_dir()
        .map_err(|error| format!("experimental worker current directory: {error}"))?;
    ensure_root_inside_invocation_repo(&repo, &options.root)?;
    let generation = json!({
        "schema_version": "ripr.complete_execution_experiment.v1",
        "nonce": nonce,
        "address_space_bytes": address_space,
        "file_bytes": file,
        "coverage": "not_established",
        "production_admission": false,
    });
    let result = (|| {
        write_pr_evidence_with_generation(&repo, &options, run_ripr_check, Some(&generation))?;
        let check: Value = serde_json::from_slice(
            &fs::read(repo.join(PR_CHECK_JSON))
                .map_err(|error| format!("experimental worker check read: {error}"))?,
        )
        .map_err(|error| format!("experimental worker check JSON: {error}"))?;
        if check
            .pointer("/analysis_outcome/analysis_complete")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(
                "experimental worker ordinary producer did not complete analysis".to_string(),
            );
        }
        let subject: Value = serde_json::from_slice(
            &fs::read(repo.join(PR_CHECK_SUBJECT_JSON))
                .map_err(|error| format!("experimental worker subject read: {error}"))?,
        )
        .map_err(|error| format!("experimental worker subject JSON: {error}"))?;
        if subject.get(GENERATION_FIELD) != Some(&generation) {
            return Err("experimental worker generation publication mismatch".to_string());
        }
        let mut artifacts = BTreeMap::new();
        for path in [
            PR_EVIDENCE_JSON,
            PR_EVIDENCE_MD,
            PR_CHECK_JSON,
            PR_CHECK_SUBJECT_JSON,
            PR_REVIEW_INPUT_JSON,
            PR_CANONICAL_DIFF,
        ] {
            let (digest, _) = digest_file(&repo.join(path))?;
            artifacts.insert(path, digest);
        }
        let receipt = json!({
            "schema_version": "ripr.complete_execution_experiment_receipt.v1",
            "nonce": nonce,
            "address_space_bytes": address_space,
            "file_bytes": file,
            "base_sha": subject["base_sha"],
            "head_sha": subject["head_sha"],
            "head_tree": subject["head_tree"],
            "check_sha256": subject["check_sha256"],
            "check_byte_count": subject["check_byte_count"],
            "canonical_diff_sha256": subject["canonical_diff_sha256"],
            "configuration_fingerprint": subject["configuration_fingerprint"],
            "index_entries": subject["canonical_finding_index_entry_count"],
            "index_bytes": subject["canonical_finding_index_byte_count"],
            "coverage": "not_established",
            "production_admission": false,
            "artifacts": artifacts,
        });
        let bytes = serde_json::to_vec(&receipt)
            .map_err(|error| format!("experimental worker receipt: {error}"))?;
        if bytes.len() > 16 * 1024 {
            return Err("experimental worker receipt exceeds its byte bound".to_string());
        }
        crate::atomic_file::write(&repo.join(RECEIPT), &bytes, RECEIPT)
    })();
    if result.is_err() {
        // A failure after publication cannot leave reusable authority.
        remove_stale_check_artifact(&repo)?;
        match fs::remove_file(repo.join(RECEIPT)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("experimental worker receipt revocation: {error}")),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_verification_refuses_missing_nonfinite_duplicate_and_mismatched_limits()
    -> Result<(), String> {
        let valid = "Max address space         1073741824 1073741824 bytes\nMax file size             16777216 16777216 bytes\nMax core file size        0 0 bytes\n";
        require_limits(valid, 1073741824, 16777216)?;
        for invalid in [
            valid.replace("1073741824 1073741824", "unlimited unlimited"),
            valid.replace("1073741824 1073741824", "1073741824 2147483648"),
            valid.replace("Max address space", "Max unknown"),
            format!("{valid}Max address space 1073741824 1073741824 bytes\n"),
            valid.replace("0 0 bytes", "0 1 bytes"),
        ] {
            let Err(error) = require_limits(&invalid, 1073741824, 16777216) else {
                return Err("invalid native resource limits were accepted".to_string());
            };
            assert!(error.starts_with("experimental worker"), "{error}");
        }
        for address_space in [0, ADDRESS_SPACE_MAX + 1] {
            let Err(error) = require_limits(valid, address_space, 16777216) else {
                return Err("invalid address-space profile was accepted".to_string());
            };
            assert!(
                error.contains("not the requested finite soft/hard limit"),
                "{error}"
            );
        }
        Ok(())
    }

    #[test]
    fn experimental_generation_is_rejected_even_when_null_or_structurally_complete() {
        for generation in [
            Value::Null,
            json!({"schema_version":"ripr.complete_execution_experiment.v1",
                "coverage":"complete", "production_admission":true}),
        ] {
            let mut packet = json!({"status":"ok"});
            packet[GENERATION_FIELD] = generation;
            assert!(reject_pr_evidence_error_packet(&packet).is_some());
        }
        assert!(reject_pr_evidence_error_packet(&json!({"status":"ok"})).is_none());
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
mod whole_worker {
    use super::super::complete_contract::{CompleteVerificationLimits, ProducerSurface};
    use super::super::complete_native::{NativeStartup, StageRootBinding};
    use super::super::complete_request::{
        CommittedRequestBinding, CompleteRequest, CompleteSubject, RequestedRoute,
        select_request_with_deadline,
    };
    use super::*;
    use crate::analysis::committed_source::staged::{RetainedDirectory, SourceAnchor};
    use crate::core_error::CoreError;
    use crate::domain::GitObjectId;
    use crate::process_owner::ObservedProcessIdentity;
    use crate::process_owner::native_limits::limits;
    use serde::Deserialize;
    use std::os::unix::ffi::OsStrExt;
    use std::sync::Arc;
    use std::time::Instant;

    pub(in crate::app::pr_evidence) const WHOLE_WORKER_FLAG: &str =
        "--complete-evidence-analyze-worker";
    const LIMITS_MAX: u64 = 16 * 1024;
    const STARTUP_MAX: u64 = 64 * 1024;
    const LITERAL_MAX: usize = 4096;
    const ARGS_MAX: usize = 32;
    const STARTUP_SCHEMA: &str = "ripr.whole_worker_startup.v1";
    const WHOLE_STAGE_PATH: &str = "/var/tmp/ripr-complete-retained";

    fn checkpoint(deadline: Instant) -> Result<(), String> {
        if Instant::now() >= deadline {
            return Err("whole worker original deadline exhausted".into());
        }
        Ok(())
    }

    fn lower_hex(value: &str, length: usize) -> bool {
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn literal(value: &str) -> Result<(), String> {
        if value.is_empty() || value.len() > LITERAL_MAX || value.contains(['\0', '\n', '\r']) {
            return Err("whole worker original literal is invalid".into());
        }
        Ok(())
    }

    #[derive(Debug)]
    struct Scalars {
        address_space_bytes: u64,
        file_bytes: u64,
        remaining_ms: u64,
        startup_cap: u64,
        deadline: Instant,
    }

    impl Scalars {
        fn parse(args: &[String], entered: Instant) -> Result<Self, String> {
            if args.len() != 4 {
                return Err("whole worker requires exactly four scalar arguments".into());
            }
            let number = |index: usize| {
                let text = &args[index];
                if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit()) {
                    return Err("whole worker scalar is not a bounded decimal integer".into());
                }
                text.parse::<u64>()
                    .map_err(|error| format!("whole worker scalar: {error}"))
            };
            let address_space_bytes = number(0)?;
            let file_bytes = number(1)?;
            let remaining_ms = number(2)?;
            let startup_cap = number(3)?;
            if remaining_ms == 0 || startup_cap == 0 || startup_cap > STARTUP_MAX {
                return Err("whole worker remaining budget or startup cap is invalid".into());
            }
            let deadline = entered
                .checked_add(Duration::from_millis(remaining_ms))
                .ok_or("whole worker original deadline overflow")?;
            checkpoint(deadline)?;
            Ok(Self {
                address_space_bytes,
                file_bytes,
                remaining_ms,
                startup_cap,
                deadline,
            })
        }

        fn matches_profile(&self, profile: &CompleteVerificationLimits) -> Result<(), String> {
            profile.validate()?;
            if profile.address_space_bytes != self.address_space_bytes
                || profile.file_size_bytes != self.file_bytes
                || self.remaining_ms > profile.deadline_ms
            {
                return Err("whole worker profile differs from admitted scalar arguments".into());
            }
            Ok(())
        }
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ParentBinding {
        pid: u32,
        start: u64,
        group: u32,
        address_space_bytes: u64,
        file_bytes: u64,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SubjectCommitment {
        requested_root: String,
        requested_base: String,
        requested_head: String,
        invocation_repository: String,
        logical_root: String,
        work_tree: String,
        base_commit: String,
        head_commit: String,
        base_tree: String,
        head_tree: String,
        origin_commit: String,
        origin_tree: String,
    }

    impl SubjectCommitment {
        fn validate(&self) -> Result<(), String> {
            for value in [
                &self.requested_root,
                &self.requested_base,
                &self.requested_head,
            ] {
                literal(value)?;
            }
            for root in [
                &self.invocation_repository,
                &self.logical_root,
                &self.work_tree,
            ] {
                literal(root)?;
                if !root.starts_with('/')
                    || root == "/"
                    || root[1..]
                        .split('/')
                        .any(|part| part.is_empty() || part == "." || part == "..")
                {
                    return Err(
                        "whole worker root commitment is not canonical absolute UTF-8".into(),
                    );
                }
            }
            if self.invocation_repository != self.logical_root
                || self.logical_root != self.work_tree
            {
                return Err("whole-head-v1 nested invocation is unsupported".into());
            }
            let width = self.base_commit.len();
            for oid in [
                &self.base_commit,
                &self.head_commit,
                &self.base_tree,
                &self.head_tree,
                &self.origin_commit,
                &self.origin_tree,
            ] {
                if !matches!(width, 40 | 64) || !lower_hex(oid, width) {
                    return Err("whole worker subject OID is not canonical".into());
                }
                GitObjectId::parse(oid).map_err(|error| error.to_string())?;
            }
            Ok(())
        }

        fn matches(&self, subject: &CompleteSubject, options: &PrEvidenceOptions) -> bool {
            options == subject.requested_options()
                && self.requested_root == options.root
                && self.requested_base == options.base
                && self.requested_head == options.head
                && subject.invocation_repository.as_os_str()
                    == Path::new(&self.invocation_repository).as_os_str()
                && subject.root.as_os_str() == Path::new(&self.logical_root).as_os_str()
                && subject.work_tree.as_os_str() == Path::new(&self.work_tree).as_os_str()
                && self.base_commit == subject.base_commit.as_str()
                && self.head_commit == subject.head_commit.as_str()
                && self.base_tree == subject.base_tree.as_str()
                && self.head_tree == subject.head_tree.as_str()
                && self.origin_commit == subject.origin_commit.as_str()
                && self.origin_tree == subject.origin_tree.as_str()
        }
    }

    fn require_whole_invocation_root(subject: &CompleteSubject) -> Result<(), String> {
        if subject.invocation_repository.as_os_str() != subject.root.as_os_str()
            || subject.root.as_os_str() != subject.work_tree.as_os_str()
        {
            return Err("whole-head-v1 nested invocation is unsupported".into());
        }
        Ok(())
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StartupHeader {
        schema_version: String,
        profile: CompleteVerificationLimits,
        stage: StageRootBinding,
        generation_nonce: String,
        parent: ParentBinding,
        build_identity: String,
        surface: ProducerSurface,
        producer_args: Vec<String>,
        committed_request: CommittedRequestBinding,
        subject_commitment: SubjectCommitment,
    }

    fn require_fixed_stage(stage: &StageRootBinding) -> Result<(), String> {
        if stage.stage.path != WHOLE_STAGE_PATH {
            return Err("whole worker stage namespace override is unsupported".into());
        }
        Ok(())
    }

    fn decode_header(bytes: &[u8], scalars: &Scalars) -> Result<StartupHeader, String> {
        checkpoint(scalars.deadline)?;
        if bytes.len() as u64 > scalars.startup_cap {
            return Err("whole worker startup exceeds its admitted byte cap".into());
        }
        let header: StartupHeader = serde_json::from_slice(bytes)
            .map_err(|error| format!("whole worker closed startup JSON: {error}"))?;
        if header.schema_version != STARTUP_SCHEMA {
            return Err("whole worker startup schema is unsupported".into());
        }
        scalars.matches_profile(&header.profile)?;
        require_fixed_stage(&header.stage)?;
        header.subject_commitment.validate()?;
        header.committed_request.validate()?;
        if !lower_hex(&header.generation_nonce, 32) {
            return Err(
                "whole worker generation nonce is not 32 lowercase hexadecimal bytes".into(),
            );
        }
        if header.producer_args.is_empty() || header.producer_args.len() > ARGS_MAX {
            return Err("whole worker producer argument count admission exceeded".into());
        }
        let mut args_bytes = 0_u64;
        for argument in &header.producer_args {
            literal(argument)?;
            args_bytes = args_bytes
                .checked_add(argument.len() as u64)
                .ok_or("whole worker producer argument byte overflow")?;
        }
        if args_bytes > scalars.startup_cap {
            return Err("whole worker producer argument byte admission exceeded".into());
        }
        // Bootstrap decoding has its fixed 64 KiB input ceiling and actual native
        // AS. This is a conservative logical payload reservation BEFORE extra
        // clones, request capture and option parsing, not allocator/RSS accounting
        // or a claim that the decoded profile preceded serde.
        let reservation = (bytes.len() as u64)
            .checked_add(1)
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| n.checked_add(64 * 1024 + LIMITS_MAX))
            .ok_or("whole worker startup logical reservation overflow")?;
        if reservation > header.profile.max_buffered_bytes {
            return Err("whole worker startup logical payload admission exceeded".into());
        }
        checkpoint(scalars.deadline)?;
        Ok(header)
    }

    fn read_header(reader: impl Read, scalars: &Scalars) -> Result<StartupHeader, String> {
        checkpoint(scalars.deadline)?;
        let bytes =
            crate::bounded_input::read_reader_to_string_with_limit(reader, scalars.startup_cap)
                .map_err(|error| format!("whole worker startup read: {error}"))?;
        checkpoint(scalars.deadline)?;
        decode_header(bytes.as_bytes(), scalars)
    }

    fn live(identity: &ObservedProcessIdentity) -> bool {
        !matches!(identity.state(), 'Z' | 'X' | 'x')
    }

    fn same_process(left: &ObservedProcessIdentity, right: &ObservedProcessIdentity) -> bool {
        left.pid() == right.pid()
            && left.parent() == right.parent()
            && left.group() == right.group()
            && left.start() == right.start()
            && live(right)
    }

    fn stable_build() -> Result<&'static str, String> {
        let identity = crate::build_identity::cache_identity();
        if identity.contains("+process:") || identity.len() > LITERAL_MAX || identity.contains('\0')
        {
            return Err("whole worker requires a stable compiled build identity".into());
        }
        Ok(identity)
    }

    fn parent_profile_from_limits(text: &str) -> Result<(u64, u64), String> {
        let (address_space, hard) = limits(text, "Max address space")?;
        // Preserve the shared observer's AS-first refusal precedence before
        // decoding FS/core. This uses the same parser, cap and diagnostic.
        if address_space == 0 || address_space > ADDRESS_SPACE_MAX || hard != address_space {
            return Err(
                "experimental worker Max address space is not the requested finite soft/hard limit"
                    .into(),
            );
        }
        let file = limits(text, "Max file size")?.0;
        require_limits(text, address_space, file)?;
        Ok((address_space, file))
    }

    fn observed_parent_limits(
        parent: &ObservedProcessIdentity,
        deadline: Instant,
    ) -> Result<(u64, u64), String> {
        checkpoint(deadline)?;
        let before = ObservedProcessIdentity::read(parent.pid())?;
        if !same_process(parent, &before) {
            return Err("whole worker parent identity changed before native limit read".into());
        }
        let mut text = String::new();
        fs::File::open(format!("/proc/{}/limits", parent.pid()))
            .map_err(|error| format!("whole worker parent limits unavailable: {error}"))?
            .take(LIMITS_MAX + 1)
            .read_to_string(&mut text)
            .map_err(|error| format!("whole worker parent limits read: {error}"))?;
        if text.len() as u64 > LIMITS_MAX {
            return Err("whole worker parent limits exceed their byte bound".into());
        }
        let profile = parent_profile_from_limits(&text)?;
        let after = ObservedProcessIdentity::read(parent.pid())?;
        if !same_process(parent, &after) {
            return Err("whole worker parent identity changed after native limit read".into());
        }
        checkpoint(deadline)?;
        Ok(profile)
    }

    fn actual_argv(args: &[String], deadline: Instant) -> Result<(), String> {
        checkpoint(deadline)?;
        let mut actual = std::env::args_os();
        if actual.next().is_none()
            || actual.next().as_deref() != Some(std::ffi::OsStr::new("pr-evidence"))
            || actual.next().as_deref() != Some(std::ffi::OsStr::new(WHOLE_WORKER_FLAG))
        {
            return Err("whole worker requires its actual closed CLI entry".into());
        }
        for argument in args {
            if actual.next().as_deref() != Some(std::ffi::OsStr::new(argument)) {
                return Err(
                    "whole worker scalar arguments differ from its actual invocation".into(),
                );
            }
        }
        if actual.next().is_some() {
            return Err("whole worker actual invocation has extra arguments".into());
        }
        checkpoint(deadline)
    }

    fn actual_libtest_argv(args: &[String], deadline: Instant) -> Result<(), String> {
        checkpoint(deadline)?;
        super::super::complete_native_test::authenticate_actual_libtest_argv(
            super::super::complete_native_test::WORKER_TEST_NAME,
            args,
        )?;
        checkpoint(deadline)
    }

    fn same_optional_path(left: &Option<PathBuf>, right: &Option<PathBuf>) -> bool {
        match (left, right) {
            (None, None) => true,
            (Some(left), Some(right)) => left.as_os_str() == right.as_os_str(),
            _ => false,
        }
    }

    fn same_check(left: &CheckInput, right: &CheckInput) -> bool {
        let CheckInput {
            root,
            base,
            diff_file,
            mode,
            format,
            include_unchanged_tests,
            perl_facts_path,
            suppression_policy,
            git_timeout,
            git_candidate,
        } = left;
        root.as_os_str() == right.root.as_os_str()
            && base == &right.base
            && same_optional_path(diff_file, &right.diff_file)
            && mode == &right.mode
            && format == &right.format
            && include_unchanged_tests == &right.include_unchanged_tests
            && same_optional_path(perl_facts_path, &right.perl_facts_path)
            && same_optional_path(suppression_policy, &right.suppression_policy)
            && git_timeout == &right.git_timeout
            && git_candidate.is_none()
            && right.git_candidate.is_none()
    }

    fn surface_seed(
        repo: &Path,
        options: &PrEvidenceOptions,
        surface: ProducerSurface,
    ) -> Result<CheckInput, String> {
        let git_timeout = match surface {
            ProducerSurface::Installed => None,
            ProducerSurface::Xtask => crate::cli::commands::git_timeout_from_env(
                false,
                std::env::var("RIPR_GIT_TIMEOUT"),
            )?
            .unwrap_or(Some(crate::app::default_cli_git_timeout())),
        };
        Ok(CheckInput {
            root: command_root_path(repo, &options.root),
            base: match surface {
                ProducerSurface::Installed => None,
                ProducerSurface::Xtask => Some(options.base.clone()),
            },
            diff_file: Some(repo.join(PR_CANONICAL_DIFF)),
            mode: Mode::Draft,
            format: OutputFormat::Json,
            include_unchanged_tests: surface == ProducerSurface::Installed,
            perl_facts_path: None,
            suppression_policy: None,
            git_timeout,
            git_candidate: None,
        })
    }

    // Private and non-Clone/Serde/Default. Only the actual closed entry below
    // constructs this local observation. It is not a parent settlement receipt.
    struct NativeAnalyzeEntry {
        scalars: Scalars,
        worker: ObservedProcessIdentity,
        parent: ObservedProcessIdentity,
        parent_limits: (u64, u64),
    }

    impl NativeAnalyzeEntry {
        fn observe(args: &[String], entered: Instant) -> Result<Self, String> {
            Self::observe_entry(args, entered, actual_argv)
        }

        fn observe_libtest(args: &[String], entered: Instant) -> Result<Self, String> {
            Self::observe_entry(args, entered, actual_libtest_argv)
        }

        fn observe_entry(
            args: &[String],
            entered: Instant,
            authenticate_argv: fn(&[String], Instant) -> Result<(), String>,
        ) -> Result<Self, String> {
            let scalars = Scalars::parse(args, entered)?;
            verify_limits(scalars.address_space_bytes, scalars.file_bytes)?;
            checkpoint(scalars.deadline)?;
            authenticate_argv(args, scalars.deadline)?;
            let worker = ObservedProcessIdentity::read(std::process::id())?;
            if !live(&worker)
                || worker.pid() != worker.group()
                || worker.parent() == 0
                || worker.start() == 0
            {
                return Err("whole worker is not an actual live owned group leader".into());
            }
            let parent = ObservedProcessIdentity::read(worker.parent())?;
            if !live(&parent) || parent.start() == 0 || parent.pid() == worker.pid() {
                return Err("whole worker actual parent is unavailable".into());
            }
            let parent_limits = observed_parent_limits(&parent, scalars.deadline)?;
            let current = ObservedProcessIdentity::read(worker.pid())?;
            if !same_process(&worker, &current) {
                return Err("whole worker native identity changed before startup input".into());
            }
            Ok(Self {
                scalars,
                worker,
                parent,
                parent_limits,
            })
        }

        fn recheck(&self) -> Result<(), String> {
            checkpoint(self.scalars.deadline)?;
            verify_limits(self.scalars.address_space_bytes, self.scalars.file_bytes)?;
            let worker = ObservedProcessIdentity::read(self.worker.pid())?;
            let parent = ObservedProcessIdentity::read(self.parent.pid())?;
            if self.worker.pid() != std::process::id()
                || !same_process(&self.worker, &worker)
                || !same_process(&self.parent, &parent)
                || worker.parent() != parent.pid()
                || worker.pid() != worker.group()
            {
                return Err("whole worker actual native process lineage changed".into());
            }
            if observed_parent_limits(&parent, self.scalars.deadline)? != self.parent_limits {
                return Err("whole worker actual parent native profile changed".into());
            }
            checkpoint(self.scalars.deadline)
        }
    }

    /// Caller-calculated logical buffer bounds, never an execution grant.
    pub(in crate::app::pr_evidence) struct CaptureBudget {
        raw: usize,
        presentation: usize,
        names: usize,
        fixed: u64,
    }

    impl CaptureBudget {
        pub(in crate::app::pr_evidence) fn new(
            profile: &CompleteVerificationLimits,
            fixed: u64,
        ) -> Result<Self, String> {
            profile.validate()?;
            let remaining = profile
                .max_buffered_bytes
                .checked_sub(fixed)
                .ok_or("whole worker capture has no retained-byte capacity")?;
            // Reserve simultaneous captures and later decoder/path-copy phases.
            // This is logical payload admission; actual native AS is separate.
            let share = remaining / 16;
            if share == 0 {
                return Err("whole worker capture has no retained-byte capacity".into());
            }
            let native = |cap: u64| {
                usize::try_from(cap)
                    .map_err(|error| format!("whole worker capture cap conversion: {error}"))
            };
            let names_bound = profile
                .max_retained_path_bytes
                .checked_add(profile.max_inventory_entries)
                .ok_or("whole worker name framing byte overflow")?;
            let budget = Self {
                raw: native(
                    share
                        .min(profile.file_size_bytes)
                        .min(profile.max_artifact_bytes[0])
                        .min(256 * 1024 * 1024),
                )?,
                presentation: native(
                    share
                        .min(profile.file_size_bytes)
                        .min(profile.max_artifact_bytes[2])
                        .min(256 * 1024 * 1024),
                )?,
                names: native(
                    share
                        .min(profile.file_size_bytes)
                        .min(profile.max_inventory_bytes)
                        .min(names_bound)
                        .min(256 * 1024 * 1024),
                )?,
                fixed,
            };
            budget.validate(profile)?;
            Ok(budget)
        }

        fn validate(&self, profile: &CompleteVerificationLimits) -> Result<(), String> {
            profile.validate()?;
            for (cap, bound) in [
                (self.raw, profile.max_artifact_bytes[0]),
                (self.presentation, profile.max_artifact_bytes[2]),
                (self.names, profile.max_inventory_bytes),
            ] {
                if cap == 0
                    || cap as u64 > bound
                    || cap as u64 > profile.file_size_bytes
                    || cap > 256 * 1024 * 1024
                {
                    return Err("whole worker original capture cap differs from profile".into());
                }
                phase_bytes(
                    profile.max_buffered_bytes,
                    &[self.fixed, stream_bytes(cap)?],
                )?;
            }
            Ok(())
        }
    }

    pub(in crate::app::pr_evidence) struct OriginalInputs {
        raw: Vec<u8>,
        presentation: String,
        changed_paths: Vec<String>,
        name_bytes: u64,
    }

    impl OriginalInputs {
        pub(in crate::app::pr_evidence) fn into_parts(self) -> (Vec<u8>, String, Vec<String>, u64) {
            (
                self.raw,
                self.presentation,
                self.changed_paths,
                self.name_bytes,
            )
        }
    }

    fn phase_bytes(limit: u64, terms: &[u64]) -> Result<u64, String> {
        let total = terms.iter().try_fold(0_u64, |sum, bytes| {
            sum.checked_add(*bytes)
                .ok_or("whole worker capture accounting overflow")
        })?;
        if total > limit {
            return Err("whole worker original capture buffer admission exceeded".into());
        }
        Ok(total)
    }

    fn stream_bytes(cap: usize) -> Result<u64, String> {
        let sentinel = cap
            .checked_add(1)
            .ok_or("whole worker capture sentinel overflow")?;
        (sentinel as u64)
            .checked_mul(2)
            .ok_or_else(|| "whole worker capture stream accounting overflow".into())
    }

    fn bound_original_names(
        original: &[u8],
        profile: &CompleteVerificationLimits,
        retained: u64,
    ) -> Result<(Vec<String>, u64), String> {
        if !original.is_empty() && original.last() != Some(&0) {
            return Err("whole worker name inventory lacks final NUL".into());
        }
        let count = original.iter().filter(|byte| **byte == 0).count();
        let path_bytes = original
            .len()
            .checked_sub(count)
            .ok_or("whole worker name framing accounting overflow")?;
        if count as u64 > profile.max_inventory_entries
            || path_bytes as u64 > profile.max_retained_path_bytes
            || original.len() as u64 > profile.max_inventory_bytes
        {
            return Err("whole worker name inventory admission exceeded".into());
        }
        let slots = count
            .checked_add(1)
            .ok_or("whole worker name slot overflow")?;
        let scratch = slots
            .checked_mul(std::mem::size_of::<&[u8]>())
            .ok_or("whole worker name parser scratch overflow")?;
        let records = count
            .checked_mul(std::mem::size_of::<PathBuf>() + std::mem::size_of::<String>())
            .ok_or("whole worker name record accounting overflow")?;
        let copies = (path_bytes as u64)
            .checked_mul(2)
            .ok_or("whole worker name payload accounting overflow")?;
        phase_bytes(
            profile.max_buffered_bytes,
            &[
                retained,
                original.len() as u64,
                scratch as u64,
                records as u64,
                copies,
            ],
        )?;
        // Shared parser owns the path grammar. Original bytes own String data.
        let parsed = crate::analysis::parse_git_path_records(original)
            .map_err(|error| format!("whole worker name inventory: {error}"))?;
        if parsed.len() != count {
            return Err("whole worker parsed name count differs from original records".into());
        }
        let mut names = Vec::new();
        names
            .try_reserve_exact(count)
            .map_err(|error| format!("whole worker name reservation: {error}"))?;
        let fields = original
            .strip_suffix(&[0])
            .unwrap_or(original)
            .split(|byte| *byte == 0);
        for (path, field) in parsed.iter().zip(fields) {
            if path.as_os_str().as_bytes() != field {
                return Err("whole worker native name bytes differ from original record".into());
            }
            let text = std::str::from_utf8(field)
                .map_err(|error| format!("whole worker original name UTF-8: {error}"))?;
            names.push(text.to_owned());
        }
        drop(parsed);
        names.sort_unstable();
        if names.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("whole worker name inventory contains duplicate records".into());
        }
        let charge = (count as u64)
            .checked_mul(std::mem::size_of::<String>() as u64)
            .and_then(|records| records.checked_add(path_bytes as u64))
            .ok_or("whole worker retained name accounting overflow")?;
        Ok((names, charge))
    }

    /// Actual original loaders under the collector restriction. This helper
    /// returns DATA only; the authentic caller supplies all native/source checks.
    fn capture_original_data(
        root: &Path,
        base: &str,
        head: &str,
        profile: &CompleteVerificationLimits,
        budget: &CaptureBudget,
        deadline: Instant,
        mut check: impl FnMut() -> Result<(), String>,
    ) -> Result<OriginalInputs, String> {
        budget.validate(profile)?;
        check()?;
        checkpoint(deadline)?;
        let raw = crate::git::with_complete_capture_restriction(deadline, budget.raw, || {
            crate::analysis::diff::load::load_canonical_pr_evidence_diff_bytes_bounded(
                root, base, head, budget.raw,
            )
        })
        .map_err(|error| error.to_string())?;
        check()?;
        phase_bytes(
            profile.max_buffered_bytes,
            &[
                budget.fixed,
                raw.len() as u64,
                stream_bytes(budget.presentation)?,
            ],
        )?;
        checkpoint(deadline)?;
        let presentation =
            crate::git::with_complete_capture_restriction(deadline, budget.presentation, || {
                crate::analysis::load_pr_evidence_diff_range(root, base, head)
                    .map_err(CoreError::message)
            })
            .map_err(|error| error.to_string())?;
        check()?;
        let retained = phase_bytes(
            profile.max_buffered_bytes,
            &[budget.fixed, raw.len() as u64, presentation.len() as u64],
        )?;
        phase_bytes(
            profile.max_buffered_bytes,
            &[retained, stream_bytes(budget.names)?],
        )?;
        checkpoint(deadline)?;
        let range = format!("{base}...{head}");
        let output = crate::git::with_complete_capture_restriction(deadline, budget.names, || {
            crate::git::run_git_output_with_deadline(
                root,
                &["diff", "--name-only", "-z", &range],
                Some(Duration::from_mins(5)),
            )
        })
        .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "whole worker full name inventory failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        check()?;
        let original_names = output.stdout;
        drop(output.stderr);
        let (changed_paths, name_bytes) = bound_original_names(&original_names, profile, retained)?;
        check()?;
        checkpoint(deadline)?;
        Ok(OriginalInputs {
            raw,
            presentation,
            changed_paths,
            name_bytes,
        })
    }

    /// Fixed retained witness allowance for the current native inspector only.
    /// Future retained output/render buffers require their own phase admission.
    pub(in crate::app::pr_evidence) const POSTFLIGHT_WITNESS_BYTES: u64 = 64 * 1024;

    fn claim_postflight_capture(attempted: &mut bool) -> Result<(), String> {
        if *attempted {
            return Err("whole worker postflight inputs capture was already attempted".into());
        }
        *attempted = true;
        Ok(())
    }

    /// Fresh original-input DATA under the caller's unchanged retained envelope.
    /// The genuine invocation method owns the separate one-shot phase claim.
    pub(in crate::app::pr_evidence) fn capture_postflight_data(
        root: &Path,
        base: &str,
        head: &str,
        profile: &CompleteVerificationLimits,
        retained_input_bytes: u64,
        deadline: Instant,
        check: impl FnMut() -> Result<(), String>,
    ) -> Result<OriginalInputs, String> {
        let fixed = phase_bytes(
            profile.max_buffered_bytes,
            &[retained_input_bytes, POSTFLIGHT_WITNESS_BYTES],
        )?;
        let budget = CaptureBudget::new(profile, fixed)?;
        capture_original_data(root, base, head, profile, &budget, deadline, check)
    }

    /// Worker-local authenticated invocation; no data constructor or cleanup grant.
    pub(in crate::app::pr_evidence) struct QualifiedWholeInvocation {
        entry: NativeAnalyzeEntry,
        subject: Option<CompleteSubject>,
        commitment: SubjectCommitment,
        options: PrEvidenceOptions,
        surface: ProducerSurface,
        check: CheckInput,
        profile: CompleteVerificationLimits,
        stage: StageRootBinding,
        generation_nonce: String,
        source: Arc<SourceAnchor>,
        build_identity: String,
        raw_attempted: bool,
        postflight_attempted: bool,
    }

    impl QualifiedWholeInvocation {
        /// Historical actual native DATA for the parent's post-exit receipt join.
        /// These observations grant neither settlement nor cleanup.
        pub(in crate::app::pr_evidence) fn observed_worker(&self) -> &ObservedProcessIdentity {
            &self.entry.worker
        }

        pub(in crate::app::pr_evidence) fn observed_parent(&self) -> &ObservedProcessIdentity {
            &self.entry.parent
        }

        /// Live worker/parent/profile/stage observations, never settlement.
        pub(in crate::app::pr_evidence) fn verify_analysis_current(&self) -> Result<(), String> {
            self.entry.recheck()?;
            if stable_build()? != self.build_identity {
                return Err("whole worker compiled build identity changed".into());
            }
            self.stage_current()?;
            checkpoint(self.entry.scalars.deadline)
        }

        pub(in crate::app::pr_evidence) fn take_subject(
            &mut self,
        ) -> Result<CompleteSubject, String> {
            if self.raw_attempted {
                return Err("whole worker canonical raw capture was already attempted".into());
            }
            self.entry.recheck()?;
            self.subject
                .take()
                .ok_or("whole worker subject was already consumed".into())
        }

        pub(in crate::app::pr_evidence) fn validate_startup(
            &self,
            startup: &NativeStartup,
            options: &PrEvidenceOptions,
            surface: ProducerSurface,
            check: &CheckInput,
        ) -> Result<(), String> {
            self.entry.recheck()?;
            if self.subject.is_none()
                || self.raw_attempted
                || options != &self.options
                || surface != self.surface
                || !same_check(check, &self.check)
                || startup.profile() != &self.profile
                || startup.deadline() != self.entry.scalars.deadline
                || self.source.deadline() != self.entry.scalars.deadline
                || startup.binding() != &self.stage
                || startup.generation_nonce() != self.generation_nonce
                || !Arc::ptr_eq(startup.source(), &self.source)
                || stable_build()? != self.build_identity
            {
                return Err("whole worker actual startup/input/source lease differs".into());
            }
            startup.verify_stage_current()?;
            self.source.verify_fresh()?;
            checkpoint(self.entry.scalars.deadline)
        }

        fn stage_current(&self) -> Result<(), String> {
            let path_limit = self.profile.max_retained_path_bytes.min(LITERAL_MAX as u64);
            let stage = RetainedDirectory::open_absolute(
                &self.stage.stage,
                path_limit,
                self.entry.scalars.deadline,
            )?;
            stage.require_role_entries(self.entry.scalars.deadline)?;
            let source =
                stage.open_child("source", &self.stage.source, self.entry.scalars.deadline)?;
            let spool =
                stage.open_child("spool", &self.stage.spool, self.entry.scalars.deadline)?;
            let artifacts = stage.open_child(
                "artifacts",
                &self.stage.artifacts,
                self.entry.scalars.deadline,
            )?;
            self.source.verify_materialized()?;
            source.verify_current(self.entry.scalars.deadline)?;
            spool.verify_current(self.entry.scalars.deadline)?;
            artifacts.verify_current(self.entry.scalars.deadline)?;
            stage.verify_current(self.entry.scalars.deadline)?;
            checkpoint(self.entry.scalars.deadline)
        }

        fn capture_current(&self) -> Result<(), String> {
            self.entry.recheck()?;
            if stable_build()? != self.build_identity {
                return Err("whole worker compiled build identity changed".into());
            }
            self.stage_current()?;
            let deadline = self.entry.scalars.deadline;
            RetainedDirectory::open_absolute(
                &self.stage.spool,
                self.profile.max_retained_path_bytes.min(LITERAL_MAX as u64),
                deadline,
            )?
            .require_empty(deadline)?;
            RetainedDirectory::open_absolute(
                &self.stage.artifacts,
                self.profile.max_retained_path_bytes.min(LITERAL_MAX as u64),
                deadline,
            )?
            .require_empty(deadline)?;
            checkpoint(deadline)
        }

        pub(in crate::app::pr_evidence) fn capture_original_inputs(
            &mut self,
            subject: &CompleteSubject,
            budget: CaptureBudget,
        ) -> Result<OriginalInputs, String> {
            if self.raw_attempted {
                return Err("whole worker original inputs capture was already attempted".into());
            }
            // Claim all phases before any validation, allocation or Git call.
            self.raw_attempted = true;
            if self.subject.is_some() || !self.commitment.matches(subject, &self.options) {
                return Err("whole worker original inputs subject differs".into());
            }
            require_whole_invocation_root(subject)?;
            self.capture_current()?;
            capture_original_data(
                &subject.root,
                subject.base_commit.as_str(),
                subject.head_commit.as_str(),
                &self.profile,
                &budget,
                self.entry.scalars.deadline,
                || self.capture_current(),
            )
        }

        /// Recapture while the same native/source owners and original clock remain held.
        /// This observes input DATA; it grants no settlement or publication.
        pub(in crate::app::pr_evidence) fn capture_postflight_inputs(
            &mut self,
            subject: &CompleteSubject,
            retained_input_bytes: u64,
            mut check: impl FnMut() -> Result<(), String>,
        ) -> Result<OriginalInputs, String> {
            // Claim before tuple/native checks, admission, allocation or Git.
            claim_postflight_capture(&mut self.postflight_attempted)?;
            if !self.raw_attempted
                || self.subject.is_some()
                || !self.commitment.matches(subject, &self.options)
            {
                return Err("whole worker postflight inputs subject differs".into());
            }
            require_whole_invocation_root(subject)?;
            capture_postflight_data(
                &subject.root,
                subject.base_commit.as_str(),
                subject.head_commit.as_str(),
                &self.profile,
                retained_input_bytes,
                self.entry.scalars.deadline,
                || {
                    self.verify_analysis_current()?;
                    check()
                },
            )
        }
    }

    /// The caller supplies the real factory/emitter continuation. No preparation-only
    /// success, analyzer invocation, settlement or publication is fabricated here.
    pub(in crate::app::pr_evidence) fn with_whole_worker(
        args: &[String],
        continuation: impl FnOnce(
            QualifiedWholeInvocation,
            NativeStartup,
            CompleteRequest,
            PrEvidenceOptions,
            ProducerSurface,
            CheckInput,
            CompleteVerificationLimits,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        let entered = Instant::now();
        let entry = NativeAnalyzeEntry::observe(args, entered)?;
        continue_whole_worker(entry, continuation)
    }

    /// Fixed ignored libtest entry; the unchanged CLI guard remains separate.
    pub(in crate::app::pr_evidence) fn with_libtest_worker(
        args: &[String],
        continuation: impl FnOnce(
            QualifiedWholeInvocation,
            NativeStartup,
            CompleteRequest,
            PrEvidenceOptions,
            ProducerSurface,
            CheckInput,
            CompleteVerificationLimits,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        let entered = Instant::now();
        let entry = NativeAnalyzeEntry::observe_libtest(args, entered)?;
        continue_whole_worker(entry, continuation)
    }

    fn continue_whole_worker(
        entry: NativeAnalyzeEntry,
        continuation: impl FnOnce(
            QualifiedWholeInvocation,
            NativeStartup,
            CompleteRequest,
            PrEvidenceOptions,
            ProducerSurface,
            CheckInput,
            CompleteVerificationLimits,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        // Blocking stdin is only byte-bounded here. The actual outer qualified
        // parent owns cancellation; clock checks do not preempt read syscalls.
        let header = read_header(std::io::stdin().lock(), &entry.scalars)?;
        entry.recheck()?;
        if header.parent.pid != entry.parent.pid()
            || header.parent.start != entry.parent.start()
            || header.parent.group != entry.parent.group()
            || (header.parent.address_space_bytes, header.parent.file_bytes) != entry.parent_limits
        {
            return Err(
                "whole worker startup parent differs from actual native observation".into(),
            );
        }
        if stable_build()? != header.build_identity {
            return Err("whole worker startup build differs from actual compiled identity".into());
        }
        let options = parse_options(&header.producer_args)?;
        if options.check || !options.base_explicit {
            return Err("whole worker requires its original explicit producer request".into());
        }
        let repo = std::env::current_dir().map_err(|error| format!("whole worker cwd: {error}"))?;
        let mut request =
            match select_request_with_deadline(&repo, &options.head, entry.scalars.deadline)? {
                RequestedRoute::Complete(request) => *request,
                RequestedRoute::Ordinary => {
                    return Err("whole worker committed request is absent".into());
                }
            };
        if request.binding() != &header.committed_request {
            return Err("whole worker committed request differs from authentic HEAD policy".into());
        }
        let subject = request.resolve_whole_subject(&options)?;
        require_whole_invocation_root(&subject)?;
        if !header.subject_commitment.matches(&subject, &options) {
            return Err(
                "whole worker complete subject differs from authentic Git resolution".into(),
            );
        }
        let check = surface_seed(&repo, &options, header.surface)?;
        let startup = NativeStartup::authenticate(
            &header.profile,
            entry.scalars.deadline,
            &header.stage,
            &header.generation_nonce,
        )?;
        startup.verify_stage_current()?;
        startup.source().verify_fresh()?;
        entry.recheck()?;
        let invocation = QualifiedWholeInvocation {
            entry,
            subject: Some(subject),
            commitment: header.subject_commitment,
            options: options.clone(),
            surface: header.surface,
            check: check.clone(),
            profile: header.profile.clone(),
            stage: header.stage,
            generation_nonce: header.generation_nonce,
            source: Arc::clone(startup.source()),
            build_identity: header.build_identity,
            raw_attempted: false,
            postflight_attempted: false,
        };
        invocation.validate_startup(&startup, &options, header.surface, &check)?;
        continuation(
            invocation,
            startup,
            request,
            options,
            header.surface,
            check,
            header.profile,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn refusal<T>(result: Result<T, String>, cause: &str) -> Result<(), String> {
            match result {
                Err(error) if error.contains(cause) => Ok(()),
                Err(error) => Err(format!("expected {cause}, observed {error}")),
                Ok(_) => Err(format!("unexpected acceptance of {cause}")),
            }
        }

        fn scalars() -> Result<Scalars, String> {
            Scalars::parse(
                &["1073741824", "16777216", "10000", "65536"].map(str::to_string),
                Instant::now(),
            )
        }

        // Wire DATA only. This never constructs NativeAnalyzeEntry,
        // NativeStartup or QualifiedWholeInvocation.
        fn header_value() -> Result<Value, String> {
            let binding = super::super::super::complete_contract::tests::fixture_binding()?;
            let mut subject = serde_json::to_value(&binding.subject)
                .map_err(|error| format!("startup subject fixture: {error}"))?;
            subject
                .as_object_mut()
                .ok_or("subject fixture is not an object")?
                .remove("changed_paths")
                .ok_or("subject fixture lacks changed_paths")?;
            let mut profile = binding.profile;
            profile.address_space_bytes = 1073741824;
            profile.file_size_bytes = 16777216;
            profile.deadline_ms = 10000;
            for cap in &mut profile.max_artifact_bytes {
                *cap = (*cap).min(profile.file_size_bytes);
            }
            profile.max_manifest_bytes = profile.max_manifest_bytes.min(profile.file_size_bytes);
            Ok(json!({
                "schema_version": STARTUP_SCHEMA,
                "profile": profile,
                "stage": {
                    "stage_nonce": "a".repeat(128),
                    "stage": {"path":WHOLE_STAGE_PATH, "dev":1, "ino":1},
                    "source": {"path":format!("{WHOLE_STAGE_PATH}/source"), "dev":1, "ino":2},
                    "spool": {"path":format!("{WHOLE_STAGE_PATH}/spool"), "dev":1, "ino":3},
                    "artifacts": {"path":format!("{WHOLE_STAGE_PATH}/artifacts"), "dev":1, "ino":4}
                },
                "generation_nonce": "b".repeat(32),
                "parent": {"pid":1,"start":2,"group":1,
                    "address_space_bytes":1073741824,"file_bytes":16777216},
                "build_identity": "wire-data-only",
                "surface": "installed",
                "producer_args": ["--root", binding.subject.requested_root,
                    "--base", binding.subject.requested_base, "--head", binding.subject.requested_head],
                "committed_request": binding.committed_request,
                "subject_commitment": subject
            }))
        }

        fn encode(value: &Value) -> Result<Vec<u8>, String> {
            serde_json::to_vec(value).map_err(|error| format!("startup fixture JSON: {error}"))
        }

        #[test]
        fn closed_worker_entry_refuses_bad_scalars_before_stdin_or_continuation()
        -> Result<(), String> {
            let called = std::cell::Cell::new(false);
            refusal(
                with_whole_worker(&[], |_, _, _, _, _, _, _| {
                    called.set(true);
                    Err("continuation must not run".into())
                }),
                "exactly four",
            )?;
            assert!(!called.get());
            for args in [
                ["1", "1", "0", "65536"],
                ["1", "1", "1", "0"],
                ["1", "1", "1", "65537"],
                ["1", "1", "invalid", "65536"],
            ] {
                refusal(
                    Scalars::parse(&args.map(str::to_string), Instant::now()),
                    "whole worker",
                )?;
            }
            let entered = Instant::now()
                .checked_sub(Duration::from_secs(1))
                .ok_or("expired scalar fixture clock unavailable")?;
            refusal(
                Scalars::parse(&["1", "1", "1", "65536"].map(str::to_string), entered),
                "original deadline exhausted",
            )?;
            Ok(())
        }

        #[test]
        fn actual_parent_profile_parser_refuses_address_space_before_file_or_core()
        -> Result<(), String> {
            let valid = "Max address space 1073741824 1073741824 bytes\nMax file size 16777216 16777216 bytes\nMax core file size 0 0 bytes\n";
            assert_eq!(parent_profile_from_limits(valid)?, (1073741824, 16777216));
            for address_space in ["0 0", "1073741824 2147483648", "2147483649 2147483649"] {
                let malformed = format!(
                    "Max address space {address_space} bytes\nMax file size unlimited unlimited bytes\nMax core file size unlimited unlimited bytes\n",
                );
                refusal(
                    parent_profile_from_limits(&malformed),
                    "Max address space is not",
                )?;
            }
            refusal(
                parent_profile_from_limits(
                    &valid.replace("16777216 16777216", "unlimited unlimited"),
                ),
                "nonfinite Max file size",
            )?;
            refusal(
                parent_profile_from_limits(&valid.replace("0 0 bytes", "0 1 bytes")),
                "core-file limit is not zero",
            )?;
            Ok(())
        }

        #[test]
        fn closed_startup_rejects_missing_duplicate_unknown_and_trailing_input()
        -> Result<(), String> {
            let scalars = scalars()?;
            let valid = header_value()?;
            let bytes = encode(&valid)?;
            let decoded = decode_header(&bytes, &scalars)?;
            assert_eq!(decoded.generation_nonce, "b".repeat(32));
            let mut missing = valid.clone();
            missing
                .as_object_mut()
                .ok_or("missing fixture object")?
                .remove("parent")
                .ok_or("fixture lacks parent")?;
            refusal(decode_header(&encode(&missing)?, &scalars), "missing field")?;
            let mut unknown = valid.clone();
            unknown["phase"] = json!("plan_inventory");
            refusal(decode_header(&encode(&unknown)?, &scalars), "unknown field")?;
            let text = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
            let duplicate = format!("{{\"schema_version\":\"{STARTUP_SCHEMA}\",{}", &text[1..]);
            refusal(
                decode_header(duplicate.as_bytes(), &scalars),
                "duplicate field",
            )?;
            let mut trailing = bytes.clone();
            trailing.extend_from_slice(b"{}");
            refusal(decode_header(&trailing, &scalars), "trailing characters")?;
            let mut nul = bytes;
            nul.push(0);
            refusal(decode_header(&nul, &scalars), "closed startup JSON")?;
            Ok(())
        }

        #[test]
        fn closed_whole_entry_refuses_stage_namespace_override() -> Result<(), String> {
            let scalars = scalars()?;
            let supported = header_value()?;
            let header = decode_header(&encode(&supported)?, &scalars)?;
            require_fixed_stage(&header.stage)?;
            for path in [
                "/tmp/ripr-complete-retained",
                "/var/tmp/ripr-complete-retained/other",
            ] {
                let mut invalid = supported.clone();
                invalid["stage"]["stage"]["path"] = json!(path);
                refusal(
                    decode_header(&encode(&invalid)?, &scalars),
                    "stage namespace override",
                )?;
            }
            Ok(())
        }

        #[test]
        fn startup_profiles_literals_and_argument_growth_fail_closed() -> Result<(), String> {
            let scalars = scalars()?;
            for (path, value, cause) in [
                (
                    "/profile/address_space_bytes",
                    json!(1073741825_u64),
                    "admitted scalar",
                ),
                ("/profile/deadline_ms", json!(9999), "admitted scalar"),
                (
                    "/generation_nonce",
                    json!("b".repeat(31)),
                    "generation nonce",
                ),
                (
                    "/subject_commitment/requested_base",
                    json!("bad\nbase"),
                    "original literal",
                ),
                (
                    "/subject_commitment/logical_root",
                    json!("/repo/./"),
                    "canonical absolute",
                ),
                (
                    "/subject_commitment/origin_tree",
                    json!("A".repeat(40)),
                    "canonical",
                ),
                (
                    "/profile/max_buffered_bytes",
                    json!(1),
                    "logical payload admission",
                ),
            ] {
                let mut invalid = header_value()?;
                *invalid.pointer_mut(path).ok_or("invalid fixture path")? = value;
                refusal(decode_header(&encode(&invalid)?, &scalars), cause)?;
            }
            let mut too_many = header_value()?;
            too_many["producer_args"] = json!(vec!["--root"; ARGS_MAX + 1]);
            refusal(
                decode_header(&encode(&too_many)?, &scalars),
                "argument count",
            )?;
            let mut too_long = header_value()?;
            too_long["producer_args"][0] = json!("x".repeat(LITERAL_MAX + 1));
            refusal(
                decode_header(&encode(&too_long)?, &scalars),
                "original literal",
            )?;
            Ok(())
        }

        #[test]
        fn startup_read_requires_real_eof_and_preserves_read_error_and_exact_cap()
        -> Result<(), String> {
            let bytes = encode(&header_value()?)?;
            let mut limits = scalars()?;
            limits.startup_cap = bytes.len() as u64;
            let decoded = read_header(std::io::Cursor::new(&bytes), &limits)?;
            assert_eq!(decoded.schema_version, STARTUP_SCHEMA);
            limits.startup_cap = limits.startup_cap.checked_sub(1).ok_or("empty fixture")?;
            refusal(
                read_header(std::io::Cursor::new(&bytes), &limits),
                "input exceeds",
            )?;
            struct FailingRead(bool);
            impl Read for FailingRead {
                fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                    if self.0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "actual partial read failure",
                        ));
                    }
                    self.0 = true;
                    if output.is_empty() {
                        return Ok(0);
                    }
                    output[0] = b'{';
                    Ok(1)
                }
            }
            refusal(
                read_header(FailingRead(false), &scalars()?),
                "actual partial read failure",
            )?;
            refusal(
                read_header(std::io::Cursor::new([0xff]), &scalars()?),
                "valid UTF-8",
            )?;
            Ok(())
        }

        #[test]
        fn actual_nested_invocation_with_absolute_whole_root_refuses_before_whole_entry()
        -> Result<(), String> {
            use crate::testing::fixture_git::{fixture_git_ok, remove_fixture_tree};

            struct Fixture(PathBuf);
            impl Drop for Fixture {
                fn drop(&mut self) {
                    let _ = remove_fixture_tree(&self.0);
                }
            }
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let fixture = Fixture(std::env::temp_dir().join(format!(
                "ripr-whole-worker-nested-{}-{stamp}",
                std::process::id(),
            )));
            fs::create_dir_all(fixture.0.join("nested")).map_err(|error| error.to_string())?;
            fixture_git_ok(
                &fixture.0,
                &["-c", "init.templateDir=", "init", "--quiet", "-b", "whole"],
            )?;
            fixture_git_ok(
                &fixture.0,
                &[
                    "config",
                    "--local",
                    "user.name",
                    "RIPR whole worker fixture",
                ],
            )?;
            fixture_git_ok(
                &fixture.0,
                &["config", "--local", "user.email", "whole@example.invalid"],
            )?;
            fixture_git_ok(
                &fixture.0,
                &["config", "--local", "commit.gpgsign", "false"],
            )?;
            fs::write(fixture.0.join("outside.rs"), "pub const OUTSIDE: u8 = 1;\n")
                .map_err(|error| error.to_string())?;
            fs::write(
                fixture.0.join("nested/inside.rs"),
                "pub const INSIDE: u8 = 1;\n",
            )
            .map_err(|error| error.to_string())?;
            fixture_git_ok(&fixture.0, &["add", "--", "outside.rs", "nested/inside.rs"])?;
            fixture_git_ok(&fixture.0, &["commit", "--quiet", "-m", "whole base"])?;
            fixture_git_ok(&fixture.0, &["tag", "whole-base"])?;
            fs::create_dir_all(fixture.0.join(".ripr")).map_err(|error| error.to_string())?;
            fs::write(
                fixture.0.join(".ripr/complete-evidence.json"),
                br#"{"schema_version":"ripr.complete_request.v1","request":"complete","profile":"whole-head-v1"}"#,
            ).map_err(|error| error.to_string())?;
            fs::write(fixture.0.join("outside.rs"), "pub const OUTSIDE: u8 = 2;\n")
                .map_err(|error| error.to_string())?;
            fs::write(
                fixture.0.join("nested/inside.rs"),
                "pub const INSIDE: u8 = 2;\n",
            )
            .map_err(|error| error.to_string())?;
            fixture_git_ok(
                &fixture.0,
                &["add", "--", ".ripr", "outside.rs", "nested/inside.rs"],
            )?;
            fixture_git_ok(&fixture.0, &["commit", "--quiet", "-m", "whole request"])?;
            let root = fs::canonicalize(&fixture.0).map_err(|error| error.to_string())?;
            let nested =
                fs::canonicalize(root.join("nested")).map_err(|error| error.to_string())?;
            let options = PrEvidenceOptions {
                root: root.to_str().ok_or("fixture root is not UTF-8")?.into(),
                base: "refs/tags/whole-base".into(),
                base_explicit: true,
                head: "HEAD".into(),
                check: false,
            };
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(30))
                .ok_or("fixture deadline overflow")?;
            let mut request = match select_request_with_deadline(&nested, "HEAD", deadline)? {
                RequestedRoute::Complete(request) => request,
                RequestedRoute::Ordinary => {
                    return Err("real committed policy was not selected".into());
                }
            };
            let subject = request.resolve_whole_subject(&options)?;
            assert_eq!(
                subject.invocation_repository.as_os_str(),
                nested.as_os_str()
            );
            assert_eq!(subject.root.as_os_str(), root.as_os_str());
            assert_eq!(subject.work_tree.as_os_str(), root.as_os_str());
            refusal(
                require_whole_invocation_root(&subject),
                "nested invocation is unsupported",
            )?;
            // The same real committed policy and whole subject are supported
            // from the existing production invocation root; no token is forged.
            let mut supported = match select_request_with_deadline(&root, "HEAD", deadline)? {
                RequestedRoute::Complete(request) => request,
                RequestedRoute::Ordinary => return Err("whole-root policy recovery failed".into()),
            };
            let subject = supported.resolve_whole_subject(&options)?;
            require_whole_invocation_root(&subject)?;
            Ok(())
        }

        #[test]
        fn surface_seed_preserves_input_base_selection_and_all_native_path_spelling()
        -> Result<(), String> {
            let options = PrEvidenceOptions {
                root: ".".into(),
                base: "original-base".into(),
                base_explicit: true,
                head: "original-head".into(),
                check: false,
            };
            let installed = surface_seed(Path::new("/repo"), &options, ProducerSurface::Installed)?;
            assert_eq!(installed.base, None);
            assert_eq!(installed.git_timeout, None);
            assert!(installed.include_unchanged_tests);
            assert_eq!(installed.root.as_os_str(), std::ffi::OsStr::new("/repo/."));
            let copied = installed.clone();
            assert!(same_check(&installed, &copied));
            let mut different = copied;
            different.root = PathBuf::from("/repo");
            assert!(!same_check(&installed, &different));
            let timeout = crate::cli::commands::git_timeout_from_env(
                false,
                std::env::var("RIPR_GIT_TIMEOUT"),
            )?
            .unwrap_or(Some(crate::app::default_cli_git_timeout()));
            let xtask = surface_seed(Path::new("/repo"), &options, ProducerSurface::Xtask)?;
            assert_eq!(xtask.base.as_deref(), Some("original-base"));
            assert_eq!(xtask.git_timeout, timeout);
            assert!(!xtask.include_unchanged_tests);
            Ok(())
        }

        fn capture_profile() -> Result<CompleteVerificationLimits, String> {
            Ok(super::super::super::complete_contract::tests::fixture_binding()?.profile)
        }

        #[test]
        fn original_name_records_use_shared_grammar_and_preserve_native_bytes() -> Result<(), String>
        {
            let profile = capture_profile()?;
            let original = "z\nline.rs\0a\tλ.py\0deleted.rs\0binary.dat\0";
            let (names, charge) = bound_original_names(original.as_bytes(), &profile, 0)?;
            assert_eq!(names, ["a\tλ.py", "binary.dat", "deleted.rs", "z\nline.rs"]);
            assert!(charge >= original.len() as u64 - 4);
            assert_eq!(
                bound_original_names(b"", &profile, 0)?.0,
                Vec::<String>::new()
            );
            refusal(bound_original_names(b"a.rs", &profile, 0), "final NUL")?;
            refusal(bound_original_names(b"a.rs\0\0", &profile, 0), "is empty")?;
            refusal(
                bound_original_names(b"a\xff.rs\0", &profile, 0),
                "not valid UTF-8",
            )?;
            refusal(
                bound_original_names(b"a.rs\0a.rs\0", &profile, 0),
                "duplicate records",
            )?;
            Ok(())
        }

        #[test]
        fn original_name_phase_refuses_count_path_and_combined_retention_before_copy()
        -> Result<(), String> {
            let profile = capture_profile()?;
            let mut limited = profile.clone();
            limited.max_inventory_entries = 1;
            refusal(
                bound_original_names(b"a\0b\0", &limited, 0),
                "inventory admission",
            )?;
            limited = profile.clone();
            limited.max_retained_path_bytes = 1;
            refusal(
                bound_original_names(b"aa\0", &limited, 0),
                "inventory admission",
            )?;
            limited = profile.clone();
            limited.max_inventory_bytes = 1;
            refusal(
                bound_original_names(b"a\0", &limited, 0),
                "inventory admission",
            )?;
            refusal(
                bound_original_names(b"a\0", &profile, profile.max_buffered_bytes),
                "buffer admission",
            )?;
            refusal(phase_bytes(u64::MAX, &[u64::MAX, 1]), "accounting overflow")?;
            refusal(stream_bytes(usize::MAX), "sentinel overflow")?;
            assert_eq!(bound_original_names(b"a\0", &profile, 0)?.0, ["a"]);
            Ok(())
        }

        struct CaptureFixture(PathBuf);
        impl Drop for CaptureFixture {
            fn drop(&mut self) {
                let _ = crate::testing::fixture_git::remove_fixture_tree(&self.0);
            }
        }
        impl CaptureFixture {
            fn new() -> Result<(Self, String, String), String> {
                use crate::testing::fixture_git::fixture_git_ok;
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|error| error.to_string())?
                    .as_nanos();
                let fixture = Self(std::env::temp_dir().join(format!(
                    "ripr-whole-original-{}-{stamp}",
                    std::process::id(),
                )));
                fs::create_dir_all(&fixture.0).map_err(|error| error.to_string())?;
                let git = |args: &[&str]| fixture_git_ok(&fixture.0, args);
                git(&["-c", "init.templateDir=", "init", "--quiet", "-b", "whole"])?;
                git(&["config", "--local", "user.name", "RIPR original capture"])?;
                git(&["config", "--local", "user.email", "capture@example.invalid"])?;
                git(&["config", "--local", "commit.gpgsign", "false"])?;
                for (path, bytes) in [
                    ("deleted.rs", b"pub const OLD: u8 = 1;\n".as_slice()),
                    ("nested/a.rs", b"pub const A: u8 = 1;\n".as_slice()),
                    ("outside.rs", b"pub const B: u8 = 1;\n".as_slice()),
                    ("binary.dat", b"\0old\xff".as_slice()),
                ] {
                    let path = fixture.0.join(path);
                    fs::create_dir_all(path.parent().ok_or("capture fixture has no parent")?)
                        .map_err(|error| error.to_string())?;
                    fs::write(path, bytes).map_err(|error| error.to_string())?;
                }
                git(&["add", "--all"])?;
                git(&["commit", "--quiet", "-m", "capture base"])?;
                let revision = || -> Result<String, String> {
                    let output = crate::git::run_git_output_with_deadline_and_limit_isolated(
                        &fixture.0,
                        &["rev-parse", "HEAD"],
                        Duration::from_secs(30),
                        4096,
                    )
                    .map_err(|error| error.to_string())?;
                    if !output.status.success() {
                        return Err(format!(
                            "capture fixture revision failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                    String::from_utf8(output.stdout)
                        .map(|text| text.trim().to_string())
                        .map_err(|error| format!("capture fixture revision UTF-8: {error}"))
                };
                let base = revision()?;
                fs::remove_file(fixture.0.join("deleted.rs")).map_err(|error| error.to_string())?;
                fs::write(fixture.0.join("nested/a.rs"), b"pub const A: u8 = 2;\n")
                    .map_err(|error| error.to_string())?;
                fs::write(fixture.0.join("outside.rs"), b"pub const B: u8 = 2;\n")
                    .map_err(|error| error.to_string())?;
                let mut binary = vec![0_u8; 65537];
                binary[65536] = 0xff;
                fs::write(fixture.0.join("binary.dat"), binary)
                    .map_err(|error| error.to_string())?;
                fs::write(fixture.0.join("λ\tline\nname.py"), b"VALUE = 2\n")
                    .map_err(|error| error.to_string())?;
                git(&["add", "--all"])?;
                git(&["commit", "--quiet", "-m", "capture head"])?;
                let head = revision()?;
                Ok((fixture, base, head))
            }
        }

        fn pure_rename_fixture() -> Result<(CaptureFixture, String, String), String> {
            use crate::testing::fixture_git::fixture_git_ok;

            let (fixture, _, _) = CaptureFixture::new()?;
            fs::write(
                fixture.0.join("Cargo.toml"),
                "[package]\nname = \"rename_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"nested/a.rs\"\n",
            )
            .map_err(|error| format!("rename fixture manifest: {error}"))?;
            fs::write(
                fixture.0.join("ripr.toml"),
                "[analysis]\nmode = \"draft\"\n[languages]\nenabled = [\"rust\"]\n",
            )
            .map_err(|error| format!("rename fixture configuration: {error}"))?;
            fs::write(fixture.0.join(".gitignore"), "target/\n")
                .map_err(|error| format!("rename fixture target ignore: {error}"))?;
            fixture_git_ok(
                &fixture.0,
                &["add", "--", "Cargo.toml", "ripr.toml", ".gitignore"],
            )?;
            fixture_git_ok(&fixture.0, &["commit", "--quiet", "-m", "rename baseline"])?;
            let revision = || -> Result<String, String> {
                let output = crate::git::run_git_output_with_deadline_and_limit_isolated(
                    &fixture.0,
                    &["rev-parse", "HEAD"],
                    Duration::from_secs(30),
                    4096,
                )
                .map_err(|error| error.to_string())?;
                if !output.status.success() {
                    return Err(format!(
                        "pure rename fixture revision failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
                String::from_utf8(output.stdout)
                    .map(|text| text.trim().to_string())
                    .map_err(|error| format!("pure rename fixture revision UTF-8: {error}"))
            };
            let base = revision()?;
            fixture_git_ok(&fixture.0, &["mv", "outside.rs", "renamed.rs"])?;
            fixture_git_ok(&fixture.0, &["commit", "--quiet", "-m", "pure rename"])?;
            let head = revision()?;
            assert_ne!(base, head, "pure rename must create a distinct real commit");
            Ok((fixture, base, head))
        }

        #[test]
        fn actual_rename_config_keeps_scoped_loader_parity_and_changes_bound_inputs()
        -> Result<(), String> {
            use super::super::super::complete_contract::sha256_bytes;
            use super::super::super::raw_coverage::{RawCoverageLimits, build_raw_coverage};
            use crate::testing::fixture_git::fixture_git_ok;

            let (fixture, base, head) = pure_rename_fixture()?;
            let profile = capture_profile()?;
            let budget = CaptureBudget::new(&profile, 0)?;
            let deadline = Instant::now()
                .checked_add(Duration::from_mins(1))
                .ok_or("rename capture fixture deadline overflow")?;
            let native = |value| {
                usize::try_from(value)
                    .map_err(|error| format!("rename fixture native admission: {error}"))
            };
            let limits = RawCoverageLimits {
                file_limit: native(profile.file_limit)?,
                max_raw_bytes: budget.raw,
                max_records: native(profile.max_raw_records)?,
                max_ledger_bytes: native(profile.max_artifact_bytes[3])?,
                max_retained_path_bytes: native(profile.max_retained_path_bytes)?,
                max_projection_bytes: native(profile.max_raw_projection_bytes)?,
            };
            let range = format!("{base}...{head}");
            let mut observations = Vec::new();
            for setting in ["true", "false"] {
                fixture_git_ok(&fixture.0, &["config", "--local", "diff.renames", setting])?;
                // Real original loaders, not a config-isolated alternate route.
                // This captures DATA only, never native or completion authority.
                let captured = capture_original_data(
                    &fixture.0,
                    &base,
                    &head,
                    &profile,
                    &budget,
                    deadline,
                    || Ok(()),
                )?;
                let ordinary_raw =
                    crate::analysis::diff::load::load_canonical_pr_evidence_diff_bytes(
                        &fixture.0, &base, &head,
                    )?;
                let ordinary_presentation =
                    crate::analysis::load_pr_evidence_diff_range(&fixture.0, &base, &head)?;
                let ordinary_names = crate::git::run_git_output_with_deadline(
                    &fixture.0,
                    &["diff", "--name-only", "-z", &range],
                    Some(Duration::from_mins(5)),
                )
                .map_err(|error| error.to_string())?;
                if !ordinary_names.status.success() {
                    return Err(format!(
                        "ordinary rename name inventory failed: {}",
                        String::from_utf8_lossy(&ordinary_names.stderr)
                    ));
                }
                let (ordinary_paths, _) =
                    bound_original_names(&ordinary_names.stdout, &profile, 0)?;
                assert_eq!(captured.raw, ordinary_raw, "{setting}");
                assert_eq!(captured.presentation, ordinary_presentation, "{setting}");
                assert_eq!(captured.changed_paths, ordinary_paths, "{setting}");
                let raw_text = std::str::from_utf8(&captured.raw)
                    .map_err(|error| format!("rename fixture raw UTF-8: {error}"))?;
                let (parsed, coverage) = build_raw_coverage(&captured.raw, limits)?;
                let summary = coverage.summary();
                if setting == "true" {
                    assert!(raw_text.contains("similarity index 100%"), "{raw_text}");
                    assert!(raw_text.contains("rename from outside.rs"), "{raw_text}");
                    assert!(raw_text.contains("rename to renamed.rs"), "{raw_text}");
                    assert_eq!(summary.added_lines, 0);
                    assert_eq!(summary.removed_lines, 0);
                    assert_eq!(captured.changed_paths, ["renamed.rs"]);
                } else {
                    assert!(raw_text.contains("deleted file mode"), "{raw_text}");
                    assert!(raw_text.contains("new file mode"), "{raw_text}");
                    assert!(raw_text.contains("-pub const B: u8 = 2;\n"), "{raw_text}");
                    // Deleted /dev/null bodies are consumed original records,
                    // not lines retained in the semantic source projection.
                    assert_eq!(summary.added_lines, 1);
                    assert_eq!(summary.removed_lines, 0);
                    assert_eq!(parsed.deleted_file_count, 1);
                    assert_eq!(parsed.changed_files.len(), 1);
                    let file = parsed
                        .changed_files
                        .first()
                        .ok_or("delete/add projection lost the new file")?;
                    assert_eq!(file.path, Path::new("renamed.rs"));
                    assert_eq!(file.added_lines.len(), 1);
                    assert!(file.removed_lines.is_empty());
                    let added = file
                        .added_lines
                        .first()
                        .ok_or("delete/add projection lost the new source line")?;
                    assert_eq!(added.line, 1);
                    assert_eq!(added.new_side_line, 1);
                    assert_eq!(added.text, "pub const B: u8 = 2;");

                    let mut deleted_body = None;
                    for row in coverage
                        .ledger_bytes()
                        .split_inclusive(|byte| *byte == b'\n')
                    {
                        let fact: Value = serde_json::from_slice(row)
                            .map_err(|error| format!("rename fixture ledger JSON: {error}"))?;
                        if fact["tag"] == "record"
                            && fact["kind"] == "body_no_path"
                            && deleted_body.replace(fact).is_some()
                        {
                            return Err("deleted body was recorded more than once".into());
                        }
                    }
                    let body = deleted_body.ok_or("ledger lost the actual deleted body")?;
                    let start = native(
                        body["start"]
                            .as_u64()
                            .ok_or("deleted body start is not a byte offset")?,
                    )?;
                    let end = native(
                        body["end"]
                            .as_u64()
                            .ok_or("deleted body end is not a byte offset")?,
                    )?;
                    assert_eq!(
                        captured
                            .raw
                            .get(start..end)
                            .ok_or("deleted body span escaped raw input")?,
                        b"-pub const B: u8 = 2;\n"
                    );
                    assert_eq!(body["remaining_before"], json!([1, 0]));
                    assert_eq!(body["remaining_after"], json!([0, 0]));
                    assert_eq!(body["consumed"], json!([1, 0]));
                    assert!(
                        body.get("projection")
                            .ok_or("deleted body lacks its projection field")?
                            .is_null()
                    );
                    assert_eq!(captured.changed_paths, ["outside.rs", "renamed.rs"]);
                }
                observations.push((captured, coverage));
            }
            let (rename, rest) = observations
                .split_first()
                .ok_or("rename capture is missing")?;
            let delete_add = rest.first().ok_or("delete/add capture is missing")?;
            assert_ne!(
                rename.1.summary().raw_sha256,
                delete_add.1.summary().raw_sha256
            );
            assert_ne!(
                rename.1.summary().projection_sha256,
                delete_add.1.summary().projection_sha256
            );
            assert_ne!(
                sha256_bytes(rename.0.presentation.as_bytes()),
                sha256_bytes(delete_add.0.presentation.as_bytes())
            );
            assert_ne!(rename.0.changed_paths, delete_add.0.changed_paths);
            Ok(())
        }

        #[test]
        fn actual_rename_config_drift_refuses_saved_canonical_input_then_recovers()
        -> Result<(), String> {
            use crate::testing::fixture_git::fixture_git_ok;

            let (fixture, base, head) = pure_rename_fixture()?;
            let options = PrEvidenceOptions {
                root: ".".into(),
                base,
                base_explicit: true,
                head,
                check: false,
            };
            fixture_git_ok(&fixture.0, &["config", "--local", "diff.renames", "true"])?;
            // Publish genuine ordinary producer artifacts, without a generation
            // marker, mocked check JSON or a complete-execution token.
            write_pr_evidence(&fixture.0, &options)?;
            check_pr_evidence(&fixture.0, &options)?;
            let saved = fs::read(fixture.0.join(PR_CANONICAL_DIFF))
                .map_err(|error| format!("saved rename canonical input: {error}"))?;
            assert!(
                std::str::from_utf8(&saved)
                    .map_err(|error| error.to_string())?
                    .contains("rename from outside.rs"),
                "ordinary producer did not retain the real pure rename"
            );
            fixture_git_ok(&fixture.0, &["config", "--local", "diff.renames", "false"])?;
            // This is the actual canonical-input admission called by saved
            // --check. The outer packet check can first notice changed names.
            let failure = validate_producer_artifacts(&fixture.0, &options)
                .err()
                .ok_or("saved canonical input admitted a different actual Git diff")?;
            assert!(
                failure.contains("does not match the requested canonical base/head diff"),
                "{failure}"
            );
            assert_eq!(
                fs::read(fixture.0.join(PR_CANONICAL_DIFF)).map_err(|error| error.to_string())?,
                saved,
                "refusal must not rewrite the saved canonical input"
            );
            fixture_git_ok(&fixture.0, &["config", "--local", "diff.renames", "true"])?;
            check_pr_evidence(&fixture.0, &options)?;
            assert_eq!(
                fs::read(fixture.0.join(PR_CANONICAL_DIFF)).map_err(|error| error.to_string())?,
                saved,
                "recovery must reuse the original ordinary artifacts"
            );
            // The native complete Postflight input recapture/join remains
            // unimplemented; this ordinary-consumer control cannot grant it.
            Ok(())
        }

        #[test]
        fn actual_original_capture_matches_u0_u3_and_full_deleted_binary_name_inventory()
        -> Result<(), String> {
            let (fixture, base, head) = CaptureFixture::new()?;
            let profile = capture_profile()?;
            let budget = CaptureBudget::new(&profile, 0)?;
            let deadline = Instant::now()
                .checked_add(Duration::from_mins(1))
                .ok_or("capture fixture deadline overflow")?;
            let called = std::cell::Cell::new(0);
            let captured = capture_original_data(
                &fixture.0,
                &base,
                &head,
                &profile,
                &budget,
                deadline,
                || {
                    called.set(called.get() + 1);
                    Ok(())
                },
            )?;
            assert!(
                called.get() >= 5,
                "capture did not reach every actual phase"
            );
            assert_eq!(
                captured.raw,
                crate::analysis::diff::load::load_canonical_pr_evidence_diff_bytes_bounded(
                    &fixture.0, &base, &head, budget.raw,
                )
                .map_err(|error| error.to_string())?
            );
            assert_eq!(
                captured.presentation,
                crate::analysis::load_pr_evidence_diff_range(&fixture.0, &base, &head)?
            );
            assert_eq!(
                captured.changed_paths,
                [
                    "binary.dat",
                    "deleted.rs",
                    "nested/a.rs",
                    "outside.rs",
                    "λ\tline\nname.py",
                ]
            );
            let zero = capture_original_data(
                &fixture.0,
                &head,
                &head,
                &profile,
                &budget,
                deadline,
                || Ok(()),
            )?;
            assert!(zero.raw.is_empty());
            assert!(zero.presentation.is_empty());
            assert!(zero.changed_paths.is_empty());
            Ok(())
        }

        #[test]
        fn original_capture_late_phase_refusal_and_each_real_stream_cap_require_recovery()
        -> Result<(), String> {
            let (fixture, base, head) = CaptureFixture::new()?;
            let profile = capture_profile()?;
            let deadline = Instant::now()
                .checked_add(Duration::from_mins(1))
                .ok_or("capture refusal fixture deadline overflow")?;
            let budget = CaptureBudget::new(&profile, 0)?;
            let called = std::cell::Cell::new(0);
            refusal(
                capture_original_data(
                    &fixture.0,
                    &base,
                    &head,
                    &profile,
                    &budget,
                    deadline,
                    || {
                        called.set(called.get() + 1);
                        if called.get() == 3 {
                            Err("actual post-presentation custody refusal".into())
                        } else {
                            Ok(())
                        }
                    },
                ),
                "post-presentation custody refusal",
            )?;
            assert_eq!(called.get(), 3);
            // Each cap is applied to a real original helper. Earlier phases
            // are admitted; no synthetic collector success or QWI is created.
            for phase in 0..3 {
                let mut budget = CaptureBudget::new(&profile, 0)?;
                match phase {
                    0 => budget.raw = 1,
                    1 => budget.presentation = 1,
                    _ => budget.names = 1,
                }
                let result = capture_original_data(
                    &fixture.0,
                    &base,
                    &head,
                    &profile,
                    &budget,
                    deadline,
                    || Ok(()),
                );
                match result {
                    Err(error)
                        if error.contains("limit")
                            || error.contains("cap")
                            || error.contains("exceed") => {}
                    Err(error) => return Err(format!("wrong actual capture cap refusal: {error}")),
                    Ok(_) => return Err(format!("original capture phase {phase} ignored its cap")),
                }
            }
            let recovered = capture_original_data(
                &fixture.0,
                &base,
                &head,
                &profile,
                &budget,
                deadline,
                || Ok(()),
            )?;
            assert!(
                recovered
                    .changed_paths
                    .iter()
                    .any(|path| path == "deleted.rs")
            );
            Ok(())
        }

        #[test]
        fn postflight_once_claim_covers_admission_and_real_late_capture_refusals()
        -> Result<(), String> {
            let (fixture, base, head) = CaptureFixture::new()?;
            let profile = capture_profile()?;
            let deadline = Instant::now()
                .checked_add(Duration::from_mins(1))
                .ok_or("postflight capture fixture clock overflow")?;
            let mut attempted = false;
            claim_postflight_capture(&mut attempted)?;
            let called = std::cell::Cell::new(0);
            refusal(
                capture_postflight_data(
                    &fixture.0,
                    &base,
                    &head,
                    &profile,
                    profile.max_buffered_bytes,
                    deadline,
                    || {
                        called.set(called.get() + 1);
                        Ok(())
                    },
                ),
                "admission",
            )?;
            assert_eq!(called.get(), 0);
            refusal(claim_postflight_capture(&mut attempted), "already attempted")?;
            let mut independent = false;
            claim_postflight_capture(&mut independent)?;
            refusal(
                capture_postflight_data(
                    &fixture.0,
                    &base,
                    &head,
                    &profile,
                    0,
                    deadline,
                    || {
                        called.set(called.get() + 1);
                        if called.get() == 3 {
                            Err("actual postflight post-presentation observation refused".into())
                        } else {
                            Ok(())
                        }
                    },
                ),
                "post-presentation observation refused",
            )?;
            assert_eq!(called.get(), 3);
            refusal(claim_postflight_capture(&mut independent), "already attempted")?;
            // Recovery is a separate DATA capture, never a reset invocation.
            let recovered = capture_postflight_data(
                &fixture.0, &base, &head, &profile, 0, deadline, || Ok(()),
            )?;
            assert!(recovered.changed_paths.iter().any(|path| path == "deleted.rs"));
            Ok(())
        }

        #[test]
        fn real_postflight_captures_keep_all_three_caps_and_the_original_expired_clock()
        -> Result<(), String> {
            let (fixture, base, head) = CaptureFixture::new()?;
            let profile = capture_profile()?;
            let deadline = Instant::now()
                .checked_add(Duration::from_mins(1))
                .ok_or("postflight cap fixture clock overflow")?;
            for phase in 0..3 {
                let mut limited = profile.clone();
                match phase {
                    0 => limited.max_artifact_bytes[0] = 1,
                    1 => limited.max_artifact_bytes[2] = 1,
                    _ => limited.max_inventory_bytes = 1,
                }
                let result = capture_postflight_data(
                    &fixture.0, &base, &head, &limited, 0, deadline, || Ok(()),
                );
                match result {
                    Err(error)
                        if error.contains("limit")
                            || error.contains("cap")
                            || error.contains("exceed") => {}
                    Err(error) => return Err(format!("wrong postflight stream refusal: {error}")),
                    Ok(_) => return Err(format!("postflight phase {phase} ignored its real cap")),
                }
            }
            refusal(
                capture_postflight_data(
                    &fixture.0, &base, &head, &profile, 0, Instant::now(), || Ok(()),
                ),
                "deadline",
            )?;
            let recovered = capture_postflight_data(
                &fixture.0, &base, &head, &profile, 0, deadline, || Ok(()),
            )?;
            assert!(!recovered.raw.is_empty());
            assert!(!recovered.presentation.is_empty());
            assert!(!recovered.changed_paths.is_empty());
            Ok(())
        }

        #[test]
        fn original_capture_expired_clock_refuses_before_git_and_capacity_is_checked()
        -> Result<(), String> {
            let profile = capture_profile()?;
            let budget = CaptureBudget::new(&profile, 0)?;
            refusal(
                capture_original_data(
                    Path::new("/missing-whole-capture"),
                    &"1".repeat(40),
                    &"2".repeat(40),
                    &profile,
                    &budget,
                    Instant::now(),
                    || Ok(()),
                ),
                "deadline",
            )?;
            refusal(
                CaptureBudget::new(&profile, profile.max_buffered_bytes),
                "capacity",
            )?;
            Ok(())
        }
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(super) use whole_worker::{
    CaptureBudget, POSTFLIGHT_WITNESS_BYTES, QualifiedWholeInvocation,
    capture_postflight_data, with_libtest_worker,
};
