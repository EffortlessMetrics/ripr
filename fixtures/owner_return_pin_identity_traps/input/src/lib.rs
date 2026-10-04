// An associated function next to a same-named free function: a bare
// `decode(..)` call names the free function, never `Codec::decode`.
pub struct Codec;

impl Codec {
    pub fn decode(input: u32) -> u32 {
        input.rotate_left(3)
    }
}

pub fn decode(input: u32) -> u32 {
    input + 1
}

// A trait default method that the receiver's impl overrides: `Fixed`
// never runs the default body.
pub trait Reader {
    fn base(&self) -> u32;

    fn next_word(&mut self) -> u32 {
        self.base() * 3
    }
}

#[derive(Default)]
pub struct Fixed;

impl Reader for Fixed {
    fn base(&self) -> u32 {
        1
    }

    fn next_word(&mut self) -> u32 {
        7
    }
}

// An input that takes the early-exit path: `checked_half(-4)` returns
// through `?` and never builds the changed `Ok(..)`.
#[derive(Debug, PartialEq, Eq)]
pub enum HalfError {
    Negative,
}

fn validate(x: i32) -> Result<i32, HalfError> {
    if x < 0 { Err(HalfError::Negative) } else { Ok(x) }
}

pub fn checked_half(x: i32) -> Result<i32, HalfError> {
    Ok(validate(x)? >> 1)
}

// A test that binds the owner's name locally calls its own binding.
pub fn scaled(x: i32) -> i32 {
    x * 10
}
