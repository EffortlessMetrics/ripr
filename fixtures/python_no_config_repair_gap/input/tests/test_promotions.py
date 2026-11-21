from promotions import trial_extension


def test_trial_extension_long_smoke():
    result = trial_extension(45, 30)
    assert result
