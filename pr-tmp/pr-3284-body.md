# Production delta

Closes #3284 — S3 of #3213, governed by **RIPR-SPEC-0154** (new, `proposed`): production-gap accounting is invariant across equivalent harness assertion forms.

- **Terminal Err-return guards credit as their assertion twins** — `if <lhs> != <rhs> { return Err(...) }` in a test body constructs `assert!(<lhs> == <rhs>)` (and `==`→`!=`, `!expr`→`assert!(expr)`) and runs it through the **existing single classifier**, so the guard carries exactly the kind and strength its assert form would. No parallel strength table. Wired into both the lexical scanner and the ra parser path so both adapters agree.
- **Three fail-closed gates** (two from adversarial review): the Err return must be the body's first statement (`{returnErr(` — commented-out or string-embedded `return Err(` never credits); compound conditions with a top-level `&&`/`||` stay unrecognized (their correct negation is not a single assert twin); opaque conditions (`matches!`, method calls, ordering) produce nothing.
- **The lexical path recognizes guards without consuming the block** (review finding B1 — the first draft joined the whole if-body and silently swallowed real assertions inside it, a main regression on the fallback path): recognition uses the condition line plus a one-line peek.
- **Repo-mode probe seeding filters evidence-role owners** — cfg(test)-module helpers inside production files were seeding repo findings through a probe path that never checked the owner's role (reproduced failing on main; production shapes still seed).
- **`#[cfg(all(test, ...))]` module members carry the evidence role** like plain `#[cfg(test)]`; `not(test)`/`any(test,..)` stay production (pinned both ways).
- **Cache generations bump** (FILE_FACT 0.3→0.4, CACHE 0.7→0.8, SHARDED, COMPACT with complete version-history comments) — TestFact assertions and repo seam content changed at package 0.10.0. The test-gated COUNT cache stays 0.2 (its envelope key embeds the outer version, so warm entries miss transitively).

# Evidence delta

- **`assertion_form_parity_assert_msg` / `_err_guard` fixture pair**: same owner, boundary value, and observable under the two equivalent forms — identical classification distribution, identical oracle kind/strength counts, identical evidence paths (verified distribution-equal; the expected outputs differ only in names, identity hashes, and the inherent 3-line length delta).
- Reproductions verified failing on main: the guard oracle absence (unit level) and the repo-mode cfg(test) probe leak (integration level, real index).
- In-crate pins: twin equality (kind + strength), opaque/commented/compound rejection, parser-path crediting through TestFacts, repo leak + production control, cfg(all)/cfg(not) roles.
- Removal evidence: before the parser-path wiring, the fixture pair flipped classification (`reachable_unrevealed` vs `weakly_exposed`) — observed directly twice (pre-wiring and via a stale-cache replay of guard-blind facts, which is what motivated the cache bump).
- Adversarial review (separate agent, eight challenge areas): both blocking findings (the block-swallow regression, the substring false-credit) repaired with gates + the peek-based rewrite; compound-condition rejection (its N1) also adopted; version-history comments completed; its N3 residual (test-second `cfg(all(...))` spellings) documented as out of scope.
- 4316 lib tests, workspace clippy clean, `goldens check` (zero drift — no existing fixture has an Err-guard in a test body, verified by grep), `fixtures`, `dogfood`, `precommit`, full policy battery green.

# Acceptance matrix (#3284)

| Acceptance | Status |
| --- | --- |
| Equivalent forms → same inventory and gap count | ✅ parity fixture pair, distribution-identical |
| Result/Ok/?/map_err/harness `.contains` create no obligations | ✅ evidence-role filtering + repo probe filter |
| Adding a focused test can reduce gaps; plumbing cannot increase them | ✅ harness shapes excluded from production inventory |
| Broad vs exact remain different | ✅ classifier unchanged for recognized forms |
| Wrong-target / unrelated-strong stay non-crediting | ✅ existing negative-control fixtures green |
| Production-source `if`/`?`/`map_err` stay ordinary subjects | ✅ role filter keyed on owner role, not syntax |
| Opaque helpers stay limited | ✅ gates pin unrecognized |
| Removal experiments fail | ✅ fixture flip observed pre-wiring |

# Non-claims

- No `match`-arm Err returns, `assert_cmd` chains, or stdout `.contains` integration forms (later corpus-table slices).
- No change to recognized-form classification strengths.
- No cross-surface role projection (#3285).
- `cfg(all(feature, test))` (test-second) spellings remain production — residual documented.

Candidate head: `455a0f22` (base `origin/main` @ `b5416cd6`).
