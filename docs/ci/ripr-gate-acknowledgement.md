# Acknowledging a soft RIPR gate finding

A downstream consumer may run the gate in `acknowledgeable` mode when a
reviewed, soft static exposure is intentionally accepted for the pull request.

## Consumer contract

Run the gate against the producer's review artifact and pass the pull request
labels as JSON:

```bash
ripr gate evaluate \
  --pr-guidance target/ripr/review/comments.json \
  --mode acknowledgeable \
  --labels-json target/ripr/ci/pull-request-labels.json \
  --out target/ripr/reports/gate-decision.json
```

The labels file is the GitHub
`pull_request.labels` projection. Both object and string-array forms are
accepted. The default acknowledgment label is `ripr-waive`; a consumer may
replace it with `--acknowledgement-label NAME`.

The gate remains fail-closed:

- `weakly_exposed` is a soft, policy-eligible finding. It remains visible and
  blocks acknowledgeable mode until the configured acknowledgment label is
  present.
- `reachable_unrevealed` is a stronger policy-eligible finding. The same
  acknowledgment path exists, but the reviewer must decide whether accepting
  it is appropriate.
- Other classes remain advisory unless a separate reviewed policy makes them
  eligible.
- A missing, malformed, or stale labels artifact is not an acknowledgment.

When no matching label is present, the blocking decision and CLI error name the
expected label. A consumer should create that label in its own repository and
require a review comment that records the finding identity, why the defensive
or fail-closed behavior is acceptable, and why no artificial test is being
added. The label is an input to the gate; it is not proof that the finding is
safe by itself.

## Workflow shape

Keep label acquisition and gate evaluation in the consumer workflow. The
consumer should:

1. write the exact `pull_request.labels` projection to a bounded artifact;
2. run `ripr gate evaluate --mode acknowledgeable --labels-json ...`;
3. upload `gate-decision.json` and its Markdown projection;
4. fail on an unacknowledged eligible finding;
5. retain the review comment and label as the human decision record.

Do not infer acknowledgment from a label name embedded in source, an issue
number, a stale report, or a green unrelated check. Bind the gate report to
the exact PR subject and producer artifact used by the workflow.

This is an acknowledgment route, not a correctness or mutation-kill claim.
