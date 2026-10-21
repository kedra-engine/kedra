"""Real files exercise exact complete release identity and integrity checks."""
import json
from pathlib import Path
import tempfile
import unittest

from release_artifacts import TARGET_FORMATS, verify
from publisher_fixtures import write_packages


class ReleaseArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.sha = "a" * 40
        write_packages(self.root, self.sha)

    def test_exact_complete_set_passes(self):
        self.assertEqual(len(verify(self.root, self.sha, "0.1.0")), 5)

    def test_missing_or_unsupported_target_group_fails(self):
        folder = self.root / ("package-" + next(iter(TARGET_FORMATS)))
        missing = self.root.with_name(self.root.name + "-missing-target")
        folder.rename(missing)
        try:
            with self.assertRaises(ValueError):
                verify(self.root, self.sha, "0.1.0")
        finally:
            missing.rename(folder)
        (self.root / "package-x86_64-apple-darwin").mkdir()
        with self.assertRaises(ValueError):
            verify(self.root, self.sha, "0.1.0")

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
