# Fix CI Shape Failures

Use this guide when a policy or shaping check fails before review.

## First Repair Pass

Run:

```bash
cargo xtask shape
```

This safely:

- runs `cargo fmt`
- sorts `.ripr/*.txt` and `policy/*.txt` allowlist entries
- creates `target/ripr/reports`
- writes `target/ripr/reports/shape.md`

Then run:

```bash
cargo xtask precommit
```

`precommit` is the cheap non-mutating guardrail. It catches formatting, policy,
spec, fixture, generated-file, and file-surface issues without running the full
review-ready gate.

## Fix-PR Shortcut

Run:

```bash
cargo xtask fix-pr
```

This runs `shape`, refreshes `target/ripr/reports/pr-summary.md`, and writes
`target/ripr/reports/fix-pr.md`.

After `fix-pr`, run:

```bash
cargo xtask check-pr
```

`check-pr` is the review-ready non-release gate. It runs the fast Rust and
policy checks, clippy, docs, and PR summary generation. Release/package checks
remain in `cargo xtask ci-full`.

## Reviewer Packet

Run:

```bash
cargo xtask pr-summary
```

This writes `target/ripr/reports/pr-summary.md` with:

- production delta
- evidence/support delta
- detected surfaces
- public contracts touched
- policy exception surfaces
- suggested reviewer focus

## When Shape Cannot Fix It

Shape does not make judgment calls.

If a check asks for an exception, edit the named allowlist manually and include:

- path or glob
- kind or pattern
- owner
- reason

Policy checks write Markdown reports under `target/ripr/reports`. Open the
matching report first; it should include why the rule exists, the fix kind, a
recommended repair, and an exception template when exceptions are valid.

If a check reports a forbidden output-language claim, change the product output
or move the wording into an explicitly allowlisted explanatory document.

If a check reports a panic-family pattern, prefer returning `Result` or
pattern-matching explicitly instead of adding an exception.
