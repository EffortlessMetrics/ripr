"""Read non-administrative PR evidence; output is not promotion admission.

The source-owned contract observes one routed check and an independent approval.
It does not establish the full required policy or the trust of a workflow revision.
"""
import json
import re
import selectors
import subprocess
import sys
import time

from source_promotion_pr_evidence import REPOSITORY, validate

MAX_PAGES = 10
MAX_BYTES = 4 * 1024 * 1024
WORKFLOW = ".github/workflows/routed-rust.yml"
PREFIX = f"repos/{REPOSITORY}/"


def read_api(path):
    if not path.startswith(PREFIX):
        raise ValueError("API path outside source repository")
    try:
        child = subprocess.Popen(["gh", "api", "--hostname", "github.com", path],
                                 stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                 stderr=subprocess.DEVNULL)
    except OSError as error:
        raise ValueError("ordinary GitHub evidence read unavailable") from error
    data = bytearray()
    until = time.monotonic() + 30
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ)
            while True:
                remaining = until - time.monotonic()
                if remaining <= 0:
                    raise ValueError("GitHub evidence read timed out")
                if not selector.select(remaining):
                    raise ValueError("GitHub evidence read timed out")
                chunk = child.stdout.read1(min(65536, MAX_BYTES + 1 - len(data)))
                if not chunk:
                    break
                data.extend(chunk)
                if len(data) > MAX_BYTES:
                    raise ValueError("GitHub evidence response exceeds byte budget")
        if child.wait(timeout=max(0.001, until - time.monotonic())) != 0:
            raise ValueError("ordinary GitHub evidence read unavailable")
    except subprocess.TimeoutExpired as error:
        raise ValueError("GitHub evidence read timed out") from error
    finally:
        if child.poll() is None:
            child.kill()
        child.wait()
        child.stdout.close()
    try:
        return json.loads(data)
    except (ValueError, UnicodeDecodeError) as error:
        raise ValueError("malformed GitHub evidence response") from error


def pages(reader, path, key=None):
    collected = []
    total = None
    for number in range(1, MAX_PAGES + 1):
        page = reader(f"{path}?per_page=100&page={number}")
        if key is not None:
            if not isinstance(page, dict) or not isinstance(page.get("total_count"), int):
                raise ValueError("missing result denominator")
            if total is None:
                total = page["total_count"]
            elif total != page["total_count"]:
                raise ValueError("result denominator changed during pagination")
            page = page.get(key)
        if not isinstance(page, list) or len(page) > 100:
            raise ValueError("malformed evidence page")
        collected.extend(page)
        if len(page) < 100:
            if total is not None and len(collected) != total:
                raise ValueError("incomplete result pagination")
            return collected
    raise ValueError("evidence pagination exceeds budget")


def observe(reader, number, head, base):
    pr = reader(f"{PREFIX}pulls/{number}")
    main = reader(f"{PREFIX}git/ref/heads/main")
    if not isinstance(pr, dict) or not isinstance(main, dict):
        raise ValueError("missing current PR or main")
    try:
        if pr["number"] != number or pr["base"]["repo"]["full_name"] != REPOSITORY or pr["head"]["repo"]["full_name"] != REPOSITORY:
            raise ValueError("PR repository identity mismatch")
        reviews = pages(reader, f"{PREFIX}pulls/{number}/reviews")
        checks = pages(reader, f"{PREFIX}commits/{head}/check-runs", "check_runs")
        normalized_reviews = []
        previous = ("", 0)
        for review in reviews:
            stamp = review["submitted_at"]
            ident = review["id"]
            if not isinstance(stamp, str) or not re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ", stamp) or not isinstance(ident, int) or isinstance(ident, bool) or (stamp, ident) <= previous or ident <= previous[1]:
                raise ValueError("review order or submission time unavailable")
            previous = stamp, ident
            if review["state"] == "DISMISSED":
                raise ValueError("dismissal timing requires independent authority")
            normalized_reviews.append({"id": ident, "author": review["user"]["login"], "state": review["state"],
                                       "commit_id": review["commit_id"], "author_association": review["author_association"]})
        selected = [c for c in checks if c.get("name") == "Ripr Rust Small Result" and c.get("app", {}).get("slug") == "github-actions" and c.get("app", {}).get("id") == 15368]
        if len(selected) != 1:
            raise ValueError("routed result missing or ambiguous")
        check = selected[0]
        url = check["details_url"]
        match = re.fullmatch(r"https://github\.com/EffortlessMetrics/ripr/actions/runs/([1-9][0-9]*)/job/([1-9][0-9]*)", url or "")
        if not match:
            raise ValueError("result lacks exact workflow job identity")
        run_id, job_id = map(int, match.groups())
        job = reader(f"{PREFIX}actions/jobs/{job_id}")
        run = reader(f"{PREFIX}actions/runs/{run_id}")
        if not isinstance(run.get("workflow_id"), int) or isinstance(run["workflow_id"], bool) or run["workflow_id"] <= 0:
            raise ValueError("workflow identity unavailable")
        workflow = reader(f"{PREFIX}actions/workflows/{run['workflow_id']}")
        prs = run.get("pull_requests")
        if (not isinstance(prs, list) or len(prs) != 1 or not isinstance(prs[0], dict)
                or prs[0].get("number") != number or prs[0].get("head", {}).get("sha") != head
                or prs[0].get("base", {}).get("sha") != base
                or prs[0].get("base", {}).get("ref") != "main"
                or prs[0].get("base", {}).get("repo", {}).get("url") != f"https://api.github.com/repos/{REPOSITORY}"
                or prs[0].get("head", {}).get("repo", {}).get("url") != f"https://api.github.com/repos/{REPOSITORY}"):
            raise ValueError("workflow run PR and base identity unavailable or different")
        if (job["id"] != job_id or job["run_id"] != run_id or job["name"] != check["name"]
                or job["status"] != "completed" or job["conclusion"] != "success"
                or job["head_sha"] != head or not isinstance(run["run_attempt"], int)
                or isinstance(run["run_attempt"], bool) or run["run_attempt"] < 1
                or job["run_attempt"] != run["run_attempt"]
                or job["check_run_url"] != f"https://api.github.com/repos/{REPOSITORY}/check-runs/{check['id']}"
                or run["id"] != run_id or run["event"] != "pull_request" or run["head_sha"] != head
                or run["status"] != "completed" or run["conclusion"] != "success"
                or run["repository"]["full_name"] != REPOSITORY or run["head_repository"]["full_name"] != REPOSITORY
                or run["path"] != WORKFLOW or run["check_suite_id"] != check["check_suite"]["id"]
                or workflow["id"] != run["workflow_id"] or workflow["path"] != WORKFLOW):
            raise ValueError("workflow run identity or result mismatch")
        snapshot = {"repository": REPOSITORY, "base_ref": pr["base"]["ref"], "state": pr["state"],
                    "draft": pr["draft"], "head_sha": pr["head"]["sha"], "base_sha": pr["base"]["sha"],
                    "current_main_sha": main["object"]["sha"], "author": pr["user"]["login"],
                    "reviews": normalized_reviews,
                    "checks": [{"name": check["name"], "app_slug": check["app"]["slug"],
                                "head_sha": check["head_sha"], "status": check["status"], "conclusion": check["conclusion"]}]}
        validate(snapshot, head, base)
        return {"snapshot": snapshot, "check_id": check["id"], "run": run, "job": job, "workflow": workflow}
    except (KeyError, TypeError) as error:
        raise ValueError("incomplete GitHub evidence") from error


def verify(reader, number, head, base):
    if not isinstance(number, int) or isinstance(number, bool) or number <= 0:
        raise ValueError("invalid PR number")
    for sha in (head, base):
        if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha):
            raise ValueError("expected head and base must be full lowercase commit IDs")
    first = observe(reader, number, head, base)
    second = observe(reader, number, head, base)
    if first != second:
        raise ValueError("live evidence changed during read")
    return {"status": "observed_subset", "repository": REPOSITORY, "pr": number,
            "head_sha": head, "base_sha": base, "check_id": first["check_id"],
            "run_id": first["run"]["id"], "workflow_id": first["workflow"]["id"],
            "non_claim": "No complete required-result policy, trusted workflow revision, current reviewer authority, atomic snapshot, admission, merge or release authority is established."}


if __name__ == "__main__":
    try:
        if len(sys.argv) != 4:
            raise ValueError("usage: source_promotion_live_evidence.py PR_NUMBER HEAD BASE")
        print(json.dumps(verify(read_api, int(sys.argv[1]), sys.argv[2], sys.argv[3]), sort_keys=True))
    except (ValueError, TypeError) as error:
        print(f"evidence rejected: {error}", file=sys.stderr)
        sys.exit(1)
