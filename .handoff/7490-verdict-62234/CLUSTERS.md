# Regression controls and witness-authority handoff

Subject: 62234ac13b0f7a8ff7e30d239f59215b808775b3 / ef7221a5c23a73670d23bc9998cfe1496b4e38f0. Production and xtask subtrees equal b51 exactly. Source not repaired by this lane.

## Three observed clusters

clusters.json enumerates every affected case. shared-authority-evidence.json binds each row to its actual raw discriminator summaries and related strong observers. These are classifications from retained live output, not a claimed single-cause patch.

| Cases | Observed missing witness | Source authority / next inspection |
|---:|---|---|
| 17 | identity_unresolved | classify/reveal.rs::reveal_outcome demotes Yes to Weak when the strongest assertion has no identity; analyze_related_assertions computes identity near the direct_owner_oracle/owner_return_pin_holds routes. classifier/evidence.rs supplies semantic pin/observer receipts. |
| 4 | observation_unverified | reveal's assertion-local confirmation/owner-return handling: grid-returns-helper, lib-pretty-total-path-qualified, old-deps-type-path-glob-import, shape-from-impl-exact. These do not carry the final identity_unresolved summary and require separate receipt/confirmation inspection. |
| 3 | same_test_pairing_missing | Boundary pairing for grid-flow-field-opaque, spec-b-0158-strip-prefix-absent, tokens-long-flag-strip-prefix. Inspect classifier/evidence.rs's returned_comparison/wrapper_boundary_probe and same-test entry/activation admission; do not combine inputs and oracles from different assertions/tests. |

For the first group, the direct lexical identity route accepts a whole owner-call operand and one specific ErrorPath unwrap_err().kind() form. A projected field, bound error value or forwarded result needs a semantic identity receipt; a direct_owner_call relation alone cannot supply it. `owner_return_pin_holds` additionally preserves import/cross-package and error-variant conditions. Actual cell-0005-same-name-owner-tested output has reach/infect/propagate/observe=yes, a strong exact field observer, then discriminator=weak at the final identity demotion. This identifies where useful evidence is withheld; it does not prove that weakening that gate wholesale is safe.

The four confirmation and three pairing cases must not be silently folded into the 17-case identity explanation. A repair to only the final identity gate cannot be assumed to resolve them. The seven additional abstentions are another independent set, named in denominators-and-missed-credit.json: accounts-balance-add, grid-delete-exact, hex-from-hex-odd-variant, shape-op-add-impl, and the three spec0228 field-write observers (direct-read, enum-variant, getter-read).

## Executed controls: intended refusal versus wrong advice

The existing relabel producer freshly executed the nine cases below, each with repeat=2, native exit 0, 66 test-command observations and zero label drift. Baseline/edited programs actually ran tests and passed; mutants compiled, and their named pass/fail outcomes matched labels. The report is regression-runtime/relabel.json. This is finite listed-mutant truth, not exhaustive mutation adequacy or a proof that every static dispatch identity is resolved.

| Retained case | Fresh runtime truth | Current analyzer verdict | Interpretation |
|---|---|---|---|
| cell-0005-same-name-owner-tested | discriminated | gap | Constructed total is observed by the real imported owner's test; false-actionable by the unchanged contract. |
| cell-0005-same-name-owner-unreached | not_discriminated | gap | Necessary twin: the tested same-named function is another owner; must not gain credit. |
| forwarder-hop-forwarded-hit | discriminated | gap | Forwarded result is checked; lost-credit/wrong-advice witness. |
| forwarder-hop-swallowed-result | not_discriminated | limited | Necessary refusal: reach without forwarding/observation is insufficient. |
| spec0106-unwrap-err-exact-variant | discriminated | gap | Exact error variant is observed through the bound unwrap_err result. |
| grid-returns-helper | discriminated | gap | Isolates observation_unverified on a real helper observer. |
| grid-returns-none | not_discriminated | gap | A reached call with no observer must not be promoted. |
| spec-b-0158-strip-prefix-absent | discriminated | gap | Isolates same_test_pairing_missing against the unchanged listed-mutant truth. |
| spec-b-0158-rfind-boundary | discriminated | credited | Current working positive control: boundary support is not globally absent. |

The separate four-case runtime-controls report (34 observations, repeat=2, zero drift) retains the self-computed expected-value negative, exact helper-JSON positive, count-versus-payload distinction and split-boundary negative. The latter two remain partially discriminated and gaps; they must not become blanket exposed merely to repair the positive cases. All 13 replayed cases retain the authored labels and expected rows.

Minimal reproduction uses the existing producer, not a new harness:

```
<candidate xtask> verdict-corpus report --cases cell-0005-same-name-owner-tested,cell-0005-same-name-owner-unreached,forwarder-hop-forwarded-hit,forwarder-hop-swallowed-result,spec0106-unwrap-err-exact-variant,grid-returns-helper,grid-returns-none,spec-b-0158-strip-prefix-absent,spec-b-0158-rfind-boundary --out <owned-report>
```

This is a suggested focused reproduction command; the complete four-corpus executions and the exact nine-case relabel command actually executed are retained in commands.json. Direct standalone CLI reproductions for the first owner, forwarder and exact-error failures also executed with native exit 0; their args/raw output are under reproductions/.

## Repair boundary and remaining uncertainty

A stricter refusal of an unestablished identity can be intentional and necessary. Emitting actionable gap advice for the unchanged discriminated cases is a separate observed failure. Prefer restoring a producer-authenticated same-assertion witness where the supported path establishes it. If a path remains statically unresolved, retain a precise limitation and its missed-credit cost; do not call unknown repaired discrimination or silently accept a larger abstention ledger. Such a bounded non-claim still needs the existing #2766 decision.

Do not remove the identity gate, trust a coarse direct-owner relation, borrow another test's strong oracle, change truth labels, bless changed expected outcomes or add accepted-false entries merely to turn the gate green. Historical 984 baseline rows/receipt are retained observations, not a freshly executed Linux baseline binary. Fresh runtime replay plus the matched positive/negative controls establishes the declared corpus distinction; it does not establish a safe complete analyzer patch or global accuracy. Central integration/#12010 owner owns the repair and subsequent exact-subject reconciliation.
