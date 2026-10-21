#!/usr/bin/env python3
"""Build a real independent app through the shared project-local Cargo alias."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def run(arguments: list[str], workspace: Path) -> None:
    print("+ " + " ".join(arguments), flush=True)
    subprocess.run(arguments, cwd=workspace, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", help="Use this exact public Kedra Git revision")
    parser.add_argument("--package", action="store_true", help="Build native formats too")
    parser.add_argument("--workspace", type=Path, help="New fixture directory; retained for inspection")
    options = parser.parse_args()
    repository = Path(__file__).resolve().parents[2]
    if options.revision and (
        len(options.revision) != 40
        or any(character not in "0123456789abcdefABCDEF" for character in options.revision)
    ):
        parser.error("--revision must be an exact 40-character Git SHA")
    if options.workspace:
        workspace = options.workspace.resolve()
        if workspace.exists():
            parser.error("--workspace must not already exist")
        workspace.parent.mkdir(parents=True, exist_ok=True)
    else:
        parent = repository / "target" / "distribution"
        parent.mkdir(parents=True, exist_ok=True)
        workspace = Path(tempfile.mkdtemp(prefix="independent app Юникод ", dir=parent))
        # copytree reserves the application root, not an existing caller directory.
        workspace = workspace / "project"
    shutil.copytree(repository / "tests/fixtures/distribution", workspace)
    configuration = workspace / ".cargo/config.toml"
    aliases_before = configuration.read_bytes()
    # These are fixed Cargo patch tables. JSON basic-string quoting preserves
    # quotes, native backslashes and Unicode in the TOML string subset used here.
    patch = "\n[patch.crates-io]\n"
    for name in ("kedra_assets", "kedra_build"):
        if options.revision:
            source = (
                'git = "https://github.com/kedra-engine/kedra.git", rev = '
                + json.dumps(options.revision)
            )
        else:
            source = "path = " + json.dumps(str(repository / "crates" / name), ensure_ascii=False)
        patch += f"{name} = {{ {source} }}\n"
    with configuration.open("a", encoding="utf-8") as stream:
        stream.write(patch)
    cargo = os.environ.get("CARGO", "cargo")
    run([cargo, "generate-lockfile"], workspace)
    run([cargo, "xtask"], workspace)
    run([cargo, "kedra", "icons", "icon.svg", "--output", "target/native-icons"], workspace)
    shutil.copy2(workspace / "target/native-icons/application.ico", workspace / "icon.ico")
    run([cargo, "kedra", "assets", "init", "assets"], workspace)
    command = "package" if options.package else "build"
    arguments = [cargo, "kedra", command, "--package", "distribution-fixture", "--output", "distribution"]
    if not options.package:
        arguments.append("--release")
    run(arguments, workspace)
    run([cargo, "xtask"], workspace)
    if not configuration.read_bytes().startswith(aliases_before):
        raise RuntimeError("Existing application aliases were modified")
    tree = subprocess.run(
        [cargo, "tree", "--locked", "--package", "distribution-fixture", "--edges", "normal"],
        cwd=workspace, text=True, capture_output=True, check=True,
    ).stdout
    for forbidden in ("kedra_editor", "kedra_import", "kedra_build", "cargo-packager"):
        if forbidden in tree:
            raise RuntimeError(f"Runtime dependency graph contains {forbidden}")
    result = json.loads((workspace / "distribution/build-result.json").read_text(encoding="utf-8"))
    binary = workspace / "distribution" / result["executable"]
    runtime_environment = os.environ.copy()
    runtime_environment.pop("KEDRA_ASSETS", None)
    subprocess.run([str(binary)], cwd=workspace.parent, env=runtime_environment, check=True)
    marker = workspace / "launch-marker"
    run([cargo, "kedra", "run", "--release", "--package", "distribution-fixture",
         "--output", "run-distribution", "--", "--launch-marker", str(marker)], workspace)
    deadline = time.monotonic() + 20
    while not marker.exists() and time.monotonic() < deadline:
        time.sleep(.05)
    if marker.read_text(encoding="utf-8") != "SUCCESS: launched independent verified application\n":
        raise RuntimeError("Independent build-and-run did not launch the verified application")
    print(f"SUCCESS: independent application and unchanged aliases verified: {workspace}", flush=True)


if __name__ == "__main__":
    main()
