"""Require selected GitHub Actions jobs; permit only path-planned skips."""

import json
import os


JOBS = frozenset(("changes", "policy", "native", "coverage", "supply-chain", "repository", "secrets", "python-test", "sites-build"))


def require_success(results: object) -> None:
    """Validate needs-shaped data without trusting its keys, values or outputs."""
    if not isinstance(results, dict) or set(results) != JOBS:
        raise ValueError("Required job results are missing or unexpected")
    changes = results["changes"]
    if not isinstance(changes, dict) or changes.get("result") != "success":
        raise ValueError("Changed-path selection did not succeed")
    outputs = changes.get("outputs")
    if not isinstance(outputs, dict) or any(
        outputs.get(key) not in ("true", "false")
        for key in ("rust", "python", "markdown", "workflows", "distribution")
    ):
        raise ValueError("Changed-path selection is missing or malformed")
    selected = {
        "native": outputs["rust"],
        "coverage": outputs["rust"],
        "supply-chain": outputs["rust"],
        "python-test": outputs["python"],
        "repository": outputs["markdown"],
    }
    for job in sorted(JOBS):
        result = results[job]
        expected = "skipped" if selected.get(job) == "false" else "success"
        if not isinstance(result, dict) or result.get("result") != expected:
            raise ValueError(f"Required job has an unexpected result: {job}")


def main() -> int:
    """Read the configured needs result; malformed input and non-success fail."""
    try:
        require_success(json.loads(os.environ.get("NEEDS_JSON", "")))
    except (ValueError, TypeError):
        # Never echo untrusted needs keys/values as GitHub service messages.
        print("FAIL: selected CI jobs must succeed; only planned skips are accepted")
        return 1
    print("PASS: all selected CI job groups succeeded; planned skips verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
