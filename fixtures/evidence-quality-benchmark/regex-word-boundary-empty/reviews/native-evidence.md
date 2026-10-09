# Regex word-boundary oracle evidence

Independent Codex review accepted the corrected test’s expected behavior as valid and the original wrong-sign test’s expected behavior as invalid. These judgments apply to the exact retained source, test and native-capture identities below. The weakened test is a two-assertion removal control and receives no validity or adequacy label.

## Semantic basis

[Upstream issue 859](https://github.com/rust-lang/regex/issues/859) distinguishes matching an empty haystack from matching a zero-width substring in surrounding context. Its separate `regex::Regex` matcher example observes `\b` ranges `0..0` and `1..1` in `a`, while `\b` does not match an empty haystack. `\B` matches the empty haystack and produces no match in `a`. The same observations were captured with both production variants. The corrected positive word-boundary assertions express this rule; the original negative assertions contradict it. Runtime discrimination alone does not establish correct expectations.

This selects the second fix from [regex PR 860](https://github.com/rust-lang/regex/pull/860): fixed commit `88a2a62d861d189faae539990f63cb9cf195bd8c`, direct parent `72f09f1aeb0ff3f703b1afdbdd21f5ff63162fb4`. The parent already contains the unrelated ASCII-union fix. The commit records that the matcher did not consume the changed HIR predicate.

## Historical native observations

The selected subject is `regex-syntax 0.6.25`, library test `hir::translate::tests::analysis_is_match_empty`. Every matrix row selected and executed one test with zero ignored tests. Captured Cargo execution and direct replay of its frozen executable agreed:

| Production | Complete test | Outcome | Test executable SHA-256 |
| --- | --- | --- | --- |
| broken | original | Pass | `0d67ef5f2b17706733aca84aa7a3afdec057dc572519c01b8cedacdd7fd58027` |
| fixed | original | Intended assertion failure at translate.rs:3160 | `05df958d2441e844045e34221e22de0e3d7251244b6cb4b5e8f63c1ea1ffe557` |
| fixed | corrected | Pass | `34ecc19c42dcd9b3abc08be2792514a4032a6dc56a0f25fd2b135491ed957e3a` |
| broken | corrected | Intended assertion failure at translate.rs:3154 | `460fb6f322189d60821ea1cf32c2b917a5e6614aef10c0b0fb872bd41a0974f2` |
| broken | weak | Pass | `cff9571c1f9c4b4ae617bdbb090b3be0a4d144d8627d307f605b1ff44231ad92` |
| fixed | weak | Pass | `b03fbf008d18cf0cec84ef2ef23ac2470cfe1f9bd394c2120a1ba5bc2956823c` |

Passes have native exit 0; intended assertion failures have exit 101. The original/fixed failure is the negative Unicode boundary assertion at line 3160; corrected/broken fails at the corresponding positive assertion at line 3154. A failing whole test stops there, so these runs do not establish execution of the later ASCII assertion. The original, corrected and weak sources contain 32, 35 and 33 assertion statements; those are source counts, not executed-subject counts. The weak file removes only the two corrected boundary assertions and retains every neighboring assertion.

Both the unmodified issue matcher program and a separately instrumented version exited 0 against each production variant. The instrumented output was `wb_empty=false; notwb_empty=true; wb_a=[0..0, 1..1]; notwb_a=[]`, followed by `matcher_witness: 4 assertions passed`. No separate ASCII matcher execution is claimed.

Matcher executable SHA-256 identities:

- fixed / original: `c49ef709cd140abeb7f181c64b75b0fb4625c6f551ebe70ed581c37ba39a436b`
- fixed / observed: `1b1973378daf5eaa1c06775f105a2dcbe5e11223c8b17d39c32547fb8c81a684`
- broken / original: `83748b18e33d9c1fe7c21416e500ab106b29d42bf7f908678edf4f098b426256`
- broken / observed: `674e897d4d3245e42b5cd782a857fb876210a948dc2c724d3408749a8830c328`

## Exact inputs and reviewed packet

Original workspace archives and all original manifests were retained. Before/after inventories preserve the 248 original files plus the generated Cargo.lock, with only the declared full production/test-file transitions. There was no manifest reduction. Cargo 1.95.0 and rustc 1.95.0 were used; the lock was generated for this replay and is not an upstream-authored lock. Setup cache misses are retained separately from test outcomes. Exact commands, working directories, compiler selections and transcripts remain in the capture records.

Input SHA-256 identities:

- Production broken: `325dc1e42eb8fb9daeb7a8a5e7f967fdee745a7a7c5e26c20dec0b6c66109ad7`
- Complete test original: `cee557e068927ef028fdd0c8b673b948aa52397ed7bd5500f1a38a51bc29ec55`
- Production fixed: `51f1642b75e298b0847855d7f490eca2e8f445c9039dd3aa91491f533ec83f15`
- Complete test corrected: `de10ee2928001567f80c6ab602de0e280a8e3a1e615cf73def91d2dcc4f9f199`
- Complete test weak: `9150ff57bf007f523ef1dc8f4c9680ea2edfd3fe55fca47f83b36b1b5834e169`
- Generated Cargo.lock: `9cbfc6b8b6b2c132365dd31fc3b66d6f6caf964fdb7214a28e2435fa7367ab2d`

- Retained issue 859 basis: `18cc3fc2bfe0e0a47653fd201484966ffbe4a7618ef68a463ea57d21734ec6df`
- Raw native packet: `b910ec9fafcdca9e1783218df877f8900313362a7741c51716d1c950a9045a0b`
- Raw native receipt: `8dca1d09ece7506e6f32302e310c55f0efc0eb6dcacb105e5cbdff88d74fe6b8`
- Reviewed answer key: `regex-word-boundary-empty/answer-key.json`, 3,147 bytes, `dc8dc5730f274bbbc7137ff0617e01a751487574577132db034c58d922796c3e`
- Reviewed native pairing: `regex-word-boundary-empty/native-pairing.json`, 20,853 bytes, `312642ff1381d2ea400a40e1c9cde51d3d141ae900265803d6cacf8c7733369d`

## Custody and measurement limits

Independent review rehashed the retained executable witnesses and checked the portable capture mappings. Each compact test capture also retains an explicit frozen-executable measurement dated 2026-10-03T09:16:44Z. That is a later measurement of the frozen executable; it does not represent a contemporaneous after-run measurement of the reused mutable compiler-output path. Historical execution, subsequent byte measurements and fresh execution are separate evidence.

The large executable witnesses are held externally. Routine source-only fixture validation checks retained capture/review identities and reports external executable bytes as NOT_REVERIFIED. Historical working-directory and executable paths in raw capture data describe the original execution environment. Reproduction requires a new run and newly recorded identities.

This evidence does not establish RIPR analyzer accuracy, product compilation or report execution, judged-panel calibration, opportunity membership, representative sampling, package qualification or release readiness. The two semantic reviews are alternate views of one case; their acceptance does not decide corpus row structure.
