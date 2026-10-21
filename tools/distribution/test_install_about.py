"""The actual isolated tool installer rejects tampered input before extraction."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class AboutInstallerTests(unittest.TestCase):
    def test_tampered_source_never_creates_or_builds_a_tool_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.crate"
            source.write_bytes(b"not the reviewed crate archive")
            output = root / "tool"
            result = subprocess.run([
                sys.executable, str(Path(__file__).with_name("install_about.py")),
                "--archive", str(source), "--output", str(output),
            ], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("checksum", result.stderr.lower())
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
