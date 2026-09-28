from src.models import InvoiceRecord


def test_invoice_record_status():
    assert InvoiceRecord.status == "paid"
