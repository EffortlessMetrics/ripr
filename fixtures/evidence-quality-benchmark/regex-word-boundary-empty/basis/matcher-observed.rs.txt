use regex::Regex;
fn main() {
    let wb = Regex::new(r"\b").unwrap();
    let notwb = Regex::new(r"\B").unwrap();
    let wb_empty = wb.is_match("");
    let notwb_empty = notwb.is_match("");
    let wb_a: Vec<_> = wb.find_iter("a").map(|m| m.range()).collect();
    let notwb_a: Vec<_> = notwb.find_iter("a").map(|m| m.range()).collect();
    println!("wb_empty={:?}; notwb_empty={:?}; wb_a={:?}; notwb_a={:?}", wb_empty, notwb_empty, wb_a, notwb_a);
    assert!(!wb_empty);
    assert!(notwb_empty);
    assert_eq!(vec![0..0, 1..1], wb_a);
    assert!(notwb_a.is_empty());
    println!("matcher_witness: 4 assertions passed");
}
