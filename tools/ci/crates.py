#!/usr/bin/env python3
"""Kedra source/CI admission and legal-content checks for Cargo-owned publication."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib

from release import api, validate_ci

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "kedra-engine/kedra"
LICENSES = ("LICENSE-MIT", "LICENSE-APACHE")


def packages() -> list[dict]:
    """Let Cargo select workspace members and interpret inherited publication fields."""
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
        cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True, timeout=120,
    )
    metadata = json.loads(result.stdout)
    members = set(metadata["workspace_members"])
    return [package for package in metadata["packages"]
            if package["id"] in members and package["publish"] != []]


def check_source(selected: list[dict]) -> None:
    """Release sources carry canonical dual-license texts and project identity."""
    if not selected:
        raise ValueError("No publishable workspace packages")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    for package in selected:
        if package["publish"] is not None and "crates-io" not in package["publish"]:
            raise ValueError("Package does not admit crates.io: " + package["name"])
        if (package["version"] != version
                or package["repository"] != "https://github.com/" + REPOSITORY
                or package["homepage"] != "https://kedra.tech/"
                or package["license"] != "MIT OR Apache-2.0"):
            raise ValueError("Publication metadata differs: " + package["name"])
        directory = Path(package["manifest_path"]).parent
        for name in LICENSES:
            if (directory / name).read_bytes() != (ROOT / name).read_bytes():
                raise ValueError("Noncanonical license text: " + package["name"] + "/" + name)
    print(f"PASS: publication metadata and canonical licenses for {len(selected)} packages", flush=True)


def preflight(source: str) -> None:
    """Admit only the clean current main source with genuine successful push CI."""
    if re.fullmatch(r"[0-9a-f]{40}", source) is None:
        raise ValueError("Source must be an exact lowercase 40-character Git SHA")
    if (os.environ.get("GITHUB_REPOSITORY") != REPOSITORY
            or os.environ.get("GITHUB_REF") != "refs/heads/main"
            or os.environ.get("GITHUB_SHA") != source):
        raise ValueError("Publication requires this exact official main workflow revision")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if head != source or api(f"repos/{REPOSITORY}/commits/main")["sha"] != source:
        raise ValueError("Source is not the current checked-out main revision")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
        raise ValueError("Publication requires an unchanged, clean source tree")
    runs = api(f"repos/{REPOSITORY}/actions/workflows/ci.yml/runs"
               f"?event=push&head_sha={source}&per_page=100")["workflow_runs"]
    run = validate_ci(runs, source, REPOSITORY)
    check_source(packages())
    print(f"PASS: exact current main source {source}; successful push CI run {run}", flush=True)


def check_archive(path: Path, package: dict, licenses: dict[str, bytes]) -> None:
    """Inspect, without extracting, the actual archive Cargo will distribute."""
    prefix = f"{package['name']}-{package['version']}/"
    before = path.read_bytes()
    with tarfile.open(path, "r:gz") as archive:
        def read(name: str) -> bytes:
            member = archive.getmember(prefix + name)
            if not member.isfile():
                raise ValueError("Required archive content is not a regular file: " + name)
            stream = archive.extractfile(member)
            if stream is None:
                raise ValueError("Required archive content is unreadable: " + name)
            with stream:
                return stream.read()
        manifest = tomllib.loads(read("Cargo.toml").decode())["package"]
        for field in ("name", "version", "repository", "homepage", "license"):
            if manifest.get(field) != package[field]:
                raise ValueError("Archive metadata differs: " + package["name"] + "/" + field)
        for name, expected in licenses.items():
            if read(name) != expected:
                raise ValueError("Archive license differs: " + package["name"] + "/" + name)
    if path.read_bytes() != before:
        raise ValueError("Archive changed during inspection")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("preflight").add_argument("--source", required=True)
    materialize = commands.add_parser("materialize")
    materialize.add_argument("--target-dir", type=Path, required=True)
    materialize.add_argument("--allow-dirty", action="store_true")
    archives = commands.add_parser("archives")
    archives.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    print("Crates publication: checking " + args.command, flush=True)
    if args.command == "preflight":
        preflight(args.source)
    elif args.command == "materialize":
        selected = packages()
        check_source(selected)
        command = ["cargo", "package", "--registry", "crates-io", "--locked",
                   "--all-features", "--no-verify",
                   "--target-dir", str(args.target_dir)]
        for package in selected:
            command.extend(["--package", package["name"]])
        if args.allow_dirty:
            command.append("--allow-dirty")
        subprocess.run(command, cwd=ROOT, check=True, timeout=1200)
    else:
        selected = packages()
        check_source(selected)
        licenses = {name: (ROOT / name).read_bytes() for name in LICENSES}
        for package in selected:
            path = args.directory / f"{package['name']}-{package['version']}.crate"
            check_archive(path, package, licenses)
            print("PASS: archive identity and license delivery: " + path.name, flush=True)
    print("PASS: crates publication policy", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.SubprocessError, tarfile.TarError) as error:
        raise SystemExit("FAIL: crates publication policy: " + str(error)) from None
