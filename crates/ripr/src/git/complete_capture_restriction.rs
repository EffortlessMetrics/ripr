//! Test-gated, cooperative capture restrictions, never whole-input authority.
//!
//! Only collector-dispatched invocation errors are sticky. Raw/parameter
//! preflights that return before dispatch MUST be propagated by the caller.
//! Scope success proves no subject, analyzer, cohort, cleanup or publication.
//! TLS does not cross caller threads; CatFileBatch has its separate contract.

use std::cell::RefCell;
use std::ffi::OsStr;
use std::process::{Command, Output};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::core_error::CoreError;

use super::{CompleteGitEnvironment, POST_KILL_DRAIN_GRACE};

const MAX_CAPTURE_BYTES: usize = 256 * 1024 * 1024;
const CANONICAL_PHASE_CEILING: Duration = Duration::from_mins(5);

#[derive(Clone)]
pub(super) struct Restriction {
    held_deadline: Instant,
    max_output_bytes: usize,
    first_fault: Rc<RefCell<Option<CoreError>>>,
}

thread_local! {
    static CURRENT: RefCell<Option<Restriction>> = const { RefCell::new(None) };
}

struct Restore(Option<Restriction>);

impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT.with(|slot| {
            *slot.borrow_mut() = self.0.take();
        });
    }
}

pub(super) fn current() -> Option<Restriction> {
    CURRENT.with(|slot| slot.borrow().clone())
}

fn record_fault(restriction: &Restriction, error: &CoreError) {
    let mut first = restriction.first_fault.borrow_mut();
    if first.is_none() {
        *first = Some(error.clone());
    }
}

/// Install tighter DATA bounds on this synchronous caller thread only.
/// The actual worker owner must independently authenticate its native profile,
/// original clock and inputs, and propagate EVERY pre-collector validation.
pub(crate) fn with_restriction<T>(
    held_deadline: Instant,
    max_output_bytes: usize,
    work: impl FnOnce() -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    let previous = current();
    let effective_deadline = previous.as_ref().map_or(held_deadline, |parent| {
        held_deadline.min(parent.held_deadline)
    });
    let admission = if max_output_bytes == 0 || max_output_bytes > MAX_CAPTURE_BYTES {
        Err(CoreError::message(
            "complete Git restriction requires a positive limit within 256 MiB",
        ))
    } else if effective_deadline
        .checked_sub(POST_KILL_DRAIN_GRACE)
        .is_none_or(|execution| execution <= Instant::now())
    {
        Err(CoreError::git_invocation_timeout(
            "complete Git capture restriction",
            0,
            false,
        ))
    } else {
        Ok(())
    };
    if let Err(error) = admission {
        if let Some(parent) = &previous {
            record_fault(parent, &error);
        }
        return Err(error);
    }
    let restriction = Restriction {
        held_deadline: effective_deadline,
        max_output_bytes: previous.as_ref().map_or(max_output_bytes, |parent| {
            max_output_bytes.min(parent.max_output_bytes)
        }),
        first_fault: previous.as_ref().map_or_else(
            || Rc::new(RefCell::new(None)),
            |parent| Rc::clone(&parent.first_fault),
        ),
    };
    CURRENT.with(|slot| *slot.borrow_mut() = Some(restriction.clone()));
    let restore = Restore(previous);
    let result = work();
    let fault = restriction.first_fault.borrow().clone();
    drop(restore);
    match result {
        Err(error) => match fault {
            Some(fault) if fault != error => {
                Err(error.with_context(format!("complete Git capture already refused: {fault}")))
            }
            _ => Err(error),
        },
        Ok(value) => match fault {
            Some(error) => Err(error),
            None => Ok(value),
        },
    }
}

fn validate_command_environment(
    command: &Command,
    inherited: impl FnMut(&str) -> bool,
) -> Result<(), CoreError> {
    super::validate_complete_git_environment(CompleteGitEnvironment::WholeInput, inherited)?;
    super::validate_complete_git_environment(CompleteGitEnvironment::WholeInput, |name| {
        command
            .get_envs()
            .any(|(key, value)| key == OsStr::new(name) && value.is_some())
    })
    .map_err(|error| error.with_context("complete capture prepared command environment"))
}

fn collect_inner(
    restriction: &Restriction,
    mut command: Command,
    timeout: Option<Duration>,
    callee_limit: Option<usize>,
    describe: &str,
    callee_held: Option<(Instant, Instant)>,
) -> Result<Output, CoreError> {
    // Match the original collector's zero-timeout and cancellation order.
    if timeout.is_some_and(|value| value.is_zero()) {
        return Err(CoreError::git_invocation_timeout(describe, 0, false));
    }
    crate::analysis::cancellation::checkpoint_typed().map_err(CoreError::from)?;
    if let Some(error) = restriction.first_fault.borrow().clone() {
        return Err(error);
    }
    let limit = callee_limit.map_or(restriction.max_output_bytes, |limit| {
        limit.min(restriction.max_output_bytes)
    });
    if limit == 0 {
        return Err(CoreError::message(
            "complete Git capture requires a positive limit within 256 MiB",
        ));
    }
    let entry = Instant::now();
    let held = callee_held.map_or(restriction.held_deadline, |(_, held)| {
        held.min(restriction.held_deadline)
    });
    let canonical = entry
        .checked_add(CANONICAL_PHASE_CEILING)
        .ok_or_else(|| CoreError::git_invocation_timeout(describe, 0, false))?;
    let mut execution = held
        .checked_sub(POST_KILL_DRAIN_GRACE)
        .map(|cutoff| cutoff.min(canonical))
        .ok_or_else(|| CoreError::git_invocation_timeout(describe, 0, false))?;
    if let Some(timeout) = timeout {
        let phase = entry
            .checked_add(timeout)
            .ok_or_else(|| CoreError::git_invocation_timeout(describe, 0, false))?;
        execution = execution.min(phase);
    }
    if let Some((cutoff, _)) = callee_held {
        execution = execution.min(cutoff);
    }
    if Instant::now() >= execution {
        return Err(CoreError::git_invocation_timeout(describe, 0, false));
    }
    validate_command_environment(&command, |name| std::env::var_os(name).is_some())?;
    command.env("GIT_NO_REPLACE_OBJECTS", "1");
    // The direct original body never redispatches through TLS.
    super::collect_output_with_reader_policy_and_held_deadline_direct(
        command,
        Some(execution.saturating_duration_since(Instant::now())),
        limit,
        describe,
        true,
        Some((execution, held)),
    )
}

pub(super) fn collect(
    restriction: &Restriction,
    command: Command,
    timeout: Option<Duration>,
    callee_limit: Option<usize>,
    describe: &str,
    callee_held: Option<(Instant, Instant)>,
) -> Result<Output, CoreError> {
    let result = collect_inner(
        restriction,
        command,
        timeout,
        callee_limit,
        describe,
        callee_held,
    );
    if let Err(error) = &result {
        record_fault(restriction, error);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::analysis::cancellation::{AnalysisAbortKind, AnalysisCancellationToken, with_token};
    use crate::git::with_complete_capture_restriction;

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn held() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }

    fn refused<T>(result: Result<T, CoreError>) -> Result<CoreError, String> {
        match result {
            Err(error) => Ok(error),
            Ok(_) => Err("expected the actual restriction to refuse".to_string()),
        }
    }

    struct Repository(PathBuf);

    impl Repository {
        fn new() -> Result<Self, String> {
            let root = std::env::temp_dir().join(format!(
                "ripr-git-restriction-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).map_err(|error| error.to_string())?;
            let repo = Self(root);
            repo.git(&[
                "-c",
                "init.templateDir=",
                "init",
                "--quiet",
                "--initial-branch=main",
            ])?;
            repo.git(&["config", "user.email", "ripr-scope@example.invalid"])?;
            repo.git(&["config", "user.name", "RIPR Capture Scope"])?;
            repo.git(&["config", "commit.gpgSign", "false"])?;
            Ok(repo)
        }

        fn git(&self, args: &[&str]) -> Result<(), String> {
            crate::testing::fixture_git::fixture_git_ok(&self.0, args)
        }

        fn head(&self) -> Result<String, String> {
            let output = super::super::run_git_output_with_deadline_and_limit_isolated(
                &self.0,
                &["rev-parse", "--verify", "HEAD"],
                crate::testing::fixture_git::FIXTURE_GIT_DEADLINE,
                4096,
            )
            .map_err(|error| error.to_string())?;
            if !output.status.success() {
                return Err(format!(
                    "isolated fixture HEAD failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            String::from_utf8(output.stdout)
                .map(|head| head.trim().to_string())
                .map_err(|error| format!("isolated fixture HEAD is not UTF-8: {error}"))
        }
    }

    impl Drop for Repository {
        fn drop(&mut self) {
            if let Err(error) = crate::testing::fixture_git::remove_fixture_tree(&self.0) {
                eprintln!("capture restriction fixture cleanup failed: {error}");
            }
        }
    }

    #[test]
    fn actual_u0_and_u3_bytes_keep_the_original_loaders() -> Result<(), String> {
        let repo = Repository::new()?;
        std::fs::write(repo.0.join("source.rs"), "fn value() -> bool { 1 < 2 }\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(repo.0.join("binary"), [0, 1, 2, 3]).map_err(|error| error.to_string())?;
        repo.git(&["add", "."])?;
        repo.git(&["commit", "--quiet", "-m", "base"])?;
        let base = repo.head()?;
        std::fs::write(repo.0.join("source.rs"), "fn value() -> bool { 1 <= 2 }\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(repo.0.join("binary"), [0, 4, 5, 6]).map_err(|error| error.to_string())?;
        repo.git(&["add", "."])?;
        repo.git(&["commit", "--quiet", "-m", "head"])?;
        let head = repo.head()?;
        let u0 = crate::analysis::diff::load::load_canonical_pr_evidence_diff_bytes_bounded(
            &repo.0,
            &base,
            &head,
            64 * 1024,
        )
        .map_err(|error| error.to_string())?;
        let u3 = crate::analysis::load_pr_evidence_diff_range(&repo.0, &base, &head)?;
        assert!(!u0.is_empty(), "actual canonical range must be nonempty");
        assert!(
            u3.contains("GIT binary patch"),
            "binary packet stimulus absent"
        );
        let scoped = with_complete_capture_restriction(held(), 64 * 1024, || {
            let raw = crate::analysis::diff::load::load_canonical_pr_evidence_diff_bytes_bounded(
                &repo.0,
                &base,
                &head,
                64 * 1024,
            )?;
            let packet = crate::analysis::load_pr_evidence_diff_range(&repo.0, &base, &head)
                .map_err(CoreError::message)?;
            Ok((raw, packet))
        })
        .map_err(|error| error.to_string())?;
        assert_eq!(scoped.0, u0);
        assert_eq!(scoped.1, u3);
        Ok(())
    }

    #[test]
    fn unbounded_stdout_and_stderr_entries_are_restricted() -> Result<(), String> {
        let repo = Repository::new()?;
        for args in [&["--version"][..], &["--ripr-invalid-option"][..]] {
            let error = refused(with_complete_capture_restriction(held(), 4, || {
                super::super::run_git_output_with_deadline(&repo.0, args, None)
            }))?;
            assert!(
                error.to_string().contains("git_output_limit_exceeded"),
                "{error}"
            );
        }
        let output = super::super::run_git_output_with_deadline(&repo.0, &["--version"], None)
            .map_err(|error| error.to_string())?;
        assert!(output.status.success());
        assert!(output.stdout.starts_with(b"git version "));
        Ok(())
    }

    #[test]
    fn nonzero_output_is_data_and_smaller_callee_cap_still_refuses() -> Result<(), String> {
        let repo = Repository::new()?;
        let output = with_complete_capture_restriction(held(), 4096, || {
            super::super::run_git_output_with_deadline(&repo.0, &["--ripr-invalid-option"], None)
        })
        .map_err(|error| error.to_string())?;
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
        let error = refused(with_complete_capture_restriction(held(), 4096, || {
            super::super::run_git_output_with_deadline_and_limit_strict(
                &repo.0,
                &["--version"],
                Duration::from_secs(30),
                4,
            )
        }))?;
        assert!(error.to_string().contains("4-byte"), "{error}");
        Ok(())
    }

    #[test]
    fn swallowed_dispatched_error_is_sticky_and_callback_error_is_primary() -> Result<(), String> {
        let repo = Repository::new()?;
        let error = refused(with_complete_capture_restriction(held(), 4, || {
            let _ignored =
                super::super::run_git_output_with_deadline(&repo.0, &["--version"], None).ok();
            Ok(())
        }))?;
        assert!(
            error.to_string().contains("git_output_limit_exceeded"),
            "{error}"
        );
        let primary = CoreError::git_invocation_timeout("callback primary control", 99, false);
        let error = refused(with_complete_capture_restriction(held(), 4, || {
            let _ignored =
                super::super::run_git_output_with_deadline(&repo.0, &["--version"], None).ok();
            Err::<(), _>(primary.clone())
        }))?;
        assert!(error.is_git_invocation_timeout(), "{error}");
        assert!(
            error.to_string().contains("callback primary control"),
            "{error}"
        );
        assert!(
            error.to_string().contains("git_output_limit_exceeded"),
            "{error}"
        );
        let expected = CoreError::git_invocation_timeout("sticky typed zero", 0, false);
        let typed = refused(with_complete_capture_restriction(held(), 4096, || {
            let _ignored = super::super::collect_output_with_deadline(
                super::super::git_command(&repo.0, &["--version"]),
                Some(Duration::ZERO),
                "sticky typed zero",
            )
            .ok();
            Ok(())
        }))?;
        assert_eq!(
            typed, expected,
            "the first swallowed error must retain its typed kind"
        );
        Ok(())
    }

    #[test]
    fn discarded_precollector_refusal_is_explicitly_outside_latch_coverage() -> Result<(), String> {
        with_complete_capture_restriction(held(), 4096, || {
            let refused_before_dispatch = super::super::run_git_output_with_deadline_and_limit(
                Path::new("/ripr-missing-precollector-control"),
                &["--version"],
                Duration::from_secs(30),
                0,
            );
            match refused_before_dispatch {
                Err(error) => {
                    assert!(error.to_string().contains("greater than zero"), "{error}");
                }
                Ok(_) => return Err(CoreError::message("zero-cap preflight accepted")),
            }
            assert!(current().is_some());
            Ok(())
        })
        .map_err(|error| error.to_string())?;
        // This DATA-only Ok is deliberately not any execution/subject receipt.
        Ok(())
    }

    #[test]
    fn nested_restrictions_share_first_fault_and_restore_tighter_bounds() -> Result<(), String> {
        let repo = Repository::new()?;
        let outer = held();
        let inner = outer - Duration::from_secs(2);
        let error = refused(with_complete_capture_restriction(outer, 4096, || {
            let nested = with_complete_capture_restriction(inner, 4, || {
                let observed =
                    current().ok_or_else(|| CoreError::message("nested restriction missing"))?;
                assert_eq!(observed.held_deadline, inner);
                assert_eq!(observed.max_output_bytes, 4);
                let _ignored =
                    super::super::run_git_output_with_deadline(&repo.0, &["--version"], None).ok();
                Ok(())
            });
            let nested_error = refused(nested).map_err(CoreError::message)?;
            assert!(
                nested_error.to_string().contains("4-byte"),
                "{nested_error}"
            );
            let restored = current()
                .ok_or_else(|| CoreError::message("outer restriction was not restored"))?;
            assert_eq!(restored.held_deadline, outer);
            assert_eq!(restored.max_output_bytes, 4096);
            let shared = refused(super::super::run_git_output_with_deadline(
                &repo.0,
                &["--version"],
                None,
            ))
            .map_err(CoreError::message)?;
            assert!(shared.to_string().contains("4-byte"), "{shared}");
            Ok(())
        }))?;
        assert!(error.to_string().contains("4-byte"), "{error}");
        with_complete_capture_restriction(outer, 8, || {
            with_complete_capture_restriction(held(), 4096, || {
                let observed = current()
                    .ok_or_else(|| CoreError::message("nested min restriction missing"))?;
                assert_eq!(observed.held_deadline, outer);
                assert_eq!(observed.max_output_bytes, 8);
                Ok(())
            })
        })
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn expired_earliest_nested_clock_never_enters_its_callback() -> Result<(), String> {
        let entered = std::cell::Cell::new(false);
        let original = Instant::now() + POST_KILL_DRAIN_GRACE + Duration::from_millis(20);
        let error = refused(with_complete_capture_restriction(original, 4096, || {
            std::thread::sleep(Duration::from_millis(30));
            let _ignored = with_complete_capture_restriction(held(), 4096, || {
                entered.set(true);
                Ok(())
            })
            .ok();
            Ok(())
        }))?;
        assert!(
            !entered.get(),
            "nested scope renewed an expired original cutoff"
        );
        assert!(error.is_git_invocation_timeout(), "{error}");
        Ok(())
    }

    #[test]
    fn prepared_and_inherited_redirects_refuse_and_removal_is_consistent() -> Result<(), String> {
        let repo = Repository::new()?;
        for name in super::super::COMPLETE_GIT_REDIRECTS
            .iter()
            .copied()
            .chain(super::super::COMPLETE_GIT_CONFIGURATION.iter().copied())
        {
            let mut command = super::super::git_command(&repo.0, &["--version"]);
            command.env(name, "scope-invalid-control");
            let error = refused(with_complete_capture_restriction(held(), 4096, || {
                super::super::collect_output_with_deadline(command, None, "prepared redirect")
            }))?;
            assert!(error.to_string().contains(name), "{error}");
            let mut removed = super::super::git_command(&repo.0, &["--version"]);
            removed.env_remove(name);
            validate_command_environment(&removed, |_| false).map_err(|error| error.to_string())?;
            let inherited = refused(validate_command_environment(&removed, |key| key == name))?;
            assert!(inherited.to_string().contains(name), "{inherited}");
        }
        Ok(())
    }

    #[test]
    fn actual_replace_refs_cannot_override_the_scoped_original_objects() -> Result<(), String> {
        let repo = Repository::new()?;
        std::fs::write(repo.0.join("source"), b"base bytes\n")
            .map_err(|error| error.to_string())?;
        repo.git(&["add", "."])?;
        repo.git(&["commit", "--quiet", "-m", "base"])?;
        let base = repo.head()?;
        std::fs::write(repo.0.join("source"), b"head bytes\n")
            .map_err(|error| error.to_string())?;
        repo.git(&["commit", "--quiet", "-a", "-m", "head"])?;
        let head = repo.head()?;
        repo.git(&["replace", &head, &base])?;
        let expression = format!("{head}:source");
        let mut ordinary = super::super::git_command(&repo.0, &["show", &expression]);
        ordinary.env_remove("GIT_NO_REPLACE_OBJECTS");
        let output = super::super::collect_output_with_deadline(
            ordinary,
            Some(Duration::from_secs(30)),
            "ordinary replace control",
        )
        .map_err(|error| error.to_string())?;
        assert!(output.status.success());
        assert_eq!(output.stdout, b"base bytes\n");
        let mut command = super::super::git_command(&repo.0, &["show", &expression]);
        command.env_remove("GIT_NO_REPLACE_OBJECTS");
        let scoped = with_complete_capture_restriction(held(), 4096, || {
            super::super::collect_output_with_deadline(command, None, "scoped replace control")
        })
        .map_err(|error| error.to_string())?;
        assert!(scoped.status.success());
        assert_eq!(scoped.stdout, b"head bytes\n");
        Ok(())
    }

    #[test]
    fn zero_and_cancellation_keep_priority_before_prepared_redirect() -> Result<(), String> {
        let repo = Repository::new()?;
        let mut zero = super::super::git_command(&repo.0, &["--version"]);
        zero.env("GIT_DIR", "forbidden");
        let error = refused(with_complete_capture_restriction(held(), 4096, || {
            super::super::collect_output_with_deadline(zero, Some(Duration::ZERO), "zero control")
        }))?;
        assert!(error.is_git_invocation_timeout(), "{error}");
        let token = AnalysisCancellationToken::new();
        assert!(token.cancel(AnalysisAbortKind::Superseded));
        let mut cancelled = super::super::git_command(&repo.0, &["--version"]);
        cancelled.env("GIT_DIR", "forbidden");
        let error = refused(with_token(&token, || {
            with_complete_capture_restriction(held(), 4096, || {
                super::super::collect_output_with_deadline(cancelled, None, "cancel control")
            })
        }))?;
        assert!(error.is_analysis_cancelled(), "{error}");
        Ok(())
    }

    #[test]
    fn scope_admission_refuses_zero_excess_and_expired_before_callback() -> Result<(), String> {
        for (deadline, limit) in [
            (held(), 0),
            (held(), MAX_CAPTURE_BYTES + 1),
            (Instant::now(), 4096),
            (Instant::now() + POST_KILL_DRAIN_GRACE, 4096),
        ] {
            let entered = std::cell::Cell::new(false);
            let error = refused(with_complete_capture_restriction(deadline, limit, || {
                entered.set(true);
                Ok(())
            }))?;
            assert!(!entered.get(), "invalid scope reached callback: {error}");
        }
        let repo = Repository::new()?;
        let output = with_complete_capture_restriction(held(), MAX_CAPTURE_BYTES, || {
            super::super::run_git_output_with_deadline(&repo.0, &["--version"], None)
        })
        .map_err(|error| error.to_string())?;
        assert!(
            output.status.success(),
            "the existing exact ceiling must remain admitted"
        );
        Ok(())
    }

    #[test]
    fn explicit_held_entry_intersects_scope_cap_and_original_runtime_cutoff() -> Result<(), String>
    {
        let repo = Repository::new()?;
        let error = refused(with_complete_capture_restriction(held(), 4, || {
            super::super::run_git_complete_output_with_held_deadline_and_limit(
                &repo.0,
                &["--version"],
                held(),
                4096,
                CompleteGitEnvironment::WholeInput,
            )
        }))?;
        assert!(error.to_string().contains("4-byte"), "{error}");
        let started = Instant::now();
        let execution = started + Duration::from_millis(200);
        let original_held = execution + POST_KILL_DRAIN_GRACE;
        super::super::HELD_CAPTURE_OBSERVATIONS.with(|values| {
            *values.borrow_mut() = Some(Vec::new());
        });
        let command = super::super::git_command(
            &repo.0,
            &["-c", "alias.ripr-scope-wait=!sleep 1", "ripr-scope-wait"],
        );
        let result = with_complete_capture_restriction(original_held, 4096, || {
            super::super::collect_output_with_reader_policy_and_held_deadline(
                command,
                Some(Duration::from_secs(30)),
                4096,
                "scope live cutoff",
                false,
                Some((held(), held())),
            )
        });
        let observations = super::super::HELD_CAPTURE_OBSERVATIONS
            .with(|values| values.borrow_mut().take())
            .ok_or("actual held collector observations absent")?;
        let error = refused(result)?;
        assert!(error.is_git_invocation_timeout(), "{error}");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(
            matches!(
                observations.first(),
                Some(super::super::HeldCaptureObservation::SpawnedLive(true))
            ),
            "actual Git child must be live before the scoped cutoff: {observations:?}"
        );
        let drains: Vec<_> = observations
            .iter()
            .filter_map(|value| match value {
                super::super::HeldCaptureObservation::Drain { deadline, .. } => *deadline,
                super::super::HeldCaptureObservation::SpawnedLive(_) => None,
            })
            .collect();
        assert_eq!(drains.len(), 2, "both actual readers must be drained");
        assert_eq!(
            drains[0], drains[1],
            "stderr must not renew the drain deadline"
        );
        assert!(drains[0] <= original_held);
        Ok(())
    }

    #[test]
    fn caller_phase_cannot_be_extended_by_scope_or_explicit_held_data() -> Result<(), String> {
        let repo = Repository::new()?;
        let error = refused(with_complete_capture_restriction(held(), 4096, || {
            super::super::collect_output_with_reader_policy_and_held_deadline(
                super::super::git_command(
                    &repo.0,
                    &["-c", "alias.ripr-scope-wait=!sleep 1", "ripr-scope-wait"],
                ),
                Some(Duration::from_millis(20)),
                4096,
                "short caller phase",
                true,
                Some((held(), held())),
            )
        }))?;
        assert!(error.is_git_invocation_timeout(), "{error}");
        Ok(())
    }

    #[test]
    fn unwind_restores_tls_and_other_threads_do_not_inherit_restrictions() -> Result<(), String> {
        let unwind = std::panic::catch_unwind(|| {
            with_complete_capture_restriction::<()>(held(), 4096, || {
                std::panic::resume_unwind(Box::new("scope unwind control"))
            })
        });
        match unwind {
            Err(_payload) => {}
            Ok(_) => return Err("scope did not unwind".to_string()),
        }
        assert!(current().is_none(), "unwind leaked caller restrictions");
        with_complete_capture_restriction(held(), 4096, || {
            let other = std::thread::spawn(|| current().is_none())
                .join()
                .map_err(|payload| {
                    CoreError::message(format!(
                        "thread control panicked (string payload: {})",
                        payload.is::<&str>()
                    ))
                })?;
            assert!(other, "TLS restriction was unexpectedly inherited");
            assert!(
                current().is_some(),
                "other thread changed caller restrictions"
            );
            Ok(())
        })
        .map_err(|error| error.to_string())?;
        assert!(current().is_none(), "scope leaked after normal return");
        Ok(())
    }
}
