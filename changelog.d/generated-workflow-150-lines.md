<!-- section: Changed -->
- `ripr init --ci github` writes a 150-line workflow, down from 385 (#5409).
  The inline-comment capture and publish logic moved from jq into two new
  commands, `ripr pr-comments existing` and `ripr pr-comments requests`, so the
  workflow only calls `gh api`. `ripr reports ci-summary` reads the base ref
  and `RIPR_*` settings from the workflow environment when its flags are left
  out. The long setting comments moved to docs/CI.md, and the report upload
  now ships all of `target/ripr`.
