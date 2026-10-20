"""Narrow complete-artifact policy; GitHub and native tooling own transport/formats."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

TARGET_FORMATS = {
    "x86_64-pc-windows-msvc": {"nsis": "exe", "zip": "zip"},
    "x86_64-unknown-linux-gnu": {"appimage": "AppImage", "tar.gz": "tar.gz"},
    "aarch64-apple-darwin": {"dmg": "dmg"},
    "x86_64-apple-darwin": {"dmg": "dmg"},
}


def verify(root: Path, source: str, version: str) -> list[dict]:
    expected_directories = {"package-" + target for target in TARGET_FORMATS}
    if {path.name for path in root.iterdir()} != expected_directories:
        raise ValueError("Release requires exactly all four target artifact groups")
    entries = []
    for target, formats in TARGET_FORMATS.items():
        folder = root / ("package-" + target)
        if folder.is_symlink() or not folder.is_dir():
            raise ValueError("Artifact groups must be ordinary directories")
        expected = {f"kedra-{version}-{target}.{extension}": kind
                    for kind, extension in formats.items()}
        files = list(folder.iterdir())
        if ({path.name for path in files} != set(expected) | {"artifact-inventory.json", "SHA256SUMS"}
                or any(path.is_symlink() or not path.is_file() for path in files)):
            raise ValueError("Missing, unexpected or symlinked artifact files")
        try:
            data = json.loads((folder / "artifact-inventory.json").read_text(encoding="utf-8"))
            if (data["schema"] != 1 or data["application"] != "kedra"
                    or data["version"] != version or data["target"] != target
                    or data["source_revision"] != source):
                raise ValueError("Artifact inventory does not match the reviewed source/target")
            artifacts = data["artifacts"]
            if (len(artifacts) != len(expected)
                    or {entry["name"] for entry in artifacts} != set(expected)):
                raise ValueError("Missing, duplicate or unexpected package identities")
            sums = ""
            for artifact in artifacts:
                name = artifact["name"]
                path = folder / name
                if artifact["format"] != expected[name]:
                    raise ValueError("Artifact format differs from the required native format")
                with path.open("rb") as stream:
                    sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
                size = path.stat().st_size
                if size <= 0 or artifact["bytes"] != size or artifact["sha256"] != sha256:
                    raise ValueError("Artifact is empty or differs from its size/hash inventory")
                sums += f"{sha256}  {name}\n"
                entries.append(dict(artifact, target=target, path=path))
            if (folder / "SHA256SUMS").read_text(encoding="utf-8") != sums:
                raise ValueError("Checksums differ from the verified artifact inventory")
        except (OSError, KeyError, TypeError, json.JSONDecodeError) as error:
            raise ValueError(f"Invalid artifact inventory for {target}: {error}") from error
    return entries
