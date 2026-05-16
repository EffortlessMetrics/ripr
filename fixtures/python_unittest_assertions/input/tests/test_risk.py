import unittest

from src.risk import risk_score


class RiskScoreTests(unittest.TestCase):
    def test_high_risk_score(self):
        self.assertEqual(risk_score(100), "high")
