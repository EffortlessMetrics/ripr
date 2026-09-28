mod internal;

pub fn outer(a: i32, b: i32) -> i32 {
    internal::inner(a, b)
}
// #3296 control: a second function named `inner` makes the callee
// name non-unique, so the typed helper transfer must refuse this
// chain and the RIPR-SPEC-0114 lexical-walk limitation stays the
// pinned outcome (the corpus case's contract).
pub fn inner(a: i32, b: i32) -> i32 {
    a + b
}
