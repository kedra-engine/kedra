"""Select freshly built executables from Cargo's JSON protocol, never output guesses."""
import json
import subprocess
import sys
from pathlib import Path


def build_executable(root, *, package, manifest, name, kind, features=()):
    """Build one bin/example and require exactly one matching executable artifact.

    Cargo owns target directories, configured targets and platform suffixes. Match
    the defining manifest and target identity, not a similarly named dependency or
    a build-script executable. Ambiguous multi-target output fails rather than
    selecting whichever artifact arrived last. Cached (`fresh`) artifacts are
    valid only when reported by this successful build. Diagnostics stream live.
    """
    if kind not in ("bin", "example"):
        raise ValueError("Only bin/example executables are supported")
    manifest = (root / manifest).resolve()
    command = ["cargo", "build", "--package", package, "--" + kind, name,
               "--locked", "--message-format=json"]
    if features:
        command += ["--features", ",".join(features)]
    print("+ " + " ".join(command), flush=True)
    artifacts = []
    finished = []
    with subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8") as process:
        try:
            for line in process.stdout:
                message = json.loads(line)
                if message["reason"] == "compiler-message":
                    rendered = message["message"].get("rendered")
                    if rendered:
                        print(rendered, end="", file=sys.stderr, flush=True)
                elif message["reason"] == "build-finished":
                    finished.append(message["success"])
                elif message["reason"] == "compiler-artifact":
                    target = message["target"]
                    if (Path(message["manifest_path"]).resolve() == manifest
                            and target["name"] == name and target["kind"] == [kind]
                            and message["profile"]["test"] is False
                            and message["executable"] is not None):
                        artifacts.append(Path(message["executable"]))
            code = process.wait()
        except BaseException:
            # Invalid protocol/interrupt must not leave a Cargo build behind.
            process.kill()
            process.wait()
            raise
    if code != 0:
        raise subprocess.CalledProcessError(code, command)
    if finished != [True]:
        raise RuntimeError("Cargo did not report exactly one successful build-finished message")
    if len(artifacts) != 1:
        raise RuntimeError(f"Cargo reported {len(artifacts)} executable artifacts for {package}/{name}")
    executable = artifacts[0]
    if not executable.is_absolute() or not executable.is_file():
        raise RuntimeError(f"Cargo executable is not an existing absolute file: {executable}")
    print(f"Built {package}/{name}: {executable}", flush=True)
    return executable
