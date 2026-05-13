class DiscountPolicy:
    def apply(self, amount, threshold):
        if amount >= threshold:
            return amount - 10
        return amount

    @staticmethod
    def normalize(amount):
        return max(amount, 0)

    @classmethod
    def from_config(cls, config):
        return cls()
