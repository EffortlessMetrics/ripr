from src.invoice import Invoice


def test_mark_paid_sets_status():
    invoice = Invoice()
    invoice.mark_paid()
    assert invoice.status == "paid"
