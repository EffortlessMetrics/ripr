# RIPR-SPEC-0170: Stable Cross-Platform Path Text

Status: proposed

Owner: product-analysis

Created: 2026-08-31

Linked issues: #3609

Linked PRs: #3611

Support-tier impact: none; see [support tiers](../status/SUPPORT_TIERS.md).
This is a deterministic identity and rendering contract for paths already
accepted by the analyzer. It does not promote arbitrary-byte path support on
platforms whose native path type cannot carry those bytes.

Policy impact: none. The existing diff-path decoder and native filesystem path
remain the authorities; this spec defines their shared textual projection.

## Problem

Diff paths and native filesystem paths can reach textual identities used by
partial-diff normalization, probe IDs, and rendered output. The shared
`stable_path_text` projection escaped `%` only while walking Unix raw bytes.
Consequently, the same textual path could be encoded differently on Windows,
and a literal `%FF` could retain the reserved byte-escape spelling there.

## Behavior

`stable_path_text` produces a platform-consistent textual projection:

- path separators are rendered as `/`;
- every literal `%` is rendered as `%25`, on every platform;
- on Unix, valid UTF-8 is retained and invalid native bytes are rendered as
  uppercase `%XX` byte escapes;
- on non-Unix platforms, the existing native lossy text form is retained, but
  its literal `%` characters use the same `%25` escape;
- native `Path` values remain the authority for filesystem access and lexical
  confinement.

The `%25` rule reserves `%XX` for the Unix invalid-byte representation. Thus a
Unix raw byte `0xFF` renders as `%FF`, while a valid path literally containing
the three characters `%FF` renders as `%25FF`.

## Required Evidence

- a cross-platform unit test proves literal `%` becomes `%25`;
- a Unix unit test proves an invalid byte and literal `%FF` remain distinct;
- the existing diff-path regression continues to prove native Unix bytes are
  preserved through parsing and downstream path consumers.

## Non-Goals

- changing native path confinement;
- adding arbitrary-byte path support to Windows or other non-Unix platforms;
- changing JSON schemas, probe-ID formats beyond the reserved escaping rule,
  or support tiers;
- claiming cross-platform filesystem byte identity where the native path type
  cannot represent the original bytes.

## Acceptance Examples

1. `pricing_%FF.rs` renders as `pricing_%25FF.rs` on Unix and non-Unix.
2. On Unix, native `pricing_\xFF.rs` renders as `pricing_%FF.rs`.
3. On Unix, the two inputs above have different textual identities.
4. Existing slash normalization and native-path confinement are unchanged.

## Test Mapping

- `crates/ripr/src/analysis/mod.rs::tests::stable_path_text_escapes_reserved_percent_on_every_platform`
- `crates/ripr/src/analysis/mod.rs::tests::stable_path_text_keeps_invalid_byte_encoding_distinct_from_literal_escape`
- `crates/ripr/src/analysis/diff/load.rs::tests::given_quote_path_disabled_repo_when_diff_loaded_then_raw_byte_and_mimic_names_stay_distinct`

## Implementation Mapping

- `crates/ripr/src/analysis/mod.rs` owns the shared textual projection.
- `crates/ripr/src/analysis/diff/path.rs` owns the C-quoted decoder feeding
  native path identity into the projection; its `DecodedPath` split (valid
  text vs Unix raw bytes) is the byte-preserving source this contract
  renders.
- `crates/ripr/src/analysis/syntax/ra.rs` consumes the projection for owner
  symbol identity so distinct raw-byte files keep distinct owners.
- `crates/ripr/src/app/check_artifact.rs` renders non-UTF-8 finding paths
  through the projection at the artifact boundary.
- `crates/ripr/src/analysis/language/rust.rs`,
  `crates/ripr/src/analysis/probes/ids.rs`, and
  `crates/ripr/src/output/path.rs` consume that projection for their existing
  textual identity/rendering surfaces.

## Metrics

- `stable_path_text_regression_pass_rate`
- `unit_test_pass_rate`
