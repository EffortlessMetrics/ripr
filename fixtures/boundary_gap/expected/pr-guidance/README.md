# PR Guidance Fixture Cases

These fixtures pin the bounded `ripr review-comments` report shapes used by
Campaign 13. They are renderer fixtures, not source-edit or generated-test
fixtures.

Each JSON/Markdown pair carries `analysis_scope` metadata. The CLI default uses
`diff_scoped_changed_files` for live diffs, while these renderer-level fixtures
use `working_set` scope because they construct the changed-file set directly.
In both cases, scope metadata is part of the contract: review guidance must
name its narrowed input instead of implying full-repo truth.

Cases:

- `exact-line`: seam maps directly to a changed line.
- `owner-function-line`: seam maps to the changed owner function.
- `same-file-line`: seam falls back to the nearest changed line in the same file.
- `summary-only`: no safe changed-line placement is available.
- `capped`: inline and summary caps suppress excess recommendations.
- `changed-test-skip`: a nearby recommended test file changed, so guidance is suppressed.
- `configured-off`: selector warnings explain configured-off seams without comments.

The fixture test is read-only. Refresh these files intentionally by inspecting
the rendered drift and updating the paired JSON/Markdown files in the same
change.
