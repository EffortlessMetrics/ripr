//! Bounded disclosure for capped-read refusals (#5022).
//!
//! The workspace read budgets cap *work*; the disclosure of that bounded
//! state must stay bounded too. The TypeScript and Python preview adapters
//! each collect one refusal record per file the read caps reject — up to the
//! workspace file-count cap (20,000 TypeScript files) per run — and the
//! adapters used to turn every record into its own `AnalysisLimitation`,
//! letting one correctly-capped monorepo emit tens of thousands of
//! limitation objects that dwarf the actual findings.
//!
//! [`bounded_read_limit_limitations`] folds a refusal list into a bounded,
//! deterministic disclosure: a stable-sorted sample of named paths plus, when
//! the sample overflows, one summary entry carrying the true refused count
//! and stating why the full per-file list is not materialized in output.
//! Fail-closed behavior is unchanged — every refused file is still refused;
//! only the disclosure is sampled.

use crate::analysis_outcome::{
    AnalysisLimitation, AnalysisLimitationKind, AnalysisRecovery, AnalysisRecoveryKind,
    AnalysisStage,
};

/// Hard bound on per-file read-limit limitation entries one adapter discloses
/// per run. A named constant, not an env knob: the disclosure must stay
/// bounded in precisely the large-workspace case the read caps exist for
/// (#5022), and an operator-tunable disclosure cap would recreate the
/// unbounded-output problem under a different default.
pub(crate) const MAX_READ_LIMIT_SAMPLE_PATHS: usize = 8;

/// Fold capped-read refusals into a bounded limitation disclosure.
///
/// - `adapter` names the producing adapter in the summary detail (for example
///   `typescript` or `python`), so JSON/SARIF consumers can key on a stable
///   machine-readable prefix.
/// - `refusals` carries one `(normalized path, refusal reason)` pair per
///   refused file, in any order.
/// - `recovery_detail` is the shared recovery text naming the env knobs that
///   raise the read caps.
///
/// The returned vector holds at most `MAX_READ_LIMIT_SAMPLE_PATHS` per-file
/// entries — the first entries after a stable sort by path, so repeated runs
/// over the same workspace emit byte-stable output regardless of
/// directory-enumeration order — plus one summary entry when the refusal
/// list overflows the sample. The summary entry's `affected_items` is the
/// true refused count; its detail names the sample bound and states that the
/// full per-file list is not materialized in output (the refusals live only
/// in memory for the run; output is the only durable record).
pub(crate) fn bounded_read_limit_limitations(
    adapter: &str,
    mut refusals: Vec<(String, String)>,
    recovery_detail: &str,
) -> Result<Vec<AnalysisLimitation>, String> {
    refusals.sort_by(|left, right| left.0.cmp(&right.0));
    let total = refusals.len();
    let recovery =
        |detail: &str| AnalysisRecovery::new(AnalysisRecoveryKind::IncreaseConfiguredLimit, detail);
    let mut limitations = Vec::new();
    for (path, reason) in refusals.iter().take(MAX_READ_LIMIT_SAMPLE_PATHS) {
        limitations.push(
            AnalysisLimitation::new(
                AnalysisLimitationKind::LanguageScopeUnsupported,
                AnalysisStage::LanguageAdapter,
                recovery(recovery_detail)?,
            )
            .with_path(path)?
            .with_affected_items(1)?
            .with_detail(reason.clone())?,
        );
    }
    if total > MAX_READ_LIMIT_SAMPLE_PATHS {
        let remainder = total - MAX_READ_LIMIT_SAMPLE_PATHS;
        limitations.push(
            AnalysisLimitation::new(
                AnalysisLimitationKind::LanguageScopeUnsupported,
                AnalysisStage::LanguageAdapter,
                recovery(recovery_detail)?,
            )
            .with_affected_items(
                u64::try_from(total)
                    .map_err(|err| format!("read-limit refusal count overflows u64: {err}"))?,
            )?
            .with_detail(format!(
                "{adapter}_read_limit_sampled: {total} workspace file(s) refused by read caps; \
                 the first {MAX_READ_LIMIT_SAMPLE_PATHS} sample paths (sorted) are listed above; \
                 {remainder} further refused path(s) are not listed individually and the full \
                 per-file list is not materialized in output (bounded disclosure cap of \
                 {MAX_READ_LIMIT_SAMPLE_PATHS}); raise the read caps to analyze the refused files"
            ))?,
        );
    }
    Ok(limitations)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECOVERY: &str = "Raise RIPR_TEST_MAX_FILE_READ_BYTES and/or RIPR_TEST_MAX_WORKSPACE_READ_BYTES, then re-run the analysis.";

    fn refusal(name: &str) -> (String, String) {
        (name.to_string(), format!("file_read_capped: {name}"))
    }

    fn read_limit_entries(limitations: &[AnalysisLimitation]) -> Vec<&AnalysisLimitation> {
        limitations
            .iter()
            .filter(|limitation| {
                limitation
                    .bounded_detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("file_read_capped"))
            })
            .collect()
    }

    #[test]
    fn under_cap_lists_every_refusal_sorted_without_summary() -> Result<(), String> {
        // Input in walk (unsorted) order: the disclosure must come out in a
        // stable sorted order either way.
        let limitations = bounded_read_limit_limitations(
            "testlang",
            vec![
                refusal("src/zeta.ts"),
                refusal("src/alpha.ts"),
                refusal("src/mid.ts"),
            ],
            RECOVERY,
        )?;
        assert_eq!(limitations.len(), 3, "no summary entry under the cap");
        assert_eq!(
            limitations
                .iter()
                .map(|limitation| limitation.path.as_deref())
                .collect::<Vec<_>>(),
            vec![
                Some("src/alpha.ts"),
                Some("src/mid.ts"),
                Some("src/zeta.ts")
            ],
            "sample must be sorted by path",
        );
        assert!(
            limitations
                .iter()
                .all(|limitation| limitation.affected_items == Some(1)),
            "per-file entries count one refused file each",
        );
        assert!(
            limitations.iter().all(|limitation| {
                matches!(
                    limitation.recovery.kind,
                    AnalysisRecoveryKind::IncreaseConfiguredLimit
                )
            }),
            "recovery must name the configured limit knob",
        );
        Ok(())
    }

    #[test]
    fn over_cap_bounds_entries_and_preserves_the_refused_total() -> Result<(), String> {
        let total = MAX_READ_LIMIT_SAMPLE_PATHS + 7;
        let refusals = (0..total)
            .map(|index| refusal(&format!("pkg/{index:04}.py")))
            .rev()
            .collect::<Vec<_>>();
        let limitations = bounded_read_limit_limitations("testlang", refusals, RECOVERY)?;
        assert_eq!(
            limitations.len(),
            MAX_READ_LIMIT_SAMPLE_PATHS + 1,
            "bounded sample plus exactly one summary entry"
        );
        let summary = limitations
            .last()
            .ok_or_else(|| "summary entry must exist when the sample overflows".to_string())?;
        let detail = summary
            .bounded_detail
            .as_deref()
            .ok_or_else(|| "summary must carry a detail".to_string())?;
        assert!(
            detail.starts_with("testlang_read_limit_sampled:"),
            "summary must carry the machine-readable prefix, got {detail:?}"
        );
        assert_eq!(
            summary.affected_items,
            Some(total as u64),
            "summary must carry the true refused count"
        );
        assert!(
            detail.contains(&total.to_string()),
            "summary detail must disclose the true refused count: {detail}"
        );
        assert!(
            detail.contains(&format!("{}", total - MAX_READ_LIMIT_SAMPLE_PATHS)),
            "summary detail must disclose the folded remainder: {detail}"
        );
        assert!(
            detail.contains("not materialized in output"),
            "summary must state why the full list is absent: {detail}"
        );
        assert!(
            summary.path.is_none(),
            "the folded summary names no single path, got {:?}",
            summary.path
        );
        assert_eq!(
            read_limit_entries(&limitations).len(),
            MAX_READ_LIMIT_SAMPLE_PATHS,
            "only the sample keeps per-file entries"
        );
        assert_eq!(
            limitations[..MAX_READ_LIMIT_SAMPLE_PATHS]
                .iter()
                .map(|limitation| limitation.path.clone())
                .collect::<Vec<_>>(),
            (0..MAX_READ_LIMIT_SAMPLE_PATHS)
                .map(|index| Some(format!("pkg/{index:04}.py")))
                .collect::<Vec<_>>(),
            "sample must be the sorted prefix of the refusal set"
        );
        Ok(())
    }

    #[test]
    fn sample_is_byte_stable_regardless_of_input_order() -> Result<(), String> {
        let forward = (0..MAX_READ_LIMIT_SAMPLE_PATHS + 3)
            .map(|index| refusal(&format!("f{index}.ts")))
            .collect::<Vec<_>>();
        let reversed = forward.iter().rev().cloned().collect::<Vec<_>>();
        let first = bounded_read_limit_limitations("testlang", forward, RECOVERY)?;
        let second = bounded_read_limit_limitations("testlang", reversed, RECOVERY)?;
        assert_eq!(
            first, second,
            "same workspace must emit identical disclosure"
        );
        Ok(())
    }

    #[test]
    fn empty_refusal_list_discloses_nothing() -> Result<(), String> {
        let limitations = bounded_read_limit_limitations("testlang", Vec::new(), RECOVERY)?;
        assert!(limitations.is_empty());
        Ok(())
    }
}
