from src.policy import DiscountPolicy


def test_discount_policy_apply():
    policy = DiscountPolicy()
    policy.apply(100, 50)
