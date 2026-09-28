use rust_catalog_description_structural_oracle::{
    build_catalog, default_check_description, entry_description_violations,
};

#[test]
fn catalog_with_single_consistent_entry_has_no_violations() {
    // Survivor shape (p1745 catalog-description row): reaches the changed
    // owner directly, and the changed wording flows into the asserted
    // catalog — but the STRONG exact oracle asserts structural consistency
    // (no violations), never the wording. The assertion shares a
    // probe-expression token (`catalog`), so the observation guard clears on
    // coincidence. A wording-only change passes this test: the row must not
    // promote once the specificity retry lands (RIPR-SPEC-0108 follow-up).
    let _ = default_check_description();
    let catalog = build_catalog();
    assert_eq!(
        entry_description_violations(&catalog),
        Vec::<String>::new()
    );
}
