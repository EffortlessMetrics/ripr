//! The generated workflow's `Add RIPR advisory summary` step.
//!
//! #4386 slice 2: the step's 1,266 lines of shell and jq now live behind
//! `ripr reports ci-summary` (`output::ci_summary`), so the adopter's
//! workflow carries one command instead of a renderer. The command reads
//! the workflow environment itself (#5409), so the step passes no flags.

/// The complete step, from `      - name: Add RIPR advisory summary` through
/// the blank line before `      - name: Upload RIPR report artifacts`.
pub(super) const ADVISORY_SUMMARY_STEP: &str = r#"      - name: Add RIPR advisory summary
        if: always()
        continue-on-error: true
        env:
          RIPR_INSTALL_OUTCOME: ${{ steps.install.outcome }}
        run: |
          if [ "${RIPR_INSTALL_OUTCOME:-}" != success ]; then
            printf '## RIPR advisory summary\n\nthe pinned ripr is not installed (Install ripr step: %s), so this run produced no RIPR reports.\n\nNext: open the Install ripr step log. Its error line names the cause and the fix; re-run the job once it is fixed.\n' "${RIPR_INSTALL_OUTCOME:-not run}" >> "$GITHUB_STEP_SUMMARY"
            echo "::error::the pinned ripr is not installed (Install ripr step: ${RIPR_INSTALL_OUTCOME:-not run}); see that step's log."; exit 1
          fi
          ripr reports ci-summary --root . >> "$GITHUB_STEP_SUMMARY"

"#;
