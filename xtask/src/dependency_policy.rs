//! Repository exception admission, independent of Cargo execution and caches.
use std::{
    collections::BTreeSet,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

mod luau;
#[cfg(test)]
mod tests;

const REVIEW_DEADLINE: u64 = 1_798_243_200; // 2026-12-26T00:00:00Z

pub(crate) fn check(root: &Path) -> crate::Result<String> {
    let policy = toml::from_str(&std::fs::read_to_string(root.join("deny.toml"))?)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    validate_policy(&policy, now)?;
    let lock = cargo_lock::Lockfile::load(root.join("Cargo.lock"))?;
    if !locked_parser_exception_valid(&lock) {
        return Err(
            "Re-review RUSTSEC-2026-0192: missing/changed parser or immediate callers".into(),
        );
    }
    if !luau::valid(&lock) {
        return Err(
            "Re-review RUSTSEC-2026-0173: changed macro versions, sources or callers".into(),
        );
    }
    if let Some(path) = std::env::var_os("KEDRA_LUAU_LOCKFILE")
        && !luau::valid(&cargo_lock::Lockfile::load(std::path::PathBuf::from(path))?)
    {
        return Err("Supplied consumer lock exceeds the reviewed Luau exception".into());
    }
    Ok("Dependency exception policy passed".into())
}

fn validate_policy(policy: &toml::Value, now: u64) -> crate::Result<()> {
    if now >= REVIEW_DEADLINE {
        return Err("Dependency advisory exceptions require review by 2026-12-26 UTC".into());
    }
    let ignored = policy
        .get("advisories")
        .and_then(|p| p.get("ignore"))
        .and_then(toml::Value::as_array)
        .ok_or("Advisory ignores missing")?;
    let ids: BTreeSet<_> = ignored
        .iter()
        .filter_map(|entry| entry.get("id").and_then(toml::Value::as_str))
        .collect();
    if ignored.len() != 3
        || ids
            != BTreeSet::from([
                "RUSTSEC-2026-0192",
                "RUSTSEC-2026-0173",
                "RUSTSEC-2024-0436",
            ])
        || ignored.iter().any(|entry| {
            entry
                .get("reason")
                .and_then(toml::Value::as_str)
                .is_none_or(str::is_empty)
        })
    {
        return Err("Advisory exceptions require exact reviewed identities and reasons".into());
    }
    let bans = policy.get("bans").ok_or("Dependency bans missing")?;
    if bans.get("multiple-versions").and_then(toml::Value::as_str) != Some("deny")
        || bans.get("wildcards").and_then(toml::Value::as_str) != Some("deny")
        || bans.get("skip-tree").is_some()
    {
        return Err("Duplicate exceptions must not become global or subtree waivers".into());
    }
    let mut identities = BTreeSet::new();
    for skip in bans
        .get("skip")
        .and_then(toml::Value::as_array)
        .ok_or("Duplicate exceptions missing")?
    {
        let name = skip
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or("Skip name missing")?;
        let version = skip
            .get("version")
            .and_then(toml::Value::as_str)
            .ok_or("Skip version missing")?;
        let exact = version
            .strip_prefix('=')
            .ok_or("Skip must pin an exact version")?;
        let parsed = cargo_metadata::semver::Version::parse(exact)?;
        if exact != parsed.to_string() || !identities.insert((name, version)) {
            return Err("Duplicate or noncanonical dependency exception".into());
        }
    }
    let licenses = policy
        .get("licenses")
        .and_then(|p| p.get("allow"))
        .and_then(toml::Value::as_array)
        .ok_or("License policy missing")?;
    if licenses
        .iter()
        .any(|license| license.as_str() == Some("MPL-2.0"))
    {
        return Err("MPL-2.0 requires exact package exceptions, not global admission".into());
    }
    Ok(())
}

fn locked_parser_exception_valid(lock: &cargo_lock::Lockfile) -> bool {
    let parsers: Vec<_> = lock
        .packages
        .iter()
        .filter(|p| p.name.as_str() == "ttf-parser")
        .collect();
    if parsers.len() != 1 {
        return false;
    }
    let parser = parsers[0];
    let callers: BTreeSet<_> = lock
        .packages
        .iter()
        .filter(|package| package.dependencies.iter().any(|dep| dep.matches(parser)))
        .map(|package| format!("{}@{}", package.name, package.version))
        .collect();
    parser.version.to_string() == "0.25.1"
        && callers == BTreeSet::from(["owned_ttf_parser@0.25.1".into()])
}
