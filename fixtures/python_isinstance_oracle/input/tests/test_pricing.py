from src.pricing import apply_discount


def test_apply_discount_type_shape():
    assert isinstance(apply_discount(100), int)
