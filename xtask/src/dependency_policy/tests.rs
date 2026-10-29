//! Reject broadened exception policy independently of the current checkout/date.
use super::*;

const POLICY: &str = r#"
[advisories]
ignore = [
  { id = "RUSTSEC-2026-0192", reason = "reviewed" },
  { id = "RUSTSEC-2026-0173", reason = "reviewed" },
  { id = "RUSTSEC-2024-0436", reason = "reviewed" },
]
[bans]
multiple-versions = "deny"
wildcards = "deny"
skip = [{name = "example", version = "=1.0.0"}]
[licenses]
allow = ["MIT", "Apache-2.0"]
"#;

#[test]
fn expiry_and_broadened_waivers_fail_closed() -> crate::Result<()> {
    let policy: toml::Value = toml::from_str(POLICY)?;
    validate_policy(&policy, REVIEW_DEADLINE - 1)?;
    assert!(validate_policy(&policy, REVIEW_DEADLINE).is_err());
    for (from, to) in [
        ("RUSTSEC-2026-0192", "UNREVIEWED"),
        ("reason = \"reviewed\"", "reason = \"\""),
        (
            "multiple-versions = \"deny\"",
            "multiple-versions = \"allow\"",
        ),
        ("wildcards = \"deny\"", "wildcards = \"allow\""),
        ("[bans]", "[bans]\nskip-tree = []"),
        ("=1.0.0", "^1.0.0"),
        (
            "skip = [{name = \"example\", version = \"=1.0.0\"}]",
            "skip = [{name = \"example\", version = \"=1.0.0\"}, {name = \"example\", version = \"=1.0.0\"}]",
        ),
        ("\"MIT\", \"Apache-2.0\"", "\"MIT\", \"MPL-2.0\""),
    ] {
        assert!(
            validate_policy(&toml::from_str(&POLICY.replace(from, to))?, 0).is_err(),
            "Accepted {to}"
        );
    }
    assert!(validate_policy(&toml::from_str("")?, 0).is_err());
    Ok(())
}

#[test]
fn parser_exception_checks_exact_versions_and_every_caller() -> crate::Result<()> {
    let fixture = r#"
version = 4
[[package]]
name = "ttf-parser"
version = "0.25.1"
[[package]]
name = "owned_ttf_parser"
version = "0.25.1"
dependencies = ["ttf-parser"]
[[package]]
name = "foreign-target-only"
version = "123.0.0"
"#;
    assert!(locked_parser_exception_valid(&fixture.parse()?));
    for changed in [
        format!(
            "{fixture}\n[[package]]\nname = \"another-font-consumer\"\nversion = \"1.0.0\"\ndependencies = [\"ttf-parser\"]\n"
        ),
        fixture.replace(
            "name = \"ttf-parser\"\nversion = \"0.25.1\"",
            "name = \"ttf-parser\"\nversion = \"0.25.2\"",
        ),
        fixture.replace(
            "name = \"owned_ttf_parser\"\nversion = \"0.25.1\"",
            "name = \"owned_ttf_parser\"\nversion = \"0.25.2\"",
        ),
        format!("{fixture}\n[[package]]\nname = \"ttf-parser\"\nversion = \"0.26.0\"\n"),
        "version = 4\n".into(),
    ] {
        assert!(!locked_parser_exception_valid(&changed.parse()?));
    }
    Ok(())
}
