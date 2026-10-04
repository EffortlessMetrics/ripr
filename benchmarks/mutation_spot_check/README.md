# Mutation spot check

`cargo xtask mutation-spot-check` checks static grip verdicts against real
cargo-mutants outcomes on real repositories. For each checkout it writes a
`repo-exposure-json` snapshot, takes a cargo-mutants `mutants.out` directory
(supplied with `--mutants-out`, or produced with `--run-mutants`), and joins
the two through `ripr calibrate cargo-mutants`, so the join has one owner.
cargo-mutants records no source revision, so before scoring the harness checks
that every mutant diff's original lines still match the checkout. If any
differ, or a mutant has no diff (cargo-mutants before 27), it refuses the
directory and says why.

There is no committed corpus. The harness takes checkouts, records each
checkout's `HEAD`, and reports against those revisions. The first run used:

| Repo | URL | Revision |
| --- | --- | --- |
| rust-hex | https://github.com/KokaKiwi/rust-hex | `e25a8701f7f6b5ec4acedffcb4a45cfb69cc34eb` |
| semver | https://github.com/dtolnay/semver | `280ebcb6edac3aa4cdc545dbff8a26c5ac4861fe` |
| strsim-rs | https://github.com/rapidfuzz/strsim-rs | `dacc84c0dc61eff0ee0ff66962bcf2e17018ad26` |
| bytesize | https://github.com/bytesize-rs/bytesize | `66a3715e33369e99cb428bcd5aee32291f8e0a94` |
| humantime | https://github.com/tailhook/humantime | `76c8929b4cc286f675322475a8e1841f35bafc57` |

```bash
cargo install cargo-mutants --locked
git clone https://github.com/dtolnay/semver ../corpus/semver
git -C ../corpus/semver checkout 280ebcb6edac3aa4cdc545dbff8a26c5ac4861fe
cargo xtask mutation-spot-check --repo semver=../corpus/semver --run-mutants --jobs 3
```

The receipt is `target/ripr/reports/mutation-spot-check.{json,md}`.

## What is scored

The calibration join is file plus line, so many joins pair a seam with a
mutant that changes something else on that line. The harness scores only
`seam_precise` joins: a cargo-mutants `BinaryOperator` or `UnaryOperator`
mutant whose original operator appears in the expression of a
`predicate_boundary` or `return_value` seam. `FnValue` mutants replace a whole
function body and other same-line joins test something other than the seam;
both are counted by grip class but never scored.

| Grip class | Claim | Caught mutant | Missed mutant |
| --- | --- | --- | --- |
| `strongly_gripped` | a discriminator exists | agree | overclaim |
| `ungripped`, `reachable_unrevealed` | no discriminator exists | false gap | agree |
| `weakly_gripped`, unknown classes | no claim one mutant settles | unscored | unscored |

Timeouts and unviable mutants are never scored. Claims are limited to the
recorded revisions, the cargo-mutants version in `outcomes.json`, and this
join rule. The report is advisory and is not a suite adequacy measure.
