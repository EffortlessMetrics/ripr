class Invoice:
    def __init__(self):
        self.status = "open"

    def mark_paid(self):
        self.status = "paid"
