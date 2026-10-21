//! Validated consumer identity, independent of this library's package version.
use semver::Version;

/// Trusted consumer-supplied identity for the existing GitHub/inventory protocol.
///
/// Configuration selects the trust root; never populate it from downloaded metadata.
/// Clones retain the same immutable identity and application version.
#[derive(Clone, Debug)]
pub struct Config {
    pub(crate) repository: String,
    pub(crate) application: String,
    pub(crate) bundle_identifier: String,
    pub(crate) executable: String,
    pub(crate) bundle_name: String,
    pub(crate) current: Version,
}

fn segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && !value.ends_with('.')
}

impl Config {
    /// Validates a GitHub `owner/repository`, artifact/application name, macOS
    /// bundle identifier, executable basename, `.app` basename and running version.
    ///
    /// Returns an error for URL/path syntax, empty identifiers, unsupported basename
    /// characters or noncanonical bundle names. It performs no IO and authorizes
    /// neither downloading nor installation. The consumer version, not the library
    /// version, controls newer-release and installation-marker checks.
    pub fn new(
        repository: &str,
        application: &str,
        bundle_identifier: &str,
        executable: &str,
        bundle_name: &str,
        current: Version,
    ) -> Result<Self, String> {
        let parts: Vec<_> = repository.split('/').collect();
        if parts.len() != 2 || !parts.iter().all(|part| segment(part)) {
            return Err("Update repository must be a GitHub owner/repository identity".into());
        }
        if !segment(application)
            || !segment(executable)
            || !segment(bundle_name)
            || !bundle_name.ends_with(".app")
            || bundle_name == ".app"
            || !bundle_identifier.contains('.')
            || !bundle_identifier.split('.').all(segment)
        {
            return Err("Invalid update application, executable or bundle identity".into());
        }
        Ok(Self {
            repository: repository.into(),
            application: application.into(),
            bundle_identifier: bundle_identifier.into(),
            executable: executable.into(),
            bundle_name: bundle_name.into(),
            current,
        })
    }

    /// Running application version supplied by the consumer.
    pub fn current_version(&self) -> &Version {
        &self.current
    }
}

#[cfg(test)]
pub(crate) fn fixture() -> Config {
    Config::new(
        "kedra-engine/kedra",
        "kedra",
        "dev.kedra.editor",
        "kedra",
        "Kedra.app",
        Version::new(0, 1, 0),
    )
    .unwrap_or_else(|error| panic!("Invalid fixture configuration: {error}"))
}
