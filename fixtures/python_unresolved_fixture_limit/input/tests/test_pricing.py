import pytest

from src.pricing import apply_discount


@pytest.fixture
def discount_case():
    return {"amount": 100, "threshold": 100, "expected": 90}


def test_apply_discount_fixture_case(discount_case):
    assert apply_discount(discount_case["amount"], discount_case["threshold"]) == discount_case[
        "expected"
    ]
