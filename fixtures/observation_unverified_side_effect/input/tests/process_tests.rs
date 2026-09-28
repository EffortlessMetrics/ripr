use observation_unverified_side_effect_fixture::{Notifier, process_order};

#[test]
fn process_order_succeeds() {
    let notifier = Notifier;
    let result = process_order(&notifier, "order-1");
    assert!(result);
}
