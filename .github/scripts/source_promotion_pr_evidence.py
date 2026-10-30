"""Structural checks for a source-promotion PR evidence snapshot.

Inputs are untrusted data. Passing this function is not live authentication,
repository policy discovery, a required-check verdict, or merge authorization.
"""

REPOSITORY = "EffortlessMetrics/ripr"
REQUIRED_RESULTS = (("Ripr Rust Small Result", "github-actions"),)


def validate(snapshot, expected_head, expected_base):
    for sha in (expected_head, expected_base):
        if not isinstance(sha, str) or len(sha) != 40 or any(c not in "0123456789abcdef" for c in sha):
            raise ValueError("expected head and base must be full lowercase commit IDs")
    if not isinstance(snapshot, dict):
        raise ValueError("missing PR evidence snapshot")
    if snapshot.get("repository") != REPOSITORY or snapshot.get("base_ref") != "main":
        raise ValueError("repository or base ref mismatch")
    if snapshot.get("state") != "open" or snapshot.get("draft") is not False:
        raise ValueError("PR is not open and ready")
    if snapshot.get("head_sha") != expected_head or snapshot.get("base_sha") != expected_base:
        raise ValueError("wrong PR subject")
    if snapshot.get("current_main_sha") != expected_base:
        raise ValueError("stale source main")
    author = snapshot.get("author")
    if not isinstance(author, str) or not author:
        raise ValueError("missing PR author")
    reviews = snapshot.get("reviews")
    if not isinstance(reviews, list):
        raise ValueError("missing review evidence")
    latest = {}
    seen_ids = set()
    for review in reviews:
        if not isinstance(review, dict):
            raise ValueError("malformed review")
        reviewer = review.get("author")
        number = review.get("id")
        if not isinstance(reviewer, str) or not reviewer or not isinstance(number, int) or isinstance(number, bool) or number <= 0:
            raise ValueError("missing review identity")
        if number in seen_ids:
            raise ValueError("duplicate review identity")
        seen_ids.add(number)
        state = review.get("state")
        if state not in ("APPROVED", "CHANGES_REQUESTED", "DISMISSED", "COMMENTED"):
            raise ValueError("unknown review state")
        if state == "COMMENTED":
            continue
        key = reviewer.casefold()
        if key not in latest or number > latest[key]["id"]:
            latest[key] = review
    if any(r.get("state") == "CHANGES_REQUESTED" for r in latest.values()):
        raise ValueError("unresolved requested changes")
    if not any(r.get("state") == "APPROVED" and r.get("commit_id") == expected_head
               and r.get("author").casefold() != author.casefold()
               and r.get("author_association") in ("OWNER", "MEMBER", "COLLABORATOR")
               for r in latest.values()):
        raise ValueError("missing independent exact-head review")
    checks = snapshot.get("checks")
    if not isinstance(checks, list):
        raise ValueError("missing required-result evidence")
    for name, app in REQUIRED_RESULTS:
        matching = [c for c in checks if isinstance(c, dict) and c.get("name") == name and c.get("app_slug") == app]
        if len(matching) != 1:
            raise ValueError("required result missing or ambiguous")
        check = matching[0]
        if check.get("head_sha") != expected_head or check.get("status") != "completed" or check.get("conclusion") != "success":
            raise ValueError("required result wrong-head, incomplete or failed")
    return {"status": "structurally_consistent", "head_sha": expected_head,
            "base_sha": expected_base, "claim": "Untrusted snapshot only; live authentication and admission remain outstanding."}
