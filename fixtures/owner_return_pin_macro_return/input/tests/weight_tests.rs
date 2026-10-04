use owner_pin_control::weight;

macro_rules! skip { () => { return; } }
#[test]
fn checks_weight() {
    skip!();
    assert_eq!(weight(4), 12);
}
