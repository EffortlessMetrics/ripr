# Triage a Finding

Use this guide when a `ripr` finding looks surprising. The goal is to decide
whether the changed behavior needs a test, whether the analyzer needs a
follow-up, or whether the team has a bounded, documented exception to carry.

`ripr` reports static exposure evidence. A finding is not a runtime test result,
and a suppression is not a replacement for a missing discriminator.

## 1. Keep the finding identity

Run the check that produced the finding and save its exact `finding_id` and
path. Detailed reports keep suppressed findings visible, so the identifier is
also the durable reference for later review.

```text
ripr check --root . --json --suppression-policy .ripr/suppressions.toml
```

The explicit policy flag is required for suppression-aware findings JSON.
Badge-specific formats load the repository manifest, but their exposure-gap
application currently matches exact `finding_id` entries; prefer an exact
finding id when the exception applies to one finding.

## 2. Inspect the evidence

Use `explain` for the human-readable finding and `context` for the compact
related-test packet:

```text
ripr explain --root . <finding-id>
ripr context --root . --at <finding-id> --json
```

Check the changed sink, the related test or oracle evidence, and any stated
static limitation. If the finding describes behavior that should be covered,
add or repair the test. If the analyzer appears to align the wrong entities,
capture the exact finding id, source location, command, and current commit when
filing the analyzer follow-up.

## 3. Choose fix, follow-up, or suppression

- **Fix the behavior gap** when the changed behavior lacks a meaningful test.
- **File an analyzer follow-up** when the evidence is misclassified or the
  static limitation needs product work. Keep the finding visible while that
  work is pending.
- **Suppress only an accepted exception** when the team has reviewed the gap,
  can name its owner and reason, and has a bounded review or expiry date.

Do not suppress a finding merely to improve a badge. Suppressed findings remain
in detailed reports and move only into the suppressed badge bucket.

## 4. Add a durable suppression

The current contract is a hand-authored `.ripr/suppressions.toml` file. Start
from the [suppression example](../suppressions.example.toml), then copy only
the entries that apply into the repository's `.ripr/suppressions.toml`.

Every entry needs:

- `kind = "exposure_gap"` with either an exact `finding_id` or a repository-
  relative `path` glob. A path glob may use `static_class` to narrow the
  matching exposure class; policy health also expects this metadata on other
  durable entries.
- `kind = "test_efficiency"` with `test`; `path` is optional but useful when
  test names repeat.
- non-blank `owner` and `reason`.

Use `/` in repository-relative paths. Keep selectors narrow, add `expires`
and policy-health dates where appropriate, and avoid unknown fields: the
manifest parser rejects them. Preview-language entries also need
`language_status = "preview"` until the repository policy promotes that
language.

The path-glob example in the example file is for an explicit
`--suppression-policy` findings run. For implicit badge suppression, use exact
`finding_id` entries; do not assume that a path glob changes badge counts.

## 5. Check suppression health

Run the read-only policy report after editing the manifest:

```text
ripr policy suppression-health --root .
```

Review the generated files:

```text
target/ripr/reports/suppression-health.json
target/ripr/reports/suppression-health.md
```

The report highlights missing ownership or reasons, stale review windows,
overbroad scope, unknown selectors, missing policy metadata, and preview
language metadata gaps. It does not create, apply, delete, or gate
suppressions.

## 6. Re-run the normal check

Run the same check mode used by the workflow or local review. Confirm that the
finding remains visible in detailed output and that only the badge counts move
to the suppressed bucket. Revisit the exception before its `review_by` or
`expires` date; an expired entry no longer applies and is reported as a
warning.

For the full schema and policy-health field definitions, see the
[configuration reference](../CONFIGURATION.md#ripr-suppressions-toml). The
repository does not currently provide a `ripr suppress` convenience command;
that is separate follow-up work.
