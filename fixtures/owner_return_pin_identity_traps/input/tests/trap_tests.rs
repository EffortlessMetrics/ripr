use owner_return_pin_identity_traps_fixture::{Fixed, HalfError, Reader, checked_half, decode};

#[test]
fn bare_decode_names_the_free_function() {
    assert_eq!(decode(8), 9);
}

#[test]
fn fixed_reader_overrides_next_word() {
    let mut reader = Fixed::default();
    assert_eq!(reader.next_word(), 7);
}

#[test]
fn negative_input_exits_early() {
    assert_eq!(checked_half(-4), Err(HalfError::Negative));
}

#[test]
fn local_scaled_binding_shadows_the_owner() {
    let scaled = |value: i32| value * 10;
    assert_eq!(scaled(3), 30);
}
