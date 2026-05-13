from src.models import build_model


def test_build_model_sets_name():
    assert build_model("Invoice").__name__ == "Invoice"
