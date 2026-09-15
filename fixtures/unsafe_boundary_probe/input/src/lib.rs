pub fn sum_bytes(pointer: *const u8, len: usize) -> u8 {
    let mut total: u8 = 0;
    unsafe {
        for offset in 0..len {
            total = total.wrapping_add(*pointer.add(offset));
        }
    }
    if total == 0 {
        0
    } else {
        total
    }
}

pub fn read_first(pointer: *const u8) -> u8 {
    let value = unsafe { *pointer };
    value
}

pub fn first_or_zero(input: &[u8]) -> u8 {
    let first = input.first();
    match first {
        Some(value) => *value,
        None => 0,
    }
}
