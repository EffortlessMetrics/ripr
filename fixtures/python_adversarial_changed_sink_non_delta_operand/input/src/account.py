class Account:
    def __init__(self, balance):
        self._balance = balance

    @property
    def balance(self):
        return max(0, self._balance)
