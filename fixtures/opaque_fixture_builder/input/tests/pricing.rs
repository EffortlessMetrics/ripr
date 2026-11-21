use opaque_fixture_builder_fixture::discounted_total;

struct DiscountCase {
    amount: i32,
    expected: i32,
}

fn discount_fixture_builder() -> DiscountCase {
    DiscountCase {
        amount: 50,
        expected: 50,
    }
}

#[test]
fn fixture_builder_case_has_no_discount() {
    let case = discount_fixture_builder();
    assert_eq!(discounted_total(case.amount), case.expected);
}
