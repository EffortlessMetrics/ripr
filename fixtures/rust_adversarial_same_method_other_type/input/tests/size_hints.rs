use rust_adversarial_same_method_other_type_fixture::Combinations;

#[test]
fn combinations_inexact_size_hints() {
    let it = Combinations { remaining: 3 };
    assert_eq!(it.size_hint().1, Some(3));
    assert_eq!(it.size_hint().0, 3);
    assert!(it.size_hint().1.is_some());
    assert_eq!(it.size_hint(), (3, Some(3)));
    assert!(it.size_hint().1 == Some(3));
}
