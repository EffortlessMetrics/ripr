from src.checkout import checkout_payload


def test_checkout_payload_smoke():
    payload = checkout_payload("ord-1")
    assert payload

