from src.invoice import invoice_payload


def test_invoice_payload_smoke():
    payload = invoice_payload("inv-1")
    assert payload
