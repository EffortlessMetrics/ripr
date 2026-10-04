use owner_pin_control::weight;

#[test]
fn checks_weight() {
    assert_eq!(weight({ return; 4 }), 12);
}
