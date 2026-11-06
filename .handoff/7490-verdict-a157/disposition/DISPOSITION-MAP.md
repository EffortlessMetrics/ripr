# Twelve missed-credit cases and expected-row drift

Exact measured source a157a1d33fca88187e532cba4f1e3f49b97b0f02, tree7d17ca7d4dcbd420cbfbd13426b8f6f580cc3773. This is a retained-evidence disposition map, not a rerun, repair, accepted-cost decision or blessed baseline. All twelve rows are Rust, discriminated truth, same scoring definitions and eligible in both full-population and discriminated-truth missed-credit denominators. All twelve committed expectations are credited/ideal; all twelve currently read limited/abstained. Expected classes are exposed, plus infection_unknown for the three boundary-value cases.

Stage order below is reach/infect/propagate/observe/discriminate; Y=yes, U=unknown, W=weak. Pre-existing means already abstained on62234; new means62234 false-actionable became abstained on17ba8 and stays so on a157. None is newly abstained on a157.

| Case | History | Current class / stages | Authority and repair/acceptance lead |
|---|---|---|---|
| accounts-balance-add | pre-existing | static_unknown; Y/Y/Y/Y/W | Field-write observer identity; spec0228 positive obligation. |
| cell-0005-wrapper-field-binding | new | propagation_unknown; Y/Y/U/Y/W | Located q.total arithmetic observer; actual overflow/preservation authority unproved. Safe limitation; explicit acceptance if deferred. |
| grid-delete-exact | pre-existing | static_unknown; Y/Y/Y/Y/W | Field-write observer identity; shared with spec0228 direct-field path. |
| grid-flow-field-opaque | new | infection_unknown; Y/U/Y/Y/Y | Opaque lookup(4/5) RHS is explicitly unsupported by case reasoning; safe unknown. Explicit acceptance if retained. |
| hex-from-hex-odd-variant | pre-existing | static_unknown; Y/Y/Y/Y/W | Feature-cfg/refused assertion plus unresolved error sink. gap_admission/spec0240; accept explicitly or authenticate execution/configuration. |
| shape-op-add-impl | pre-existing | static_unknown; Y/Y/Y/Y/W | Implicit Add dispatch unresolved. implicit_call/spec0240; prove nominal operands and exact impl before credit. |
| shape-refcell-interior-mutation | new | static_unknown; Y/Y/Y/Y/W | Local RefCell observer matched; workspace expansion/getter dispatch not closed. refcell_counter/builder_override; supply authority, not a bypass. |
| spec-b-0158-strip-prefix-absent | new | infection_unknown; Y/U/Y/Y/Y | Bounded strip_prefix None/default chain; spec0158#2 positive repair lead. Preserve per-call selected-arm identity. |
| spec0228-field-write-direct-read | pre-existing | static_unknown; Y/Y/Y/Y/W | Field-write→same-receiver direct read; spec0228#1 positive repair lead. |
| spec0228-field-write-enum-variant | pre-existing | static_unknown; Y/Y/Y/Y/W | Field-write→exact enum read; spec0228#6 positive repair lead. |
| spec0228-field-write-getter-read | pre-existing | static_unknown; Y/Y/Y/Y/W | Field-write→same-receiver getter read; spec0228#2 positive repair lead. |
| tokens-long-flag-strip-prefix | new | infection_unknown; Y/U/Y/Y/Y | Literal --ab/--a strip_prefix→len boundary; spec0158 bounded transfer positive repair lead. |

All five field-write cases share the final guard in classifier/evidence.rs: a field-assignment probe with weak discrimination and no assigned_field_weakly_read is withheld as static_unknown. Their raw probes already carry family field_construction, so this is not evidence that no probe was emitted. Follow assigned_field_of / assigned_field_weak_read_start in classify/activation.rs and the exact observer producer. The older problem narrative of proposed spec0228 describes pre-typed-probe behavior; use current retained native outputs for this source. The spec names product / analysis as owner and lists no linked issue; no individual assignee is inferred.

The value-transfer surface belongs to proposed spec0158, issue#3295, parent#3215, building on#3294. Its closed literal string families include strip_prefix and len, but arbitrary lookup calls remain unsupported. The three current predicate and return-value findings consistently preserve infection_unknown rather than letting the return probe override the boundary refusal. The RefCell, arithmetic, cfg and implicit-dispatch refusals are deliberate safety mechanisms. That explains why advice is withheld; it does not establish acceptance of the usefulness cost or prove those contracts cannot be implemented more precisely. No limitation is pre-approved here. Central retains source/integration ownership and should assign any disjoint work; these are surfaces, not claims of active ownership.

## Expected-row drift is separate

32 Rust rows differ from committed expectations; expected-row-drift.json contains every exact field delta. Thirteen verdict/outcome changes are the twelve above plus grid-iter-property (expected ideal/gap, actual abstained/limited; truth partially_discriminated, outside discriminated-truth missed-credit numerator). The other19 keep verdict/outcome:

- Eight remove exposed from exposed+weakly_exposed, leaving weakly_exposed: checkout-review-split-boundary-tests, grid-boundary-property, lib-rstest-named-cases-skip-boundary, shape-default-impl-one-field, spec0186-ex03-discarded-boundary-result, spec0186-ex04-dead-branch-argument, spec0226-gate-split-boundary-agreement, trap-iter-boundary-missing.
- Eleven remove propagation_unknown from infection_unknown+propagation_unknown, leaving infection_unknown: grid-flow-field-direct; macro-export-crate-call, macro-export-group-import, macro-export-path-boundary, macro-module-child-boundary, macro-module-child-far-rows, macro-module-import-boundary; spec-b-0158-dynamic-default, spec-b-0158-method-outside-families, spec-b-0158-non-identity-closure, spec-b-0158-shadowed-binding.

changed_since_labeling flags also differ in the retained exact delta list. These are derived comparisons to labeling observations, not new truth labels. TypeScript, Python and Perl expected rows match exactly (373/385/94 rows). No row was blessed. The same-key getter now matches its credited expectation and is not among the32.

## Authorized decision route

RIPR-SPEC-0219, Gate behavior: each missed-credit row needs its own sorted unique [[entry]] (case, reason) in fixtures/rust-verdict-corpus/missed-credit.toml. A whole-corpus check fails on any missing entry and on stale entries that no longer abstain; counts are derived, not a raised aggregate ceiling. A reviewed acceptance must name and justify each retained limitation; this packet does not grant it. An implementation repair should restore the existing discriminating fixture and remove any now-stale accepted entry. Separately, after substantive dispositions and review, the existing verdict-corpus bless route can refresh generated expected observations; a bless is not an accepted-cost decision and must not alter corpus truth to hide a regression. Existing selection/promotion holds remain.

Perl is independent: benchmarks/dx_scoreboard/README.md defines same-ID plus same-scoring-definition comparison and explicit failure for incomparable evidence. It records#10809's earlier producer migration, accepted four individual safe refusals via#10874 and follow-up#10897, but explicitly keeps aggregate abstention failing until#10525 repairs the producer relations or the owner accepts the rise as#7325 did for Rust. Our retained baseline diagnosis additionally found zero same-definition overlap for the current five projections, so re-admission needs its own attributable observation/provenance policy decision. Acceptance of four cases is not approval of aggregate rise or present baseline migration. Do not rewrite baseline values/provenance to manufacture comparability.

Live issue/assignee verification was attempted with gh issue view7490 and10809 on EffortlessMetrics/ripr-swarm; each returned GraphQL Forbidden, execution known failed. Those reads were stopped without retry or alternate route. Repository-native source/spec ownership and references above are verified; current assignees and latest live issue decisions are not established. Prior Actions proxy403 was not retried. No source, label, expectation, ledger, baseline, CI or release mutation was performed.
