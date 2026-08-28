from src.account import Account


def test_account_init():
    account = Account(100)
    assert account._balance == 100
