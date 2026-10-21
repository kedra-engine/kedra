#!/usr/bin/env python3
"""GitHub release preflight and validated draft-to-publication lifecycle."""
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import subprocess
import tomllib
from pathlib import Path
from urllib.parse import quote

from release_artifacts import verify


def validate_identity(source: str, tag: str, version: str) -> tuple[str, str]:
    if re.fullmatch(r"[0-9a-f]{40}", source) is None:
        raise ValueError("Source must be an exact lowercase 40-character Git SHA")
    if re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", tag) is None or tag != "v" + version:
        raise ValueError("Release tag must exactly match the Cargo workspace version")
    return source, tag


def validate_ci(runs: list[dict], source: str, repository: str) -> int:
    for run in runs:
        if (run.get("head_sha") == source and run.get("conclusion") == "success"
                and run.get("status") == "completed" and run.get("event") == "push"
                and run.get("path") == ".github/workflows/ci.yml"
                and run.get("head_repository", {}).get("full_name") == repository):
            return int(run["id"])
    raise ValueError("Exact source revision has no successful repository push CI run")


def api(path: str, *arguments: str) -> object:
    result = subprocess.run(["gh", "api", path, *arguments], text=True,
                            stdout=subprocess.PIPE, check=True)
    return json.loads(result.stdout)


def validate_package_result(result: str | None) -> None:
    if result != "success":
        raise ValueError("Publisher requires a successful current-run package matrix")


def validate_workflow_trees(source: list[dict], current: list[dict]) -> None:
    selected = {entry["sha"] for entry in source
                if entry["name"] == "workflows" and entry["type"] == "dir"}
    default = {entry["sha"] for entry in current
               if entry["name"] == "workflows" and entry["type"] == "dir"}
    if len(selected) != 1 or selected != default:
        raise ValueError("Release source workflows differ from the default branch; GITHUB_TOKEN cannot publish this target")


def publish(directory: Path, source: str, tag: str, repository: str) -> None:
    run_id = os.environ["GITHUB_RUN_ID"]
    if not run_id.isdecimal() or os.environ.get("GITHUB_REF") != "refs/heads/main":
        raise ValueError("Publication requires the current official main workflow run")
    # The scheduler's needs result is authoritative; REST job status can lag even
    # after every step and the workflow itself have completed.
    validate_package_result(os.environ.get("RELEASE_PACKAGE_RESULT"))
    entries = verify(directory, source, tag[1:])
    metadata = directory.parent / "verified-release-metadata"
    metadata.mkdir()
    sums = metadata / "SHA256SUMS"
    sums.write_text("".join(f"{entry['sha256']}  {entry['name']}\n" for entry in entries), encoding="utf-8")
    inventory = metadata / "artifact-inventory.json"
    inventory.write_text(json.dumps({
        "schema": 1, "application": "kedra", "version": tag[1:], "source_revision": source,
        "artifacts": [{key: value for key, value in entry.items() if key != "path"} for entry in entries],
    }, indent=2) + "\n", encoding="utf-8")
    notes = metadata / "release-notes.md"
    notes.write_text(f"""# Kedra {tag} — early desktop prerelease

Source: `{source}`. All native packages passed exact-revision CI and package checks.

- Windows x64: NSIS installer and portable ZIP; macOS Apple Silicon: ARM64 DMG;
  Linux x64: AppImage and portable TAR.GZ. macOS requires version 15 or newer.
- Unsigned and not notarized: Gatekeeper/SmartScreen warnings are expected. Do not
  disable OS protections globally. Verify SHA256SUMS before installing.
- Linux was built on Ubuntu 24.04. AppImage is not universally compatible with
  older glibc, graphics drivers or missing FUSE. Windows OS floor remains unverified.
- Package inspection verifies native architecture, resources, CLI diagnostics and
  installed notices. Physical input, GUI console appearance, OS trust and complete
  native rendered UI acceptance remain unverified. Local macOS capture evidence
  was unavailable while the desktop session was locked.
- This editor is early development: visual document editing, Save and game runtime/
  Play are not implemented. See the repository README for current scope.
- Packages include dependency license notices and checksum-verified MPL sources.
  Checksums establish integrity, not trusted publisher authentication.

Install and limitation details: https://github.com/{repository}/blob/{source}/docs/distribution.md
""", encoding="utf-8")
    def gh(*arguments: str) -> None:
        subprocess.run(["gh", "release", *arguments, "--repo", repository], check=True)
    print("Publisher: creating a new draft; no existing tag/release is overwritten", flush=True)
    prefix = f"repos/{repository}"
    created = api(f"{prefix}/releases", "--method", "POST",
                  "-f", "tag_name=" + tag, "-f", "target_commitish=" + source,
                  "-F", "draft=true", "-F", "prerelease=true",
                  "-f", "name=Kedra " + tag, "-F", "body=@" + str(notes))
    release_id = created["id"]
    if type(release_id) is not int or release_id <= 0:
        raise ValueError("Draft creation returned no valid release ID")
    release_path = f"{prefix}/releases/{release_id}"
    paths = [entry["path"] for entry in entries] + [sums, inventory]
    gh("upload", tag, *map(str, paths))
    # GitHub's by-tag endpoint is for published releases, not drafts.
    release = api(release_path)
    expected = {path.name: path for path in paths}
    if (release["id"] != release_id or release["tag_name"] != tag
            or not release["draft"] or not release["prerelease"]
            or release["target_commitish"] != source
            or len(release["assets"]) != len(expected)
            or {asset["name"] for asset in release["assets"]} != set(expected)):
        raise ValueError("Draft identity/upload set differs; leaving the incomplete draft unpublished")
    import hashlib
    for asset in release["assets"]:
        path = expected[asset["name"]]
        with path.open("rb") as stream:
            digest = "sha256:" + hashlib.file_digest(stream, "sha256").hexdigest()
        if asset["size"] != path.stat().st_size or asset.get("digest") != digest:
            raise ValueError("Uploaded asset differs; leaving draft unpublished")
    # Create the tag atomically at the reviewed source if draft creation did not.
    refs = api(f"{prefix}/git/matching-refs/tags/{quote(tag, safe='')}")
    exact = [ref for ref in refs if ref["ref"] == "refs/tags/" + tag]
    if exact:
        if len(exact) != 1 or exact[0]["object"]["sha"] != source:
            raise ValueError("Tag conflict appeared while uploading; leaving draft unpublished")
    else:
        subprocess.run(["gh", "api", "--method", "POST", f"{prefix}/git/refs",
                        "-f", "ref=refs/tags/" + tag, "-f", "sha=" + source], check=True)
    api(release_path, "--method", "PATCH", "-F", "draft=false", "-F", "prerelease=true")
    final = api(f"{prefix}/releases/tags/{quote(tag, safe='')}")
    ref = api(f"{prefix}/git/ref/tags/{quote(tag, safe='')}")
    if (final["id"] != release_id or final["draft"] or not final["prerelease"]
            or ref["object"]["sha"] != source):
        raise ValueError("Published release identity readback failed")
    print("SUCCESS: complete prerelease published at " + final["html_url"], flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--publish-directory", type=Path)
    options = parser.parse_args()
    # Reject malformed input before inserting it into any API route.
    if re.fullmatch(r"[0-9a-f]{40}", options.source) is None:
        parser.error("--source must be an exact lowercase Git SHA")
    repository = os.environ["GITHUB_REPOSITORY"]
    if repository != "kedra-engine/kedra":
        raise ValueError("Release preflight is restricted to the Kedra source repository")
    prefix = "repos/" + repository
    print("Release preflight: verifying immutable source and Cargo identity", flush=True)
    content = api(f"{prefix}/contents/Cargo.toml?ref={options.source}")
    manifest = tomllib.loads(base64.b64decode(content["content"]).decode("utf-8"))
    source, tag = validate_identity(options.source, options.tag, manifest["workspace"]["package"]["version"])
    comparison = api(f"{prefix}/compare/{source}...main")
    if comparison["status"] not in ("ahead", "identical"):
        raise ValueError("Release source must belong to protected main history")
    validate_workflow_trees(api(f"{prefix}/contents/.github?ref={source}"),
                            api(f"{prefix}/contents/.github?ref=main"))
    print("Release preflight: checking actual successful CI at the exact source", flush=True)
    runs = api(f"{prefix}/actions/workflows/ci.yml/runs?head_sha={source}&event=push&per_page=100")
    run_id = validate_ci(runs["workflow_runs"], source, repository)
    checks = api(f"{prefix}/commits/{source}/check-runs?per_page=100")
    required = [check for check in checks["check_runs"]
                if check["name"] == "CI required" and check["app"]["id"] == 15368]
    if not required or required[0]["conclusion"] != "success":
        raise ValueError("Current GitHub Actions CI required result is missing or unsuccessful")
    releases = api(f"{prefix}/releases?per_page=100")
    if any(release["tag_name"] == tag for release in releases):
        raise ValueError("Release already exists; never replace a draft or published release")
    refs = api(f"{prefix}/git/matching-refs/tags/{quote(tag, safe='')}")
    if any(ref["ref"] == "refs/tags/" + tag for ref in refs):
        raise ValueError("Tag already exists; this workflow only creates a new release identity")
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as stream:
            stream.write(f"source={source}\ntag={tag}\nci-run={run_id}\n")
    print(f"SUCCESS: read-only preflight for {tag} at {source}, CI run {run_id}", flush=True)
    if options.publish_directory:
        publish(options.publish_directory.resolve(), source, tag, repository)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"FAILED: release preflight: {error}") from error
