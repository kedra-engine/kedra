"""Kedra's distributed package identity/legal-content and source admission policy."""
import io
import os
from pathlib import Path
import tarfile
import tempfile
import tomllib
import unittest
from unittest import mock

import crates


class CratePublicationContracts(unittest.TestCase):
    def test_source_rejects_foreign_metadata_and_registry_without_changing_sources(self):
        manifest = crates.ROOT / "crates/kedra_docking/Cargo.toml"
        original = manifest.read_bytes()
        version = tomllib.loads((crates.ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
        package = {"name": "kedra_docking", "version": version, "publish": None,
                   "manifest_path": str(manifest),
                   "repository": "https://github.com/kedra-engine/kedra",
                   "homepage": "https://kedra.tech/", "license": "MIT OR Apache-2.0"}
        crates.check_source([package])
        for field, value in [("version", "99.0.0"), ("publish", ["private"]),
                             ("repository", "https://github.com/other/project"),
                             ("homepage", "https://example.org/"), ("license", "MIT")]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                crates.check_source([{**package, field: value}])
        self.assertEqual(manifest.read_bytes(), original)

    def test_archive_retains_identity_and_both_canonical_licenses_without_mutation(self):
        package = {"name": "kedra_docking", "version": "0.1.3",
                   "repository": "https://github.com/kedra-engine/kedra",
                   "homepage": "https://kedra.tech/", "license": "MIT OR Apache-2.0"}
        licenses = {name: (crates.ROOT / name).read_bytes() for name in crates.LICENSES}
        manifest = ("[package]\n" + "".join(f'{key} = "{value}"\n'
                                           for key, value in package.items())).encode()
        # The fixture models our distribution contract, not Cargo's packaging implementation.
        members = {"Cargo.toml": manifest, **licenses}
        cases = [members, {k: v for k, v in members.items() if k != "LICENSE-MIT"},
                 {**members, "LICENSE-APACHE": b"Wrong copyright and terms"},
                 {**members, "Cargo.toml": manifest.replace(b"kedra-engine/kedra", b"other/project")}]
        with tempfile.TemporaryDirectory() as directory:
            for index, content in enumerate(cases):
                with self.subTest(case=index):
                    path = Path(directory) / "kedra_docking-0.1.3.crate"
                    with tarfile.open(path, "w:gz") as archive:
                        for name, data in content.items():
                            entry = tarfile.TarInfo("kedra_docking-0.1.3/" + name)
                            entry.size = len(data)
                            archive.addfile(entry, io.BytesIO(data))
                    original = path.read_bytes()
                    if index == 0:
                        crates.check_archive(path, package, licenses)
                    else:
                        with self.assertRaises((ValueError, KeyError)):
                            crates.check_archive(path, package, licenses)
                    self.assertEqual(path.read_bytes(), original)
        self.assertEqual(licenses, {name: (crates.ROOT / name).read_bytes()
                                    for name in crates.LICENSES})

    def test_invalid_source_is_rejected_before_registry_or_github_access(self):
        for source in ["main", "", "A" * 40, "a" * 39, "a" * 40 + "; echo wrong"]:
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, "Git SHA"):
                crates.preflight(source)

    def test_foreign_or_unreviewed_workflow_context_is_rejected_without_github_access(self):
        source = "a" * 40
        context = {"GITHUB_REPOSITORY": "kedra-engine/kedra", "GITHUB_REF": "refs/heads/main",
                   "GITHUB_SHA": source}
        with mock.patch.object(crates, "api", side_effect=AssertionError("Unexpected network access")):
            for field, value in [("GITHUB_REPOSITORY", "other/kedra"),
                                 ("GITHUB_REF", "refs/pull/1/merge"),
                                 ("GITHUB_SHA", "b" * 40)]:
                with self.subTest(field=field), mock.patch.dict(os.environ, {**context, field: value}):
                    with self.assertRaisesRegex(ValueError, "official main"):
                        crates.preflight(source)


if __name__ == "__main__":
    unittest.main()
