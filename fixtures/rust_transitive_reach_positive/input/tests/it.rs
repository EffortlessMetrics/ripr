use rust_transitive_reach_positive_fixture::outer;

#[test]
fn test_outer_returns_difference() {
    let result = outer(10, 3);
    assert_eq!(result, 7);
}