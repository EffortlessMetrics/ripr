//! The generated workflow's `Add RIPR advisory summary` step.
//!
//! #4386 slice 2: the step's 1,266 lines of shell and jq now live behind
//! `ripr reports ci-summary` (`output::ci_summary`), so the adopter's
//! workflow carries one command instead of a renderer. The step passes
//! the workflow environment the shell used to read as flag values; an
//! unset variable arrives empty and keeps its shell meaning.

/// The complete step, from `      - name: Add RIPR advisory summary` through
/// the blank line before `      - name: Check RIPR advisory artifacts`.
pub(super) const ADVISORY_SUMMARY_STEP: &str = r#"      - name: Add RIPR advisory summary
        if: always()
        continue-on-error: true
        env:
          RIPR_BASE_REF: ${{ github.base_ref || github.event.repository.default_branch }}
        run: |
          ripr reports ci-summary --root . \
            --base-ref "$RIPR_BASE_REF" \
            --upload-sarif "${RIPR_UPLOAD_SARIF:-}" \
            --gate-baseline "${RIPR_GATE_BASELINE:-}" \
            --comment-mode "${RIPR_COMMENT_MODE:-}" \
            >> "$GITHUB_STEP_SUMMARY"

"#;
