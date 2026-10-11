//! Bounded projection of a finding's observed values for machine output.
//!
//! A finding collects the values from every related test, and the related-test
//! heuristic is permissive: on a real workspace one finding can relate to
//! thousands of tests and carry more than ten thousand observed values. The
//! check JSON renders that list twice (`activation.observed_values` and the
//! promoted `observed_values`) plus one `assertion_texts` entry per line, so a
//! 500-line diff produced a 187 MB report. The renderers therefore project a
//! bounded subset and disclose the pre-cap count.
//!
//! The cap only affects rendering. Classification, missing discriminators and
//! every count computed from `finding.activation.observed_values` still use the
//! full vector.

use crate::domain::{ValueContext, ValueFact};

/// Describe a retained source value without claiming assertion execution or
/// admitted observation. ValueFact also represents refused assertion source
/// and bounded static value transfer; its presence is not an oracle witness.
pub(crate) fn source_value_evidence_line(fact: &ValueFact) -> String {
    format!(
        "source {} value {} at line {}",
        fact.context.as_str().replace('_', " "),
        fact.value,
        fact.line
    )
}

/// Cap on observed values rendered per finding in check JSON and SARIF.
/// The pre-cap count is disclosed as `observed_values_total` whenever the cap
/// drops a value. Mirrors `MAX_RELATED_TESTS_PER_FINDING_JSON`.
pub(crate) const MAX_OBSERVED_VALUES_PER_FINDING: usize = 32;

/// The observed values a renderer should emit, in their original order.
///
/// Under the cap this is every value, unchanged. Over the cap it keeps the
/// values that say most about the inputs the tests use (call arguments, table
/// rows, builder calls, enum variants, returns) ahead of bare assertion
/// arguments, then restores the original order so the output stays stable
/// and reads like the uncapped list.
pub(crate) fn bounded_observed_values(facts: &[ValueFact]) -> Vec<&ValueFact> {
    if facts.len() <= MAX_OBSERVED_VALUES_PER_FINDING {
        return facts.iter().collect();
    }
    // After each input, this prefix holds the best min(seen, 32) distinct
    // ordinals, sorted by the legacy (context rank, original ordinal) key.
    // Inserting a better key and dropping the worst preserves that invariant.
    // Only selector scratch is bounded here; the full facts and renderer
    // allocations remain outside this fixed 32-usize array.
    let mut ranked = [0usize; MAX_OBSERVED_VALUES_PER_FINDING];
    let mut retained = 0;
    for (index, fact) in facts.iter().enumerate() {
        let key = (context_rank(&fact.context), index);
        let position = ranked[..retained]
            .partition_point(|&ordinal| (context_rank(&facts[ordinal].context), ordinal) < key);
        if position == MAX_OBSERVED_VALUES_PER_FINDING {
            continue;
        }
        let end = retained.min(MAX_OBSERVED_VALUES_PER_FINDING - 1);
        ranked.copy_within(position..end, position + 1);
        ranked[position] = index;
        retained = (retained + 1).min(MAX_OBSERVED_VALUES_PER_FINDING);
    }
    ranked.sort_unstable();
    ranked.into_iter().map(|index| &facts[index]).collect()
}

/// The pre-cap count to disclose, or `None` when nothing was dropped.
pub(crate) fn elided_observed_values_total(facts: &[ValueFact]) -> Option<usize> {
    (facts.len() > MAX_OBSERVED_VALUES_PER_FINDING).then_some(facts.len())
}

fn context_rank(context: &ValueContext) -> u8 {
    match context {
        ValueContext::FunctionArgument => 0,
        ValueContext::TableRow => 1,
        ValueContext::BuilderMethod => 2,
        ValueContext::EnumVariant => 3,
        ValueContext::ReturnValue => 4,
        ValueContext::AssertionArgument => 5,
        ValueContext::Unknown => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(line: usize, context: ValueContext) -> ValueFact {
        ValueFact {
            line,
            text: format!("assert_eq!(f({line}), {line});"),
            value: line.to_string(),
            context,
        }
    }

    #[test]
    fn under_the_cap_every_value_is_kept_in_order() {
        let facts = (1..=MAX_OBSERVED_VALUES_PER_FINDING)
            .map(|line| fact(line, ValueContext::AssertionArgument))
            .collect::<Vec<_>>();

        let bounded = bounded_observed_values(&facts);

        assert_eq!(bounded, facts.iter().collect::<Vec<_>>());
        assert_eq!(elided_observed_values_total(&facts), None);
    }

    #[test]
    fn over_the_cap_input_values_outrank_assertion_arguments_and_keep_order() {
        // 40 assertion arguments first, then 3 call arguments at later lines.
        let mut facts = (1..=40)
            .map(|line| fact(line, ValueContext::AssertionArgument))
            .collect::<Vec<_>>();
        facts.extend([
            fact(50, ValueContext::FunctionArgument),
            fact(51, ValueContext::TableRow),
            fact(52, ValueContext::FunctionArgument),
        ]);

        let bounded = bounded_observed_values(&facts);

        assert_eq!(bounded.len(), MAX_OBSERVED_VALUES_PER_FINDING);
        assert_eq!(elided_observed_values_total(&facts), Some(43));
        // The three input values survive although they come last.
        let lines = bounded.iter().map(|fact| fact.line).collect::<Vec<_>>();
        assert_eq!(&lines[lines.len() - 3..], &[50, 51, 52]);
        // The rest are the earliest assertion arguments, still in source order.
        assert_eq!(
            &lines[..MAX_OBSERVED_VALUES_PER_FINDING - 3],
            (1..=29).collect::<Vec<_>>().as_slice()
        );
    }

    // Retain the full-sort algorithm and comparator from baseline blob
    // 2fdb84596c655f7a85d65325fdc0904747ea5363 independently of production.
    // Literal 32 and the separate rank table also catch cap/ranking mutants.
    fn legacy_full_sort(facts: &[ValueFact]) -> Vec<&ValueFact> {
        if facts.len() <= 32 {
            return facts.iter().collect();
        }
        let mut ranked = facts.iter().enumerate().collect::<Vec<_>>();
        ranked.sort_by_key(|(index, fact)| (legacy_context_rank(&fact.context), *index));
        ranked.truncate(32);
        ranked.sort_by_key(|(index, _)| *index);
        ranked.into_iter().map(|(_, fact)| fact).collect()
    }

    fn legacy_context_rank(context: &ValueContext) -> u8 {
        match context {
            ValueContext::FunctionArgument => 0,
            ValueContext::TableRow => 1,
            ValueContext::BuilderMethod => 2,
            ValueContext::EnumVariant => 3,
            ValueContext::ReturnValue => 4,
            ValueContext::AssertionArgument => 5,
            ValueContext::Unknown => 6,
        }
    }

    fn all_contexts() -> [ValueContext; 7] {
        [
            ValueContext::FunctionArgument,
            ValueContext::TableRow,
            ValueContext::BuilderMethod,
            ValueContext::EnumVariant,
            ValueContext::ReturnValue,
            ValueContext::AssertionArgument,
            ValueContext::Unknown,
        ]
    }

    fn same_objects_in_order(left: &[&ValueFact], right: &[&ValueFact]) -> bool {
        left.len() == right.len() && left.iter().zip(right).all(|(a, b)| std::ptr::eq(*a, *b))
    }

    fn assert_legacy_equivalent(facts: &[ValueFact]) {
        let original = facts.to_vec();
        let expected = legacy_full_sort(facts);
        let actual = bounded_observed_values(facts);
        assert_eq!(actual.len(), facts.len().min(32));
        assert!(same_objects_in_order(&actual, &expected));
        assert_eq!(facts, original.as_slice());
        assert_eq!(
            elided_observed_values_total(facts),
            (facts.len() > 32).then_some(facts.len())
        );
    }

    #[test]
    fn legacy_equivalence_at_cap_boundaries_and_large_inputs() {
        for len in [0, 1, 31, 32, 33, 65, 5001, 10017] {
            for context in all_contexts() {
                let facts = (0..len)
                    .map(|ordinal| fact(len - ordinal, context.clone()))
                    .collect::<Vec<_>>();
                assert_legacy_equivalent(&facts);
            }
        }
        // A scan cutoff beyond the small cases must still lose to late inputs.
        let mut late_best = (0..6000)
            .map(|ordinal| fact(ordinal, ValueContext::Unknown))
            .collect::<Vec<_>>();
        late_best.extend((6000..6033).map(|ordinal| fact(ordinal, ValueContext::FunctionArgument)));
        assert_legacy_equivalent(&late_best);
    }

    #[test]
    fn every_context_tier_and_adversarial_input_order_match_legacy() {
        let contexts = all_contexts();
        for cutoff_rank in 0..contexts.len() {
            let mut facts = Vec::new();
            for rank in (0..contexts.len()).rev() {
                let count = if rank < cutoff_rank {
                    3
                } else if rank == cutoff_rank {
                    40
                } else {
                    41
                };
                for _ in 0..count {
                    facts.push(fact(1000 - facts.len(), contexts[rank].clone()));
                }
            }
            // Worst contexts arrive first; every tier competes at the cutoff.
            assert_legacy_equivalent(&facts);
            facts.reverse();
            assert_legacy_equivalent(&facts);
            facts.rotate_left(31);
            assert_legacy_equivalent(&facts);
            facts.rotate_right(32);
            assert_legacy_equivalent(&facts);
        }
    }

    #[test]
    fn same_line_provenance_and_equal_content_distinct_objects_are_preserved() {
        let mut facts = (0..96)
            .map(|ordinal| ValueFact {
                line: 7,
                text: format!("different source {ordinal}"),
                value: "same value".to_owned(),
                context: ValueContext::FunctionArgument,
            })
            .collect::<Vec<_>>();
        assert_legacy_equivalent(&facts);
        facts.fill(fact(7, ValueContext::FunctionArgument));
        // Equal contents are separate input objects: value equality alone
        // would miss a replacement, deduplication or reconstructed reference.
        assert_legacy_equivalent(&facts);
    }

    #[test]
    fn exhaustive_four_context_patterns_match_legacy() {
        let contexts = all_contexts();
        for pattern in 0..7usize.pow(4) {
            let mut digits = [0; 4];
            let mut remaining = pattern;
            for digit in &mut digits {
                *digit = remaining % 7;
                remaining /= 7;
            }
            let mut facts = (0..33)
                .map(|ordinal| fact(ordinal % 5, contexts[digits[ordinal % 4]].clone()))
                .collect::<Vec<_>>();
            assert_legacy_equivalent(&facts);
            facts.reverse();
            assert_legacy_equivalent(&facts);
        }
    }

    #[test]
    fn generated_lengths_contexts_and_provenance_match_legacy() {
        let contexts = all_contexts();
        let mut state = 0x6a09_e667_f3bc_c909u64;
        for case in 0..256 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let len = if case >= 252 {
                [31, 32, 33, 6001][case - 252]
            } else {
                (state % 129) as usize
            };
            let mut facts = (0..len)
                .map(|ordinal| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    ValueFact {
                        line: (state % 13) as usize,
                        text: format!("source {}", ordinal % 9),
                        value: (state % 5).to_string(),
                        context: contexts[(state % 7) as usize].clone(),
                    }
                })
                .collect::<Vec<_>>();
            assert_legacy_equivalent(&facts);
            facts.reverse();
            assert_legacy_equivalent(&facts);
        }
    }

    #[test]
    fn oracle_rejects_truncation_rank_order_late_ties_and_line_dedup_mutants() {
        let mut late_best = (0..32)
            .map(|ordinal| fact(ordinal, ValueContext::Unknown))
            .collect::<Vec<_>>();
        late_best.extend([
            fact(32, ValueContext::FunctionArgument),
            fact(33, ValueContext::FunctionArgument),
        ]);
        let expected = legacy_full_sort(&late_best);
        let truncated = late_best.iter().take(32).collect::<Vec<_>>();
        assert!(!same_objects_in_order(&truncated, &expected));
        let mut rank_order = expected.clone();
        rank_order.sort_by_key(|fact| legacy_context_rank(&fact.context));
        assert!(!same_objects_in_order(&rank_order, &expected));
        assert_legacy_equivalent(&late_best);

        let ties = vec![fact(7, ValueContext::TableRow); 64];
        let expected = legacy_full_sort(&ties);
        let late_ties = ties.iter().skip(32).collect::<Vec<_>>();
        assert!(!same_objects_in_order(&late_ties, &expected));
        let mut deduplicated = expected.clone();
        deduplicated.dedup_by_key(|fact| fact.line);
        assert!(!same_objects_in_order(&deduplicated, &expected));
        assert_legacy_equivalent(&ties);
    }
}
