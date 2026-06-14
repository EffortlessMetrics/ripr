use observation_verified_side_effect_fixture::{Notifier, process_order};

#[test]
fn process_order_sends_exact_order_id() {
    let notifier = Notifier::new();
    process_order(&notifier, "order-42");
    assert_eq!(*notifier.sent.borrow(), vec!["order-42".to_string()]);
}
