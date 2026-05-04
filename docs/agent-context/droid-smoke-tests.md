# Droid Smoke Tests

Run these after changing Droid workflows, Droid review guidance, or Droid model configuration.

## Automatic review

1. Open a same-repo draft PR.
2. Confirm Droid Auto Review starts.
3. Confirm the run initializes with `custom:MiniMax-M2.7-0`.
4. Confirm output is not naked LGTM.
5. Confirm clean review includes:
   - inspected surfaces;
   - checks performed;
   - why no comments;
   - residual risk;
   - validation signal.

## Manual review

Comment:

```text
@droid review
```

Expected:

- trusted actor guard allows the run;
- MiniMax BYOK model is used;
- comments follow `[P0|P1|P2]` and repair-queue format.

## Manual security review

Comment:

```text
@droid security
```

Expected:

- security review runs;
- no unrelated code edits;
- findings include severity and fix direction.

## Full security scan

Run the scheduled security scan manually with `workflow_dispatch`.

Expected:

- scan creates or updates a security report PR;
- report path is under `.factory/security/reports/`;
- no secrets are printed in logs or artifacts.
