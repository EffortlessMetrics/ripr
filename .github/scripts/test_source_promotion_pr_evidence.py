import copy
import unittest
import source_promotion_pr_evidence as target

HEAD = "a" * 40
BASE = "b" * 40

def fixture():
    return {"repository": target.REPOSITORY, "base_ref": "main", "state": "open", "draft": False,
            "head_sha": HEAD, "base_sha": BASE, "current_main_sha": BASE, "author": "writer",
            "reviews": [{"id": 1, "author": "reviewer", "state": "APPROVED", "commit_id": HEAD, "author_association": "MEMBER"}],
            "checks": [{"name": "Ripr Rust Small Result", "app_slug": "github-actions", "head_sha": HEAD, "status": "completed", "conclusion": "success"}]}

class EvidenceTests(unittest.TestCase):
    def test_synthetic_positive_is_only_structural(self):
        self.assertEqual(target.validate(fixture(), HEAD, BASE)["status"], "structurally_consistent")

    def test_missing_wrong_head_stale_and_failed_are_rejected(self):
        samples = []
        for field in ("reviews", "checks"):
            s = fixture(); s[field] = []; samples.append(s)
        for field in ("head_sha", "base_sha", "current_main_sha"):
            s = fixture(); s[field] = "c" * 40; samples.append(s)
        for field, value in (("commit_id", "c" * 40), ("author", "writer"), ("author_association", "NONE")):
            s = fixture(); s["reviews"][0][field] = value; samples.append(s)
        for field, value in (("head_sha", "c" * 40), ("status", "in_progress"), ("conclusion", "failure"), ("app_slug", "other")):
            s = fixture(); s["checks"][0][field] = value; samples.append(s)
        s = fixture(); s["checks"].append(copy.deepcopy(s["checks"][0])); samples.append(s)
        s = fixture(); s["reviews"].append({"id": 2, "author": "reviewer", "state": "CHANGES_REQUESTED", "commit_id": HEAD, "author_association": "MEMBER"}); samples.append(s)
        s = fixture(); s["reviews"].extend([{"id": 2, "author": "other", "state": "CHANGES_REQUESTED", "commit_id": HEAD, "author_association": "MEMBER"}, {"id": 3, "author": "other", "state": "COMMENTED", "commit_id": HEAD, "author_association": "MEMBER"}]); samples.append(s)
        for sample in samples:
            with self.subTest(sample=sample), self.assertRaises(ValueError):
                target.validate(sample, HEAD, BASE)

if __name__ == "__main__":
    unittest.main()
