"""Release preflight fails closed on source identity and genuine CI results."""
import unittest

from release import validate_identity, validate_ci, validate_matrix


class ReleasePolicyTests(unittest.TestCase):
    def test_publisher_requires_each_actual_native_job_to_succeed(self):
        targets = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu",
                   "aarch64-apple-darwin", "x86_64-apple-darwin"]
        jobs = [{"name": f"Package ({target})", "status": "completed", "conclusion": "success"}
                for target in targets]
        validate_matrix(jobs)
        with self.assertRaises(ValueError):
            validate_matrix(jobs[:-1])
        with self.assertRaises(ValueError):
            validate_matrix(jobs + [jobs[0]])
        for conclusion in ["failure", "cancelled", "skipped", None]:
            with self.subTest(conclusion=conclusion), self.assertRaises(ValueError):
                validate_matrix([dict(jobs[0], conclusion=conclusion), *jobs[1:]])

    def test_identity_requires_exact_sha_and_cargo_version(self):
        sha = "a" * 40
        self.assertEqual(validate_identity(sha, "v0.1.0", "0.1.0"), (sha, "v0.1.0"))
        for source, tag, version in [
            ("main", "v0.1.0", "0.1.0"), (sha, "v0.2.0", "0.1.0"),
            (sha, "../../unsafe", "0.1.0"), (sha, "v0.1.0\n", "0.1.0"),
        ]:
            with self.subTest(source=source, tag=tag), self.assertRaises(ValueError):
                validate_identity(source, tag, version)

    def test_only_successful_source_repository_push_ci_is_accepted(self):
        sha = "a" * 40
        run = {"head_sha": sha, "conclusion": "success", "status": "completed",
               "event": "push", "path": ".github/workflows/ci.yml", "id": 123,
               "head_repository": {"full_name": "kedra-engine/kedra"}}
        self.assertEqual(validate_ci([run], sha, "kedra-engine/kedra"), 123)
        for field, value in [
            ("head_sha", "b" * 40), ("conclusion", "failure"),
            ("status", "in_progress"), ("event", "pull_request"),
            ("path", ".github/workflows/unrelated.yml"),
            ("head_repository", {"full_name": "untrusted/fork"}),
        ]:
            changed = dict(run, **{field: value})
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_ci([changed], sha, "kedra-engine/kedra")
        with self.assertRaises(ValueError):
            validate_ci([], sha, "kedra-engine/kedra")


if __name__ == "__main__":
    unittest.main()
