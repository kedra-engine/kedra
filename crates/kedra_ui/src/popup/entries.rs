//! A bounded command tree from display paths; paths are never filesystem locators.

/// A menu node; commands retain their typed identity independently of the label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry<T> {
    /// An executable command, with an accessible display label.
    Command {
        /// Accessible display label.
        label: String,
        /// Host-owned typed command identity.
        command: T,
    },
    /// A branch containing commands or further branches in insertion order.
    Submenu {
        /// Accessible branch label without an arrow suffix.
        label: String,
        /// Ordered child entries.
        children: Vec<Entry<T>>,
    },
}
impl<T> Entry<T> {
    /// Visible text without ancestor segments.
    pub fn label(&self) -> &str {
        match self {
            Self::Command { label, .. } | Self::Submenu { label, .. } => label,
        }
    }
}
/// Build menu levels from slash-separated labels such as `Create/Scene…`.
/// This is presentation data, not an asset/filesystem path. Leading, trailing or
/// empty segments, duplicate commands and leaf/branch conflicts are errors.
/// Limits are 256 input commands, 8 levels and 1024 UTF-8 bytes per display path.
/// On error no partial tree is returned. Unicode labels and insertion order survive.
pub fn from_paths<T>(
    commands: impl IntoIterator<Item = (impl AsRef<str>, T)>,
) -> Result<Vec<Entry<T>>, String> {
    let mut entries = Vec::new();
    for (index, (path, command)) in commands.into_iter().enumerate() {
        let path = path.as_ref();
        let parts: Vec<_> = path.split('/').collect();
        if index >= 256
            || path.len() > 1024
            || parts.len() > 8
            || parts
                .iter()
                .any(|s| s.trim().is_empty() || s.chars().any(char::is_control))
        {
            return Err("Invalid menu display path or menu budget exceeded".into());
        }
        insert(&mut entries, &parts, command)?;
    }
    Ok(entries)
}
fn insert<T>(entries: &mut Vec<Entry<T>>, parts: &[&str], command: T) -> Result<(), String> {
    let label = parts[0];
    let existing = entries.iter().position(|e| e.label() == label);
    if parts.len() == 1 {
        if existing.is_some() {
            return Err(format!("Duplicate command or branch conflict: {label}"));
        }
        entries.push(Entry::Command {
            label: label.into(),
            command,
        });
    } else {
        let index = existing.unwrap_or_else(|| {
            entries.push(Entry::Submenu {
                label: label.into(),
                children: Vec::new(),
            });
            entries.len() - 1
        });
        let Entry::Submenu { children, .. } = &mut entries[index] else {
            return Err(format!("Command used as a submenu: {label}"));
        };
        insert(children, &parts[1..], command)?;
    }
    Ok(())
}
