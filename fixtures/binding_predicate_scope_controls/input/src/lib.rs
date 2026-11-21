pub fn sibling_a(input: &str) -> usize {
    let end = input.len();
    end
}

pub fn sibling_b(other: &str) -> bool {
    let end = other.len();
    end == 9
}

pub fn shadowed(flag: bool) -> bool {
    let end = 1;
    let adjusted = {
        let end = 2;
        end > 1
    };
    flag && adjusted
}

pub fn mutated(seed: usize) -> bool {
    let mut end = seed;
    end = end + 1;
    end == 2
}

pub fn documented(delim: char) -> usize {
    let end = delim.len_utf8();
    // end == start is only documentation.
    let label = "end == start";
    delim.len_utf8()
}

pub fn destructured(pair: (usize, usize)) -> bool {
    let (end, other) = pair;
    end == other
}
