use owner_pin_control::weight;

#[test]
fn checks_weight() {
    let _unused = || assert_eq!(weight(4), 12);
}
