from src.price import final_price


def test_final_price_discount():
    assert final_price(100) == 88
