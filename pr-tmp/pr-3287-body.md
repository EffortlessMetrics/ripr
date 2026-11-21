# Production delta

Closes #3287 — invalidates every cache capable of retaining pre-#3273/#3286 semantic facts, immediately after #3304 as the issue's sequencing requires.

# Bumps (with version-history reasons naming the semantic change at package 0.10.0)

- `FILE_FACT_CACHE_SCHEMA_VERSION` **0.2 → 0.3** — `FunctionFact.is_test` widened to cfg(test)-module membership (#3273) and helper-evidence admission changed (#3286); old per-file facts carry pre-#3273 role booleans.
- `CACHE_SCHEMA_VERSION` **0.6 → 0.7** — full classified seams derive from those facts.
- `SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION` **0.2 → 0.3** — sharded entries cannot bypass the outer generation.
- `COMPACT_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION` **0.3 → 0.4** — compact evidence derives from the same facts.
- `COUNT_CACHE_SCHEMA_VERSION` (test-gated) **0.1 → 0.2** — badge class counts shifted with the role change.

The corpus-fingerprint cache stays at 0.2 deliberately: it stores content-hash mappings only, not facts or classifications — a hit yields a content hash that remains correct. Every bumped layer enforces the generation twice: as a directory component and inside the embedded key identity (`matches_key`).

# Evidence delta

Three generation-transition tests in `seam_cache.rs`:

1. **Previous-generation miss** — an envelope seeded under schema `0.2` with identical package/path/content identity misses the current key (and the `assert_ne!` fails if the bump is reverted); the stale file stays on disk but cannot satisfy current analysis.
2. **Cold-then-warm fidelity** — cold-populated current-generation facts for the #3286 fixture shape warm-load as a real hit preserving the corrected semantics (helper `is_test`, `TestFact` reserved for the actual test).
3. **Relocation + sharded law** — `RIPR_CACHE_DIR` honors the same generation; full/compact/sharded directories are generation-scoped and the sharded path embeds the outer generation.

Also fixed: `seam_inventory`'s test helper now references the count-cache constant instead of a hardcoded `"0.1"` (the bump would otherwise have silently orphaned its own test), and `rerun.rs`'s reuse check flows through the constant (old rerun artifacts now correctly flagged).

# Verification

4294 lib tests, workspace clippy clean, `goldens check` (zero drift), `dogfood`, `fixtures`, `precommit`, and the policy battery (no-panic, static-language, local-context, architecture, workspace-shape) all pass. No source bytes, `cargo clean`, or package bump required for corrected behavior.

# Non-claims

No cache format/codec change; no eviction of old directories (orphaned, gc'd as before); no new cache surfaces.

Candidate head: `12971333` (base `origin/main` @ `07b03916`).
