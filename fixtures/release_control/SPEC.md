# Release control lens fixture

Spec: RIPR-SPEC-0144

This manifest-only corpus exercises the temporary 0.11 release-control lens
tracked by [#2766](https://github.com/EffortlessMetrics/ripr-swarm/issues/2766).
The snapshots are captured inputs, not release authority or candidate proof.

## Given

- a current `main` SHA and matching #2379 authority snapshot;
- complete portfolio, open-PR, and active-claim inventories;
- a complete worktree inventory observation;
- explicit release dispositions for every captured open PR.
- an optional #2766 `candidate_selection` authority; when present, it is
  evaluated independently from the open-PR inventory.

## When

- `cargo xtask release-control --input <snapshot.json>` normalizes the input;
- `cargo xtask release-control --live` collects bounded current GitHub/main and
  worktree observations, then fails closed when portfolio or claim authority is
  incomplete.

## Then

- JSON and Markdown retain the same sorted PR disposition records;
- `release_required` is the only disposition that may be merge-eligible;
- a complete snapshot is stable under input-order changes.
- missing candidate selection remains `scope_pending` and cannot imply a
  hard-cut or qualification-ready candidate;
- the candidate-state negative corpus rejects each staged false-ready state.

## Must Not

- infer release eligibility from an issue number, branch name, or PR age;
- treat missing, stale, or contradictory authority as merge-eligible;
- close, merge, relabel, rebase, create, delete, publish, or qualify anything.
