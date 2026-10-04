//! The generated workflow's `Add RIPR advisory summary` step.
//!
//! #4386 slice 2: the step's 1,266 lines of shell and jq now live behind
//! `ripr reports ci-summary` (`output::ci_summary`), so the adopter's
//! workflow carries one command instead of a renderer. The step passes
//! the workflow environment the shell used to read as flag values; an
//! unset variable arrives empty and keeps its shell meaning.

/// The complete step, from `      - name: Add RIPR advisory summary` through
/// the blank line before `      - name: Upload RIPR report artifacts`.
pub(super) const ADVISORY_SUMMARY_STEP: &str = r#"      - name: Add RIPR advisory summary
        if: always()
        continue-on-error: true
        env:
          RIPR_BASE_REF: ${{ github.base_ref || github.event.repository.default_branch }}
          RIPR_INSTALL_OUTCOME: ${{ steps.install.outcome }}
        run: |
          # Check the outcome, not only PATH: a runner can carry an older
          # ripr that would render this run's summary with the wrong version.
          if [ "${RIPR_INSTALL_OUTCOME:-}" != success ] || ! command -v ripr >/dev/null 2>&1; then
            {
              echo '## RIPR advisory summary'
              echo
              echo "the pinned ripr is not installed (Install ripr step: ${RIPR_INSTALL_OUTCOME:-not run}), so this run produced no RIPR reports."
              echo
              echo 'Next: open the Install ripr step log. Its error line names the cause and the fix; re-run the job once it is fixed.'
            } >> "$GITHUB_STEP_SUMMARY"
            echo "::error::the pinned ripr is not installed (Install ripr step: ${RIPR_INSTALL_OUTCOME:-not run}); see that step's log."
            exit 1
          fi
          ripr reports ci-summary --root . \
            --base-ref "$RIPR_BASE_REF" \
            --upload-sarif "${RIPR_UPLOAD_SARIF:-}" \
            --gate-baseline "${RIPR_GATE_BASELINE:-}" \
            --comment-mode "${RIPR_COMMENT_MODE:-}" \
            >> "$GITHUB_STEP_SUMMARY"

"#;
