# github_unanalyzed_states

Pinned full-stream golden output for the GitHub annotation renderer's
unanalyzed-run-state warnings (#5011). The human and JSON surfaces disclose
these states as NOT clean; the GitHub stream must not read as all-clear for
the same `CheckOutput`.

Each `expected/*.txt` file is the byte-exact complete renderer output for one
`CheckOutput` state, produced by `output::github::render` with zero findings
and no analysis outcome. The renderer tests in
`crates/ripr/src/output/github.rs` compare against these files with
`assert_eq!`, so warning order, wording, escaping, and stream shape stay
pinned.

- `unanalyzed_working_tree.txt` — `unanalyzed_working_tree: true`: the
  uncommitted-changes limitation and the `--worktree` remedy route, mirroring
  `UNANALYZED_WORKING_TREE_NOTE` on the human surface.
- `no_scope_provided.txt` — `no_scope_provided: true` with no established
  base: the diff-first scope note and the `ripr check --base BASE` remedy,
  mirroring the human no-scope note.
- `preview_advisory.txt` — a detected-but-not-analyzed preview advisory. The
  wire string is deliberately unregistered ("cobol") so the recovery text is
  identical in every feature-lane build; real preview languages have
  feature-dependent recovery text (adapter compiled in or not), which the
  renderer tests cover with feature-independent fragment assertions.

Regeneration: hand-trace `render` in `crates/ripr/src/output/github.rs` (the
warning lines are built by `unanalyzed_state_warnings`); there is no separate
generator. Update the file and the matching renderer test in the same change.

This directory is registered in `is_manifest_only_fixture_dir`
(`xtask/src/reports/fixtures.rs`): it is a pinned-output corpus, not a
runnable BDD fixture (it has no `input/` workspace or `diff.patch`, and an
end-to-end `check.json` golden would embed sha256 digests only a live run can
produce). Its dedicated validator is the renderer unit tests above, which
compare the full stream byte-for-byte in every feature-lane test run.
