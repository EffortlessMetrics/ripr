# Audit: `#3198` acceptance against current main

Audit authority: `#3297`. Audited main: `62a535db` full SHA
`62a535db6a2bcc3c095369be86a1a207c03b97e4` (#3321 squash), observed
2026-08-18, repository `EffortlessMetrics/ripr-swarm`. No concurrent PR
touched the judged-panel surfaces during the audit window (open-PR scan
found none referencing `rust-judged-panel`).
Landed authority: #3246 (squash `2125d6ad`), #3251 (squash `93987f2b`),
`#3266`/`#3269` (squashes `15f9c679`, `62c2cdd8`), #3272 (squash `0446c60d`).
Superseded donors: #3201, #3256 (both closed), drafts #3265/#3267
(closed as superseded).

## Procedure receipts (all on `62a535db`, Windows host)

| Command | Result |
| --- | --- |
| `cargo xtask rust-judged-panel check` (run 1) | pass — manifest, 3 subjects, portable packets valid |
| `cargo xtask rust-judged-panel check` (run 2) | pass — identical output |
| `cargo xtask rust-judged-panel replay --out target/ripr/rust-judged-panel` | pass — run `62a535db6a2b-364572-0`, 3 cases, current published |
| `cargo xtask rust-judged-panel packet --host-current target/ripr/rust-judged-panel/current.json` | pass — 3-case set published to `portable/generations/6bcab950…` |
| `cargo xtask rust-judged-panel check` (after packet) | pass |
| `cargo test -p xtask --locked --offline rust_judged_panel -- --nocapture` | 51/51 pass |
| `cargo xtask check-generated-clean` (committed generation state) | pass — subject: committed `portable/current.json` → generation `44455406…` |
| `cargo xtask check-generated-clean` (audit-run generation state) | pass — subject: refreshed `portable/current.json` → generation `6bcab950…`; tree then restored to the committed state and re-verified |
| `cargo clippy -p xtask --all-targets --locked --offline -- -D warnings` | pass |

The audit-run generation was **not** committed: committed generations are
published by their landing PRs, and the audit PR changes no analyzer
behavior. The committed generation (`44455406…`) validates on its own
(`check` green before and after the audit run; `check-generated-clean`
green in both states).

## Classification legend

```text
satisfied_on_main              landed code/test/artifact on current main
defect_or_missing_contract     concrete current-main gap
superseded_by_authority_correction  replaced by the 2026-08-12 normative
                               correction or later authority merges
explicit_non_goal              outside #3198 by its own non-goals
not_established                true but unclaimed; stated boundary
```

## Reconciliation matrix

### Positive and production-path reachability

| #3198 acceptance row | Classification | Owning evidence on `main` |
| --- | --- | --- |
| Replay consumes the canonical manifest through the merged crate-private loader; removing/bypassing it fails | satisfied_on_main | replay route: `rust_judged_panel.rs` replay command loads and validates the manifest through `load_and_validate_at` then materializes each subject via the #3246 authority (`host_run.rs` run loop); check route: `subject.rs` `canonical_check_reaches_subject_authority` (deleting `subjects.json` fails the canonical check); every packet test loads through `load_and_validate_at` |
| All three rows materialize as exact Git repositories with deterministic base/head/tree and byte digests | satisfied_on_main | #3246 `subject.rs`; tests `changed_governed_byte_is_rejected_before_materialization`, `resealed_changed_byte_is_rejected_by_git_identity`, `canonical_subjects_materialize_with_stable_identities_after_relocation` |
| Executes the just-built workspace binary; records digest, version, profile, features, config, argv, input identity | satisfied_on_main (superseded wording: `--ripr-bin` is test-only) | #3251 `host_run.rs` owned fresh `cargo build -p ripr --locked --offline`; packet fields `producer_*`, `binary_sha256`, `argv`, `analyzer_input_identity` verified in every committed packet |
| Raw stdout/stderr bytes + digests + one deterministic bounded packet; rerun byte-stability | satisfied_on_main | host receipts under ignored `target/` (`host_evidence` block); determinism pinned by relocation/wall-clock tests and the packet digest chain |
| `should_gap` retains the exact gap/weak evidence for the missing equality discriminator | satisfied_on_main | committed packet: `weakly_exposed`, `missing: ["Missing discriminator value: amount == threshold"]`, exact recommendation |
| `should_stay_quiet` retains exactly one `exposed`/`no_action` result, not the gap row's output | satisfied_on_main | committed packet: `classification: exposed`, `expected_actionability: no_action`, `missing: []`; the exact observer evidence is governed by the subject contract — `manifest.json` row (anchor `src/lib.rs:2`, owner `discounted_total`, behavior `predicate_boundary`, expression `amount >= threshold`) over `subjects/boundary-exact-equality/source.after.rs` (the `>=` predicate) and `subjects/boundary-exact-equality/tests/pricing.rs` (the aligned exact return assertion supplying the equality input); the packet's finding identity `probe:src_lib.rs:predicate:00a5a375` joins that anchor |
| `should_limit` retains the named limitation, no producer-owned repair target | satisfied_on_main | committed packet: `no_static_path`, `static_limit_kind: rust_macro_wrapped_test_call_unresolved`, `inspect_static_limitation` |
| Packet order and multi-violation diagnostics deterministic | satisfied_on_main | `reports_all_independent_violations_deterministically`; case-id-ordered publication |

### Discriminating negative and alternate proof

| #3198 acceptance row | Classification | Owning evidence on `main` |
| --- | --- | --- |
| Reject stale/mismatched case, anchor, tree, row/diff/binary/config/input identity | satisfied_on_main | `raw_or_typed_identity_tamper_is_rejected`, `stale_or_missing_analyzer_identity_is_rejected_against_materialized_diff`, `current_pointer_must_join_exact_run_index`, `validate_at_rejects_full_digest_chain_reseal`, `rejects_all_authority_family_reseals` |
| Reject zero/multiple joins; no token/position proxy | satisfied_on_main | `anchor_rejects_wrong_file_line_and_nearby_behavior`, `missing_or_duplicate_case_cannot_form_complete_set` |
| Reject raw-digest mismatch and stale packet self-digest | satisfied_on_main | reseal family above; `rejects_resealed_stale_generation_member_paths`; strict nested/outer readback tests |
| Timeout / spawn failure / nonzero / malformed / incomplete / typed-limited → explicit fail-closed disposition | satisfied_on_main | `host_run.rs` typed envelopes (`timed_out`, `nonzero_exit`, `malformed_output`, …); `successful_output_distinguishes_malformed_incomplete_and_limited` |
| Stale PATH `ripr` cannot be selected when `--ripr-bin` names the just-built binary | superseded_by_authority_correction | the 2026-08-12 correction: the production command owns a fresh locked/offline build and executes those exact bytes; arbitrary `--ripr-bin` is a crate-private test port only. Equivalent protection: `changed_built_binary_is_rejected` |
| Scratch relocation / wall-clock change does not alter semantic identity; governed byte change does | satisfied_on_main | relocation test + `changed_governed_byte_is_rejected_before_materialization` |
| Anti-hardcode: runner/packet logic is data-driven | satisfied_on_main | `alternate_valid_seed_proves_validator_is_not_hard_coded` |
| Judgment null, runtime `not_run`, no numerator/denominator/rate/badge/support field | satisfied_on_main | every committed packet: `judgment.disposition: unjudged` (all fields null), `runtime_calibration.status: not_run`, `non_claims` state both boundaries |
| Swapped identical-diff pricing packets rejected | satisfied_on_main | `complete_wrong_subject_substitution_fails_independent_binding` |
| Hostile Git config cannot change subject identity | satisfied_on_main | `hostile_git_environment_cannot_change_subject_identity` |
| Injected failure after case two leaves no authoritative partial run | satisfied_on_main | `failure_after_two_cases_preserves_previous_current`, `recovers_interrupted_staging_before_publication` |

### Normative-correction additions (2026-08-12 comment)

| Requirement | Classification | Evidence |
| --- | --- | --- |
| Independent checked subject descriptor authority; generated hashes cannot validate themselves | satisfied_on_main | #3246 `subjects.json` + descriptor tests |
| Portable semantic packet under `metrics/` distinct from host-bound receipts under `target/` | satisfied_on_main | #3266/#3269/#3272; `portable/generations/*/packets` + `host_evidence` block marked `host_bound_not_committed` |
| Validate-before-current reuse; every retained member validated before advancement | satisfied_on_main | `reuse_validates_every_member_before_current`, `retained_validator_reaches_committed_generation` |
| Canonical portable-root confinement | satisfied_on_main | `rejects_noncanonical_portable_spellings`; host output confinement test |
| Windows current replacement / rollback / stale backup | satisfied_on_main | `replaces_existing_current_on_second_publication`, `failed_current_replacement_preserves_previous_pointer`, `reconciles_stale_backup_before_publication` |
| Coordinated full-authority re-seal rejected | satisfied_on_main | `validate_at_rejects_full_digest_chain_reseal`, `rejects_all_authority_family_reseals` |
| Nonzero cargo-test policy selection | satisfied_on_main | #3272; typed nonzero/timeout/malformed policy envelopes in `host_run.rs` |

### Programme planes that #3198 never owned

These remain open under parent `#3164` (`status/partial`), not under
`#3198`. They are recorded here so no future builder mistakes them for
`#3198` residuals:

| Plane | Classification | Current state |
| --- | --- | --- |
| Independent judgment (reviewed disposition separate from analyzer output) | explicit_non_goal (of `#3198`) | `judgment.*` null in every packet; `non_claims` names it; #3164 PR C/D owns it |
| Runtime / mutation ground truth | explicit_non_goal | `runtime_calibration: not_run`; #3164 later slice |
| Rates / badge / gate / support from three cases | explicit_non_goal | `non_claims`; three synthetic cases are not a denominator |
| Cryptographic / external authenticity | not_established | attestations are **reviewed, unsigned** authority; a coherent re-sealed full-authority chain is rejected by digest, but authenticity beyond review is not claimed and remains out of scope |
| Windows junction/reparse-point confinement | not_established | symlink-escape tests run only `when_supported`; no junction/reparse proof exists. Status: `NOT_ESTABLISHED` — see README boundaries |

## Confidence, uncertainty, and search receipts

- **All-state search receipt**: `gh issue list --state all --search rust-judged-panel` and `gh pr list --state all --search rust-judged-panel` at audit time returned no other open owner of the replay/packet claim beyond the merged authority chain; superseded donors `#3201`/`#3256` and drafts `#3265`/`#3267` are closed (`gh pr list --state all --search rust-judged-panel` receipt: 3357 open — this audit; 3246/3192/3251 merged; 3201 closed). Confidence: high (the landed authority carries its own discriminating tests, all re-run green).
- **Remaining uncertainty**: the portable packets' semantic content was verified for all three cases against the governed subjects; the *host* evidence (raw stdout/stderr) is retained only under ignored `target/` and was inspected for the audit run (`62a535db6a2b-364572-0`), not for every historical generation. The `not_established` rows (external authenticity, Windows junction/reparse) carry their own stated uncertainty above.
- **Verification-method boundary**: this audit ran the canonical commands and the full 51-test authority suite on the audited head; it did not re-derive Git identities by hand or replay on a non-Windows host.

## Audit conclusion

Every #3198 acceptance row is `satisfied_on_main` or
`superseded_by_authority_correction` with an owning test, artifact, or
merge SHA on current main. The null-judgment / `not_run` / no-rate rows
are satisfied as negative constraints. No `defect_or_missing_contract`
row was found. #3198 can close with this matrix; the remaining
programme planes belong to parent #3164 and any next slice must be a
bounded child of that programme, not a reopened #3198.

## Boundary statements (audit-added, no behavior change)

- **Reviewed unsigned authority**: retained attestations establish
  reviewed coherence, not cryptographic authenticity. A coherent
  full-authority rewrite is rejected by the digest chain, and
  authenticity beyond review remains explicitly out of scope.
- **Windows junction/reparse confinement: NOT_ESTABLISHED.** The
  symlink-escape discriminators run only where symlink support exists;
  no junction- or reparse-point proof has been produced on any host.
  This is a stated boundary, not a claim.
