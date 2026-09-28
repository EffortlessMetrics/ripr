use parcels::{bulky_fee, fragile_fee, parcel_weight, tax_bps};

#[test]
fn heavy_crate_pays_fragile_fee_and_eu_tax() {
    let weight = parcel_weight("crate");
    assert_eq!(fragile_fee(weight), 400);
    assert_eq!(tax_bps("EU"), 2000);
}

#[test]
fn bulky_fee_starts_at_fifty_litres() {
    let volume = 50;
    assert_eq!(bulky_fee(volume), 900);
}
