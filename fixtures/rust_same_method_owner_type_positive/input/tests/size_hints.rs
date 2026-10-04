use rust_same_method_owner_type_positive_fixture::WhileSome;

#[test]
fn while_some_size_hint_upper_bound() {
    let it = WhileSome { remaining: 2 };
    assert_eq!(it.size_hint().1, Some(it.remaining));
}
