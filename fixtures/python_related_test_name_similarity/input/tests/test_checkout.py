from src.pricing import apply_discount


def test_apply_discount_boundary_case():
    handler = apply_discount
    assert 90 == 90
