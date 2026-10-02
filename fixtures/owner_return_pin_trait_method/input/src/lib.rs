#[derive(Debug, PartialEq, Eq)]
pub struct TryGetError {
    pub requested: usize,
    pub available: usize,
}

fn sign_extend(value: u64, nbytes: usize) -> i64 {
    let shift = (8 - nbytes) * 8;
    ((value << shift) as i64) >> shift
}

pub trait Buf {
    fn remaining(&self) -> usize;
    fn chunk(&self) -> &[u8];
    fn advance(&mut self, count: usize);

    fn try_get_uint(&mut self, nbytes: usize) -> Result<u64, TryGetError> {
        if self.remaining() < nbytes {
            return Err(TryGetError {
                requested: nbytes,
                available: self.remaining(),
            });
        }
        let mut value = 0u64;
        for byte in &self.chunk()[..nbytes] {
            value = (value << 8) | u64::from(*byte);
        }
        self.advance(nbytes);
        Ok(value)
    }

    fn try_get_int(&mut self, nbytes: usize) -> Result<i64, TryGetError> {
        Ok(sign_extend(self.try_get_uint(nbytes)?, nbytes))
    }
}

impl Buf for &[u8] {
    fn remaining(&self) -> usize {
        self.len()
    }

    fn chunk(&self) -> &[u8] {
        self
    }

    fn advance(&mut self, count: usize) {
        *self = &self[count..];
    }
}
