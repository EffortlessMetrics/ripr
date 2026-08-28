use rust_missing_discriminator_evidence_fixture::classify_boundary;

#[test]
fn rejects_boundary() {
    assert!(classify_boundary(100).is_err());
}
