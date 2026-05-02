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
cargo xtask ci-fast
```

## Fix-PR Shortcut

Run:

```bash
cargo xtask fix-pr
```

This currently runs `shape` and writes `target/ripr/reports/fix-pr.md`.
Full PR summary generation is planned as the next automation slice.

## When Shape Cannot Fix It

Shape does not make judgment calls.

If a check asks for an exception, edit the named allowlist manually and include:

- path or glob
- kind or pattern
- owner
- reason

If a check reports a forbidden output-language claim, change the product output
or move the wording into an explicitly allowlisted explanatory document.

If a check reports a panic-family pattern, prefer returning `Result` or
pattern-matching explicitly instead of adding an exception.
