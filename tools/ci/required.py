"""Fail closed unless every expected GitHub Actions dependency succeeded."""

import json
import os


JOBS = frozenset(("policy", "native", "coverage", "supply-chain", "repository", "secrets", "python-qa"))


def require_success(results: object) -> None:
    """Validate needs-shaped data without trusting its keys, values or outputs."""
    if not isinstance(results, dict) or set(results) != JOBS:
        raise ValueError("Required job results are missing or unexpected")
    for job in sorted(JOBS):
        result = results[job]
        if not isinstance(result, dict) or result.get("result") != "success":
            raise ValueError(f"Required job did not succeed: {job}")


def main() -> int:
    """Read the configured needs result; malformed input and non-success fail."""
    try:
        require_success(json.loads(os.environ.get("NEEDS_JSON", "")))
    except (ValueError, TypeError):
        # Never echo untrusted needs keys/values as GitHub service messages.
        print("FAIL: every expected CI job must finish successfully")
        return 1
    print("PASS: all seven required CI job groups succeeded")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
