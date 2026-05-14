# Annotation Policy

Annotations are advisory routing hints. They are not gate decisions, inline PR
comments, merge decisions, or proof that a finding is correct.

## Default Posture

The default posture is:

```text
summary and artifacts always
warning annotations when safely placeable
inline review comments off
blocking gates off unless explicitly configured
```

Generated CI may append Markdown summaries and upload artifacts by default.
It may emit non-blocking check annotations from `comments[]`. It must not post
inline PR review comments by default.

## Annotation Source

Only this collection can annotate:

```text
target/ripr/review/comments.json -> comments[]
```

The annotation consumer must ignore:

```text
summary_only[]
suppressed[]
warnings[]
```

Those collections remain visible in summaries and artifacts. They are not safe
changed-line annotation sources. This applies to warnings from both
`target/ripr/pr/repo-exposure.json` and `target/ripr/review/comments.json`.

## Changed-Line Rule

Every annotation must point at a changed line. The producer or consumer must
enforce that the annotation target is one of:

1. exact changed seam line;
2. nearest changed line in the same owner function;
3. nearest changed line in the same file.

If none of those placements is safe, the item belongs in `summary_only[]`.
Bad placement is worse than no placement.

## Severity And Blocking

Annotations are warning-level and non-blocking.

Allowed:

```text
::warning file=src/lib.rs,line=42::Add one focused discriminator test.
```

Not allowed by default:

```text
::error ...
exit 1 because comments[] is non-empty
post inline review comments
convert summary_only[] into annotations
```

Configured gate decisions, when present, must point to their own authority
artifact. A summary, badge, annotation, or review-comment report does not
become pass/fail authority by projection.

## Missing Artifacts

Missing `comments.json` should exit cleanly in advisory CI and write an
explicit summary message:

```text
No RIPR review guidance artifact was produced.
```

Missing artifacts can fail only when the command contract itself is being
checked, such as `cargo xtask ripr-review-comments --check`, or when a later
explicit gate policy says the artifact is required.

## Inline Comment Opt-In

Inline PR comments are a separate opt-in publisher. Any publisher must:

- post only from `comments[]`;
- target only changed lines;
- cap comments;
- deduplicate by `dedupe_key`;
- never post `summary_only[]`;
- keep comments advisory;
- leave a machine-readable publish plan or receipt.

The default fleet contract does not require inline comments.
