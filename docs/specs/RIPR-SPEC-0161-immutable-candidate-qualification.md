# RIPR-SPEC-0161: Immutable candidate isolation and parity qualification

Status: proposed

Issue: #3279 (parent #3237; builds on #3276 / #3277 / #3278)

## Problem

The typed model, object producer, and CLI surface can all exist while
the implementation still leaks mutable repository state into a subject
run or diverges from ordinary committed analysis. #3237 should close
only after adversarial end-to-end tests prove the immutable subject
behaves as claimed.

## Behavior

- **Config isolation** (the qualification's first real defect, found by
  the reproduction): a bound subject configures itself. The candidate
  tree's own `ripr.toml` — read from the tree object via `git show
  <tree>:ripr.toml` — replaces the worktree file for the run; a tree without one uses the PURE default config — no worktree fact (including the enabled-languages list) may enter the fallback (#3279 review B1: toggling the worktree languages flipped subject completeness). `source_path` is cleared so the
  recorded identity cannot claim the worktree file as its source. The
  CLI binds the subject **before** loading config so `apply_to_check_input`
  consumes the candidate config, not the worktree's.
- **Replayable paths** (the inherited #3395 follow-up): finding/probe
  locations name the user's repository root, never the ephemeral
  materialization directory. The relative path inside the candidate tree
  is preserved exactly; only the prefix is rebased from the materialized
  root back to the named repository root at the same seam that set it.
- **The falsifier corpus** (`tests/immutable_candidate_falsifier.rs`,
  7 tests against real two-commit fixture repositories and the built
  binary):
  1. the issue's exact reproduction — post-bind worktree source, test,
     `ripr.toml`, and staged-index mutations produce byte-identical
     output;
  2. same-tree committed parity — the subject run and the ordinary
     `--base` range-diff run over the same commits agree on findings,
     classifications, ordering, and summaries after removing only the
     declared non-portable telemetry (identity block, mode string, root,
     base echo);
  3. emitted identities match the request exactly (candidate tree OID,
     per-format empty-tree base OID, `sha256:` diff identity) and no
     `ripr-git-candidate` temp path appears anywhere in the output;
  4. delete and rename shapes resolve from objects with a dirty
     worktree (a resurrection of the deleted file never enters the run),
     and a type change (regular file to symlink) fails closed naming
     the entry;
  5. invalid subjects fail closed inside the named boundary — never
     clean zero findings;
  6. the removal experiment: the findings carry the candidate bytes
     (`expression: "2"`), and worktree-only bytes never appear — a
     producer that silently substituted the worktree would flip both
     discriminators;
  7. a completed subject run leaves no per-run materialization roots
     behind (children of the shared parent, with a settle bound for
     concurrent corpus runs); and a treeless-config control toggles the
     worktree enabled-languages list and requires identical subject
     output (the review's B1 reproduction pinned).

## Required Evidence

- The reproduction flip: before the config fix, a worktree `ripr.toml`
  changing `mode = "draft"` to `mode = "deep"` changed the subject
  run's rendered mode; before the path rebase, `probe.file` carried
  `the ephemeral materialization directory under the system temp root (ripr-git-candidate/<pid>-<nanos>/<tree>/src/lib.rs)`. After
  both, the four-mutation reproduction is byte-identical.
- Corpus green across repeated runs; zero golden drift (no fixture
  output changes — the fixes only affect the new subject path and its
  own outputs).

## Required guards

- The candidate config read never opens the worktree file; a tree
  without `ripr.toml` uses the default config (no fabricated config).
- The path rebase preserves the relative path exactly; a path outside
  the materialized root is left untouched (fail-open on the rewrite is
  honest — the analyzer produced it from candidate bytes).
- Parity comparisons null only the declared non-portable telemetry;
  findings, classifications, related tests, and ordering must match.

## Acceptance Examples

- Accept: bind base B and candidate tree T, mutate the worktree source,
  an unchanged test, `ripr.toml`, and the live index — identical output.
- Reject: a worktree config changing a subject run; a temp-root path in
  output; a subject run reading resurrected worktree bytes.

## Test Mapping

`tests/immutable_candidate_falsifier.rs` (the corpus);
`config::config_for_candidate` + `git_candidate_execution::
candidate_config_bytes` (config isolation); `pipeline::
rebase_finding_paths_to_repository` (replayable paths).

## Non-Goals

- No #3212/#3213 semantics, downstream hook migration, release
  publication, universal performance guarantee, or mid-run mutation
  races (the corpus mutates pre-invocation; a synchronization hook for
  mid-run mutation is a follow-up if the corpus is extended).

## Implementation Mapping

- `config.rs` — `config_for_candidate`.
- `analysis/git_candidate_execution.rs` — `candidate_config_bytes`.
- `analysis/pipeline.rs` — the path rebase.
- `tests/immutable_candidate_falsifier.rs` — the corpus.

## Metrics

No new metric; corpus pass/fail is the qualification signal.
