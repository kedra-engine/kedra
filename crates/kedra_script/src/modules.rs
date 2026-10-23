//! Host-admitted source snapshots over mlua's standard Luau require resolver.
//!
//! Luau owns import syntax, aliases, configuration parsing and cache semantics.
//! This adapter only supplies a bounded virtual tree; it never opens the filesystem.
use crate::lua::{self, Lua};
use lua::NavigateError;
use lua::Require;
use std::{collections::BTreeMap, io};

/// An explicitly supplied snapshot of project-relative modules and `.luaurc` files.
///
/// Paths use `/`, are case-sensitive and cannot escape the snapshot. This is source
/// input to an authorized program, not a second persistent asset identity catalog.
/// Hosts obtain published project sources through their existing source authority;
/// embedded native programs can supply `include_str!` sources directly.
#[derive(Clone, Default)]
pub struct Sources(BTreeMap<String, String>);

impl Sources {
    /// Admit one canonical relative `.luau`, `.lua` or `.luaurc` file.
    /// Duplicate paths, invalid paths, files above 64 KiB and snapshots above
    /// 256 files / 8 MiB fail without changing the snapshot. No code is evaluated.
    pub fn insert(
        &mut self,
        path: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<(), String> {
        let path = path.into();
        let source = source.into();
        validate_path(&path)?;
        if source.len() > 64 * 1024
            || self.0.len() >= 256
            || self.0.values().map(String::len).sum::<usize>() + source.len() > 8 * 1024 * 1024
            || self.0.contains_key(&path)
        {
            return Err("Duplicate script source or source snapshot limit exceeded".into());
        }
        self.0.insert(path, source);
        Ok(())
    }
}

pub(super) fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 1024
        || path
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':'))
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        || !(path.ends_with(".luau") || path.ends_with(".lua") || path.ends_with(".luaurc"))
    {
        return Err("Expected a canonical project-relative Luau source path".into());
    }
    Ok(())
}

pub(super) fn install(lua: &Lua, sources: Sources, entry: &str, source: &str) -> lua::Result<()> {
    let mut sources = sources;
    sources.insert(entry, source).map_err(lua::Error::runtime)?;
    lua.globals().set(
        "require",
        lua.create_require_function(Navigator {
            sources,
            path: String::new(),
            resolved: None,
            identity_only: false,
        })?,
    )
}

struct Navigator {
    sources: Sources,
    path: String,
    resolved: Option<String>,
    identity_only: bool,
}

impl Navigator {
    fn navigate(&mut self, path: String) -> Result<(), NavigateError> {
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{path}/")
        };
        let mut matches = Vec::new();
        if !path.ends_with("/init") && path != "init" {
            for ext in ["luau", "lua"] {
                let candidate = format!("{path}.{ext}");
                if self.sources.0.contains_key(&candidate) {
                    matches.push(candidate);
                }
            }
        }
        for ext in ["luau", "lua"] {
            let candidate = format!("{prefix}init.{ext}");
            if self.sources.0.contains_key(&candidate) {
                matches.push(candidate);
            }
        }
        if matches.len() > 1 {
            return Err(NavigateError::Ambiguous);
        }
        if matches.is_empty() && !self.sources.0.keys().any(|key| key.starts_with(&prefix)) {
            return Err(NavigateError::NotFound);
        }
        self.path = path;
        self.resolved = matches.pop();
        Ok(())
    }
}

impl Require for Navigator {
    fn is_require_allowed(&self, chunk_name: &str) -> bool {
        chunk_name
            .strip_prefix('@')
            .is_some_and(|path| self.sources.0.contains_key(path))
    }

    fn reset(&mut self, chunk_name: &str) -> Result<(), NavigateError> {
        let path = chunk_name
            .strip_prefix('@')
            .ok_or(NavigateError::NotFound)?;
        if !self.sources.0.contains_key(path) {
            return Err(NavigateError::NotFound);
        }
        // A requiring chunk is already a file, unlike a require target without extension.
        self.path = path.to_owned();
        self.resolved = Some(path.to_owned());
        Ok(())
    }

    fn jump_to_alias(&mut self, _: &str) -> Result<(), NavigateError> {
        // Absolute or otherwise external alias targets never grant disk access.
        Err(NavigateError::NotFound)
    }

    fn to_parent(&mut self) -> Result<(), NavigateError> {
        if self.path.is_empty() {
            return Err(NavigateError::NotFound);
        }
        self.navigate(
            self.path
                .rsplit_once('/')
                .map_or("", |(parent, _)| parent)
                .to_owned(),
        )
    }

    fn to_child(&mut self, name: &str) -> Result<(), NavigateError> {
        if name.is_empty()
            || matches!(name, "." | "..")
            || name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
        {
            return Err(NavigateError::NotFound);
        }
        self.navigate(if self.path.is_empty() {
            name.to_owned()
        } else {
            format!("{}/{name}", self.path)
        })
    }

    fn has_module(&self) -> bool {
        self.resolved.is_some()
    }

    fn cache_key(&self) -> String {
        let path = self.resolved.clone().unwrap_or_default();
        if self.identity_only {
            format!("sequence:{path}")
        } else {
            path
        }
    }

    fn has_config(&self) -> bool {
        let path = if self.path.is_empty() {
            ".luaurc".into()
        } else {
            format!("{}/.luaurc", self.path)
        };
        self.sources.0.contains_key(&path)
    }

    fn config(&self) -> io::Result<Vec<u8>> {
        let path = if self.path.is_empty() {
            ".luaurc".into()
        } else {
            format!("{}/.luaurc", self.path)
        };
        self.sources
            .0
            .get(&path)
            .map(|text| text.as_bytes().to_vec())
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn loader(&self, lua: &Lua) -> lua::Result<lua::Function> {
        let path = self
            .resolved
            .as_ref()
            .ok_or_else(|| lua::Error::runtime("Missing admitted module"))?;
        if self.identity_only {
            let path = path.clone();
            return lua.create_function(move |_, ()| Ok(path.clone()));
        }
        let source = self
            .sources
            .0
            .get(path)
            .ok_or_else(|| lua::Error::runtime("Missing admitted source"))?;
        lua.load(source)
            .set_name(format!("@{path}"))
            .into_function()
    }
}

pub(crate) fn sequence_resolver(lua: &Lua, program: &crate::Program) -> lua::Result<()> {
    let mut sources = program.sources.clone();
    sources
        .insert(program.path.clone(), program.source.clone())
        .map_err(lua::Error::runtime)?;
    for child in &program.sequences {
        if let Some(existing) = sources.0.get(&child.path) {
            if existing != &child.source {
                return Err(lua::Error::runtime("Conflicting admitted sequence source"));
            }
        } else {
            sources
                .insert(child.path.clone(), child.source.clone())
                .map_err(lua::Error::runtime)?;
        }
    }
    let resolver = lua.create_require_function(Navigator {
        sources,
        path: String::new(),
        resolved: None,
        identity_only: true,
    })?;
    lua.set_named_registry_value("_kedra_sequence_resolver", resolver)
}
