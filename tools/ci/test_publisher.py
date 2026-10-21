"""Publisher lifecycle against documented GitHub API/CLI boundaries, without writes."""
import base64
import copy
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release
from publisher_fixtures import write_packages


class PublisherTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "packages"
        self.root.mkdir()
        self.source = "a" * 40
        self.tag = "v0.1.0"
        self.prefix = "repos/kedra-engine/kedra"
        write_packages(self.root, self.source)
        self.draft = {"id": 42, "tag_name": self.tag, "draft": True, "prerelease": True,
                      "target_commitish": self.source, "assets": [],
                      "html_url": "https://github.com/kedra-engine/kedra/releases/tag/v0.1.0"}
        self.created = False
        self.published = False
        self.tag_source = None
        self.damage = None
        self.upload_error = False
        self.patch_error = False
        self.readback_source = None
        self.addCleanup(patch.stopall)
        patch.dict(os.environ, {"GITHUB_RUN_ID": "123", "GITHUB_REF": "refs/heads/main",
                                "RELEASE_PACKAGE_RESULT": "success"}).start()
        patch.object(release, "api", side_effect=self.api).start()
        patch.object(release.subprocess, "run", side_effect=self.run_command).start()

    def api(self, path, *arguments):
        if path == self.prefix + "/releases":
            notes = self.root.parent / "verified-release-metadata/release-notes.md"
            self.assertEqual(arguments, ("--method", "POST", "-f", "tag_name=" + self.tag,
                                        "-f", "target_commitish=" + self.source,
                                        "-F", "draft=true", "-F", "prerelease=true",
                                        "-f", "name=Kedra " + self.tag, "-F", "body=@" + str(notes)))
            self.created = True
            return copy.deepcopy(self.draft)
        if path == self.prefix + "/releases/42":
            if arguments:
                self.assertEqual(arguments, ("--method", "PATCH", "-F", "draft=false",
                                             "-F", "prerelease=true"))
                self.assertEqual(len(self.draft["assets"]), 7)
                self.assertEqual(self.tag_source, self.source)
                if self.patch_error:
                    raise subprocess.CalledProcessError(1, ["gh", "api", path])
                self.published = True
                self.draft["draft"] = False
            result = copy.deepcopy(self.draft)
            if self.damage == "digest":
                result["assets"][0]["digest"] = "sha256:" + "0" * 64
            elif self.damage == "size":
                result["assets"][0]["size"] += 1
            elif self.damage == "missing":
                result["assets"].pop()
            elif self.damage == "source":
                result["target_commitish"] = "b" * 40
            elif self.damage == "tag":
                result["tag_name"] = "v9.9.9"
            return result
        if path == self.prefix + "/releases/tags/" + self.tag:
            if not self.published:
                raise subprocess.CalledProcessError(1, ["gh", "api", path], stderr="HTTP 404: published releases only")
            return copy.deepcopy(self.draft)
        if path == self.prefix + "/git/matching-refs/tags/" + self.tag:
            return ([{"ref": "refs/tags/" + self.tag, "object": {"sha": self.tag_source}}]
                    if self.tag_source else [])
        if path == self.prefix + "/git/ref/tags/" + self.tag:
            return {"object": {"sha": self.readback_source or self.tag_source}}
        self.fail("Unexpected API route: " + path)

    def run_command(self, arguments, **kwargs):
        if arguments[:3] == ["gh", "release", "create"]:
            # Old implementation can create/upload, then fails looking up its
            # unpublished draft through the published-by-tag endpoint.
            self.created = True
        elif arguments[:3] == ["gh", "release", "upload"]:
            self.assertTrue(self.created)
            if self.upload_error:
                raise subprocess.CalledProcessError(1, arguments)
            self.assertEqual(arguments[3], self.tag)
            for name in arguments[4:-2]:
                path = Path(name)
                self.draft["assets"].append({
                    "name": path.name, "size": path.stat().st_size,
                    "digest": "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest(),
                })
        elif arguments[:4] == ["gh", "api", "--method", "POST"]:
            self.assertEqual(arguments[4], self.prefix + "/git/refs")
            self.assertIn("sha=" + self.source, arguments)
            self.tag_source = self.source
        else:
            self.fail("Unexpected command: " + repr(arguments))
        return subprocess.CompletedProcess(arguments, 0)

    def publish(self):
        release.publish(self.root, self.source, self.tag, "kedra-engine/kedra")

    def test_full_lifecycle_uses_draft_id_before_published_tag_lookup(self):
        self.publish()
        self.assertTrue(self.published)
        self.assertEqual(len(self.draft["assets"]), 7)

    def test_wrong_draft_identity_or_uploaded_digest_never_publishes(self):
        for damage in ("digest", "size", "missing", "source", "tag"):
            with self.subTest(damage=damage):
                self.damage = damage
                self.draft["assets"] = []
                metadata = self.root.parent / "verified-release-metadata"
                if metadata.exists():
                    for path in metadata.iterdir():
                        path.unlink()
                    metadata.rmdir()
                with self.assertRaises(ValueError):
                    self.publish()
                self.assertFalse(self.published)
                self.assertIsNone(self.tag_source)

    def test_upload_failure_leaves_draft_unpublished(self):
        self.upload_error = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.publish()
        self.assertTrue(self.created)
        self.assertFalse(self.published)

    def test_conflicting_tag_is_never_rebound_or_published(self):
        self.tag_source = "b" * 40
        with self.assertRaises(ValueError):
            self.publish()
        self.assertEqual(self.tag_source, "b" * 40)
        self.assertFalse(self.published)

    def test_corrupt_package_is_rejected_before_creating_a_draft(self):
        package = next(self.root.glob("package-*/*.exe"))
        package.write_bytes(b"corrupt")
        with self.assertRaises(ValueError):
            self.publish()
        self.assertFalse(self.created)
        self.assertFalse(self.published)

    def test_patch_failure_leaves_verified_tag_and_unpublished_draft(self):
        self.patch_error = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.publish()
        self.assertFalse(self.published)
        self.assertTrue(self.draft["draft"])
        self.assertEqual(self.tag_source, self.source)

    def test_final_tag_mismatch_is_reported_even_after_publication(self):
        self.readback_source = "b" * 40
        with self.assertRaisesRegex(ValueError, "identity readback failed"):
            self.publish()
        self.assertTrue(self.published)

    def test_preflight_refuses_existing_draft_before_any_publication(self):
        workflow = [{"name": "workflows", "type": "dir", "sha": "c" * 40}]
        manifest = b'[workspace.package]\nversion = "0.1.0"\n'
        responses = {
            f"{self.prefix}/contents/Cargo.toml?ref={self.source}": {
                "content": base64.b64encode(manifest).decode(),
            },
            f"{self.prefix}/compare/{self.source}...main": {"status": "identical"},
            f"{self.prefix}/contents/.github?ref={self.source}": workflow,
            f"{self.prefix}/contents/.github?ref=main": workflow,
            f"{self.prefix}/actions/workflows/ci.yml/runs?head_sha={self.source}&event=push&per_page=100": {
                "workflow_runs": [{"id": 123, "head_sha": self.source, "status": "completed",
                                   "conclusion": "success", "event": "push",
                                   "path": ".github/workflows/ci.yml",
                                   "head_repository": {"full_name": "kedra-engine/kedra"}}],
            },
            f"{self.prefix}/commits/{self.source}/check-runs?per_page=100": {
                "check_runs": [{"name": "CI required", "app": {"id": 15368}, "conclusion": "success"}],
            },
            f"{self.prefix}/releases?per_page=100": [self.draft],
        }
        with patch.dict(os.environ, {"GITHUB_REPOSITORY": "kedra-engine/kedra"}), \
                patch("sys.argv", ["release.py", "--source", self.source, "--tag", self.tag,
                                   "--publish-directory", str(self.root)]), \
                patch.object(release, "api", side_effect=responses.__getitem__):
            with self.assertRaisesRegex(ValueError, "Release already exists"):
                release.main()
        self.assertFalse(self.created)
        self.assertFalse(self.published)


if __name__ == "__main__":
    unittest.main()
