//! Closed runtime-result classification for one calibration attempt (#4795).
//!
//! A process exit, zero executed subjects, timeout, compile failure, or
//! equivalent mutant cannot be normalized into `caught` or `survived`.

use super::{
    INSTRUMENT_COMPILE, INSTRUMENT_PROCESS, INSTRUMENT_TIMEOUT, RESULT_CAUGHT, RESULT_EQUIVALENT,
    RESULT_INCONCLUSIVE, RESULT_INSTRUMENT, RESULT_NOT_RUN, RESULT_STALE, RESULT_SURVIVED,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AttemptFacts {
    pub(super) identity_ok: bool,
    pub(super) selector_ok: bool,
    pub(super) equivalent: bool,
    pub(super) timed_out: bool,
    pub(super) compile_failed: bool,
    pub(super) process_failed: bool,
    pub(super) exit_code: Option<i32>,
    pub(super) intended: u64,
    pub(super) discovered: u64,
    pub(super) selected: u64,
    pub(super) executed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ClassifiedAttempt {
    pub(super) runtime_result: &'static str,
    pub(super) instrument_kind: Option<&'static str>,
    pub(super) non_calibration_reason: Option<&'static str>,
}

pub(super) fn classify_attempt(facts: &AttemptFacts) -> ClassifiedAttempt {
    if !facts.identity_ok {
        return ClassifiedAttempt {
            runtime_result: RESULT_STALE,
            instrument_kind: None,
            non_calibration_reason: Some("stale_identity"),
        };
    }
    if !facts.selector_ok {
        return ClassifiedAttempt {
            runtime_result: RESULT_STALE,
            instrument_kind: None,
            non_calibration_reason: Some("wrong_selector"),
        };
    }
    if facts.timed_out {
        return ClassifiedAttempt {
            runtime_result: RESULT_INSTRUMENT,
            instrument_kind: Some(INSTRUMENT_TIMEOUT),
            non_calibration_reason: Some("timeout"),
        };
    }
    if facts.process_failed {
        return ClassifiedAttempt {
            runtime_result: RESULT_INSTRUMENT,
            instrument_kind: Some(INSTRUMENT_PROCESS),
            non_calibration_reason: Some("process_failure"),
        };
    }
    if facts.compile_failed {
        return ClassifiedAttempt {
            runtime_result: RESULT_INSTRUMENT,
            instrument_kind: Some(INSTRUMENT_COMPILE),
            non_calibration_reason: Some("compile_failure"),
        };
    }
    if facts.equivalent {
        return ClassifiedAttempt {
            runtime_result: RESULT_EQUIVALENT,
            instrument_kind: None,
            non_calibration_reason: Some("equivalent_or_unusable"),
        };
    }
    if facts.intended == 0 || facts.selected == 0 || facts.executed == 0 {
        return ClassifiedAttempt {
            runtime_result: RESULT_INCONCLUSIVE,
            instrument_kind: None,
            non_calibration_reason: Some("zero_executed_subjects"),
        };
    }
    match facts.exit_code {
        Some(0) => ClassifiedAttempt {
            runtime_result: RESULT_SURVIVED,
            instrument_kind: None,
            non_calibration_reason: None,
        },
        Some(_) => ClassifiedAttempt {
            runtime_result: RESULT_CAUGHT,
            instrument_kind: None,
            non_calibration_reason: None,
        },
        None => ClassifiedAttempt {
            runtime_result: RESULT_NOT_RUN,
            instrument_kind: None,
            non_calibration_reason: Some("missing_exit"),
        },
    }
}

#[cfg(test)]
pub(super) fn compile_failed_from_output(stderr: &[u8], stdout: &[u8]) -> bool {
    let text = [stderr, stdout].concat();
    let lowered = String::from_utf8_lossy(&text).to_ascii_lowercase();
    lowered.contains("could not compile")
        || lowered.contains("error: could not compile")
        || lowered.contains("compilation failed")
}

pub(super) const fn not_run() -> ClassifiedAttempt {
    ClassifiedAttempt {
        runtime_result: RESULT_NOT_RUN,
        instrument_kind: None,
        non_calibration_reason: Some("not_run"),
    }
}
