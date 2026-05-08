# PR Guidance Fixture Cases

These fixtures pin the bounded `ripr review-comments` report shapes used by
Campaign 13. They are renderer fixtures, not source-edit or generated-test
fixtures.

Cases:

- `exact-line`: seam maps directly to a changed line.
- `owner-function-line`: seam maps to the changed owner function.
- `same-file-line`: seam falls back to the nearest changed line in the same file.
- `summary-only`: no safe changed-line placement is available.
- `capped`: inline and summary caps suppress excess recommendations.
- `changed-test-skip`: a nearby recommended test file changed, so guidance is suppressed.
- `configured-off`: selector warnings explain configured-off seams without comments.

Refresh intentionally with:

```bash
$env:RIPR_UPDATE_PR_GUIDANCE_FIXTURES = "1"
cargo test -p ripr review_comments_pr_guidance_fixtures_pin_required_cases --lib
Remove-Item Env:\RIPR_UPDATE_PR_GUIDANCE_FIXTURES
```
