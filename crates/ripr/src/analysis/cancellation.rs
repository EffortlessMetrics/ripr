//! Cooperative cancellation for synchronous analysis work.
//!
//! LSP refreshes run in `spawn_blocking`, so dropping the async join handle
//! cannot stop the analysis closure.  This small, dependency-free context
//! lets long-running analysis loops observe that their desired request has
//! been superseded or cancelled and return before publishing a partial
//! result.

use std::cell::RefCell;
use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use std::time::{Duration, Instant};

const ACTIVE: u8 = 0;
const SUPERSEDED: u8 = 1;
const CANCELLED: u8 = 2;
const DEADLINE_EXCEEDED: u8 = 3;

pub(crate) type AnalysisClock = Arc<dyn Fn() -> Instant + Send + Sync>;

struct AnalysisBudget {
    started: Instant,
    limit: Duration,
    now: AnalysisClock,
}

struct CancellationState {
    reason: AtomicU8,
    budget: Option<AnalysisBudget>,
}

#[derive(Clone)]
pub(crate) struct AnalysisCancellationToken(Arc<CancellationState>);

impl fmt::Debug for AnalysisCancellationToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnalysisCancellationToken")
            .field("reason", &self.abort_kind())
            .field("has_budget", &self.0.budget.is_some())
            .finish()
    }
}

impl PartialEq for AnalysisCancellationToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for AnalysisCancellationToken {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnalysisAbortKind {
    Superseded,
    Cancelled,
    DeadlineExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AnalysisCancellation {
    pub(crate) kind: AnalysisAbortKind,
}

impl fmt::Display for AnalysisCancellation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "analysis cancelled: {:?}", self.kind)
    }
}

impl AnalysisCancellationToken {
    pub(crate) fn new() -> Self {
        Self(Arc::new(CancellationState {
            reason: AtomicU8::new(ACTIVE),
            budget: None,
        }))
    }

    /// One owned monotonic clock and origin; no ambient test clock or worker
    /// preemption. A budget is observed only at cooperative checkpoints.
    pub(crate) fn with_budget(started: Instant, limit: Duration, now: AnalysisClock) -> Self {
        Self(Arc::new(CancellationState {
            reason: AtomicU8::new(ACTIVE),
            budget: Some(AnalysisBudget {
                started,
                limit,
                now,
            }),
        }))
    }

    pub(crate) fn remaining_budget(&self) -> Option<Duration> {
        self.0.budget.as_ref().map(|budget| {
            budget
                .limit
                .saturating_sub((budget.now)().saturating_duration_since(budget.started))
        })
    }

    /// Pure query: classifying an ordinary source failure must not advance
    /// the clock and replace it with a later deadline failure.
    pub(crate) fn abort_kind(&self) -> Option<AnalysisAbortKind> {
        match self.0.reason.load(Ordering::Acquire) {
            SUPERSEDED => Some(AnalysisAbortKind::Superseded),
            CANCELLED => Some(AnalysisAbortKind::Cancelled),
            DEADLINE_EXCEEDED => Some(AnalysisAbortKind::DeadlineExceeded),
            _ => None,
        }
    }

    pub(crate) fn cancel(&self, kind: AnalysisAbortKind) -> bool {
        let value = match kind {
            AnalysisAbortKind::Superseded => SUPERSEDED,
            AnalysisAbortKind::Cancelled => CANCELLED,
            AnalysisAbortKind::DeadlineExceeded => DEADLINE_EXCEEDED,
        };
        self.0
            .reason
            .compare_exchange(ACTIVE, value, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn checkpoint(&self) -> Result<(), AnalysisCancellation> {
        if self.abort_kind().is_none() && self.remaining_budget().is_some_and(|left| left.is_zero())
        {
            self.cancel(AnalysisAbortKind::DeadlineExceeded);
        }
        // Re-read the winner after CAS, including a concurrent earlier reason.
        self.abort_kind()
            .map_or(Ok(()), |kind| Err(AnalysisCancellation { kind }))
    }
}

thread_local! {
    static CURRENT_TOKEN: RefCell<Option<AnalysisCancellationToken>> = const { RefCell::new(None) };
}

struct ContextGuard(Option<AnalysisCancellationToken>);

impl Drop for ContextGuard {
    fn drop(&mut self) {
        let previous = self.0.take();
        CURRENT_TOKEN.with(|slot| {
            *slot.borrow_mut() = previous;
        });
    }
}

pub(crate) fn with_token<T>(token: &AnalysisCancellationToken, work: impl FnOnce() -> T) -> T {
    with_optional_token(Some(token), work)
}

/// Capture the owning request before dispatching work to a different thread.
pub(crate) fn current_token() -> Option<AnalysisCancellationToken> {
    CURRENT_TOKEN.with(|slot| slot.borrow().clone())
}

/// Install exactly the captured context, including no token. Restoring the
/// previous context prevents cancellation leaking between reused pool jobs.
pub(crate) fn with_optional_token<T>(
    token: Option<&AnalysisCancellationToken>,
    work: impl FnOnce() -> T,
) -> T {
    let previous = CURRENT_TOKEN.with(|slot| slot.replace(token.cloned()));
    let _guard = ContextGuard(previous);
    work()
}

pub(crate) fn checkpoint() -> Result<(), String> {
    CURRENT_TOKEN.with(|slot| {
        slot.borrow().as_ref().map_or(Ok(()), |token| {
            token.checkpoint().map_err(|error| error.to_string())
        })
    })
}

pub(crate) fn remaining_budget() -> Option<Duration> {
    CURRENT_TOKEN.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|token| token.remaining_budget())
    })
}

/// Pure query for the active token's recorded abort. Unlike [`checkpoint`],
/// this never expires a budget or writes a deadline reason.
pub(crate) fn current_abort_kind() -> Option<AnalysisAbortKind> {
    CURRENT_TOKEN.with(|slot| slot.borrow().as_ref().and_then(|token| token.abort_kind()))
}

pub(crate) fn is_cancellation_error(error: &str) -> bool {
    error.starts_with("analysis cancelled:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn optional_worker_context_restores_after_unwind() -> Result<(), String> {
        let outer = AnalysisCancellationToken::new();
        outer.cancel(AnalysisAbortKind::Cancelled);
        let inner = AnalysisCancellationToken::new();
        let result = with_token(&outer, || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_optional_token(Some(&inner), || {
                    assert_eq!(current_token(), Some(inner.clone()));
                    std::panic::resume_unwind(Box::new("intentional context restoration control"));
                });
            }))
        });
        assert_eq!(
            result
                .err()
                .and_then(|payload| payload.downcast_ref::<&str>().copied()),
            Some("intentional context restoration control")
        );
        assert_eq!(current_token(), None);
        // Also verify restoration while the outer request is still installed.
        with_token(&outer, || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_optional_token(None, || std::panic::resume_unwind(Box::new(7usize)))
            }));
            assert_eq!(
                result
                    .err()
                    .and_then(|payload| payload.downcast_ref::<usize>().copied()),
                Some(7)
            );
            assert_eq!(
                checkpoint().err().as_deref(),
                Some("analysis cancelled: Cancelled")
            );
        });
        checkpoint()
    }

    #[test]
    fn optional_worker_context_clears_and_restores_an_outer_request() -> Result<(), String> {
        let token = AnalysisCancellationToken::new();
        token.cancel(AnalysisAbortKind::Cancelled);
        with_token(&token, || {
            assert_eq!(current_token(), Some(token.clone()));
            with_optional_token(None, || {
                assert_eq!(current_token(), None);
                checkpoint()
            })?;
            assert_eq!(
                checkpoint().err().as_deref(),
                Some("analysis cancelled: Cancelled")
            );
            Ok::<(), String>(())
        })?;
        assert_eq!(current_token(), None);
        checkpoint()
    }

    #[test]
    fn owned_budget_expires_and_default_token_has_no_deadline() -> Result<(), String> {
        let elapsed = Arc::new(AtomicUsize::new(0));
        let clock_elapsed = Arc::clone(&elapsed);
        let started = Instant::now();
        let clock: AnalysisClock = Arc::new(move || {
            started + Duration::from_millis(clock_elapsed.load(Ordering::SeqCst) as u64)
        });
        let token =
            AnalysisCancellationToken::with_budget(started, Duration::from_millis(10), clock);
        token.checkpoint().map_err(|error| error.to_string())?;
        elapsed.store(10, Ordering::SeqCst);
        if token.checkpoint().err().map(|error| error.kind)
            != Some(AnalysisAbortKind::DeadlineExceeded)
        {
            return Err("owned budget did not expire at equality".to_string());
        }
        let default = AnalysisCancellationToken::new();
        if default.remaining_budget().is_some() || default.abort_kind().is_some() {
            return Err("default LSP token acquired an automatic deadline".to_string());
        }
        default.checkpoint().map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn expired_clock_preserves_first_reason_and_pure_error_query() -> Result<(), String> {
        let calls = Arc::new(AtomicUsize::new(0));
        let clock_calls = Arc::clone(&calls);
        let started = Instant::now();
        let token = AnalysisCancellationToken::with_budget(
            started,
            Duration::ZERO,
            Arc::new(move || {
                clock_calls.fetch_add(1, Ordering::SeqCst);
                started
            }),
        );
        if token.abort_kind().is_some() || calls.load(Ordering::SeqCst) != 0 {
            return Err("error classification advanced the deadline clock".to_string());
        }
        if !token.cancel(AnalysisAbortKind::Superseded)
            || token.checkpoint().err().map(|error| error.kind)
                != Some(AnalysisAbortKind::Superseded)
            || calls.load(Ordering::SeqCst) != 0
        {
            return Err("expired deadline replaced or polled an earlier cancellation".to_string());
        }
        Ok(())
    }

    #[test]
    fn cancellation_is_visible_inside_scoped_context() -> Result<(), String> {
        let token = AnalysisCancellationToken::new();
        with_token(&token, checkpoint)?;
        if !token.cancel(AnalysisAbortKind::Superseded) {
            return Err("first cancellation should win".to_string());
        }
        let result = with_token(&token, checkpoint);
        if !result
            .as_ref()
            .is_err_and(|error| error.contains("Superseded"))
        {
            return Err(format!("expected superseded cancellation, got {result:?}"));
        }
        Ok(())
    }

    #[test]
    fn nested_active_token_restores_cancelled_outer_context() -> Result<(), String> {
        let outer = AnalysisCancellationToken::new();
        let inner = AnalysisCancellationToken::new();
        if outer != outer.clone() || outer == inner {
            return Err("token identity must follow its owned shared state".to_string());
        }
        if !outer.cancel(AnalysisAbortKind::Superseded) {
            return Err("fixture could not cancel its outer token".to_string());
        }

        with_token(&outer, || {
            with_token(&inner, checkpoint)?;
            if !checkpoint().is_err_and(|error| error.contains("Superseded")) {
                return Err("nested token did not restore the cancelled outer context".to_string());
            }
            Ok(())
        })?;
        // The active token's constructor must remain deadline-free for LSP
        // callers, and an outer cancellation must not leak beyond its scope.
        inner.checkpoint().map_err(|error| error.to_string())?;
        checkpoint()?;
        Ok(())
    }

    #[test]
    fn deadline_exceeded_is_reported_by_checkpoint() -> Result<(), String> {
        let token = AnalysisCancellationToken::new();
        if !token.cancel(AnalysisAbortKind::DeadlineExceeded) {
            return Err("deadline cancellation should win on an active token".to_string());
        }
        let result = with_token(&token, checkpoint);
        if !result
            .as_ref()
            .is_err_and(|error| error.contains("DeadlineExceeded"))
        {
            return Err(format!(
                "expected deadline-exceeded cancellation, got {result:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn deadline_cancel_loses_to_an_earlier_superseded() -> Result<(), String> {
        let token = AnalysisCancellationToken::new();
        if !token.cancel(AnalysisAbortKind::Superseded) {
            return Err("first cancellation should win".to_string());
        }
        if token.cancel(AnalysisAbortKind::DeadlineExceeded) {
            return Err("deadline cancel must lose to an earlier superseded".to_string());
        }
        let result = with_token(&token, checkpoint);
        if !result
            .as_ref()
            .is_err_and(|error| error.contains("Superseded"))
        {
            return Err(format!(
                "expected the earlier superseded outcome to be preserved, got {result:?}"
            ));
        }
        Ok(())
    }
}
