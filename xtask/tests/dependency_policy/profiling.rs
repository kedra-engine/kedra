//! Owner-approved macro advisory scope, not general dependency pin assertions.

use super::REVIEW_DEADLINE;
use cargo_lock::Lockfile;
use std::time::{SystemTime, UNIX_EPOCH};

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
const CHAIN: [(&str, &str); 3] = [
    ("paste", "v1.0.15"),
    ("perfetto-sdk", "v1.2.1"),
    ("kedra_profiling", "v0.1.3"),
];

fn valid(lock: &Lockfile, now: u64, chain: &[(&str, &str)]) -> bool {
    if now >= REVIEW_DEADLINE || chain != CHAIN {
        return false;
    }
    CHAIN.iter().all(|(name, version)| {
        let packages: Vec<_> = lock
            .packages
            .iter()
            .filter(|package| package.name.as_str() == *name)
            .collect();
        packages.len() == 1
            && packages[0].version.to_string() == version[1..]
            && if *name == "kedra_profiling" {
                packages[0].source.is_none()
            } else {
                packages[0]
                    .source
                    .as_ref()
                    .is_some_and(|source| source.to_string() == REGISTRY)
            }
    })
}

fn main_lock() -> Result<Lockfile, Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root missing")?;
    Ok(Lockfile::load(root.join("Cargo.lock"))?)
}

#[test]
fn reviewed_paste_exception_checks_locked_reachability_and_expiry()
-> Result<(), Box<dyn std::error::Error>> {
    // Cargo.lock includes inactive optional image-codec chains. Cargo owns
    // feature/target reachability: inspect its real all-target inverse tree,
    // rather than treating every optional lock edge as active or writing a walker.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root missing")?;
    let output = std::process::Command::new(env!("CARGO"))
        .current_dir(root)
        .args([
            "tree",
            "--color",
            "never",
            "--locked",
            "--offline",
            "--all-features",
            "--target",
            "all",
            "--invert",
            "paste",
            "--depth",
            "2",
            "--prefix",
            "none",
            "--no-dedupe",
            "--format",
            "{p}",
        ])
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    let chain: Vec<_> = text
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            (
                fields.next().unwrap_or_default(),
                fields.next().unwrap_or_default(),
            )
        })
        .collect();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    assert!(
        valid(&main_lock()?, now, &chain),
        "Re-review RUSTSEC-2024-0436: expired deadline or changed paste/Perfetto versions, sources or active callers: {text}"
    );
    Ok(())
}

#[test]
fn paste_exception_rejects_expiry_foreign_sources_and_expanded_reachability()
-> Result<(), Box<dyn std::error::Error>> {
    let lock = main_lock()?;
    assert!(valid(&lock, REVIEW_DEADLINE - 1, &CHAIN));
    assert!(!valid(&lock, REVIEW_DEADLINE, &CHAIN));
    for name in ["paste", "perfetto-sdk", "kedra_profiling"] {
        let index = lock
            .packages
            .iter()
            .position(|package| package.name.as_str() == name)
            .ok_or("reviewed package missing")?;
        let mut changed = lock.clone();
        changed.packages[index].version = "99.0.0".parse()?;
        assert!(!valid(&changed, 0, &CHAIN), "accepted changed {name}");
        let mut changed = lock.clone();
        changed.packages[index].source = Some(
            "git+https://example.invalid/unreviewed?rev=1234567890123456789012345678901234567890#1234567890123456789012345678901234567890".parse()?);
        assert!(!valid(&changed, 0, &CHAIN), "accepted foreign {name}");
        let mut duplicate = lock.clone();
        duplicate.packages.push(lock.packages[index].clone());
        assert!(!valid(&duplicate, 0, &CHAIN));
    }
    for index in 0..CHAIN.len() {
        let mut changed = CHAIN.to_vec();
        changed[index] = ("unreviewed-consumer", "v1.0.0");
        assert!(!valid(&lock, 0, &changed));
    }
    let mut expanded = CHAIN.to_vec();
    expanded.push(("unreviewed-consumer", "v1.0.0"));
    assert!(!valid(&lock, 0, &expanded));
    assert!(!valid(&lock, 0, &CHAIN[..2]));
    assert!(!valid(&"version = 4\n".parse()?, 0, &CHAIN));
    Ok(())
}
