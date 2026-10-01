//! Row-derived denominators, aggregates, and summary agreement for one
//! validated run receipt. The arithmetic is derived from validated rows
//! only; every hand-entered summary number must agree with it in both
//! directions, an overflowing aggregate is a structured failure naming the
//! summary field it would feed (checked arithmetic, never a panic), and
//! `repos_run == 0` is `not_run`, never a vacuous pass.

use std::collections::BTreeMap;

use serde_json::Value;

use super::receipt::SUMMARY_KEYS;
use super::{
    ALIGNMENT_VOCABULARY, CLASSIFICATION_VOCABULARY, Diagnostic, fail, known_value_or_fail,
    opt_distribution, opt_number, opt_string, opt_u64,
};

const GATE_STATUSES: [&str; 3] = ["not_run", "pass", "review"];
/// One validated row reduced to what denominator/aggregate derivation needs.
pub(super) struct RowSummary {
    pub(super) counts_as_run: bool,
    pub(super) crashed: bool,
    pub(super) parse_failed: bool,
    pub(super) timed_out: bool,
    pub(super) skipped: bool,
    pub(super) clone_failed: bool,
    pub(super) stability: Option<bool>,
    pub(super) runtime_ms: Option<u64>,
    pub(super) classification: Option<BTreeMap<String, u64>>,
    pub(super) alignment: Option<BTreeMap<String, u64>>,
}

#[derive(Debug, Default)]
pub(super) struct Derived {
    pub(super) total: usize,
    pub(super) run: usize,
    crashed: usize,
    parse_failed: usize,
    timed_out: usize,
    skipped: usize,
    clone_failed: usize,
    /// `None` when any run row lacks stability evidence.
    stable: Option<usize>,
    /// (min, median, max, total) over run rows; `None` when any run row lacks
    /// a runtime.
    runtime: Option<(u64, u64, u64, u64)>,
    classification: BTreeMap<String, u64>,
    alignment: BTreeMap<String, u64>,
}

impl Derived {
    fn crash_rate(&self) -> f64 {
        ratio(self.crashed, self.run)
    }

    fn parse_failure_rate(&self) -> f64 {
        ratio(self.parse_failed, self.run)
    }

    fn stability_rate(&self) -> Option<f64> {
        self.stable.map(|stable| {
            if self.run == 0 {
                1.0
            } else {
                stable as f64 / self.run as f64
            }
        })
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}
/// Derives the denominator and aggregates from validated rows only — the
/// arithmetic every hand-entered summary number must agree with. Checked
/// throughout: an overflowing aggregate is a structured failure naming the
/// summary field it would feed, never a panic.
pub(super) fn derive_denominator(rows: &[RowSummary], display: &str) -> Result<Derived, String> {
    let mut derived = Derived {
        total: rows.len(),
        ..Derived::default()
    };
    let mut runtimes: Vec<u64> = Vec::new();
    let mut stability_complete = true;
    for row in rows {
        if row.crashed {
            derived.crashed += 1;
        }
        if row.parse_failed {
            derived.parse_failed += 1;
        }
        if row.timed_out {
            derived.timed_out += 1;
        }
        if row.skipped {
            derived.skipped += 1;
        }
        if row.clone_failed {
            derived.clone_failed += 1;
        }
        if !row.counts_as_run {
            continue;
        }
        derived.run += 1;
        match row.stability {
            Some(true) => {
                let stable = derived.stable.get_or_insert(0);
                *stable += 1;
            }
            Some(false) => {
                derived.stable.get_or_insert(0);
            }
            None => stability_complete = false,
        }
        if let Some(runtime) = row.runtime_ms {
            runtimes.push(runtime);
        }
        // Analyzed rows always carry both distributions (enforced at row
        // level); terminal rows never enter the aggregates.
        if let Some(class) = &row.classification {
            merge_distribution(
                &mut derived.classification,
                class,
                "classification_counts",
                display,
            )?;
        }
        if let Some(align) = &row.alignment {
            merge_distribution(&mut derived.alignment, align, "alignment_counts", display)?;
        }
    }
    if !stability_complete {
        derived.stable = None;
    }
    if !runtimes.is_empty() && runtimes.len() == derived.run {
        runtimes.sort_unstable();
        let mut total: u64 = 0;
        for runtime in &runtimes {
            total = total.checked_add(*runtime).ok_or_else(|| {
                fail(
                    display,
                    "summary.runtime_ms_total",
                    format!(
                        "aggregate overflow: runtime total exceeds u64 when summed across {} run row(s)",
                        runtimes.len()
                    ),
                )
            })?;
        }
        derived.runtime = Some((
            runtimes[0],
            runtimes[runtimes.len() / 2],
            runtimes[runtimes.len() - 1],
            total,
        ));
    }
    Ok(derived)
}

fn merge_distribution(
    target: &mut BTreeMap<String, u64>,
    source: &BTreeMap<String, u64>,
    field: &str,
    display: &str,
) -> Result<(), String> {
    for (name, count) in source {
        let bucket = target.entry(name.clone()).or_insert(0);
        *bucket = bucket.checked_add(*count).ok_or_else(|| {
            fail(
                display,
                &format!("summary.{field}.{name}"),
                format!(
                    "aggregate overflow: bucket `{name}` exceeds u64 when summed across run rows"
                ),
            )
        })?;
    }
    Ok(())
}

/// Fails closed when hand-entered summary numbers disagree with the derived
/// rows; discloses absent core fields as incomplete. The summary itself is
/// owned in full (#3733 review): analyzed receipts must carry every aggregate
/// the emitter writes, and zero-run receipts must not carry a nonzero
/// analysis-bearing aggregate.
pub(super) fn validate_summary_agreement(
    display: &str,
    summary: &serde_json::Map<String, Value>,
    derived: &Derived,
    incomplete: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    // Summary ownership (#3733 review). With analyzed rows the receipt must
    // carry the emitted summary in full: the sweep records every aggregate on
    // every receipt, and a deleted field would silently disable its
    // row-agreement check. Stability aggregates are required exactly when the
    // rows fully evidence stability; the under-evidenced path discloses
    // instead of failing on an omission the emitter could not have written.
    const STABILITY_AGGREGATES: [&str; 3] = [
        "gap_id_stable_count",
        "gap_id_unstable_count",
        "gap_id_stability_rate",
    ];
    if derived.run > 0 {
        let stability_required = derived.stable.is_some();
        for field in SUMMARY_KEYS {
            if STABILITY_AGGREGATES.contains(&field) && !stability_required {
                continue;
            }
            if !summary_records_value(summary, field) {
                return Err(fail(
                    display,
                    &format!("summary.{field}"),
                    "required summary aggregate is missing: the sweep records the full summary on every receipt, and an omitted field would silently disable its row-agreement check",
                ));
            }
        }
    } else {
        // Zero-run law: `repos_run == 0` is `not_run`, never a vacuous pass —
        // extended to the summary. Every analysis-bearing aggregate must be
        // zero or absent; a recorded nonzero value claims analysis that never
        // happened.
        for field in [
            "runtime_ms_min",
            "runtime_ms_median",
            "runtime_ms_max",
            "runtime_ms_total",
            "gap_id_stable_count",
            "gap_id_unstable_count",
        ] {
            if let Some(value) = opt_u64(display, summary, field)?
                && value != 0
            {
                return Err(fail(
                    display,
                    &format!("summary.{field}"),
                    format!(
                        "repos_run == 0: aggregate must be zero or absent, got {value} — nothing ran, so a nonzero analysis-bearing aggregate is fabricated"
                    ),
                ));
            }
        }
        if let Some(value) = opt_number(display, summary, "gap_id_stability_rate")? {
            // The live emitter records `gap_id_stability_rate: 1.0` on every
            // zero-run report (eval_sweep.rs `compute_metrics` empty-set
            // guard), so a real untouched receipt must validate. Exactly that
            // value is accepted as a named incomplete disclosure — a vacuous
            // zero-run stability rate, not a measured claim — and the
            // disclosure keeps the receipt verdict `incomplete`, never a
            // pass. Any other nonzero rate stays a fabricated claim.
            if (value - 1.0).abs() <= 1e-9 {
                incomplete.push(Diagnostic::new(
                    display,
                    "summary.gap_id_stability_rate",
                    "vacuous zero-run stability rate: the emitter records 1.0 when repos_run == 0; no stability was measured, typed incomplete, not a pass",
                ));
            } else if value != 0.0 {
                return Err(fail(
                    display,
                    "summary.gap_id_stability_rate",
                    format!(
                        "repos_run == 0: stability rate must be zero, the emitter's vacuous 1.0, or absent, got {value} — nothing ran, so any other nonzero stability claim is fabricated"
                    ),
                ));
            }
        }
        for (field, vocabulary) in [
            ("classification_counts", &CLASSIFICATION_VOCABULARY[..]),
            ("alignment_counts", &ALIGNMENT_VOCABULARY[..]),
        ] {
            if let Some(counts) = opt_distribution(display, summary, field)? {
                for (name, count) in &counts {
                    if *count != 0 {
                        return Err(fail(
                            display,
                            &format!("summary.{field}.{name}"),
                            format!(
                                "repos_run == 0: bucket `{name}` claims {count} but nothing ran — analysis-bearing aggregates must be zero or absent at zero runs"
                            ),
                        ));
                    }
                }
                // A recorded distribution must be the emitter's zero-filled
                // shape (eval_sweep.rs `to_json` writes every bucket, at zero
                // runs included): a missing key — `absent`/`unknown` included
                // — is a dropped field, not a zero, and would erase the
                // field-not-emitted distinction (#3733 review).
                for name in vocabulary {
                    if !counts.contains_key(*name) {
                        return Err(fail(
                            display,
                            &format!("summary.{field}.{name}"),
                            format!(
                                "repos_run == 0: recorded distribution omits required bucket `{name}`; the emitter zero-fills every bucket, so a recorded distribution must carry the full emitted key set"
                            ),
                        ));
                    }
                }
            }
        }
    }

    // Denominator agreement.
    match opt_u64(display, summary, "repos_total")? {
        Some(value) if value as usize != derived.total => {
            return Err(fail(
                display,
                "summary.repos_total",
                format!(
                    "hand-edited aggregate: summary claims {value} selected row(s) but the receipt carries {}",
                    derived.total
                ),
            ));
        }
        _ => {}
    }
    match opt_u64(display, summary, "repos_run")? {
        Some(value) if value as usize != derived.run => {
            return Err(fail(
                display,
                "summary.repos_run",
                format!(
                    "hand-edited aggregate: summary claims {value} run row(s) but the rows derive {}",
                    derived.run
                ),
            ));
        }
        _ => {}
    }

    // Outcome counts.
    let count_fields: [(&str, usize); 5] = [
        ("crash_count", derived.crashed),
        ("parse_failure_count", derived.parse_failed),
        ("timed_out_count", derived.timed_out),
        ("repos_skipped", derived.skipped),
        ("repos_clone_failed", derived.clone_failed),
    ];
    for (field, expected) in count_fields {
        match opt_u64(display, summary, field)? {
            Some(value) if value as usize != expected => {
                return Err(fail(
                    display,
                    &format!("summary.{field}"),
                    format!(
                        "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                    ),
                ));
            }
            _ => {}
        }
    }

    // Stability counts (derivable only when every run row carries evidence).
    if let Some(stable) = derived.stable {
        let unstable = derived.run.saturating_sub(stable);
        for (field, expected) in [
            ("gap_id_stable_count", stable),
            ("gap_id_unstable_count", unstable),
        ] {
            match opt_u64(display, summary, field)? {
                Some(value) if value as usize != expected => {
                    return Err(fail(
                        display,
                        &format!("summary.{field}"),
                        format!(
                            "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                        ),
                    ));
                }
                _ => {}
            }
        }
    } else if derived.run > 0 {
        // Missing stability evidence must not silently disable the aggregate
        // check: a recorded value cannot be verified against the rows, and an
        // unrecorded one is disclosed incomplete (never invented).
        let recorded = [
            "gap_id_stable_count",
            "gap_id_unstable_count",
            "gap_id_stability_rate",
        ]
        .iter()
        .any(|field| summary_records_value(summary, field));
        if recorded {
            return Err(fail(
                display,
                "summary.gap_id_stable_count",
                "stability aggregate recorded but analyzed rows lack complete stability evidence (gap_ids_stable / repeat.gap_ids_stable on every run row); the value cannot be derived",
            ));
        }
        incomplete.push(Diagnostic::new(
            display,
            "summary.gap_id_stable_count",
            "stability aggregates not derivable: analyzed rows lack complete stability evidence; absent values are disclosed, not invented",
        ));
    }

    // Runtime aggregates (only derivable when every run row carries one).
    if let Some((min, median, max, total)) = derived.runtime {
        for (field, expected) in [
            ("runtime_ms_min", min),
            ("runtime_ms_median", median),
            ("runtime_ms_max", max),
            ("runtime_ms_total", total),
        ] {
            match opt_u64(display, summary, field)? {
                Some(value) if value != expected => {
                    return Err(fail(
                        display,
                        &format!("summary.{field}"),
                        format!(
                            "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                        ),
                    ));
                }
                _ => {}
            }
        }
    }

    // Rates.
    for (field, expected) in [
        ("crash_rate", derived.crash_rate()),
        ("parse_failure_rate", derived.parse_failure_rate()),
    ] {
        match opt_number(display, summary, field)? {
            Some(value) if (value - expected).abs() > 1e-9 => {
                return Err(fail(
                    display,
                    &format!("summary.{field}"),
                    format!(
                        "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                    ),
                ));
            }
            _ => {}
        }
    }
    if let Some(expected) = derived.stability_rate() {
        match opt_number(display, summary, "gap_id_stability_rate")? {
            Some(value) if (value - expected).abs() > 1e-9 => {
                return Err(fail(
                    display,
                    "summary.gap_id_stability_rate",
                    format!(
                        "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                    ),
                ));
            }
            _ => {}
        }
    }

    // Distribution agreement: exact map equality against the row-derived key
    // set — no unknown buckets, no missing buckets the rows establish, and
    // zero-valued buckets participate like any other (the sweep writes every
    // bucket). With zero run rows the zero-run law above already bounds every
    // recorded bucket to zero and requires the full emitted key set; the
    // recorded key set is still vocabulary-checked.
    for (field, vocabulary, derived_counts) in [
        (
            "classification_counts",
            &CLASSIFICATION_VOCABULARY[..],
            &derived.classification,
        ),
        (
            "alignment_counts",
            &ALIGNMENT_VOCABULARY[..],
            &derived.alignment,
        ),
    ] {
        if let Some(summary_counts) = opt_distribution(display, summary, field)? {
            if derived_counts.is_empty() {
                for name in summary_counts.keys() {
                    if !vocabulary.contains(&name.as_str()) {
                        return Err(fail(
                            display,
                            &format!("summary.{field}.{name}"),
                            format!(
                                "unknown aggregate bucket `{name}`; known vocabulary: {}",
                                vocabulary.join(", ")
                            ),
                        ));
                    }
                }
            } else {
                check_distribution_equality(display, field, &summary_counts, derived_counts)?;
            }
        }
    }

    // `gate_reason` is owned by the emitted summary; when recorded it must be
    // a non-empty string (its emitted shape), not a wrong-typed stand-in.
    opt_string(display, summary, "gate_reason")?;

    // Gate semantics: the supplied gate_status must EQUAL the gate derived
    // from the rows — `not_run` at zero runs, `pass` only with zero crashes
    // and full stability evidence, `review` otherwise. A wrong `not_run` or
    // `review` is as hand-edited as a wrong count.
    let expected_gate = if derived.run == 0 {
        "not_run"
    } else if derived.crashed == 0 && derived.stable == Some(derived.run) {
        "pass"
    } else {
        "review"
    };
    match opt_string(display, summary, "gate_status")?.as_deref() {
        Some(gate) => {
            known_value_or_fail(
                display,
                "summary.gate_status",
                gate,
                &GATE_STATUSES,
                "gate status",
            )?;
            if gate != expected_gate {
                if derived.run == 0 {
                    return Err(fail(
                        display,
                        "summary.gate_status",
                        format!(
                            "repos_run == 0 is `not_run`, never `{gate}`: a zero-run receipt must not claim an analyzed verdict"
                        ),
                    ));
                }
                if gate == "pass" {
                    if derived.crashed > 0 {
                        return Err(fail(
                            display,
                            "summary.gate_status",
                            format!(
                                "hand-edited aggregate: `pass` claimed but {} row(s) crashed",
                                derived.crashed
                            ),
                        ));
                    }
                    return Err(fail(
                        display,
                        "summary.gate_status",
                        "`pass` claimed without full per-row stability evidence (gap_ids_stable / repeat.gap_ids_stable on every run row)",
                    ));
                }
                return Err(fail(
                    display,
                    "summary.gate_status",
                    format!(
                        "hand-edited aggregate: gate status `{gate}` does not equal the gate derived from the rows (`{expected_gate}`; repos_run={}, crashes={}, stability evidence complete={})",
                        derived.run,
                        derived.crashed,
                        derived.stable == Some(derived.run)
                    ),
                ));
            }
        }
        None => incomplete.push(Diagnostic::new(
            display,
            "summary.gate_status",
            "gate status not recorded",
        )),
    }
    Ok(())
}

/// True when the summary records a present, non-null value for `field`.
fn summary_records_value(summary: &serde_json::Map<String, Value>, field: &str) -> bool {
    matches!(summary.get(field), Some(value) if !value.is_null())
}

/// Exact map equality between a recorded summary distribution and the
/// row-derived key set: every summary bucket must be row-established (extra
/// buckets are denied by name) and every derived bucket must be present at
/// the derived count.
fn check_distribution_equality(
    display: &str,
    field: &str,
    summary_counts: &BTreeMap<String, u64>,
    derived_counts: &BTreeMap<String, u64>,
) -> Result<(), String> {
    for name in summary_counts.keys() {
        if !derived_counts.contains_key(name) {
            return Err(fail(
                display,
                &format!("summary.{field}.{name}"),
                format!(
                    "summary distribution carries bucket `{name}` that the rows never establish (extra buckets are denied; the known keys come from the row distributions)"
                ),
            ));
        }
    }
    for (name, count) in derived_counts {
        match summary_counts.get(name) {
            Some(actual) if actual == count => {}
            Some(actual) => {
                return Err(fail(
                    display,
                    &format!("summary.{field}.{name}"),
                    format!(
                        "hand-edited aggregate: summary claims {actual} but the rows derive {count}"
                    ),
                ));
            }
            None => {
                return Err(fail(
                    display,
                    &format!("summary.{field}.{name}"),
                    format!(
                        "hand-edited aggregate: rows derive {count} for `{name}` but the summary omits it"
                    ),
                ));
            }
        }
    }
    Ok(())
}
