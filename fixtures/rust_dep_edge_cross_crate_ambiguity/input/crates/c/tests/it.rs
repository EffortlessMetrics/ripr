use edge_a::score;
use edge_a::SCORE_BONUS;

#[test]
fn dependent_test_asserts_owner_score_value() {
    assert_eq!(score(7), 7 + SCORE_BONUS);
}
