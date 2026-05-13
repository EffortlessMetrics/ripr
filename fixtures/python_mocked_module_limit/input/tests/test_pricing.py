from unittest.mock import Mock


def test_apply_discount_with_mock():
    service = Mock()
    assert apply_discount(100) == 90
