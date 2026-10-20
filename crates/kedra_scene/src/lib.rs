//! Authored component graphs compiled and instantiated through ordinary Bevy.
//!
//! Source documents, immutable compilations and live instances are separate
//! owners. No editor, import process, filesystem catalog scan or Save writer is
//! installed by this crate. Consumers register an explicit component policy and
//! native asset source, then load catalogs using [`assets::PrefabPlugin`].
pub mod assets;
pub mod composition;
pub mod document;
pub mod environment;
pub mod instance;
pub mod reflection;
#[cfg(feature = "render")]
pub mod rendering;
pub mod spatial;
mod value;

/// Source, composition, binding or instantiation failure with contextual detail.
/// Invalid candidates never replace a published source revision.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);

/// A fallible scene operation. Failure does not authorize substituting a source.
pub type Result<T> = std::result::Result<T, Error>;

impl From<ron::error::SpannedError> for Error {
    fn from(error: ron::error::SpannedError) -> Self {
        Self(error.to_string())
    }
}
impl From<ron::Error> for Error {
    fn from(error: ron::Error) -> Self {
        Self(error.to_string())
    }
}

pub(crate) fn failure(message: impl Into<String>) -> Error {
    Error(message.into())
}
