"""Approval gate: passes only when a person with the right role in governance/roles.toml approved the latest commit.

The role depends on the pull request's branch: requirements/* -> [product_owners] (gate B), design/* -> [architects]
(gate D), anything else -> [approvers] (in a domain repository: the tech lead, gate C).

Runs in GitHub Actions on every pull request and review. Reads roles.toml from the base branch
(main), never from the pull request. Writes a summary to the job page.
"""
import json
import os
import re
import sys
import tomllib
import urllib.request

API = "https://api.github.com"


def gh(path: str):
    req = urllib.request.Request(API + path)
    req.add_header("Authorization", f"Bearer {os.environ['GITHUB_TOKEN']}")
    req.add_header("Accept", "application/vnd.github+json")
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read())


def main() -> int:
    repo = os.environ["GITHUB_REPOSITORY"]
    pr_number = os.environ["PR_NUMBER"]
    roles_path = os.environ.get("ROLES_FILE", "base/governance/roles.toml")
    with open(roles_path, "rb") as f:
        roles = tomllib.load(f)
    allow_self = bool(roles.get("settings", {}).get("allow_self_approval", False))
    pr = gh(f"/repos/{repo}/pulls/{pr_number}")
    branch = pr["head"]["ref"]
    role = ("product_owners" if branch.startswith("requirements/") else
            "architects" if branch.startswith("design/") else "approvers")
    if role not in roles:
        role = "approvers"
    approvers = {u["github"].lower(): u for u in roles.get(role, {}).get("users", [])}

    head_sha = pr["head"]["sha"]
    commits = gh(f"/repos/{repo}/pulls/{pr_number}/commits?per_page=100")
    operators = set()
    for c in commits:
        operators |= {m.lower() for m in re.findall(r"^Operator:\s*(\S+)", c["commit"]["message"], re.M)}

    # Latest review per person; only approvals of the current head commit count.
    latest = {}
    for rv in gh(f"/repos/{repo}/pulls/{pr_number}/reviews?per_page=100"):
        if rv["state"] in ("APPROVED", "CHANGES_REQUESTED", "DISMISSED"):
            latest[rv["user"]["login"].lower()] = rv

    rows, passed, warnings = [], False, []
    for login, rv in latest.items():
        if rv["state"] != "APPROVED":
            rows.append((login, rv["state"].lower(), "not an approval"))
            continue
        if rv["commit_id"] != head_sha:
            rows.append((login, "approved", "older version: re-approval needed"))
            continue
        person = approvers.get(login)
        if not person:
            rows.append((login, "approved", f"not in [{role}]: does not count"))
            continue
        is_operator = person.get("email", "").lower() in operators
        if is_operator and not allow_self:
            rows.append((login, "approved", "is the operator of this change: blocked (self-approval)"))
            continue
        if is_operator:
            warnings.append(f"{person['name']} approved a change they operated (allowed by the demo setting allow_self_approval).")
            rows.append((login, "approved", "counts (WARNING: self-approval, demo setting)"))
        else:
            rows.append((login, "approved", "counts"))
        passed = True

    summary = ["## Approval gate", "",
               f"Operator(s) of this change: {', '.join(sorted(operators)) or 'none recorded'}",
               f"Branch: {branch} -> approval needed from [{role}] (from main): {', '.join(sorted(approvers)) or 'none'}",
               f"Self-approval allowed: {'yes (demo setting)' if allow_self else 'no'}", "",
               "| Reviewer | Review | Result |", "|---|---|---|"]
    summary += [f"| {a} | {b} | {c} |" for a, b, c in rows] or ["| – | – | no reviews yet |"]
    summary += ["", f"**Result: {'PASSED' if passed else 'WAITING FOR APPROVAL'}**"]
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as f:
            f.write("\n".join(summary) + "\n")
    print("\n".join(summary))
    for w in warnings:
        print(f"::warning title=Self-approval::{w}")
    if not passed:
        print(f"::error title=Approval gate::A person listed under [{role}] in governance/roles.toml must approve the latest commit.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
