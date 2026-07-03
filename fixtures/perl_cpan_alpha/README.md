# perl_cpan_alpha — two-binary proof fixture (Campaign 31 item 3)

This fixture is the convergence proof for the Perl repair-routing usable alpha.
It is a CPAN-style Perl project (`lib/Pricing.pm` + `t/pricing.t` +
`Makefile.PL`) with three diff variants that prove the three honest outcomes
the consumer (`ripr check --perl-facts`) must produce.

## Layout

```
input/
  Makefile.PL                 CPAN-style module marker (so a real Perl facts exporter can index the project)
  lib/Pricing.pm              package with calculate_discount (>= 100 boundary) + dynamic_method
  t/pricing.t                 Test::More: ok() (weak) + is() (exact) oracles
  diff.patch                  boundary change: >= 100 -> > 100      (outcomes 1 & 2)
  boundary_change.diff        same boundary change                  (outcomes 1 & 2)
  dynamic_dispatch.diff       adds my $method = ...; shift->$method() (outcome 3)
expected/
  check.json                  golden check output (currently zeros; see note)
  human.txt                   golden human output
  CHANGELOG.md                bless reason
  regression-packets/         PRODUCER-INDEPENDENT consumer baseline (see below)
    actionable-weak-ok.json
    already-observed-exact-is.json
    dynamic-limited.json
```

## The two-binary proof (real Perl facts exporter → ripr)

The real proof is `tests/perl_two_binary_harness.rs`, which runs:

```text
perl-ripr-facts --schema ripr-perl-facts-v1 --root <input> --base ... --head ... --fact-classes ... --diff <variant> --out <tmp>
ripr check --perl-facts <tmp> --json
```

and asserts the three outcomes against REAL producer output. This harness is
**gated on a Perl facts exporter being on PATH** and skips cleanly (with a
diagnostic) when absent, because the producer is owned by the
`perl-ripr-facts` crate in perl-lsp-swarm (post-#3294); ripr-swarm does not
build or vendor it. The exporter must be a thin batch CLI over
parser/workspace/semantic-facts crates — NOT the LSP server.

## The regression packets (producer-INDEPENDENT baseline)

`expected/regression-packets/*.json` are committed, hand-authored
`ripr-perl-facts-v1` packets — one per outcome — that mirror what a real
Perl facts exporter run would emit. They are **regression fixtures, NOT producer proof**.
They carry:

- real top-level `packet_fingerprint` (recomputed over identity-bearing facts);
- real inner `digest` values for `lib/Pricing.pm` and `t/pricing.t` (the first
  committed packets with real inner digests — they pass item 2's on-disk
  freshness check when consumed with the real fixture root).

They are consumed by the lib tests in
`crates/ripr/src/analysis/language/perl/tests.rs` (`cpan_alpha_*`) to prove the
consumer pipeline is intact independent of the producer.

## Outcomes

| # | Outcome | Diff | Oracle | Honest classification |
|---|---|---|---|---|
| 1 | Actionable | `diff.patch` | weak `ok()` | `ReachableUnrevealed` — candidate for a bounded test-only repair (NOT Exposed) |
| 2 | Already-observed | `boundary_change.diff` | exact `is()` aligned to changed sink | `Exposed`, no repair gap (H2 sink alignment) |
| 3 | Limited | `dynamic_dispatch.diff` | n/a (dynamic dispatch) | `partial` packet + named `dynamic_dispatch` limitation, no repair finding |

## Architecture note (post perl-lsp-swarm #3294)

The RIPR fact emitter was relocated from the LSP surface into a dedicated
`perl-ripr-facts` batch exporter. ripr-swarm now treats the producer as a
"Perl facts exporter" (not the LSP server). `perllsp`/`perl-lsp` may remain
as compatibility wrappers only if they delegate to the same batch exporter
without starting an LSP session. The canonical producer is `perl-ripr-facts`;
the canonical config is:

```toml
[perl]
producer = "perl-ripr-facts"
executable = "perl-ripr-facts"
```

RIPR never starts an LSP server, speaks JSON-RPC, calls hover/references/
completion providers, depends on editor state, or parses Perl itself.

## Must Not

- Emit a public repair packet for the dynamic-dispatch case.
- Emit a public repair packet without the shared validator passing.
- Credit a weak oracle as `exposed`.
