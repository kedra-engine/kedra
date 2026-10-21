"""Small real artifact files for publisher and artifact-policy tests."""
import hashlib
import json
from pathlib import Path


def write_packages(root: Path, source: str) -> None:
    formats = {
        "x86_64-pc-windows-msvc": {"nsis": "exe", "zip": "zip"},
        "x86_64-unknown-linux-gnu": {"appimage": "AppImage", "tar.gz": "tar.gz"},
        "aarch64-apple-darwin": {"dmg": "dmg"},
    }
    for target, extensions in formats.items():
        folder = root / ("package-" + target)
        folder.mkdir()
        entries = []
        for kind, extension in extensions.items():
            name = f"kedra-0.1.0-{target}.{extension}"
            content = (name + " test payload").encode()
            (folder / name).write_bytes(content)
            entries.append({"name": name, "format": kind, "bytes": len(content),
                            "sha256": hashlib.sha256(content).hexdigest()})
        (folder / "artifact-inventory.json").write_text(json.dumps({
            "schema": 1, "application": "kedra", "version": "0.1.0",
            "target": target, "source_revision": source, "artifacts": entries,
        }))
        (folder / "SHA256SUMS").write_text("".join(
            f"{entry['sha256']}  {entry['name']}\n" for entry in entries))
