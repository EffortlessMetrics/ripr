use owner_pin_control::weight;

#[test]
fn checks_weight() {
    let check = || assert_eq!(weight(4), 12);
    check();
}
