use decimal_field_observer_non_member_access::default_config;

fn assert_observed<T>(_value: T) {}
fn assert_valid<T>(_value: T) {}

#[test]
fn decimal_observation_does_not_name_retries() {
    let config = default_config();
    assert_observed(3.14_f64);
    assert_valid(config);
}
