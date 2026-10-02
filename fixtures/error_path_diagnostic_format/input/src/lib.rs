use std::io::{self, Read};

pub fn read_all<R: Read>(mut rdr: R) -> io::Result<usize> {
    let mut buf = [0_u8; 32];
    let n = rdr.read(&mut buf)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_successfully() {
        let rdr: &[u8] = b"homer lisa";
        let read_error = 42;
        let got = read_all(rdr);
        assert_eq!(got.ok(), Some(10));
        assert_eq!(rdr.len(), 10, "{}", read_error);
    }
}
