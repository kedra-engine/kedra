"""Release preflight fails closed on source identity and genuine CI results."""
import unittest

from release import validate_identity, validate_ci, validate_package_result, validate_workflow_trees


class ReleasePolicyTests(unittest.TestCase):
    def test_source_workflows_must_match_default_branch_for_github_token(self):
        workflows = {"name": "workflows", "type": "dir", "sha": "a" * 40}
        validate_workflow_trees([workflows], [workflows])
        validate_workflow_trees([workflows, {"name": "ci-paths.yml", "type": "file", "sha": "b" * 40}],
                                [workflows])
        for source, current in (([], [workflows]), ([workflows], []),
                                ([workflows], [dict(workflows, sha="b" * 40)])):
            with self.subTest(source=source, current=current), self.assertRaises(ValueError):
                validate_workflow_trees(source, current)

    def test_publisher_requires_successful_scheduler_package_result(self):
        validate_package_result("success")
        for result in ["failure", "cancelled", "skipped", "in_progress", "", None]:
            with self.subTest(result=result), self.assertRaises(ValueError):
                validate_package_result(result)

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
