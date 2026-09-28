def send_alert(client, level):
    return client.post("/alerts", {"level": level, "priority": "critical"})
