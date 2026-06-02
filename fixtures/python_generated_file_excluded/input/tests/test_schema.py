from src.schema_pb2 import encode_status


def test_encode_status():
    assert encode_status("paid")["status"] == "paid"
