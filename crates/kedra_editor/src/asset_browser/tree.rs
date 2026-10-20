//! Lazily expanded folders; bounded immediate-directory reads, never recursive scans.
use super::model::{Kind, Listing};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
const MAX_FOLDERS: usize = 512;
#[derive(Default)]
pub(super) struct Tree {
    expanded: BTreeSet<PathBuf>,
    children: BTreeMap<PathBuf, Vec<PathBuf>>,
    errors: BTreeMap<PathBuf, String>,
    clock: u64,
}
impl Tree {
    pub fn reveal(&mut self, path: &Path) {
        for ancestor in path.ancestors() {
            if self.expanded.len() >= MAX_FOLDERS && !self.expanded.contains(ancestor) {
                self.errors.insert(
                    PathBuf::new(),
                    "Tree expansion limit reached (512 folders)".into(),
                );
                break;
            }
            self.expanded.insert(ancestor.to_path_buf());
        }
    }
    pub fn toggle(&mut self, path: PathBuf) {
        if !self.expanded.remove(&path) && self.expanded.len() < MAX_FOLDERS {
            self.expanded.insert(path);
        }
    }
    pub fn expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }
    pub fn next(&self) -> Option<(u64, PathBuf)> {
        self.rows().into_iter().find_map(|(path, _)| {
            (self.expanded.contains(&path)
                && !self.children.contains_key(&path)
                && !self.errors.contains_key(&path))
            .then_some((self.clock, path))
        })
    }
    pub fn complete(&mut self, clock: u64, path: PathBuf, result: Result<Listing, String>) {
        if clock != self.clock {
            return;
        }
        match result {
            Ok(list) => {
                if list.truncated {
                    self.errors.insert(
                        path.clone(),
                        "Folder list is incomplete (512-entry limit)".into(),
                    );
                }
                let children: Vec<_> = list
                    .entries
                    .into_iter()
                    .filter(|e| e.kind == Kind::Folder)
                    .map(|e| path.join(e.name))
                    .collect();
                let count: usize = self.children.values().map(Vec::len).sum();
                let bytes: usize = self
                    .children
                    .values()
                    .flatten()
                    .chain(children.iter())
                    .map(|p| p.as_os_str().as_encoded_bytes().len())
                    .sum();
                if count + children.len() > 4096 || bytes > 1024 * 1024 {
                    self.errors.insert(
                        path,
                        "Tree cache limit reached; collapse branches and Refresh".into(),
                    );
                } else {
                    self.children.insert(path, children);
                }
            }
            Err(error) => {
                self.errors.insert(path, error);
            }
        }
    }
    pub fn error(&self, path: &Path) -> Option<&str> {
        self.errors.get(path).map(String::as_str)
    }
    pub fn refresh(&mut self) {
        self.children.clear();
        self.errors.clear();
        self.clock = self.clock.saturating_add(1);
    }
    pub fn rows(&self) -> Vec<(PathBuf, usize)> {
        let mut result = Vec::new();
        let mut stack = vec![(PathBuf::new(), 0)];
        while let Some((path, depth)) = stack.pop() {
            if result.len() == MAX_FOLDERS {
                break;
            }
            result.push((path.clone(), depth));
            if self.expanded.contains(&path)
                && let Some(children) = self.children.get(&path)
            {
                stack.extend(children.iter().rev().map(|p| (p.clone(), depth + 1)));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collapsed_and_obsolete_branches_do_not_repopulate_or_read_recursively() {
        let mut tree = Tree::default();
        tree.reveal(Path::new("a/b"));
        assert_eq!(tree.next(), Some((0, PathBuf::new())));
        tree.complete(0, PathBuf::new(), Err("unreadable".into()));
        assert!(tree.next().is_none());
        tree.refresh();
        tree.complete(0, PathBuf::new(), Err("obsolete".into()));
        assert!(tree.error(Path::new("")).is_none());
        assert_eq!(tree.next(), Some((1, PathBuf::new())));
        tree.toggle(PathBuf::new());
        assert!(tree.next().is_none());
        assert_eq!(tree.rows(), vec![(PathBuf::new(), 0)]);
    }
}
