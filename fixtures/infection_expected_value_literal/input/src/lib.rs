/// Tax in basis points for the given region code.
pub fn tax_bps(region: &str) -> u32 {
    match region {
        "EU" => 2000,
        _ => 0,
    }
}

/// Nominal weight in grams for a parcel kind.
pub fn parcel_weight(kind: &str) -> u32 {
    match kind {
        "crate" => 5000,
        _ => 500,
    }
}

/// Handling fee in cents for fragile parcels.
pub fn fragile_fee(weight_grams: u32) -> u32 {
    if weight_grams >= 2_000 {
        400
    } else {
        150
    }
}

/// Surcharge in cents for bulky parcels.
pub fn bulky_fee(volume_litres: u32) -> u32 {
    if volume_litres >= 50 {
        900
    } else {
        0
    }
}
