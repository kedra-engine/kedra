"""Native inspection refuses missing/unreviewed inputs before extracting anything."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import os

from inspect_package import inspect_appimage


class InspectionTests(unittest.TestCase):
    @unittest.skipIf(os.name == "nt", "POSIX executable-script fixture for AppImage inspection")
    def test_broken_apprun_cannot_pass_by_launching_the_inner_binary(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inner = (f"#!{sys.executable}\nimport sys\n"
                     "if sys.argv[1:] == ['--version']: print('kedra 0.1.0')\n"
                     "elif sys.argv[1:3] == ['assets', 'verify']: print('SUCCESS')\n"
                     "else: print('error: unknown option', file=sys.stderr); sys.exit(2)\n")
            wrapper = f"#!{sys.executable}\nraise SystemExit(61)\n"
            image = root / "fixture.AppImage"
            image.write_text(
                f"#!{sys.executable}\nfrom pathlib import Path\n"
                "p = Path('squashfs-root')\n"
                "for name in ['usr/bin', 'usr/lib/kedra/assets/fonts', 'usr/lib/kedra/licenses/covered-sources']: (p/name).mkdir(parents=True, exist_ok=True)\n"
                f"(p/'usr/bin/kedra').write_text({inner!r})\n"
                f"(p/'AppRun').write_text({wrapper!r})\n"
                "(p/'usr/bin/kedra').chmod(0o755)\n(p/'AppRun').chmod(0o755)\n"
                "(p/'usr/lib/kedra/assets/fonts/Inter-OFL.txt').write_text('fixture')\n"
                "(p/'usr/lib/kedra/licenses/dependency-notices.html').write_text('fixture')\n"
                "(p/'usr/lib/kedra/licenses/covered-sources/fixture.crate').write_text('fixture')\n"
            )
            image.chmod(0o755)
            with self.assertRaises(subprocess.CalledProcessError):
                inspect_appimage(image, root, "0.1.0")

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
