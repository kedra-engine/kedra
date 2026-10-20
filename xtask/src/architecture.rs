//! Kedra dependency policy; Cargo schema parsing belongs to cargo_metadata.
use cargo_metadata::{DependencyKind, Metadata, MetadataCommand};

const LAYERS: &[&str] = &[
    "core",
    "runtime",
    "authoring",
    "import",
    "editor",
    "tooling",
];

/// Whether a production dependency between two package layers is permitted.
///
/// ```
/// assert!(xtask::allowed("editor", "runtime"));
/// assert!(!xtask::allowed("runtime", "editor"));
/// ```
pub fn allowed(from: &str, to: &str) -> bool {
    match from {
        "core" => to == "core",
        "runtime" => matches!(to, "core" | "runtime"),
        "authoring" => matches!(to, "core" | "runtime" | "authoring"),
        "import" => matches!(to, "core" | "runtime" | "authoring" | "import"),
        "editor" => to != "tooling" && LAYERS.contains(&to),
        "tooling" => LAYERS.contains(&to),
        _ => false,
    }
}

struct Package<'a> {
    name: &'a str,
    layer: Option<&'a str>,
    license: Option<&'a str>,
    production_dependencies: Vec<&'a str>,
}

pub(crate) fn check() -> crate::Result<String> {
    let metadata = MetadataCommand::new()
        .no_deps()
        .other_options(vec!["--locked".into()])
        .exec()?;
    validate_metadata(&metadata)?;
    Ok(format!(
        "Architecture policy passed for {} workspace package(s).",
        metadata.workspace_members.len()
    ))
}

fn validate_metadata(metadata: &Metadata) -> crate::Result<()> {
    let members = metadata.workspace_packages();
    for package in &members {
        validate_lint_inheritance(package.manifest_path.as_std_path())?;
    }
    let packages: Vec<_> = members
        .into_iter()
        .map(|package| Package {
            name: package.name.as_ref(),
            layer: package.metadata["kedra"]["layer"].as_str(),
            license: package.license.as_deref(),
            // Include build, optional, renamed and inactive target dependencies.
            production_dependencies: package
                .dependencies
                .iter()
                .filter(|dependency| dependency.kind != DependencyKind::Development)
                .map(|dependency| dependency.name.as_str())
                .collect(),
        })
        .collect();
    validate(&packages).map_err(Into::into)
}

fn validate_lint_inheritance(path: &std::path::Path) -> crate::Result<()> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("{}: cannot read manifest: {error}", path.display()))?;
    let manifest = source
        .parse::<toml::Table>()
        .map_err(|error| format!("{}: invalid manifest: {error}", path.display()))?;
    let inherits = manifest
        .get("lints")
        .and_then(toml::Value::as_table)
        .and_then(|lints| lints.get("workspace"))
        .and_then(toml::Value::as_bool);
    if inherits != Some(true) {
        return Err(format!(
            "{}: every workspace package must declare [lints] workspace = true",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn validate(packages: &[Package<'_>]) -> std::result::Result<(), String> {
    if packages.is_empty() {
        return Err("workspace has no packages".into());
    }
    let mut layers = std::collections::BTreeMap::new();
    for package in packages {
        let name = package.name;
        let layer = package
            .layer
            .ok_or_else(|| format!("{name}: missing package.metadata.kedra.layer"))?;
        if !LAYERS.contains(&layer) {
            return Err(format!("{name}: unknown layer {layer}"));
        }
        if package.license != Some("MIT OR Apache-2.0") {
            return Err(format!("{name}: expected MIT OR Apache-2.0 license"));
        }
        layers.insert(name, layer);
    }
    for package in packages {
        let from = layers[package.name];
        for target in &package.production_dependencies {
            if let Some(to) = layers.get(target)
                && !allowed(from, to)
            {
                return Err(format!(
                    "forbidden dependency: {} ({from}) -> {target} ({to})",
                    package.name
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package<'a>(name: &'a str, layer: &'a str, dependencies: Vec<&'a str>) -> Package<'a> {
        Package {
            name,
            layer: Some(layer),
            license: Some("MIT OR Apache-2.0"),
            production_dependencies: dependencies,
        }
    }

    #[test]
    fn runtime_cannot_depend_on_host_layers() {
        for layer in ["editor", "import", "authoring", "tooling"] {
            assert!(
                validate(&[
                    package("game", "runtime", vec!["host"]),
                    package("host", layer, vec![])
                ])
                .is_err()
            );
        }
    }

    #[test]
    fn editor_can_use_runtime_and_external_dependencies() {
        assert!(
            validate(&[
                package("host", "editor", vec!["game", "bevy"]),
                package("game", "runtime", vec![])
            ])
            .is_ok()
        );
    }

    #[test]
    fn invalid_or_missing_policy_fails_closed() {
        for layer in [None, Some("typo")] {
            let mut item = package("game", "core", vec![]);
            item.layer = layer;
            assert!(validate(&[item]).is_err());
        }
        assert!(validate(&[]).is_err());
        for license in [None, Some("UNLICENSED")] {
            let mut item = package("game", "core", vec![]);
            item.license = license;
            assert!(validate(&[item]).is_err());
        }
    }

    #[test]
    fn allowed_edges_are_closed_under_transitive_composition() {
        for a in LAYERS {
            for b in LAYERS {
                for c in LAYERS {
                    if allowed(a, b) && allowed(b, c) {
                        assert!(allowed(a, c), "{a} -> {b} -> {c}");
                    }
                }
            }
        }
    }

    #[test]
    fn cargo_metadata_adapter_checks_optional_renamed_and_target_edges() -> crate::Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"game\", \"host\"]\nresolver = \"3\"\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n",
        )?;
        for (name, layer) in [("game", "runtime"), ("host", "editor")] {
            std::fs::create_dir_all(root.path().join(name).join("src"))?;
            std::fs::write(root.path().join(name).join("src/lib.rs"), "")?;
            std::fs::write(
                root.path().join(name).join("Cargo.toml"),
                format!(
                    "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nlicense = \"MIT OR Apache-2.0\"\n[package.metadata.kedra]\nlayer = \"{layer}\"\n[lints]\nworkspace = true\n"
                ),
            )?;
        }
        let manifest = root.path().join("game/Cargo.toml");
        let base = std::fs::read_to_string(&manifest)?;
        for (section, optional, permitted) in [
            ("dependencies", "optional = true", false),
            ("target.'cfg(windows)'.build-dependencies", "", false),
            ("dev-dependencies", "", true),
        ] {
            std::fs::write(
                &manifest,
                format!(
                    "{base}\n[{section}.alias]\npackage = \"host\"\npath = \"../host\"\n{optional}\n"
                ),
            )?;
            let metadata = MetadataCommand::new()
                .current_dir(root.path())
                .no_deps()
                .exec()?;
            assert_eq!(validate_metadata(&metadata).is_ok(), permitted, "{section}");
        }
        Ok(())
    }

    #[test]
    fn new_workspace_members_must_inherit_lints() -> crate::Result<()> {
        let root = tempfile::tempdir()?;
        let manifest = root.path().join("Cargo.toml");
        let source = "[package]\nname = \"lint_fixture\"\nversion = \"0.0.0\"\nlicense = \"MIT OR Apache-2.0\"\n[package.metadata.kedra]\nlayer = \"core\"\n[workspace]\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n";
        std::fs::create_dir(root.path().join("src"))?;
        std::fs::write(root.path().join("src/lib.rs"), "")?;
        std::fs::write(&manifest, source)?;
        let metadata = MetadataCommand::new()
            .current_dir(root.path())
            .no_deps()
            .exec()?;
        assert!(
            validate_metadata(&metadata).is_err(),
            "missing lint inheritance must fail"
        );
        std::fs::write(&manifest, format!("{source}\n[lints]\nworkspace = true\n"))?;
        let metadata = MetadataCommand::new()
            .current_dir(root.path())
            .no_deps()
            .exec()?;
        assert!(validate_metadata(&metadata).is_ok());
        Ok(())
    }
}
