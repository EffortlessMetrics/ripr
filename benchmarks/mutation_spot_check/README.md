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

Large repositories take hours to mutate in full. `--mutants-arg <name>=<arg>`
passes one argument to that repository's cargo-mutants run, and can be
repeated. Use it to narrow the run (`--file`, `--re`) or to choose packages
(`--package`, `--workspace`, which can widen it). The receipt records the
arguments, so the run can be reproduced, and the scoreboard marks rates from
runs with extra arguments.
Each value is one argument (`--re=decode`, not `--re decode`). Options the
harness owns or that would stop it writing outcomes, such as `--output`,
`--jobs`, `--timeout-multiplier`, `--in-place`, `--manifest-path` or
`--list-files`, are refused.

Package selection follows cargo-mutants. In a workspace whose root is a
package, it mutates only that package unless given `--workspace` or
`--package`. In a virtual workspace it mutates the `default-members`, or every
package when there are none. Zola's root is a package, so reaching
`components/site` needs `--workspace`:

```bash
cargo xtask mutation-spot-check --repo zola=../corpus/zola --run-mutants \
  --mutants-arg zola=--workspace --mutants-arg zola=--file=components/site/src/queue.rs
```

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
