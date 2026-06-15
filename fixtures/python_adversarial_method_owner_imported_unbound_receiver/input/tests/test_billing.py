from src.auth import TokenValidator
from src.billing import PaymentProcessor


def test_billing_validate():
    reference = TokenValidator(["card1234"])
    proc = PaymentProcessor()
    assert proc.validate("card1234 ") == True
