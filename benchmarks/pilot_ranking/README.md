# Pilot ranking answer key

`ripr pilot` sends a developer to its top-ranked seams first, so a wrong top
pick is the error people meet first. This directory is a checked-in answer key
for those picks: five small crates pinned to exact commits, and the outcome of
every viable cargo-mutants mutant at each pin.

```bash
cargo xtask pilot-ranking check                      # offline: manifest and labels are valid
cargo xtask pilot-ranking fetch --allow-network      # pinned checkouts under target/ripr/pilot-ranking/checkouts
cargo xtask pilot-ranking score                      # builds release ripr, writes target/ripr/reports/pilot-ranking.{json,md}
cargo xtask dx-scoreboard --boards ranking \
  --ingest target/ripr/reports/pilot-ranking.json \
  --baseline metrics/dx-scoreboard/pilot-ranking-baseline.json --gate
```

`score --ripr <binary>` scores a binary you already built, so a ranking change
can be compared before and after without cargo-mutants. A full score of the
five crates takes seconds once ripr is built.

## What is pinned

| Crate | Revision | Labeled (caught / missed) | Unlabeled |
| --- | --- | --- | --- |
| semver | `280ebcb6edac` | 268 / 60 | 32 unviable, 5 timeout |
| bytesize | `66a3715e3336` | 186 / 144 | 23 unviable, 9 timeout |
| humantime | `76c8929b4cc2` | 539 / 18 | 17 unviable |
| strsim-rs | `dacc84c0dc61` | 272 / 79 | 3 unviable, 4 timeout |
| rust-hex | `e25a8701f7f6` | 61 / 20 | 4 unviable, 1 timeout |

The labels come from one full cargo-mutants 27.1.0 run per crate with no extra
arguments (the runs behind the mutation spot check in
`benchmarks/mutation_spot_check/`). No upstream source is vendored: a label
records a mutant's name, genre, file, line, column, containing function lines
and outcome.

## How a pick is judged

Labels belong to mutant locations, not to ripr seams, so any seam pilot ranks
is judged, including seams it never ranked before. The judge is the mutation
spot check's (`xtask/src/reports/mutation_spot_check/pilot.rs`), in order:

1. `seam`: viable operator mutants inside the seam's predicate or return
   expression.
2. `line`: with none, viable non-whole-body mutants on the seam's line.
3. `owner`: with none, whole-body mutants of the innermost containing function.

Any missed mutant confirms the pick; all caught refutes it; no mutant at any
tier leaves it unscored. The receipt counts each tier separately because
`line` and `owner` are coarser than `seam`.

## What is reported

For the top 5 and top 10 picks of each crate, pooled and per crate:

- **precision**: confirmed over confirmed plus refuted;
- **scored share**: picks a label could judge. It sits beside precision
  because precision can rise by making picks unjudgeable;
- **distinct functions**: the owning functions among the picks.

The `ranking` scoreboard gates the pooled top-5 and top-10 precision, top-10
scored share, top-10 distinct-function share and the top-10 pick count (so a
ranking that drops picks cannot raise the rates). It also gates the confirmed
and refuted counts at each cut, because turning an unscored pick into a
refuted one can leave the rates inside their floors while adding a wrong
recommendation. The floors sit just under one pick, because nothing here
varies between runs. Every row is tagged with the corpus version as its
repository, so a baseline from another corpus version is listed as
uncompared rather than compared across different crates. A run that cannot fetch or
score a crate, or that `--repo` narrowed to a subset, reports every row
incomplete, which the gate treats as lost completion rather than a comparable
rate.

## Limits

The numbers hold for these five revisions, cargo-mutants 27.1.0 and the
judge's join rule. A missed mutant shows the tests did not notice that one
change; it is not a suite adequacy measure. Five small libraries are not every
codebase: the mutation spot check and the shared Rust corpus cover more.

## Refreshing

A new pin is a new `corpus_version`, never an in-place edit:

Mutate a checkout outside this workspace (cargo-mutants builds the crate, and
Cargo would otherwise see this repository's workspace above it):

```bash
cargo xtask pilot-ranking fetch --allow-network --root ../pilot-ranking --repo semver
cargo mutants --dir ../pilot-ranking/semver --output ../pilot-ranking/semver-run --jobs 2
cargo xtask pilot-ranking label --repo semver=../pilot-ranking/semver \
  --mutants-out semver=../pilot-ranking/semver-run/mutants.out
```

Label from a full run: options that change which mutants cargo-mutants tries
(`--re`, `--file`, `--package`, `--shard`) leave real mutants out of the key,
so a pick there would score as unscored instead of refuted. Options that only
change scheduling, such as `--jobs`, are fine. `outcomes.json` does not
record selection arguments, so `label` runs an unfiltered
`cargo mutants --list --json` on the pinned checkout (cargo-mutants must be
installed) and refuses a `mutants.out` whose mutants differ from that list.
`check` also refuses a label file whose `cargo_mutants_args` is not empty.
`label` refuses a `mutants.out` whose mutant diffs do not match the
pinned checkout, a mutant without an outcome, and an outcome without a mutant.
It prints the caught and missed counts; record them as the repository's
`labeled` entry in `manifest.json`, which `check` holds each label file to, so
a truncated label file cannot pass as the full run. After
an intended ranking improvement, refresh the baseline from the new receipt:

```bash
cargo xtask dx-scoreboard --boards ranking --ingest target/ripr/reports/pilot-ranking.json
cp target/ripr/reports/dx-scoreboard.json metrics/dx-scoreboard/pilot-ranking-baseline.json
```
