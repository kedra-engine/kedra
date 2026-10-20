//! Narrow dependency exceptions must not become permanent blanket waivers.
use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

const REVIEW_DEADLINE: u64 = 1_798_243_200; // 2026-12-26T00:00:00Z

fn parser_exception_valid(now: u64, version: &str, callers: &BTreeSet<String>) -> bool {
    now < REVIEW_DEADLINE
        && version == "0.25.1"
        && callers == &BTreeSet::from(["owned_ttf_parser@0.25.1".into()])
}

#[test]
fn parser_exception_rejects_expiry_changed_version_and_new_callers() {
    let callers = BTreeSet::from(["owned_ttf_parser@0.25.1".into()]);
    assert!(parser_exception_valid(
        REVIEW_DEADLINE - 1,
        "0.25.1",
        &callers
    ));
    assert!(!parser_exception_valid(REVIEW_DEADLINE, "0.25.1", &callers));
    assert!(!parser_exception_valid(0, "0.25.2", &callers));
    let mut changed = callers;
    changed.insert("another-font-consumer@1.0.0".into());
    assert!(!parser_exception_valid(0, "0.25.1", &changed));
}

#[test]
fn unmaintained_exception_is_time_bounded_and_only_reaches_existing_wrapper()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root missing")?;
    let policy: toml::Value = toml::from_str(&std::fs::read_to_string(root.join("deny.toml"))?)?;
    let ignored = policy["advisories"]["ignore"]
        .as_array()
        .ok_or("advisory ignores missing")?;
    assert_eq!(
        ignored.len(),
        1,
        "new advisory exceptions require explicit review"
    );
    assert_eq!(ignored[0]["id"].as_str(), Some("RUSTSEC-2026-0192"));
    // All platforms remain in the audited graph without requiring their source
    // archives in this host's Cargo cache. Cargo owns lock freshness via --locked;
    // cargo-lock owns format and exact dependency identity resolution.
    let lockfile = cargo_lock::Lockfile::load(root.join("Cargo.lock"))?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    assert!(
        locked_parser_exception_valid(&lockfile, now),
        "Re-review RUSTSEC-2026-0192: expired deadline, missing/changed parser, or changed immediate caller in Cargo.lock"
    );
    Ok(())
}

fn locked_parser_exception_valid(lockfile: &cargo_lock::Lockfile, now: u64) -> bool {
    let parsers: Vec<_> = lockfile
        .packages
        .iter()
        .filter(|p| p.name.as_str() == "ttf-parser")
        .collect();
    if parsers.len() != 1 {
        return false;
    }
    let parser = parsers[0];
    let callers = lockfile
        .packages
        .iter()
        .filter(|package| package.dependencies.iter().any(|dep| dep.matches(parser)))
        .map(|package| format!("{}@{}", package.name, package.version))
        .collect();
    parser_exception_valid(now, &parser.version.to_string(), &callers)
}

#[test]
fn locked_graph_policy_is_cache_independent_and_checks_every_caller()
-> Result<(), Box<dyn std::error::Error>> {
    // Foreign-platform package identities need no registry or package sources.
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
    let graph: cargo_lock::Lockfile = fixture.parse()?;
    assert!(locked_parser_exception_valid(&graph, REVIEW_DEADLINE - 1));
    assert!(!locked_parser_exception_valid(&graph, REVIEW_DEADLINE));
    let added_caller = format!(
        "{fixture}\n[[package]]\nname = \"another-font-consumer\"\nversion = \"1.0.0\"\ndependencies = [\"ttf-parser\"]\n"
    );
    assert!(!locked_parser_exception_valid(&added_caller.parse()?, 0));
    let changed = fixture.replace(
        "name = \"ttf-parser\"\nversion = \"0.25.1\"",
        "name = \"ttf-parser\"\nversion = \"0.25.2\"",
    );
    assert!(!locked_parser_exception_valid(&changed.parse()?, 0));
    let changed_wrapper = fixture.replace(
        "name = \"owned_ttf_parser\"\nversion = \"0.25.1\"",
        "name = \"owned_ttf_parser\"\nversion = \"0.25.2\"",
    );
    assert!(!locked_parser_exception_valid(&changed_wrapper.parse()?, 0));
    let duplicate =
        format!("{fixture}\n[[package]]\nname = \"ttf-parser\"\nversion = \"0.26.0\"\n");
    assert!(!locked_parser_exception_valid(&duplicate.parse()?, 0));
    assert!(!locked_parser_exception_valid(&"version = 4\n".parse()?, 0));
    Ok(())
}

#[test]
fn policy_runs_without_cargo_or_a_populated_package_cache() -> Result<(), Box<dyn std::error::Error>>
{
    let cache = tempfile::tempdir()?;
    let output = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "unmaintained_exception_is_time_bounded_and_only_reaches_existing_wrapper",
        ])
        .env("CARGO_HOME", cache.path())
        .env("CARGO", cache.path().join("cargo-must-not-be-invoked"))
        .output()?;
    assert!(
        output.status.success(),
        "offline lock policy unexpectedly invoked Cargo or needed cached packages: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::read_dir(cache.path())?.next().is_none());
    Ok(())
}

#[test]
fn duplicate_skips_are_exact_not_subtree_or_global_waivers()
-> Result<(), Box<dyn std::error::Error>> {
    let policy: toml::Value = toml::from_str(include_str!("../../deny.toml"))?;
    assert_eq!(policy["bans"]["multiple-versions"].as_str(), Some("deny"));
    assert_eq!(policy["bans"]["wildcards"].as_str(), Some("deny"));
    assert!(policy["bans"].get("skip-tree").is_none());
    let mut identities = BTreeSet::new();
    for skip in policy["bans"]["skip"]
        .as_array()
        .ok_or("duplicate exceptions missing")?
    {
        let name = skip["name"].as_str().ok_or("skip name missing")?;
        let version = skip["version"].as_str().ok_or("skip version missing")?;
        assert!(version.starts_with('='));
        let parsed = cargo_metadata::semver::Version::parse(&version[1..])?;
        assert_eq!(version, format!("={parsed}"));
        assert!(identities.insert((name, version)), "duplicate skip entry");
    }
    assert!(
        !policy["licenses"]["allow"]
            .as_array()
            .ok_or("license policy missing")?
            .iter()
            .any(|license| license.as_str() == Some("MPL-2.0"))
    );
    Ok(())
}
