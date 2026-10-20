"""Real files exercise exact complete release identity and integrity checks."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from release_artifacts import TARGET_FORMATS, verify


class ReleaseArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.sha = "a" * 40
        for target, formats in TARGET_FORMATS.items():
            folder = self.root / ("package-" + target)
            folder.mkdir()
            entries = []
            for kind, extension in formats.items():
                name = f"kedra-0.1.0-{target}.{extension}"
                content = (name + " test payload").encode()
                (folder / name).write_bytes(content)
                entries.append({"name": name, "format": kind, "bytes": len(content),
                                "sha256": hashlib.sha256(content).hexdigest()})
            (folder / "artifact-inventory.json").write_text(json.dumps({
                "schema": 1, "application": "kedra", "version": "0.1.0",
                "target": target, "source_revision": self.sha, "artifacts": entries,
            }))
            (folder / "SHA256SUMS").write_text("".join(
                f"{entry['sha256']}  {entry['name']}\n" for entry in entries))

    def test_exact_complete_set_passes(self):
        self.assertEqual(len(verify(self.root, self.sha, "0.1.0")), 6)

    def test_missing_target_extra_file_wrong_hash_and_wrong_sha_fail(self):
        target = next(iter(TARGET_FORMATS))
        folder = self.root / ("package-" + target)
        extra = folder / "unexpected.exe"
        extra.write_bytes(b"unreviewed")
        with self.assertRaises(ValueError):
            verify(self.root, self.sha, "0.1.0")
        extra.unlink()
        inventory = folder / "artifact-inventory.json"
        data = json.loads(inventory.read_text())
        package = folder / data["artifacts"][0]["name"]
        before = package.read_bytes()
        package.write_bytes(b"corrupt")
        with self.assertRaises(ValueError):
            verify(self.root, self.sha, "0.1.0")
        package.write_bytes(before)
        with self.assertRaises(ValueError):
            verify(self.root, "b" * 40, "0.1.0")
        inventory.unlink()
        with self.assertRaises(ValueError):
            verify(self.root, self.sha, "0.1.0")

    def test_path_traversal_duplicate_and_symlink_are_rejected(self):
        folder = self.root / ("package-" + next(iter(TARGET_FORMATS)))
        inventory = folder / "artifact-inventory.json"
        data = json.loads(inventory.read_text())
        original = json.dumps(data)
        data["artifacts"][0]["name"] = "../outside.exe"
        inventory.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            verify(self.root, self.sha, "0.1.0")
        data = json.loads(original)
        data["artifacts"].append(data["artifacts"][0])
        inventory.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            verify(self.root, self.sha, "0.1.0")
        inventory.write_text(original)
        payload = folder / data["artifacts"][0]["name"]
        source = self.root / "outside"
        source.write_bytes(payload.read_bytes())
        payload.unlink()
        try:
            payload.symlink_to(source)
        except OSError:
            self.skipTest("Host cannot create symlinks")
        with self.assertRaises(ValueError):
            verify(self.root, self.sha, "0.1.0")


if __name__ == "__main__":
    unittest.main()
