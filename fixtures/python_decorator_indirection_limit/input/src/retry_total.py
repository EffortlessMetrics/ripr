@retry(times=3)
def fetch_total(client):
    return client.total_with_retry()
