# perl_cpan_alpha — two-binary proof fixture (Campaign 31 item 3)

This fixture is the convergence proof for the Perl repair-routing usable alpha.
It is a CPAN-style Perl project (`lib/Pricing.pm` + `t/pricing.t` +
`Makefile.PL`) with three diff variants that prove the three honest outcomes
the consumer (`ripr check --perl-facts`) must produce.

## Layout

```
input/
  Makefile.PL                 CPAN-style module marker (so a real perllsp can index the project)
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

## The two-binary proof (real perllsp → ripr)

The real proof is `tests/perl_two_binary_harness.rs`, which runs:

```text
perllsp ripr-facts --schema ripr-perl-facts-v1 --root <input> --base ... --head ... --fact-classes ... --diff <variant> --out <tmp>
ripr check --perl-facts <tmp> --json
```

and asserts the three outcomes against REAL producer output. This harness is
**gated on `perllsp`/`perl-lsp` being on PATH** and skips cleanly (with a
diagnostic) when absent, because the producer is owned by **perl-lsp-swarm
(Phase B)**; ripr-swarm does not build or vendor it.

## The regression packets (producer-INDEPENDENT baseline)

`expected/regression-packets/*.json` are committed, hand-authored
`ripr-perl-facts-v1` packets — one per outcome — that mirror what a real
perllsp run would emit. They are **regression fixtures, NOT producer proof**.
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

## Open producer-side questions (for perl-lsp-swarm Phase B)

1. **Binary name.** ripr-swarm's `app/check.rs` spawn path uses `perllsp`
   (no hyphen); `analysis/language/perl/mod.rs` and SPEC-0064 line 103 use
   `perl-lsp` (hyphenated). The harness tries both. perl-lsp-swarm should
   declare the canonical name.
2. **CLI arg surface.** SPEC-0064 line 103 specifies
   `perl-lsp ripr-facts --schema --root --base --head --fact-classes --out`
   (with `--diff`). The live `invoke_perl_lsp_producer` (app/check.rs) uses a
   non-spec surface (`--ripr-facts`/`--ripr-schema`/`--ripr-root`/`--ripr-out`)
   and does not pass `--base`/`--head`/`--diff`. Reconciling
   `invoke_perl_lsp_producer` to the SPEC is item 4 (D14 managed-producer
   hardening); the harness uses the SPEC-canonical surface.

## Must Not

- Emit a public repair packet for the dynamic-dispatch case.
- Emit a public repair packet without the shared validator passing.
- Credit a weak oracle as `exposed`.
