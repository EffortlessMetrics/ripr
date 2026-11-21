def build_model(name):
    return type(name, (), {"kind": "preview"})
