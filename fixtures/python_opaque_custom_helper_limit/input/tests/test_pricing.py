from src.pricing import apply_discount


def assert_discounted(result):
    assert result < 100


def test_apply_discount_custom_helper():
    result = apply_discount(100, 50)
    assert_discounted(result)
