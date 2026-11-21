use harness_dead_fixture::{limit_exceeds_default, parse_limit};
use libtest_mimic::Trial;

// Dead construction (#3604/#3636): trials collected in an unused helper
// that is never passed to a harness run entry point. The registration is
// real and the subject facts exist, but no executable test fact joins
// the denominator, so these oracles must not credit exposure.
fn dead_trials() -> Vec<Trial> {
    vec![
        Trial::test("parses_limit", || {
            assert_eq!(parse_limit("8080"), 8080);
        }),
        Trial::test("limit_exceeds", || {
            assert!(limit_exceeds_default("8080"));
        }),
    ]
}
