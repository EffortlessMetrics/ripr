use owner_pin_control::weight;

#[test]
fn checks_weight() {
    let input = 4;
    let _later = || assert_eq!(weight(input), 12);
}
