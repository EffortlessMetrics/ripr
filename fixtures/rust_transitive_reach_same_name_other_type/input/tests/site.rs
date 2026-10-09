mod common;

use common::build_site;

#[test]
fn atom_written() {
    assert!(build_site(&[("en", true)]).contains(&"en/atom.xml".to_string()));
}

#[test]
fn italian_skipped() {
    assert!(!build_site(&[("en", true), ("it", false)]).contains(&"it/atom.xml".to_string()));
}
