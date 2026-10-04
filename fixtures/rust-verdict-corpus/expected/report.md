# Rust verdict corpus report

Spec: RIPR-SPEC-0219. Corpus version: 2026-10-04.5. Cases: 104.

| Rate | Count | Rate |
| --- | --- | --- |
| False verdicts (all cases) | 37/104 | 0.3558 |
| False actionable (of discriminated) | 33/56 | 0.5893 |
| False exposed (of not fully discriminated) | 4/48 | 0.0833 |
| False silent (of not fully discriminated) | 0/48 | 0.0000 |
| Ideal verdict | 33/104 | 0.3173 |
| Abstained (limited or silent where acceptable) | 34/104 | 0.3269 |
| Findings with a contradiction | 2/138 | 0.0145 |

By subject origin. Authored cases are written to fill cells the upstream cases leave empty, so only the upstream rates describe real-world tests.

| Origin | Cases | False verdicts | False actionable | False exposed | False silent | Ideal | Abstained |
| --- | --- | --- | --- | --- | --- | --- | --- |
| authored | 70 | 27/70 | 23/36 | 4/34 | 0/34 | 28/70 | 15/70 |
| upstream | 34 | 10/34 | 10/20 | 0/14 | 0/14 | 5/34 | 19/34 |

| Case | Origin | Truth | Ideal | Observed | Classes | Outcome | Changed since labeling | Contradictions |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `serde-format-u8-hundreds` | upstream | discriminated | credited | gap | reachable_unrevealed | false_actionable | no | none |
| `serde-format-u8-tens` | upstream | discriminated | credited | limited | propagation_unknown | abstained | no | none |
| `semver-caret-minor-ge` | upstream | discriminated | credited | limited | no_static_path | abstained | no | none |
| `semver-tilde-pre-ge` | upstream | discriminated | credited | limited | infection_unknown, propagation_unknown | abstained | no | none |
| `semver-less-pre` | upstream | discriminated | credited | limited | infection_unknown, propagation_unknown | abstained | no | none |
| `semver-greater-patch` | upstream | discriminated | credited | limited | no_static_path | abstained | no | none |
| `semver-digit-upper` | upstream | partially_discriminated | gap | limited | infection_unknown | abstained | no | none |
| `semver-max-comparators` | upstream | partially_discriminated | gap | limited | no_static_path | abstained | no | none |
| `semver-caret-zero-minor` | upstream | partially_discriminated | gap | limited | no_static_path | abstained | no | none |
| `hex-from-hex-odd-variant` | upstream | discriminated | credited | gap | reachable_unrevealed, weakly_exposed | false_actionable | no | none |
| `hex-decode-slice-odd` | upstream | discriminated | credited | gap | reachable_unrevealed | false_actionable | no | none |
| `hex-decode-slice-length` | upstream | partially_discriminated | gap | gap | reachable_unrevealed | ideal | no | none |
| `hex-encode-slice-length` | upstream | partially_discriminated | gap | gap | reachable_unrevealed | ideal | no | none |
| `itoa-two-digit-tail` | upstream | not_discriminated | gap | limited | no_static_path | abstained | no | none |
| `regex-syntax-word-byte` | upstream | partially_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `regex-syntax-max-scalar-two-byte` | upstream | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `serde-derive-rename-variant-lower` | upstream | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `serde-derive-rename-field-upper` | upstream | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `semver-leading-zero` | upstream | discriminated | credited | limited | no_static_path | abstained | no | none |
| `semver-op-greater-eq` | upstream | discriminated | credited | gap | reachable_unrevealed | false_actionable | no | none |
| `itoa-four-digit-loop` | upstream | not_discriminated | gap | limited | no_static_path | abstained | no | none |
| `semver-digit-upper-first-run` | upstream | discriminated | credited | limited | infection_unknown | abstained | no | none |
| `bytesize-format-unit-first-run` | upstream | discriminated | credited | limited | infection_unknown | abstained | no | none |
| `rusqlite-singlethreaded-magic` | upstream | discriminated | credited | limited | no_static_path, static_unknown | abstained | no | none |
| `semver-digits-ten` | upstream | partially_discriminated | gap | limited | no_static_path | abstained | no | none |
| `semver-req-separator` | upstream | discriminated | credited | limited | no_static_path | abstained | no | none |
| `strsim-sorensen-dice-equal` | upstream | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `bytesize-as-kib-div` | upstream | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `bytesize-as-mb-div` | upstream | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `bytesize-as-kb-div` | upstream | not_discriminated | gap | limited | no_static_path | abstained | no | no_static_path_with_related_tests |
| `bytesize-as-mib-div` | upstream | not_discriminated | gap | limited | no_static_path | abstained | no | no_static_path_with_related_tests |
| `strsim-jaro-winkler-threshold-shift` | upstream | partially_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `atuin-ai-history-output-capability` | upstream | not_discriminated | gap | gap | infection_unknown, weakly_exposed | ideal | no | none |
| `atuin-otel-traces-suffix-not` | upstream | not_discriminated | gap | limited | infection_unknown | abstained | no | none |
| `pricing-gold-threshold` | authored | discriminated | credited | credited | exposed | ideal | no | none |
| `pricing-free-shipping-boundary` | authored | partially_discriminated | gap | gap | exposed, weakly_exposed | ideal | no | none |
| `pricing-gold-discount-rate` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `pricing-flat-shipping-fee` | authored | not_discriminated | gap | limited | static_unknown | abstained | no | none |
| `pricing-tier-label-gold` | authored | discriminated | credited | credited | exposed | ideal | no | none |
| `pricing-quote-total-field` | authored | partially_discriminated | gap | credited | exposed | false_exposed | no | none |
| `ledger-ship-log-push` | authored | discriminated | credited | gap | exposed, weakly_exposed | false_actionable | no | none |
| `ledger-receive-refresh-low-stock` | authored | not_discriminated | gap | credited | exposed | false_exposed | no | none |
| `ledger-receipt-remaining` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `ledger-insufficient-available` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `ledger-shipped-total` | authored | partially_discriminated | gap | limited | static_unknown | abstained | no | none |
| `ledger-stock-insert` | authored | discriminated | credited | gap | exposed, weakly_exposed | false_actionable | no | none |
| `ledger-ship-exact-stock` | authored | not_discriminated | gap | limited | infection_unknown | abstained | no | none |
| `ledger-sku-family-unsafe` | authored | discriminated | credited | credited | exposed, static_unknown | ideal | no | none |
| `ledger-sku-variant-unsafe` | authored | partially_discriminated | gap | credited | exposed, static_unknown | false_exposed | no | none |
| `config-missing-equals-line` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `config-empty-key-error` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `config-log-level-warn` | authored | discriminated | credited | gap | exposed, weakly_exposed | false_actionable | no | none |
| `config-bool-false-arm` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `config-port-zero` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `config-default-host` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `config-duplicate-key-case` | authored | not_discriminated | gap | limited | infection_unknown | abstained | no | none |
| `ledger-sku-family-end` | authored | discriminated | credited | limited | infection_unknown | abstained | no | none |
| `accounts-balance-add` | authored | discriminated | credited | limited | static_unknown | abstained | no | none |
| `accounts-trailer-crc` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `accounts-parse-too-long-variant` | authored | partially_discriminated | gap | gap | exposed, weakly_exposed | ideal | no | none |
| `accounts-last-byte-unchecked` | authored | not_discriminated | gap | limited | static_unknown | abstained | no | none |
| `checkout-fee-closure-never-called` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `checkout-fee-assert-under-false-flag` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `checkout-fee-unpolled-async-assert` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `checkout-fee-cfg-disabled-test` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `checkout-fee-err-return-guard` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `checkout-withdraw-guarded-match-pin` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `checkout-withdraw-sibling-variant` | authored | not_discriminated | gap | gap | exposed, weakly_exposed | ideal | no | none |
| `checkout-refund-matches-variant` | authored | discriminated | credited | gap | exposed, weakly_exposed | false_actionable | no | none |
| `checkout-deposit-cap-happy-path-only` | authored | not_discriminated | gap | gap | exposed, weakly_exposed | ideal | no | none |
| `checkout-tax-self-computed-expected` | authored | not_discriminated | gap | credited | exposed | false_exposed | no | none |
| `checkout-daily-limit-imported-const` | authored | discriminated | credited | gap | propagation_unknown, weakly_exposed | false_actionable | no | none |
| `checkout-minimum-same-file-const` | authored | discriminated | credited | gap | propagation_unknown, weakly_exposed | false_actionable | no | none |
| `checkout-review-split-boundary-tests` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `checkout-bulk-custom-assert-macro` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `checkout-rate-same-method-other-type` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `checkout-region-literal-match-helper` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `checkout-announce-stdout-sink` | authored | not_discriminated | gap | limited | propagation_unknown | abstained | no | none |
| `checkout-record-discarded-result` | authored | discriminated | credited | limited | static_unknown | abstained | no | none |
| `checkout-persist-swallowed-ok` | authored | discriminated | credited | limited | propagation_unknown | abstained | no | none |
| `tokens-scanner-state-arm` | authored | discriminated | credited | credited | exposed | ideal | no | none |
| `tokens-recursive-label-arm` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `tokens-word-start-helper` | authored | partially_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `tokens-normalize-helper-chain` | authored | partially_discriminated | gap | limited | propagation_unknown | abstained | no | none |
| `tokens-base-six-hop-chain` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `tokens-inner-rate-macro-reach` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `tokens-inner-bonus-test-macro-call` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `tokens-add-fee-integration-api` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `tokens-ext-start-map-or-binding` | authored | partially_discriminated | gap | limited | propagation_unknown | abstained | no | none |
| `tokens-fits-binding-predicate` | authored | discriminated | credited | limited | propagation_unknown | abstained | no | none |
| `tokens-long-flag-strip-prefix` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `tokens-byte-at-unsafe-fn` | authored | discriminated | credited | gap | static_unknown, weakly_exposed | false_actionable | no | none |
| `shop-score-imported-across-crates` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `shop-rebate-same-name-other-crate` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `shop-discount-path-dependent-test` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `shop-item-cents-trait-method` | authored | discriminated | credited | credited | exposed | ideal | no | none |
| `shop-item-total-associated-vs-free` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `shop-tier-gold-arm-unreached` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `shop-cart-add-other-collection-observed` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `shop-quote-total-result-field` | authored | discriminated | credited | credited | exposed | ideal | no | none |
| `shop-cap-literal-only-expected` | authored | not_discriminated | gap | gap | weakly_exposed | ideal | no | none |
| `shop-gate-let-bound-input` | authored | discriminated | credited | gap | weakly_exposed | false_actionable | no | none |
| `roles-limit-const-value` | authored | discriminated | credited | limited | static_unknown | abstained | no | none |
| `roles-cfg-test-helper-input` | authored | discriminated | credited | silent | none | abstained | no | none |

Non-claims:

- Rates describe this corpus only; they are not a population estimate for Rust code or for ripr in general.
- Authored cases were written to fill verdict and probe-family cells the upstream cases leave empty; their rates are reported separately under by_origin and are not real-world rates.
- Truth comes from the listed mutants, not from exhaustive mutation; a discriminated label means every listed mutant made the test command fail.
- The harness runs ripr on retained upstream excerpts and on whole authored crates; excerpt findings matched the full pinned checkout at labeling time and must be re-checked when a verdict changes.
- The harness does not run mutation testing, cargo test, or any network access.
