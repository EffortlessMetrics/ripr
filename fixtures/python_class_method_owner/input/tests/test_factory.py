from src.factory import PriceRule


def test_price_rule_from_config():
    assert PriceRule.from_config({"discount": 4}) is not None
