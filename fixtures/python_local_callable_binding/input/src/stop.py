class stop_after_attempt:
    def __init__(self, max_attempt_number):
        self.max_attempt_number = max_attempt_number

    def __call__(self, attempt_number):
        return attempt_number >= self.max_attempt_number
