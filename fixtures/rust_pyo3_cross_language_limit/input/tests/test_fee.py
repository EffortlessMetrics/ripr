import feecalc

def test_fee():
    assert feecalc.fee(150) == 5
