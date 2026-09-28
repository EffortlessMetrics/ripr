class PaymentProcessor:
    def validate(self, card):
        return len(card.strip()) == 9
