# Production delta

Closes #3283 — S2 of #3213, governed by **RIPR-SPEC-0153** (new, `proposed`): one producer-owned typed source role replaces the split path predicates for Rust files.

- **`analysis/workspace/source_role.rs`** — `SourceRole` (production subject, test/bench/example/fixture evidence, production-like opt-in, reserved unknown) with priority-ordered derivation: the `[analysis] production_like_targets` opt-in → declared Cargo targets (`[[test]]`/`[[bench]]` explicit `path` entries) → package layout → production default. **The pre-#3283 repo production contract carries over exactly** (`xtask/`, files without a `src` component, `tests.rs` stems stay non-production — pinned by a contract-carryover test), with one declared divergence: nested-src layouts under `examples/`/`benches/` are not Cargo-discoverable targets and stay production (diff behavior since before #3283).
- **`analysis/workspace/cargo_targets.rs`** — manifest enumeration anchored to the workspace root; explicit target paths confirm evidence outside the default layouts; malformed manifests yield nothing (fail-closed toward production); `tests/` keeps the legacy any-segment rule; `benches/`/`examples/` classify only in Cargo-autodiscovery shapes (`<dir>/<name>.rs`, `<dir>/<name>/main.rs`).
- **Wiring** — diff probe seeding and the repo seam-inventory production set (rust.rs + three seam_inventory sites) route through `classify_with`; changed Cargo benches no longer seed obligations for harness plumbing (the reproduced gap: `criterion_group!`/`criterion_main!` probes on main). Filename conventions never classify alone.
- **Config** — the opt-in parses as workspace-relative paths (absolute/backslash values fail closed), threads through `AnalysisOptions`, joins the check-artifact identity as FindingAffecting (`CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION` 1→2) and the repo-exposure consumed-config list (3→4). No cache-generation bump needed: the workspace key already hashes the `ripr.toml` text and every manifest (verified by review), and `FunctionFact.is_test` semantics are untouched.

# Evidence delta

- **Reproduced the gap first**: a changed `benches/exposure.rs` seeded five production probes on main; zero after the slice.
- **`benches_harness_evidence` corpus fixture** (the review's blocking test-oracle finding): four bench findings on main's binary, zero on the candidate's — the golden net now has sensitivity to the new dimension; production owner keeps ordinary classification.
- Discriminating proofs: disabling the declared-target branch makes the confirmation fixture fail (with a probeable harness helper — the first version was vacuous and strengthened after scratch-verification); the opt-in restore test pins target-only scope.
- Unit pins: layout + autodiscovery shapes + override priority + windows normalization + seeding/evidence partition; manifest extraction (explicit paths, no-path entries, malformed TOML, autodiscovery flags); config parse/identity/absolute-reject.
- Adversarial review (separate agent, six challenge areas): its two blocking findings repaired — the repo production-set widening (now pinned non-widening, empirically re-verified: zero xtask seams in a scratch repo run) and the corpus insensitivity (fixture added); the LSP divergence it found is now explicitly documented in-code as #3285 work, with the two LSP scope tests moved to a path that still demonstrates the partition drop.
- 4309 lib tests, workspace clippy clean, `goldens check`, `fixtures`, `dogfood`, `precommit`, full policy battery green.

# Acceptance matrix (#3283)

| Acceptance | Status |
| --- | --- |
| tests/** helpers/tests evidence role, no production findings | ✅ existing pins + suite |
| Cargo benches evidence by default, no harness obligations | ✅ regression test + corpus fixture |
| Inline #[cfg(test)] from #3273 remains green | ✅ suite |
| src/test_helper.rs stays production | ✅ pin |
| Confirmed *_test.rs targets evidence; unconfirmed not excluded | ✅ declared-target fixture (discriminating) |
| Registered fixtures/receipts stay evidence | ✅ layout pins |
| Opt-in restores production for the selected target only | ✅ pin |
| Ambiguous/generated typed unknown/limited | ✅ reserved variant, test-gated |
| Related-test/oracle evidence unchanged or strengthened | ✅ suite + declared-target evidence pin |

# Non-claims

- No assertion-form parity (#3284); no cross-surface role projection (#3285 — the LSP partition convergence is documented there).
- No generated-file/custom-harness producer (`UnknownRole` reserved).
- No cache generation bump (justification in the spec, verified by review).

Candidate head: `a89147a8` (base `origin/main` @ `6a15d412`).
