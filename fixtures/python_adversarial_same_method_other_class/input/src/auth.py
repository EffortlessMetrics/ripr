class TokenValidator:
    def __init__(self, valid):
        self._valid = valid

    def validate(self, token):
        return token.strip() in self._valid
