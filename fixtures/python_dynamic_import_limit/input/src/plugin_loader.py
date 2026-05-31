import importlib

def run_plugin(module_name, payload):
    return importlib.import_module(module_name).handle(payload, strict=True)
