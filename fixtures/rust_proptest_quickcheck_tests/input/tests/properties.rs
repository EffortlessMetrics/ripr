use rust_proptest_quickcheck_tests::gate;

proptest! {
    #[test]
    fn gate_threshold(x in 0u32..100) {
        prop_assert_eq!(gate(x), x > 10);
    }

    fn unmarked_helper(x in 0u32..100) {
        let _ = gate(x);
    }
}

quickcheck! {
    fn qc_gate(x: u32) -> bool {
        gate(x) == (x > 10)
    }
}
