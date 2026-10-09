use owner_return_pin_trait_method_fixture::{Buf, TryGetError};

#[test]
fn try_get_int_sign_extends() {
    let mut a = &[0xff, 0xff, 0xff][..];
    assert_eq!(a.try_get_int(3), Ok(-1));
    assert_eq!(a.remaining(), 0);
}

#[test]
fn try_get_int_reports_short_input() {
    let mut short = &[0x01, 0x02, 0x03][..];
    assert_eq!(
        short.try_get_int(4),
        Err(TryGetError {
            requested: 4,
            available: 3
        })
    );
}
