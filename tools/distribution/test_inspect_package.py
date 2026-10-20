"""Native inspection refuses missing/unreviewed inputs before extracting anything."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class InspectionTests(unittest.TestCase):
    def test_missing_inventory_fails_before_native_installation(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = subprocess.run([
                sys.executable, str(Path(__file__).with_name("inspect_package.py")),
                str(directory), "--target", "aarch64-apple-darwin",
                "--source", "a" * 40, "--version", "0.1.0",
            ], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("inventory", result.stderr.lower())
            self.assertEqual(list(directory.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
