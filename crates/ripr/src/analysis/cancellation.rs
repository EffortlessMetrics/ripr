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

const ACTIVE: u8 = 0;
const SUPERSEDED: u8 = 1;
const CANCELLED: u8 = 2;
const DEADLINE_EXCEEDED: u8 = 3;

#[derive(Clone, Debug)]
pub(crate) struct AnalysisCancellationToken(Arc<AtomicU8>);

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
        Self(Arc::new(AtomicU8::new(ACTIVE)))
    }

    pub(crate) fn cancel(&self, kind: AnalysisAbortKind) -> bool {
        let value = match kind {
            AnalysisAbortKind::Superseded => SUPERSEDED,
            AnalysisAbortKind::Cancelled => CANCELLED,
            AnalysisAbortKind::DeadlineExceeded => DEADLINE_EXCEEDED,
        };
        self.0
            .compare_exchange(ACTIVE, value, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn checkpoint(&self) -> Result<(), AnalysisCancellation> {
        match self.0.load(Ordering::Acquire) {
            SUPERSEDED => Err(AnalysisCancellation {
                kind: AnalysisAbortKind::Superseded,
            }),
            CANCELLED => Err(AnalysisCancellation {
                kind: AnalysisAbortKind::Cancelled,
            }),
            DEADLINE_EXCEEDED => Err(AnalysisCancellation {
                kind: AnalysisAbortKind::DeadlineExceeded,
            }),
            _ => Ok(()),
        }
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
    let previous = CURRENT_TOKEN.with(|slot| slot.replace(Some(token.clone())));
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

pub(crate) fn is_cancellation_error(error: &str) -> bool {
    error.starts_with("analysis cancelled:")
}

#[cfg(test)]
mod tests {
    use super::*;

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
