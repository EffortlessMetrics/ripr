use std::io::{self, Read};

pub fn read_all<R: Read>(mut rdr: R) -> io::Result<usize> {
    let mut buf = [0_u8; 32];
    let n = rdr.read(&mut buf)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("closed"))
        }
    }

    #[test]
    fn reports_read_failure() {
        let rdr = BrokenReader;
        let check = || { assert_eq!(read_all(rdr).unwrap_err().kind(), io::ErrorKind::Other); };
        check();
    }
}
