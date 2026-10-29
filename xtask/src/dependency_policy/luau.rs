//! Exact reachability and expiry for the reviewed BMS macro advisory exception.

use cargo_lock::{Lockfile, Package};
use std::collections::BTreeSet;

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

pub(super) fn valid(lock: &Lockfile) -> bool {
    // No affected package in a lock means the exception grants no permission
    // there; new affected callers still fail.
    if !lock
        .packages
        .iter()
        .any(|p| p.name.as_str() == "proc-macro-error2")
    {
        return true;
    }
    [
        ("proc-macro-error2", "2.0.1", &["mlua_derive@0.11.0"][..]),
        ("mlua_derive", "0.11.0", &["mlua@0.11.6"][..]),
        ("mlua", "0.11.6", &["bevy_mod_scripting_lua@0.23.0"][..]),
        (
            "bevy_mod_scripting_lua",
            "0.23.0",
            &[
                "bevy_mod_scripting@0.23.0",
                "bevy_mod_scripting_functions@0.23.0",
            ][..],
        ),
    ]
    .into_iter()
    .all(|(name, version, callers)| exact_package(lock, name, version, callers))
}

fn exact_package(lock: &Lockfile, name: &str, version: &str, allowed: &[&str]) -> bool {
    let packages: Vec<_> = lock
        .packages
        .iter()
        .filter(|p| p.name.as_str() == name)
        .collect();
    if packages.len() != 1 {
        return false;
    }
    let package = packages[0];
    if package.version.to_string() != version
        || package
            .source
            .as_ref()
            .is_none_or(|source| source.to_string() != REGISTRY)
    {
        return false;
    }
    let callers: Vec<&Package> = lock
        .packages
        .iter()
        .filter(|p| p.dependencies.iter().any(|dep| dep.matches(package)))
        .collect();
    let identities: BTreeSet<_> = callers
        .iter()
        .map(|p| format!("{}@{}", p.name, p.version))
        .collect();
    identities == allowed.iter().map(|name| (*name).to_owned()).collect()
        && callers.iter().all(|p| {
            p.source
                .as_ref()
                .is_some_and(|source| source.to_string() == REGISTRY)
        })
}

// A small Cargo-format graph exercises the policy independently of Cargo/network
// caches. Actual resolved consumer graphs use the same validator above.
#[cfg(test)]
const GRAPH: &str = r#"
version = 4
[[package]]
name = "proc-macro-error2"
version = "2.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
[[package]]
name = "mlua_derive"
version = "0.11.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["proc-macro-error2"]
[[package]]
name = "mlua"
version = "0.11.6"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["mlua_derive"]
[[package]]
name = "bevy_mod_scripting_lua"
version = "0.23.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["mlua"]
[[package]]
name = "bevy_mod_scripting_functions"
version = "0.23.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["bevy_mod_scripting_lua"]
[[package]]
name = "bevy_mod_scripting"
version = "0.23.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["bevy_mod_scripting_lua", "bevy_mod_scripting_functions"]
"#;

#[test]
fn macro_exception_rejects_versions_sources_and_any_new_caller()
-> Result<(), Box<dyn std::error::Error>> {
    let lock: Lockfile = GRAPH.parse()?;
    assert!(valid(&lock));
    for (name, version) in [
        ("proc-macro-error2", "2.0.1"),
        ("mlua_derive", "0.11.0"),
        ("mlua", "0.11.6"),
        ("bevy_mod_scripting_lua", "0.23.0"),
        ("bevy_mod_scripting_functions", "0.23.0"),
        ("bevy_mod_scripting", "0.23.0"),
    ] {
        let changed = GRAPH.replace(
            &format!("name = \"{name}\"\nversion = \"{version}\""),
            &format!("name = \"{name}\"\nversion = \"99.0.0\""),
        );
        assert!(!valid(&changed.parse()?), "Accepted changed {name}");
    }
    for target in [
        "proc-macro-error2",
        "mlua_derive",
        "mlua",
        "bevy_mod_scripting_lua",
    ] {
        let extra = format!(
            "{GRAPH}\n[[package]]\nname = \"new-caller\"\nversion = \"1.0.0\"\ndependencies = [\"{target}\"]\n"
        );
        assert!(!valid(&extra.parse()?), "Accepted new caller of {target}");
    }
    let no_sources = GRAPH.replace(&format!("source = \"{REGISTRY}\"\n"), "");
    assert!(!valid(&no_sources.parse()?));
    let duplicate =
        format!("{GRAPH}\n[[package]]\nname = \"proc-macro-error2\"\nversion = \"2.0.2\"\n");
    assert!(!valid(&duplicate.parse()?));
    let empty: Lockfile = "version = 4\n".parse()?;
    assert!(valid(&empty));
    Ok(())
}
