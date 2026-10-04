# Historical regex word-boundary oracle case

This fixture retains one historical source/native case with two independently
reviewed oracle views in the corpus's separate semantic_oracle_controls
collection. Native observations, semantic review and historical RIPR output
have distinct records. No calibration result, blind opportunity, package
qualification or release follows from them. The 82 static/calibration cases
keep their existing denominator.

The upstream parent is `72f09f1aeb0ff3f703b1afdbdd21f5ff63162fb4`; the direct fix
is `88a2a62d861d189faae539990f63cb9cf195bd8c`, only the second commit of PR 860.
The parent already contains the unrelated ASCII-union repair. Both complete
workspace archives and their 248-blob inventories are retained under `upstream/`.
MIT, Apache and Unicode licenses are retained. The full upstream patch is
separate from the production-only, corrected-test and exact-removal patches.

The selected subject is regex-syntax 0.6.25 library test
`hir::translate::tests::analysis_is_match_empty`. Each native command selected
and executed exactly one test, with 321 filtered out. Each selected compiler
artifact was separately retained and directly replayed with the exact selector.

| Complete test | Fixed production | Broken production |
| --- | --- | --- |
| Corrected | Pass | Intended assertion failure at translate.rs:3154 |
| Original wrong-sign | Intended assertion failure at translate.rs:3160 | Pass |
| Corrected minus two boundary assertions | Pass | Pass |

The original test has 32 source assertions; corrected has 35. The three added
neighbors are `a|`, `|a` and `a||b`. Moving the two boundary assertions from
negative to positive does not change the count. Weak has 33 after removing
exactly the Unicode and ASCII positive boundary assertions. A failing whole
test stops at the first Unicode assertion; later ASCII assertion execution is
not established by those failures. Weak is solely a removal control, with no
validity or adequacy label inferred from its passes.

The independent matcher witness ran against both production variants. Both
observe `\b` rejecting an empty haystack yet returning `0..0` and `1..1` in
`a`; `\B` accepts an empty haystack and has no match in `a`. The upstream commit
states that the changed HIR predicate was not then consumed by the matcher.
The semantic basis is the empty-substring-in-context distinction, separate from
whether an oracle discriminates between the two implementations.

## Capture, current custody and review

`answer-key.json` names the exact claim, basis, full sources/tests and selector.
`native-pairing.json` binds all six compact capture files and the generated lock.
Each capture retains its selected compiler artifact metadata, exact test
discovery, Cargo transcript, frozen executable replay and full-workspace input
fences. `verification/` contains explicitly dated later rehashes of retained
frozen executables. These are not separately retained contemporaneous
measurements of the mutable compiler path.

The historical capture declares external retention of ten executables.
Ordinary fixture validation does not establish their current availability. It
checks the retained compact records and review identities; it must
report external executable custody as NOT_REVERIFIED. It does not rerun tests,
inspect unavailable executable bytes, authenticate the producer or infer domain
semantics. Current independent review of the retained bytes is a separate fact.

The proposal contains two alternate declarations for the same historical case:
corrected/valid and original/invalid. They cannot become two simultaneous legacy
`cases` rows with the same ID; the separate controls are unique by `(id, variant)`.
Any later case-ID, claim, basis, source, test, capture or
verdict change requires an updated exact reviewed-subject binding. No accepted
review text may simply be carried forward to a changed subject.

`reviews/native-evidence.md` preserves the review of the original portable
locators. The current independent-review JSON records bind the byte-identical
inputs under `.rs.txt` locators after a separate packaging review. Historical
execution paths and native input inventories retain their original spellings.

## Reproduction envelope

First materialize the complete original parent archive into one fixed replay
root. Keep all original workspace members and manifests, including the root,
bench, regex-capi, regex-debug and regex-syntax manifests. Do not turn the
regex-syntax member into a standalone crate or silently reduce the workspace.

Upstream has no Cargo.lock. Cargo 1.95 generated the retained lock from those
unchanged original manifests: 56 packages, including five local members and
51 registry packages. Offline setup first lacked the docopt index entry; a
later selected-package compile lacked aho-corasick 0.7.20. The bounded online
resolution/download steps are retained as setup history, not test outcomes.
Copy the declared generated lock into the restored root for a locked replay.
Preserve its digest; if new resolution is necessary, record a new lock and
separate the new observations from this historical capture.

Use private final and intermediate output directories. Explicitly set both
CARGO_TARGET_DIR and CARGO_BUILD_BUILD_DIR, plus CARGO_BUILD_JOBS=2 and
CARGO_INCREMENTAL=0. Honor current resource admission and the shared Cargo lock.
The original observation used pinned Cargo/rustc 1.95.0; exact versions and binary
identities are retained in the original capture. No toolchain or manifest pin
was changed.

For each row, copy the full declared production source into
`regex-syntax/src/hir/mod.rs` and the full declared test file into
`regex-syntax/src/hir/translate.rs`. Fence all 248 original inputs plus the lock
before and after each command; only those two source paths vary. The parent
CHANGELOG remains unchanged in this production/test experiment.

The `.rs.txt` suffix identifies immutable source data retained for reproduction.
Copy bytes without text conversion from the selected asset to the corresponding
Rust path below. The source/test digests in `answer-key.json` and the complete
native input inventories bind the result. The matcher program descriptors are
in `basis/independent-matcher-capture.json`. Licenses and original archive paths
remain unchanged.

| Retained data asset | Reconstructed Rust destination |
| --- | --- |
| `sources/fixed.rs.txt` | `regex-syntax/src/hir/mod.rs` for fixed production |
| `sources/broken.rs.txt` | `regex-syntax/src/hir/mod.rs` for broken production |
| `tests/corrected.rs.txt` | `regex-syntax/src/hir/translate.rs` for corrected tests |
| `tests/original.rs.txt` | `regex-syntax/src/hir/translate.rs` for original tests |
| `tests/weak.rs.txt` | `regex-syntax/src/hir/translate.rs` for the removal control |
| `basis/matcher-original.rs.txt` | `matcher-original.rs` in the separate witness source directory |
| `basis/matcher-observed.rs.txt` | `matcher-observed.rs` in the separate witness source directory |

Compile with `cargo test --locked --offline --manifest-path Cargo.toml -p
regex-syntax --lib --no-run --message-format=json`, select the single library
test artifact for regex-syntax 0.6.25, and list the exact subject from that
artifact. Then run `cargo test --locked --offline --manifest-path Cargo.toml -p
regex-syntax --lib hir::translate::tests::analysis_is_match_empty -- --exact`.
Retain the artifact and directly replay its exact selector. A zero subject,
setup failure, timeout, wrong assertion or stale executable is not a substitute
for any required native outcome. Input hashes and a Git stamp alone do not
establish artifact behavior.

Replay the independent matcher program in `basis/matcher-original.rs.txt` against
the original workspace root regex library. `basis/matcher-observed.rs.txt` preserves
the same four semantic assertions and prints the observed values. Both programs
were compiled and run for fixed and broken production; their separate native
identities and outputs are in `basis/independent-matcher-capture.json`.

RIPR observations for corrected, original and weak tests completed against the
exact 68770f7c producer. All three JSON outputs are byte-identical and report one
exposed/strong call-deletion finding. Read static-observation.json before using
that result: its eight related tests are a capped REPORTED list, not the established
complete internal selection. Absence means not reported. The setter-omission
counterfactual differs from the historical argument change, so no
deletion-specific false-exposure claim follows. Semantic validity, historical native
discrimination and this static observation remain separate axes.

## Required local evidence graph

The reviewed key distinguishes the changed production path from the library
entry point and assigns explicit source-document/matcher kinds to its basis.
The matcher has its own regex 1.5.5 compiler subject. Its declared corrected
native test variant binds workspace inputs, not execution of the HIR test by
standalone matcher programs.

All six native before/after inventory pairs are required dependencies. Their
complete 249-row footprint must equal the original parent inventory plus the
generated lock and two declared full-file substitutions. The matcher programs,
outputs and four input inventories are also required local dependencies; their
recorded package, argv, exit and value/output identities are checked. No Rust
source inference or new execution substitutes for the independent judgment.

The finite graph contains 120 unique referenced files. Four static narrative
and correction-history attachments receive outer byte/JSON-object checks;
their historical paths and superseded claims are not current runtime authority.
README, the three license copies, three auxiliary patches and the curated
historical review note remain required documentary/package contents, pinned by
preservation tests. They cannot replace the full source/test assets, native
captures or current reviewed-subject records.

Custody totals count the six primary native test observations per accepted
view. Matcher executables and linked libraries are separately external and
NOT_REVERIFIED. No current external-file availability or byte rehash follows
from fixture acceptance.
