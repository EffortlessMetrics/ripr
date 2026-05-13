from external.client import remote_total

def total(client):
    return remote_total(client, include_tax=True)
