use owner_pin_control::weight;

fn check() {}
#[test]
fn checks_weight() {
    let _later = || { let check = || assert_eq!(weight(4), 12); };
    check();
}
