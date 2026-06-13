# Fixture: ts_full_repo_guidance

Spec: RIPR-SPEC-0089

## Given

A workspace that contains only TypeScript source files (`src/discount.ts`,
`src/utils.ts`) and a `package.json` — no Rust crate whatsoever.

## When

```bash
ripr check --root fixtures/ts_full_repo_guidance/input --format repo-exposure-md
ripr check --root fixtures/ts_full_repo_guidance/input --format repo-exposure-json
```

## Then

Both repo-exposure outputs emit a named `typescript_diff_first` guidance
disclosure in `limitations[]` (JSON) / `## Limitations` section (Markdown).
The disclosure carries:

- `category`: `typescript_diff_first`
- `ts_file_count`: 2
- `repair_route`: pointing to `ripr check --base origin/main` or `--diff <file>`

The `seams` array is empty — no fabricated TypeScript seams are emitted.

## Must Not

- Emit any TypeScript seam or finding.
- Use mutation-runtime vocabulary (runtime-testing terms forbidden by the
  static-language gate: see `check-static-language` output for the blocked list).
- Fire for a Rust workspace (regression guard: the guidance fires only when
  Rust files are absent and TS files are present).
