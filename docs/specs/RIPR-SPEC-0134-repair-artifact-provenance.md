# RIPR-SPEC-0134: Repair Artifact Provenance

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked ADRs:

- [ADR 0020](../adr/0020-repair-artifacts-carry-producer-identity.md)

Linked issues:

- [#1977](https://github.com/EffortlessMetrics/ripr-swarm/issues/1977) - bind
  analysis and verification artifacts to repository, revision, schema, and
  content identities.
- [#1941](https://github.com/EffortlessMetrics/ripr-swarm/issues/1941) - agent
  verify and receipt trust boundary.

Support-tier impact:

- No support-tier promotion. This contract makes static artifact currentness
  and integrity explicit; it does not claim runtime verification, correctness,
  mutation results, or merge authority.
- Claim boundaries remain governed by the [support tiers](../status/SUPPORT_TIERS.md).

## Problem

Saved snapshots previously reached `agent verify` when they merely contained a
parseable `seams` or `findings` array. That allowed fabricated or stale input to
be treated as movement evidence.

## Behavior

`ripr check --format repo-exposure-json` emits an additive top-level `artifact`
identity envelope. The envelope has `kind = "repo_exposure"`, identity schema
`"1"`, producer/tool version, analyzed root, Git HEAD when available, format,
mode, base revision, worktree state, bounded analysis-input identity,
snapshot identity, creation command/profile, and `content_sha256`.

The analysis-input identity is portable semantic/configuration identity
(#2823): an explicitly versioned `input:v3:fnv1a64:<16 lowercase hex>`
covering the
identity version, mode, profile (bound to mode by this producer), base, named
workspace inputs (manifest and lockfile content identities), the
repo-exposure producer-consumed configuration boundary (exactly the three
oracle-strength fields — the Rust-only seam inventory consumes nothing else
from `ripr.toml`), and analyzer version — never the
concrete checkout root or a host-specific path spelling. Equivalent checkouts
of the same commit under different roots share one input identity; the
concrete root remains separate envelope evidence (`repository.root`) that the
verifier compares with exact canonical-path equality. Only the current
`input:v3:` identity version with the exact digest shape validates as current
evidence; any other version is rejected as an unsupported input identity
version, any malformed digest shape as a malformed input identity digest, and
a previous-version migration boundary stays deferred until a real migration
producer exists. The content commitment uses the `raw_json_placeholder_v1` rule: hash the exact
artifact bytes after replacing the one `content_sha256` value with the fixed
zero digest placeholder. The producer emits the resulting digest in the final
artifact. This rule is stable, bounded-memory, and detects later byte changes.

`ripr agent verify` accepts only repo-exposure artifacts with this envelope. It
rejects missing or unsupported identity, root mismatch, invalid HEAD,
malformed or duplicate commitments, content mismatch, and incomparable base or
analysis-input identities before movement calculation. It adds
`artifact_currentness` to its
advisory output with one of:

- `current`;
- `historical_noncurrent`;
- `historical_before_current_after`;
- `current_before_historical_after`;
- `dirty_before`;
- `dirty_after`;
- `dirty_both`.

The pair token states what each side of the pair is (#3027): a dirty side is
named (`dirty_before`, `dirty_after`, or `dirty_both`) rather than every mixed
pair collapsing into one dirty label, and the clean expected transaction —
the repository moved past the before artifact while the after artifact is
current — is `historical_before_current_after`. A fully current pair fails
the movement gate and a current-before/historical-after pair fails the
lineage gate, so `current` and `current_before_historical_after` close the
vocabulary without being reachable verify outcomes today.

When Git identity is unavailable, the producer discloses `unavailable` in the
artifact and the verifier rejects it as unsuitable evidence.

`ripr agent receipt` revalidates the referenced before and after artifacts and
recomputes the canonical agent-verify movement before rendering a receipt. The
supplied verify JSON must exactly match that recomputed output, including its
currentness and movement fields. Hand-authored or altered movement evidence is
rejected before receipt issuance.

Verify schema `0.2` (#2922) binds the result to the exact artifact bytes it
compared: canonical output carries `inputs.before_content_sha256` and
`inputs.after_content_sha256`, the validated `artifact.content_sha256`
commitments of the pair. Schema `0.3` (#3027) keeps that binding and corrects
the pair-level `artifact_currentness` value domain, a breaking family change
for consumers dispatching on the old catch-all `dirty_worktree` pair token.
The receipt's canonical recomputation therefore
rejects a verify result replayed against different or mutated artifact bytes —
including mutations invisible to the movement render — with one typed
`[not_canonical]` reason, and rejects any verify JSON whose schema version is
not the canonical one with `[unsupported_schema]` before any artifact work. A
0.2 document that is canonical in every other way is rejected the same way;
there is no migration path, only a fresh verify. A
verify result produced while the pair was current is stale after repository
movement and is rejected on the same canonical comparison; a fresh verify
after movement succeeds but discloses `historical_noncurrent`.

## Non-claims

- The envelope is not a digital signature or proof against a compromised RIPR
  process.
- `agent verify` still compares static before/after evidence only; it does not
  execute tests or runtime mutation testing.
- The schema `0.2`/`0.3` content-commitment binding (#2922, #3027) is
  byte-level replay
  defense, not a signature: it detects replayed, stale, or mutated evidence,
  but command execution binding, configuration binding, and receipt signatures
  remain follow-up slices under #1941.

## Non-Goals

- No runtime test or mutation execution.
- No receipt signature, remote attestation, or merge-policy change. The #2922
  replay defense is a content-commitment byte binding enforced by the existing
  canonical comparison, not a new trust authority.
- No configuration or command-execution binding beyond the producer metadata
  recorded here.

## Required Evidence

- Producer output tests cover identity and streaming output.
- CLI smoke tests cover a valid bound pair, a historical comparable pair,
  mixed pair-currentness disclosure (historical-before/current-after,
  dirty-before, dirty-after, and dirty-both, #3027), tampered bytes,
  incomparable input identities,
  unsupported schema, malformed typed seam, plausible uncommitted JSON,
  fabricated verify JSON, altered verify movement, incomparable base revision,
  incomparable analysis inputs, verify replay against mutated artifact bytes,
  tampered pair-binding fields, unsupported verify schema versions, stale
  verify after repository movement, a receipt target absent from both states,
  and an unmoved retained target whose receipt stays `unchanged` while a
  different seam moves.
- The editor repair-loop fixture consumes bound artifacts and records explicit
  currentness.
- The integrated installed-candidate negative corpus (`cargo xtask
  release-negative-corpus --version <version>`, #2824) runs the packaged
  candidate through the authentic readiness chain in a controlled external
  fixture and injects exactly one mutation per case — artifact identity,
  revision, snapshot, and commitment mutations; pair comparability, lineage,
  and movement mutations; verify schema, replay, staleness, and tamper
  mutations; and receipt issuance failures. Each case asserts the process
  exit status first and the closed reason token second, restores the original
  bytes/state byte-exactly (verified by digest), and reruns the original
  command to a passing control before a per-case failure receipt is retained
  under `target/ripr/release-negative-corpus/`. Output-contract breaches
  (stdout rendered on a rejection, a rejected issuance creating or updating
  its out file, a stale prior receipt digest drifting, a retained mutation
  source missing) are recorded as first-class receipt `violations`, and any
  recorded violation fails the case even when every outcome token matched.
  The report summary discloses matrix completeness from the case registry:
  a slice that lands only some case families reports
  `run_status: "families_deferred"` with explicit `covered_families` /
  `deferred_families` lists; the full matrix reports `run_status:
  "complete"`. Deferred negatives without a
  real producer on main (migration claims of fresh production, binary/artifact
  inventory disagreement) are recorded as explicit `not_applicable`
  dispositions, never fabricated rejections.

## Acceptance Examples

### Historical-before/current-after bound pair

A before snapshot bound to a superseded revision and an after snapshot bound to
the current HEAD — the expected clean before/after transaction — produce
advisory movement with
`artifact_currentness = "historical_before_current_after"` (#3027).

### Tampered or fabricated input

Changing any byte after emission, omitting the `artifact` envelope, or supplying
an unsupported schema fails before movement calculation.

## Test Mapping

- `crates/ripr/src/agent/artifact.rs` tests the fixed commitment protocol and
  duplicate-field rejection, plus the closed pair-currentness vocabulary
  (`pair_currentness_label`, #3027) and the portable input-identity contract (#2823):
  identity portability across equivalent checkout roots, concrete-root
  rejection at an equivalent clone, revision-only snapshot movement, semantic
  input drift (mode, base, config, manifest, lockfile), the scoped
  producer-consumed config boundary (unconsumed typescript/perl/languages
  settings stay comparable), rerun byte-stability, the root-bound v1 removal
  experiment, and previous-version plus malformed-digest identity rejection.
- `crates/ripr/src/app/agent_receipt.rs` tests the fail-closed verify schema
  gate (`[unsupported_schema]`) ahead of any artifact IO.
- `crates/ripr/tests/cli_smoke.rs` tests valid, tampered, fabricated, and
  editor-loop cases, plus receipt rejection of fabricated and altered verify
  output, incomparable base revisions, incomparable analysis inputs,
  byte-different re-renderings of canonical verify output, verify replay
  against mutated artifact bytes, tampered pair-binding digests and status,
  older/newer verify schema versions, stale verify replay after repository
  movement, an absent receipt target, and the unmoved-retained-target
  projection honesty case.
- `xtask/src/reports/release_negative.rs` (#2824) is the integrated
  installed-candidate negative corpus: it shares the release-readiness
  package/install and authentic-journey helpers (the installed binary stays
  the only validator), and its unit tests pin the mutation/receipt machinery
  — recommit binding and stale-commitment detection, unique-anchor
  replacement, hex-identity shifting, exit-status-before-token rejection
  evaluation, receipt finalization, the required case matrix, the deferred
  `not_applicable` dispositions, fail-closed fixture copying, and the
  JSON/Markdown report shape.

## Implementation Mapping

- `crates/ripr/src/agent/artifact.rs` owns identity and commitment validation.
- `crates/ripr/src/output/repo_exposure.rs` emits the bounded two-pass artifact.
- `crates/ripr/src/cli/commands.rs` validates both inputs before movement.
- `crates/ripr/src/app/agent_receipt.rs` validates inputs and recomputes
  agent-verify output before receipt issuance.
- `crates/ripr/src/output/outcome/render_json.rs` discloses currentness and
  renders the artifact content-commitment binding (`AgentVerifyArtifactBinding`
  in `crates/ripr/src/output/outcome/mod.rs`).
- `xtask/src/reports/release_negative.rs` orchestrates the #2824 negative
  corpus (fixture cloning, one-mutation injection, execution, retention,
  byte-exact restoration, reporting); it owns no validation authority itself
  and reuses the `xtask/src/reports/release.rs` candidate-install and
  authentic-chain helpers.

## Metrics

- `repair_artifact_provenance_status_accepted` records that this first
  provenance slice is available.
- Future slices under #1941 must add execution, configuration, and receipt
  currentness metrics without reusing this field as runtime proof.
For portable workspace identities, CRLF is normalized to LF before hashing;
standalone CR bytes are preserved so invalid input cannot collide with valid LF
input. Changing this normalization is an identity-algorithm change and requires
a new identity version. The prior `input:v2:` shape is unsupported.
