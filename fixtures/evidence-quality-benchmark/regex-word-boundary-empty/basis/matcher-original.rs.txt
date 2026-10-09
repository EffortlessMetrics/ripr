use regex::Regex;

fn main() {
    let wb = Regex::new(r"\b").unwrap();
    let notwb = Regex::new(r"\B").unwrap();
    
    assert!(!wb.is_match(""));
    assert!(notwb.is_match(""));
    
    let got: Vec<_> = wb.find_iter("a").map(|m| m.range()).collect();
    assert_eq!(vec![0..0, 1..1], got);
    
    let got: Vec<_> = notwb.find_iter("a").map(|m| m.range()).collect();
    assert!(got.is_empty());
}
