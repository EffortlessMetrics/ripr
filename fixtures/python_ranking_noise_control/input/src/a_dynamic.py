def call_named(client, method_name):
    return getattr(client, method_name)()
