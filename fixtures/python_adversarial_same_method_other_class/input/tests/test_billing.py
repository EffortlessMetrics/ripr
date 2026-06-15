from src.billing import PaymentProcessor


def test_billing_validate():
    proc = PaymentProcessor()
    assert proc.validate("card1234 ") == True
