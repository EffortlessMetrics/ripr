from src.plugin_loader import run_plugin


def test_run_plugin_returns_payload_status():
    assert run_plugin("plugins.invoice", {"id": "inv_123"}) == "ok"
