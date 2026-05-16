import unittest

from src.risk import risk_score


class RiskScoreTests(unittest.TestCase):
    def test_risk_score_high(self):
        value = risk_score(120)
        self.assertTrue(value)
