//! Read native package metadata through Cargo and the maintained packager schema.
use cargo_metadata::{MetadataCommand, Package, TargetKind};
use cargo_packager::Config;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub(crate) enum NativeTarget {
    #[value(name = "x86_64-pc-windows-msvc")]
    Windows,
    #[value(name = "x86_64-unknown-linux-gnu")]
    Linux,
    #[value(name = "aarch64-apple-darwin")]
    MacArm,
    #[value(name = "x86_64-apple-darwin")]
    MacIntel,
}

impl NativeTarget {
    pub fn host() -> Result<Self, String> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => Ok(Self::Windows),
            ("linux", "x86_64") => Ok(Self::Linux),
            ("macos", "aarch64") => Ok(Self::MacArm),
            ("macos", "x86_64") => Ok(Self::MacIntel),
            _ => Err("Unsupported build host; use a supported native desktop runner".into()),
        }
    }

    pub fn triple(self) -> &'static str {
        match self {
            Self::Windows => "x86_64-pc-windows-msvc",
            Self::Linux => "x86_64-unknown-linux-gnu",
            Self::MacArm => "aarch64-apple-darwin",
            Self::MacIntel => "x86_64-apple-darwin",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Distribution {
    pub assets: PathBuf,
    pub profile: String,
    pub namespace: String,
    pub icon: PathBuf,
    pub notices: Vec<PathBuf>,
    #[serde(default, rename = "license-config")]
    pub license_config: Option<PathBuf>,
}

pub(crate) struct Application {
    pub package: Package,
    pub workspace: PathBuf,
    pub target_directory: PathBuf,
    pub target: NativeTarget,
    pub binary: String,
    pub native: Config,
    pub distribution: Distribution,
}

impl Application {
    pub fn load(
        manifest: &Path,
        selected: Option<&str>,
        target: Option<NativeTarget>,
    ) -> Result<Self, String> {
        let target = target.unwrap_or(NativeTarget::host()?);
        if target != NativeTarget::host()? {
            return Err(format!(
                "Use a native {} host; cross-platform SDK bootstrapping is not supported",
                target.triple()
            ));
        }
        let manifest = manifest
            .canonicalize()
            .map_err(|e| format!("Application manifest: {e}"))?;
        let metadata = MetadataCommand::new()
            .current_dir(
                manifest
                    .parent()
                    .ok_or("Application manifest directory missing")?,
            )
            .manifest_path(&manifest)
            .no_deps()
            .exec()
            .map_err(|error| format!("Cargo metadata: {error}"))?;
        let candidates: Vec<_> = metadata
            .workspace_packages()
            .into_iter()
            .filter(|p| {
                selected.map_or_else(
                    || p.metadata.get("packager").is_some(),
                    |name| p.name.as_str() == name,
                )
            })
            .collect();
        let [package] = candidates.as_slice() else {
            return Err("Select exactly one application with --package; package.metadata.packager is required".into());
        };
        let package = (*package).clone();
        let mut native: Config = serde_json::from_value(
            package
                .metadata
                .get("packager")
                .cloned()
                .ok_or("Application requires package.metadata.packager")?,
        )
        .map_err(|error| format!("Invalid native package metadata: {error}"))?;
        if native
            .resources
            .as_ref()
            .is_some_and(|resources| !resources.is_empty())
        {
            return Err("Additional packager.resources are not supported; declare compiled delivery inputs in distribution.assets and license files in distribution.notices".into());
        }
        if native.appimage.as_ref().is_some_and(|config| {
            config
                .libs
                .as_ref()
                .is_some_and(|values| !values.is_empty())
                || config
                    .bins
                    .as_ref()
                    .is_some_and(|values| !values.is_empty())
                || config
                    .files
                    .as_ref()
                    .is_some_and(|values| !values.is_empty())
                || config
                    .linuxdeploy_plugins
                    .as_ref()
                    .is_some_and(|values| !values.is_empty())
        }) {
            return Err("Additional appimage libs, bins, files and plugins require explicit native provenance/source support and are not supported".into());
        }
        if native.product_name.is_empty()
            || native.product_name.contains(['/', '\\'])
            || native.product_name.chars().any(char::is_control)
        {
            return Err("Native productName must be a nonempty display name without path separators/control characters".into());
        }
        let identifier = native
            .identifier
            .as_deref()
            .ok_or("Native package identifier is required")?;
        if identifier.is_empty()
            || !identifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(
                "Native identifier must contain only ASCII letters, digits, periods or hyphens"
                    .into(),
            );
        }
        if !native.version.is_empty() && native.version != package.version.to_string() {
            return Err("Native package version differs from Cargo package version".into());
        }
        native.version = package.version.to_string();
        if matches!(target, NativeTarget::MacArm | NativeTarget::MacIntel) {
            // The linker signature does not seal a complete .app. Let the
            // maintained packager sign nested code and resources inside-out.
            native
                .macos
                .get_or_insert_with(Default::default)
                .signing_identity
                .get_or_insert_with(|| "-".into());
        }
        let binaries: Vec<_> = package
            .targets
            .iter()
            .filter(|t| t.kind.contains(&TargetKind::Bin))
            .collect();
        let [binary] = binaries.as_slice() else {
            return Err("Application must have exactly one Cargo binary target".into());
        };
        let binary = binary.name.clone();
        let mut distribution: Distribution = serde_json::from_value(
            package.metadata["kedra"]
                .get("distribution")
                .cloned()
                .ok_or("Application requires package.metadata.kedra.distribution")?,
        )
        .map_err(|error| format!("Invalid distribution metadata: {error}"))?;
        let workspace = metadata
            .workspace_root
            .as_std_path()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let package_root = package
            .manifest_path
            .parent()
            .ok_or("Package directory missing")?
            .as_std_path();
        let resolve = |path: &Path| -> Result<PathBuf, String> {
            let path = package_root
                .join(path)
                .canonicalize()
                .map_err(|e| format!("Distribution input {}: {e}", path.display()))?;
            if !path.starts_with(&workspace) {
                return Err(
                    "Distribution inputs must remain inside the application workspace".into(),
                );
            }
            Ok(path)
        };
        distribution.assets = resolve(&distribution.assets)?;
        distribution.icon = resolve(&distribution.icon)?;
        distribution.license_config = distribution
            .license_config
            .as_deref()
            .map(resolve)
            .transpose()?;
        distribution.notices = distribution
            .notices
            .iter()
            .map(|p| resolve(p))
            .collect::<Result<_, _>>()?;
        if !distribution.assets.is_dir()
            || !distribution.icon.is_file()
            || distribution.notices.iter().any(|p| !p.is_file())
        {
            return Err("Assets must be a directory; icon/notices must be files".into());
        }
        Ok(Self {
            package,
            workspace,
            target_directory: metadata.target_directory.into_std_path_buf(),
            target,
            binary,
            native,
            distribution,
        })
    }
}
