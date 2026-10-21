#!/usr/bin/env python3
"""Install reviewed cargo-about source with an isolated, freshly audited lockfile."""
from __future__ import annotations

import argparse
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import urllib.request

VERSION = "0.9.2"
CHECKSUM = "0cd19d99696eb83f0a2d6ab7a347b14968d2980416c8cca827ded220e6e9c4bb"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True,
                        help="New local tool directory outside any Cargo workspace")
    parser.add_argument("--archive", type=Path, help="Already downloaded source .crate")
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        parser.error("--output must be a new directory; previous tool installations are preserved")
    print(f"Preparing checksum-verified cargo-about {VERSION} source", flush=True)
    if args.archive:
        archive = args.archive.read_bytes()
    else:
        request = urllib.request.Request(
            f"https://crates.io/api/v1/crates/cargo-about/{VERSION}/download",
            headers={"User-Agent": "Kedra distribution tooling"},
        )
        with urllib.request.urlopen(request, timeout=60) as response:
            archive = response.read(32 * 1024 * 1024)
    if hashlib.sha256(archive).hexdigest() != CHECKSUM:
        raise ValueError("cargo-about source checksum mismatch; nothing extracted or executed")
    cargo = os.environ.get("CARGO", "cargo")
    ancestor = next(path for path in output.parents if path.is_dir())
    workspace = subprocess.run(
        [cargo, "locate-project", "--workspace", "--message-format", "plain"],
        cwd=ancestor, capture_output=True, text=True,
    )
    if workspace.returncode == 0:
        raise ValueError("Tool source must be outside an existing Cargo workspace; use RUNNER_TEMP or an OS tool-cache directory")
    output.mkdir(parents=True)
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as source:
        source.extractall(output / "source", filter="data")
    manifest = output / "source" / f"cargo-about-{VERSION}" / "Cargo.toml"
    commands = [
        [cargo, "update", "--manifest-path", str(manifest)],
        [cargo, "deny", "--manifest-path", str(manifest), "--locked", "check", "advisories"],
        [cargo, "install", "--path", str(manifest.parent), "--features", "cli", "--locked",
         "--root", str(output), "--target-dir", str(output / "build")],
    ]
    for command in commands:
        print("+ " + " ".join(command), flush=True)
        subprocess.run(command, check=True)
    executable = output / "bin" / ("cargo-about.exe" if os.name == "nt" else "cargo-about")
    subprocess.run([str(executable), "--version"], check=True)
    if github_path := os.environ.get("GITHUB_PATH"):
        with open(github_path, "a", encoding="utf-8") as stream:
            stream.write(str(executable.parent) + "\n")
    print(f"SUCCESS: audited isolated tool installed; source and lock retained at {output}", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError, tarfile.TarError) as error:
        raise SystemExit(f"FAILED: {error}") from error
