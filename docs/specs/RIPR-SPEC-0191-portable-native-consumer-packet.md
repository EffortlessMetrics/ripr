# RIPR-SPEC-0191: Portable native RIPR consumer packet

Status: proposed

Owner: agent/portable-consumer

Created: 2026-09-29

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4713 — portable offline native ripr consumer packet
- #5037 — truthful host applicability of native consumer coverage
- #4493 — wheel/npm install qualification may reuse this consumer later
- #4521 — final native payload identity remains a separate supply route
- #4714 — precompiled Rust-test replay is out of scope

Linked PRs:

Support-tier impact:

- None. This spec adds a stdlib packet consumer for already-qualified native
  payloads. It does not promote a language, distribution channel, or support
  claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register `tools/python/portable-ripr-consumer/**/*.py` in
  `policy/non-rust-allowlist.toml` as tooling, not a fixture exception.
- Retain that path in the non-Rust programming retention classifier.
- Count `subprocess.Popen` in the process-policy ledger.

## Problem

Python-capable agent environments can receive a qualified native `ripr`
payload as a mounted artifact, but they often have no Cargo, rustc, or
execution-kernel network. The repository already owns explicit-binary
launch, digest checks, and typed execution states. The missing object is a
portable, dependency-light consumer that composes those authorities without
a second semantic model.

## Behavior

A packet directory contains `manifest.json`, the native payload
(`ripr` or `ripr.exe`), `run.py`, and this README. `run.py` uses the Python
3.11+ standard library only and:

1. Verifies packet and payload SHA-256 values before launch.
2. Resolves the payload as a packet-relative path, then launches that
   absolute path with an argument array and no shell.
3. Never searches PATH for `ripr`, never downloads, and never invokes a
   compiler.
4. Requires an explicit subject root and writable output directory.
5. Honors `--foreign-cwd` so analysis still binds the explicit root.
6. Allowlists only `check` and `pilot`. `pilot` launches with `--out {out_dir}`
   and classifies `pilot-summary.json`, not terminal stdout. The launched argv
   verb must match the allowlisted operation. `--diff` is valid only for
   `check`.
7. Classifies digest mismatch, missing executable, incompatible payload,
   timeout, malformed or partial product JSON, zero required subjects,
   subject-tree drift, unwritable output, launch failure, and complete
   results as distinct receipt classes. Product `analysis_outcome` kinds are
   the producer-owned set; invented kinds fail closed. `--subject-digest`
   is rechecked after launch; a subject-tree file cap is
   `environment_unavailable`, not drift.
8. Writes `packet-consumption-receipt.json` that keeps packet, payload, and
   subject identity separate.
9. Validates existing product JSON by `schema_version` and projects counts
   without rewriting classifications.

## Required Evidence

- Stdlib `py_compile` of `run.py`.
- Source invariant: no PATH lookup helper, no network client import, no
  compiler argv.
- Stub packet: complete result, PATH decoy ignored, foreign cwd, digest
  mismatch (payload, consumer script, and packet_digest field), missing
  executable, missing manifest, platform mismatch, relative-path escape,
  malformed JSON, empty stdout, typed product limitation, incomplete
  analysis outcome, zero subjects (required vs allowed), timeout,
  nonzero payload exit, unknown operation, unwritable output, subject-tree
  drift, argv-template verb mismatch, pilot `--diff` rejection, post-launch
  subject digest, subject-file cap, output-artifact symlink, write-probe
  symlink, leftover `pilot-summary.json`, and a digest-mismatch receipt that
  records the observed payload hash rather than the expected manifest digest.
- Classification matrix imports `classify_product_json` so product-class
  edges are discriminated without a payload launch. The matrix includes the
  producer kinds `partial_with_limitations`, `unsupported_input`, and
  `analysis_failed`, and rejects invented kinds.
- Native packet: worktree `ripr` analyzes the Rust `boundary_gap` fixture
  from a foreign cwd with a PATH decoy and Cargo off PATH, producing a
  nonempty findings list and `schema_version` `0.2`. That subject stays
  valid under `--features lang-rust` only. A second native journey runs
  `--operation pilot` and classifies `pilot-summary.json`.
- Host coverage: the native packet target and Python runtime/classifier
  controls are Unix-only. Windows retains the real common packet-digest and
  source controls. The canonical non-Rust ledger declares the native selector
  under `covered_by_unix`; file-policy reports it as inapplicable on Windows,
  rather than accepting a zero-test enumeration. Malformed or unknown host
  declarations and empty applicable selections fail closed. The policy facade
  checks the report's real selected identities and host applicability.

## Non-Goals

- PyPI/npm publication, `import ripr`, PyO3, or a downloader
- Support-tier promotion or generic remote execution
- Claiming an unbuilt RIPR source change was compiled
- Wheel/npm channel qualification (#4493)
- Final native payload identity (#4521)
- Precompiled Rust-test replay (#4714)
- A native Windows consumer journey; host-scoped coverage does not implement it

## Acceptance Examples

- A staged stub packet with a planted PATH `ripr` still runs the packet
  payload and records `classification=complete`.
- Replacing the payload bytes without updating the manifest records
  `digest_mismatch` and does not launch.
- The worktree binary inside a packet reports the Rust `boundary_gap`
  finding while Cargo is absent from PATH.

## Test Mapping

- `xtask/src/tests.rs::file_policy_allowlist_accepts_host_scoped_coverage`
- `xtask/src/tests.rs::file_policy_allowlist_rejects_malformed_host_coverage`
- `xtask/src/tests.rs::policy_checker_facade_runs_current_repo_checks`
- `xtask/src/policy/file_policy.rs::tests::test_covered_by_host_selection_retains_common_and_discloses_inapplicable`
- `xtask/src/policy/file_policy.rs::tests::test_covered_by_empty_applicable_host_selector_is_rejected`
- `xtask/src/policy/file_policy.rs::tests::test_covered_by_unknown_host_family_is_rejected`

- `xtask/src/portable_consumer.rs::tests::run_py_compiles_with_stdlib_python`
- `xtask/src/portable_consumer.rs::tests::consumer_source_does_not_search_path_or_open_a_network_client`
- `xtask/src/portable_consumer.rs::tests::product_json_classes_are_discriminated_without_launch`
- `xtask/src/portable_consumer.rs::tests::complete_stub_packet_runs_the_packet_payload_not_a_path_decoy`
- `xtask/src/portable_consumer.rs::tests::foreign_cwd_still_passes_the_explicit_subject_root`
- `xtask/src/portable_consumer.rs::tests::digest_mismatch_fails_closed_before_launch`
- `xtask/src/portable_consumer.rs::tests::consumer_script_digest_mismatch_fails_closed_before_launch`
- `xtask/src/portable_consumer.rs::tests::packet_digest_field_mismatch_fails_closed_before_launch`
- `xtask/src/portable_consumer.rs::tests::missing_executable_fails_closed`
- `xtask/src/portable_consumer.rs::tests::missing_manifest_is_incompatible_payload`
- `xtask/src/portable_consumer.rs::tests::platform_mismatch_fails_closed_before_launch`
- `xtask/src/portable_consumer.rs::tests::relative_path_escape_fails_closed_before_launch`
- `xtask/src/portable_consumer.rs::tests::malformed_product_json_fails_closed`
- `xtask/src/portable_consumer.rs::tests::empty_stdout_is_partial_product_output`
- `xtask/src/portable_consumer.rs::tests::typed_product_limitation_is_not_complete`
- `xtask/src/portable_consumer.rs::tests::incomplete_analysis_outcome_is_partial_product_output`
- `xtask/src/portable_consumer.rs::tests::zero_subjects_fail_closed_when_required`
- `xtask/src/portable_consumer.rs::tests::zero_subjects_are_complete_when_not_required`
- `xtask/src/portable_consumer.rs::tests::payload_timeout_is_classified_not_as_a_semantic_failure`
- `xtask/src/portable_consumer.rs::tests::nonzero_payload_exit_is_execution_failure`
- `xtask/src/portable_consumer.rs::tests::unknown_operation_fails_closed`
- `xtask/src/portable_consumer.rs::tests::unwritable_output_fails_closed`
- `xtask/src/portable_consumer.rs::tests::subject_digest_drift_fails_closed`
- `xtask/src/portable_consumer.rs::tests::arm64_machine_names_match_rust_aarch64`
- `xtask/src/portable_consumer.rs::tests::pilot_reads_the_summary_artifact_not_terminal_stdout`
- `xtask/src/portable_consumer.rs::tests::missing_pilot_summary_is_partial_product_output`
- `xtask/src/portable_consumer.rs::tests::stale_pilot_summary_is_not_classified_as_complete`
- `xtask/src/portable_consumer.rs::tests::argv_template_must_start_with_the_allowlisted_operation`
- `xtask/src/portable_consumer.rs::tests::pilot_rejects_diff_before_launch`
- `xtask/src/portable_consumer.rs::tests::subject_file_cap_is_environment_unavailable_not_drift`
- `xtask/src/portable_consumer.rs::tests::subject_digest_is_rechecked_after_launch`
- `xtask/src/portable_consumer.rs::tests::existing_output_symlink_is_unwritable`
- `xtask/src/portable_consumer.rs::tests::write_probe_symlink_is_unwritable`
- `xtask/src/portable_consumer.rs::tests::packet_digest_matches_the_producer_formula`
- `crates/ripr/tests/portable_consumer_packet.rs::native_packet_analyzes_a_boundary_gap_without_path_or_compiler_fallback`
- `crates/ripr/tests/portable_consumer_packet.rs::native_packet_pilot_consumes_the_summary_artifact`

## Implementation Mapping

- `xtask/src/main.rs` — canonical non-Rust ledger parser and scoped test commands
- `xtask/src/policy/file_policy.rs` — dynamic host selection, enumeration, report
- `policy/non-rust-allowlist.toml` — common and Unix-only coverage declaration
- `tools/python/portable-ripr-consumer/run.py` — stdlib consumer
- `xtask/src/portable_consumer.rs` — packet staging and stub oracles
- `crates/ripr/tests/portable_consumer_packet.rs` — native payload journey

## Metrics

- `portable_consumer_complete` — complete packet receipts
- `portable_consumer_fail_closed` — fail-closed receipt classes

These metrics are advisory counts. They are not support-tier evidence or
analyzer-correctness claims.
