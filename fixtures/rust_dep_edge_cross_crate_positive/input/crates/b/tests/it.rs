use edge_b::score;

#[test]
fn beta_smoke_calls_own_score() {
    let _ = score(9);
}
