# Learning: Python parser substrate ecosystem gap

Date: 2026-05-12
Campaign: 27 (Language Adapter Preview)
Related ADR: [ADR 0009](../adr/0009-python-parser-substrate.md)

## Context

Campaign 27 introduces a `LanguageAdapter` seam in `crates/ripr` and ships
preview adapters for TypeScript and Python alongside the Rust reference
adapter. The Python adapter is pinned by RIPR-SPEC-0028 (syntax-first owner,
test, assertion, related-test, and probe extraction from `*.py` files, with
explicit `StaticLimitKind` for things syntax-first analysis cannot classify).

The first version of [ADR 0009](../adr/0009-python-parser-substrate.md)
picked `ruff_python_parser`: Astral's static-tooling-focused parser, the
exact technical fit for syntax-first fact extraction. Implementation tried to
land that pick across three scoped PRs and surfaced three substantive
substrate surprises in sequence. This learning records the surprises and the
validation step that would have caught them before the ADR was written.

## What we learned

### 1. `ruff_python_parser` is not on crates.io

`ruff_python_parser` is a workspace crate inside
[astral-sh/ruff](https://github.com/astral-sh/ruff). The Ruff workspace
deliberately marks the crate `publish = false`, so it cannot be added as a
normal Cargo dependency. The only consumption paths are:

- **git dependency**: rejected by `policy/dependency_allowlist.txt`
  (production analyzer crates must come from crates.io for reproducibility
  and supply-chain reasons);
- **third-party vendored fork** (`littrs-ruff-python-parser`,
  `rustpython-ruff_python_parser`): rejected because neither is maintained by
  Astral, which ties the adapter to a single contributor's pace and
  willingness to track upstream Ruff changes — exactly the supply-chain risk
  a canonical Astral publication would solve.

Caught when the scaffold PR tried to add the Cargo dep. The original ADR
already named `rustpython-parser` as the documented natural fallback under
its Revisit Criteria, so the correction was a contraction onto that
fallback rather than a fresh search. PRs: #794 (initial ADR), #801 (ADR
correction onto `rustpython-parser`), #804 (scaffold).

### 2. `rustpython-parser`'s default features pull LGPL-3.0-only

`rustpython-parser`'s default feature set enables `malachite-bigint`
(LGPL-3.0-only) for Python arbitrary-precision integer literals. `cargo deny
check licenses` failed on the first dependency-add attempt because the
workspace license policy does not include LGPL-3.0-only.

Mitigation in the Cargo manifest:

```toml
rustpython-parser = { version = "...", default-features = false, features = ["location", "num-bigint"] }
```

- `num-bigint` is MIT/Apache-2.0 and is a drop-in replacement for the
  `malachite-bigint` default;
- `location` keeps span data on the AST, which the adapter needs to attach
  static facts to source ranges.

License surface is part of the substrate decision, not a separate
post-decision audit step.

### 3. `rustpython-parser` transitively depends on the unmaintained `rust-unic` family

`rustpython-parser` pulls in the `rust-unic` Unicode crate family
(`unic-char-range`, `unic-common`, `unic-char-property`, `unic-emoji-char`,
`unic-ucd-ident`). The `rust-unic` project is unmaintained; RustSec issued
six separate advisories against the family in 2025:

- RUSTSEC-2025-0075 (`unic-char-range`)
- RUSTSEC-2025-0080 (`unic-common`)
- RUSTSEC-2025-0081 (`unic-char-property`)
- RUSTSEC-2025-0090 (`unic-emoji-char`)
- RUSTSEC-2025-0098 (`rust-unic` umbrella announcement)
- RUSTSEC-2025-0100 (`unic-ucd-ident`)

Each advisory's Solution line is the same: "No safe upgrade is available."

Mitigation in `deny.toml`: each advisory id is suppressed individually under
`advisories.ignore` with a verbose comment explaining what the crate is, why
the suppression is justified, and the revisit trigger. The crates are not
flagged for any known vulnerability — they ship Unicode tables that `ripr`
never touches directly, and there is no upgrade target available — but the
suppression is recorded per-id so a future audit reviews each line on its
own merit rather than treating "rust-unic is fine" as a blanket carve-out.

The revisit triggers are explicit in
[ADR 0009 § Revisit Criteria](../adr/0009-python-parser-substrate.md):

- `rustpython-parser` migrates off `rust-unic` to `icu_properties` /
  `unicode-ident`, **or**
- Astral publishes `ruff_python_parser` to crates.io under stable
  versioning.

Either event retires this learning and triggers a parser substrate
re-evaluation.

## Structural observation

This is not unique bad luck — it is a structural gap in the Rust ecosystem
for Python static analysis. Compare with the TypeScript side:

| Language | Clean, focused, published, well-maintained parser? |
| --- | --- |
| TypeScript-in-Rust | `oxc_parser` — yes (ADR 0008) |
| Python-in-Rust | no equivalent; `ruff_python_parser` is the technical fit but unpublished |

Every project doing static Python analysis from Rust currently hits the same
three layers of trade-off: parser availability, license features, and the
`rust-unic` transitive advisory cluster. Whichever crate fills this gap — a
focused syntax-only spinout from Ruff's parser, or Astral publishing the
existing one — will be widely adopted. Until then, every consumer of Python
syntax facts in Rust pays the same `rustpython-parser` + cargo-deny
configuration cost.

## What to do next time

Validate a parser pick *before* writing the ADR, in this order:

1. **Crates.io availability.** `cargo search <crate>` and inspect the crate
   page. If the crate name resolves only inside a monorepo or only as a
   third-party re-publication, treat it as unavailable for production
   dependency purposes regardless of technical fit.
2. **Default-feature license surface.** `cargo info <crate> --features`
   (or `cargo info <crate>` on recent toolchains) to see the default feature
   set and the transitive license profile. Default features that pull
   copyleft licenses (LGPL, AGPL, GPL) are a substrate-shape consideration,
   not a config-time afterthought.
3. **Transitive advisory profile.** In a throwaway scratch crate, add the
   dependency and run `cargo deny check` (advisories, licenses, bans). Any
   advisory id that resolves to "no safe upgrade" inside the transitive
   tree is part of the substrate cost, not a separate maintenance task.
4. **Substrate revisit trigger.** Each ADR-level trade-off captured under
   1–3 needs a concrete trigger ("if X publishes Y to crates.io" or "if
   crate migrates off Z") so the trade-off is reviewed when the conditions
   change, not opportunistically.

Doing 1–3 on a throwaway project takes minutes and produces concrete
evidence; doing them inside a real PR series means the ADR is corrected
under merge pressure and the substrate trade-offs surface one PR at a time.

## Revisit triggers

This learning is retired when either of
[ADR 0009 § Revisit Criteria](../adr/0009-python-parser-substrate.md) fires:

- `rustpython-parser` migrates off `rust-unic` to a maintained Unicode
  substrate (`icu_properties`, `unicode-ident`, or equivalent), eliminating
  the six suppressed advisory ids in `deny.toml`;
- Astral publishes `ruff_python_parser` to crates.io under stable
  versioning, restoring the original ADR rationale and likely also
  eliminating the LGPL default-feature and `rust-unic` trade-offs.

At that point the substrate pick should be re-evaluated against the
restored options before any new Python static-fact work is built on top of
the current configuration.

## Cross-references

- [ADR 0009: Python Parser Substrate](../adr/0009-python-parser-substrate.md)
- [`policy/dependency_allowlist.txt`](../../policy/dependency_allowlist.txt)
  (entry for `rustpython-parser`; rationale captured inline)
- [`deny.toml`](../../deny.toml) (`advisories.ignore` block with per-id
  comments for the six `rust-unic` advisories)
- Issue #770 (Campaign 27 parser-substrate issue)
- PR #794 (initial ADR 0009; picked `ruff_python_parser`)
- PR #801 (ADR 0009 correction onto `rustpython-parser`)
- PR #804 (Python adapter scaffold)
