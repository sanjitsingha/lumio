//! In-memory file name index. Directory paths are interned once and entries
//! store only their own name plus a parent index, which keeps memory low.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

use crate::platform;
use crate::search::fuzzy::{fold, Query};

pub const FLAG_DIR: u8 = 1;
/// Prefix/word/acronym/substring matches, or a tight subsequence.
pub const MIN_FILE_SCORE: u32 = 380;
pub const FLAG_DELETED: u8 = 2;

pub struct Entry {
    pub parent: u32,
    pub name: Box<str>,
    pub folded: Box<str>,
    pub flags: u8,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.flags & FLAG_DIR != 0
    }
    pub fn is_deleted(&self) -> bool {
        self.flags & FLAG_DELETED != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexConfig {
    pub roots: Vec<PathBuf>,
    /// Folder/file names (case-insensitive, `*` wildcards) or absolute paths.
    pub exclusions: Vec<String>,
    pub include_hidden: bool,
    pub max_entries: usize,
    pub max_depth: usize,
}

impl IndexConfig {
    pub fn fingerprint(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.roots.hash(&mut h);
        self.exclusions.hash(&mut h);
        self.include_hidden.hash(&mut h);
        self.max_depth.hash(&mut h);
        h.finish()
    }

    pub fn is_excluded(&self, name: &str, full_path: &str) -> bool {
        let lower_name = name.to_lowercase();
        self.exclusions.iter().any(|pattern| {
            let p = pattern.trim();
            if Path::new(p).is_absolute() {
                let normalized = p.trim_end_matches(['/', '\\']);
                path_has_prefix(full_path, normalized)
            } else {
                wildcard_match(&p.to_lowercase(), &lower_name)
            }
        })
    }

    /// True when any component of `path` below a root is excluded/hidden.
    pub fn path_is_filtered(&self, path: &Path) -> bool {
        let Some(root) = self.roots.iter().find(|r| path.starts_with(r)) else { return true };
        let Ok(rel) = path.strip_prefix(root) else { return true };
        let mut current = root.clone();
        for comp in rel.components() {
            let name = comp.as_os_str().to_string_lossy();
            current.push(comp);
            if (!self.include_hidden && name.starts_with('.')) || self.is_excluded(&name, &current.to_string_lossy()) {
                return true;
            }
        }
        false
    }
}

fn path_has_prefix(path: &str, prefix: &str) -> bool {
    let (path, prefix) = if cfg!(windows) { (path.to_lowercase(), prefix.to_lowercase()) } else { (path.to_string(), prefix.to_string()) };
    path == prefix || (path.starts_with(&prefix) && path[prefix.len()..].starts_with(['/', '\\']))
}

/// `*` matches any run of characters; everything else is literal.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = text;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            match rest.strip_prefix(part) {
                Some(r) => rest = r,
                None => return false,
            }
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            match rest.find(part) {
                Some(pos) => rest = &rest[pos + part.len()..],
                None => return false,
            }
        }
    }
    true
}

pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with(['/', '\\']) {
        format!("{dir}{name}")
    } else {
        format!("{dir}{MAIN_SEPARATOR}{name}")
    }
}

#[derive(Default)]
pub struct IndexData {
    pub dirs: Vec<Box<str>>,
    pub dir_lookup: HashMap<Box<str>, u32>,
    pub entries: Vec<Entry>,
    pub roots: usize,
    pub deleted: usize,
    pub limit_reached: bool,
    pub unreadable: usize,
}

impl IndexData {
    pub fn add_dir(&mut self, path: String) -> u32 {
        if let Some(&idx) = self.dir_lookup.get(path.as_str()) {
            return idx;
        }
        let idx = self.dirs.len() as u32;
        let boxed: Box<str> = path.into();
        self.dir_lookup.insert(boxed.clone(), idx);
        self.dirs.push(boxed);
        idx
    }

    pub fn live_entries(&self) -> usize {
        self.entries.len() - self.deleted
    }

    pub fn counts(&self) -> (usize, usize) {
        let dirs = self.entries.iter().filter(|e| !e.is_deleted() && e.is_dir()).count();
        (self.live_entries() - dirs, dirs)
    }

    pub fn full_path(&self, entry: &Entry) -> String {
        join(&self.dirs[entry.parent as usize], &entry.name)
    }

    /// Full walk of every root (breadth-first, so shallow files are indexed first
    /// when the entry limit is reached). `cancelled` is polled between folders.
    pub fn build(config: &IndexConfig, cancelled: &dyn Fn() -> bool) -> IndexData {
        let mut data = IndexData::default();
        let mut queue = VecDeque::new();
        for root in &config.roots {
            if root.is_dir() {
                let idx = data.add_dir(root.to_string_lossy().into_owned());
                queue.push_back((idx, 0usize));
            } else {
                log::warn!("index root {} is not a folder; skipped", root.display());
            }
        }
        data.roots = data.dirs.len();
        data.walk(queue, config, cancelled);
        data
    }

    fn walk(&mut self, mut queue: VecDeque<(u32, usize)>, config: &IndexConfig, cancelled: &dyn Fn() -> bool) -> Vec<u32> {
        let mut visited = Vec::new();
        while let Some((dir, depth)) = queue.pop_front() {
            if cancelled() {
                break;
            }
            if self.live_entries() >= config.max_entries {
                self.limit_reached = true;
                break;
            }
            visited.push(dir);
            for child in self.scan_dir(dir, config) {
                if depth + 1 < config.max_depth {
                    queue.push_back((child, depth + 1));
                }
            }
        }
        visited
    }

    /// Adds the direct children of `dir`; returns the child folders.
    fn scan_dir(&mut self, dir: u32, config: &IndexConfig) -> Vec<u32> {
        let dir_path = self.dirs[dir as usize].to_string();
        let mut children = Vec::new();
        let reader = match fs::read_dir(&dir_path) {
            Ok(r) => r,
            Err(e) => {
                self.unreadable += 1;
                log::debug!("cannot list {dir_path}: {e}");
                return children;
            }
        };
        for item in reader.flatten() {
            let Some((name, is_dir)) = self.accept(&item, &dir_path, config) else { continue };
            let full = join(&dir_path, &name);
            self.entries.push(Entry {
                parent: dir,
                folded: fold(&name).into(),
                name: name.into(),
                flags: if is_dir { FLAG_DIR } else { 0 },
            });
            if is_dir {
                children.push(self.add_dir(full));
            }
            if self.live_entries() >= config.max_entries {
                self.limit_reached = true;
                break;
            }
        }
        children
    }

    fn accept(&self, item: &fs::DirEntry, dir_path: &str, config: &IndexConfig) -> Option<(String, bool)> {
        let name = item.file_name().into_string().ok()?;
        if !config.include_hidden && platform::is_hidden(item, &name) {
            return None;
        }
        let file_type = item.file_type().ok()?;
        // Symlinks are listed but never followed (prevents loops and escaping the roots).
        let is_dir = file_type.is_dir();
        if config.is_excluded(&name, &join(dir_path, &name)) {
            return None;
        }
        Some((name, is_dir))
    }

    /// Re-reads one folder after a change notification. Returns new child
    /// folders (already walked) so the caller can watch them.
    pub fn rescan(&mut self, dir_path: &str, config: &IndexConfig) -> Vec<u32> {
        let Some(&dir) = self.dir_lookup.get(dir_path) else { return Vec::new() };
        if !Path::new(dir_path).is_dir() {
            self.remove_subtree(dir);
            return Vec::new();
        }
        let mut existing: HashMap<Box<str>, usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.parent == dir && !e.is_deleted())
            .map(|(i, e)| (e.name.clone(), i))
            .collect();
        let mut new_dirs = VecDeque::new();
        if let Ok(reader) = fs::read_dir(dir_path) {
            for item in reader.flatten() {
                let Some((name, is_dir)) = self.accept(&item, dir_path, config) else { continue };
                if let Some(i) = existing.remove(name.as_str()) {
                    if self.entries[i].is_dir() == is_dir {
                        continue;
                    }
                    self.mark_deleted(i); // type changed: replace the entry
                }
                if self.live_entries() >= config.max_entries {
                    self.limit_reached = true;
                    break;
                }
                let full = join(dir_path, &name);
                self.entries.push(Entry {
                    parent: dir,
                    folded: fold(&name).into(),
                    name: name.into(),
                    flags: if is_dir { FLAG_DIR } else { 0 },
                });
                if is_dir {
                    new_dirs.push_back((self.add_dir(full), 0));
                }
            }
        }
        for (_, i) in existing {
            self.mark_deleted(i);
        }
        self.walk(new_dirs, config, &|| false)
    }

    fn mark_deleted(&mut self, i: usize) {
        if self.entries[i].is_deleted() {
            return;
        }
        self.entries[i].flags |= FLAG_DELETED;
        self.deleted += 1;
        if self.entries[i].is_dir() {
            let path = self.full_path(&self.entries[i]);
            if let Some(&d) = self.dir_lookup.get(path.as_str()) {
                self.remove_subtree(d);
            }
        }
    }

    /// Drops every entry below `dir` (the folder itself stays in its parent
    /// until that parent is rescanned).
    fn remove_subtree(&mut self, dir: u32) {
        let root = self.dirs[dir as usize].to_string();
        let doomed: HashSet<u32> = self
            .dir_lookup
            .iter()
            .filter(|(p, _)| path_has_prefix(p, &root))
            .map(|(_, &i)| i)
            .collect();
        for entry in self.entries.iter_mut().filter(|e| doomed.contains(&e.parent) && !e.is_deleted()) {
            entry.flags |= FLAG_DELETED;
            self.deleted += 1;
        }
        // Roots stay addressable so they can be rescanned if they reappear.
        let roots = self.roots as u32;
        self.dir_lookup.retain(|_, i| *i < roots || !doomed.contains(i));
    }

    /// Removes deleted entries once they make up a large share of the index.
    pub fn compact_if_needed(&mut self) {
        if self.deleted > 2_000 && self.deleted * 4 > self.entries.len() {
            self.entries.retain(|e| !e.is_deleted());
            self.deleted = 0;
        }
    }

    /// Returns `(entry index, score)` pairs, best first. Names need a strong
    /// match (see [`MIN_FILE_SCORE`]): with thousands of files, loose
    /// letter-scatter matches would bury the useful results.
    pub fn search(&self, query: &Query, files: bool, folders: bool, limit: usize) -> Vec<(usize, u32)> {
        if !files && !folders {
            return Vec::new();
        }
        let mut hits: Vec<(usize, u32)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_deleted() && if e.is_dir() { folders } else { files })
            .filter(|(_, e)| query.may_match(&e.folded, false))
            .filter_map(|(i, e)| query.score(&e.folded, false).filter(|s| *s >= MIN_FILE_SCORE).map(|s| (i, s)))
            .collect();
        hits.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| self.entries[a.0].name.len().cmp(&self.entries[b.0].name.len()))
                .then_with(|| self.entries[a.0].name.cmp(&self.entries[b.0].name))
        });
        hits.truncate(limit);
        hits
    }

    /// Whether `path` is a live entry of the index; returns `Some(is_dir)`.
    pub fn lookup(&self, path: &str) -> Option<bool> {
        let p = Path::new(path);
        let parent = p.parent()?.to_string_lossy();
        let name = p.file_name()?.to_string_lossy();
        let &dir = self.dir_lookup.get(parent.as_ref())?;
        self.entries
            .iter()
            .find(|e| e.parent == dir && !e.is_deleted() && *e.name == *name)
            .map(|e| e.is_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn config(root: &Path) -> IndexConfig {
        IndexConfig {
            roots: vec![root.to_path_buf()],
            exclusions: vec!["node_modules".into(), "*.tmp".into()],
            include_hidden: false,
            max_entries: 10_000,
            max_depth: 8,
        }
    }

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        fs::create_dir_all(r.join("Documents/Reports")).unwrap();
        fs::create_dir_all(r.join("project/node_modules/lib")).unwrap();
        fs::create_dir_all(r.join(".secret")).unwrap();
        fs::write(r.join("Documents/Reports/annual-report-2025.pdf"), b"x").unwrap();
        fs::write(r.join("Documents/notes.md"), b"x").unwrap();
        fs::write(r.join("Documents/scratch.tmp"), b"x").unwrap();
        fs::write(r.join("project/node_modules/lib/index.js"), b"x").unwrap();
        fs::write(r.join("project/main.rs"), b"x").unwrap();
        fs::write(r.join(".secret/key.txt"), b"x").unwrap();
        dir
    }

    fn names(data: &IndexData, query: &str) -> Vec<String> {
        data.search(&Query::new(query), true, true, 20).into_iter().map(|(i, _)| data.entries[i].name.to_string()).collect()
    }

    #[test]
    fn builds_and_respects_exclusions_and_hidden() {
        let dir = tree();
        let data = IndexData::build(&config(dir.path()), &|| false);
        let all: HashSet<String> = data.entries.iter().map(|e| e.name.to_string()).collect();
        for expected in ["Documents", "Reports", "annual-report-2025.pdf", "notes.md", "project", "main.rs"] {
            assert!(all.contains(expected), "{expected} missing");
        }
        for excluded in ["node_modules", "index.js", "scratch.tmp", ".secret", "key.txt"] {
            assert!(!all.contains(excluded), "{excluded} should be excluded");
        }
        assert_eq!(data.counts(), (3, 3));
    }

    #[test]
    fn searches_files_and_folders() {
        let dir = tree();
        let data = IndexData::build(&config(dir.path()), &|| false);
        assert_eq!(names(&data, "annual"), vec!["annual-report-2025.pdf"]);
        let report = names(&data, "report");
        assert_eq!(report.len(), 2);
        assert_eq!(report[0], "annual-report-2025.pdf", "whole-word match outranks a prefix of a longer word");
        assert_eq!(report[1], "Reports");
        let only_dirs: Vec<_> = data.search(&Query::new("report"), false, true, 10);
        assert!(only_dirs.iter().all(|(i, _)| data.entries[*i].is_dir()));
        let p = dir.path().join("Documents").join("notes.md");
        assert_eq!(data.lookup(&p.to_string_lossy()), Some(false));
        assert_eq!(data.lookup(&dir.path().join("Documents").to_string_lossy()), Some(true));
        assert_eq!(data.lookup(&dir.path().join("nope.txt").to_string_lossy()), None);
    }

    /// Windows marks hidden files with an attribute rather than a dot-name.
    #[cfg(windows)]
    #[test]
    fn hidden_attribute_is_respected_on_windows() {
        use windows::core::HSTRING;
        use windows::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN};
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("Private Notes");
        fs::create_dir_all(&secret).unwrap();
        fs::write(secret.join("diary.txt"), b"x").unwrap();
        fs::write(dir.path().join("visible.txt"), b"x").unwrap();
        unsafe { SetFileAttributesW(&HSTRING::from(secret.as_os_str()), FILE_ATTRIBUTE_HIDDEN) }.unwrap();
        let data = IndexData::build(&config(dir.path()), &|| false);
        let all: HashSet<String> = data.entries.iter().map(|e| e.name.to_string()).collect();
        assert!(all.contains("visible.txt"));
        assert!(!all.contains("Private Notes") && !all.contains("diary.txt"));
    }

    #[test]
    fn scattered_letter_matches_are_not_file_results() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("trigger-meta.ts"), b"x").unwrap();
        fs::write(dir.path().join("terminal-notes.txt"), b"x").unwrap();
        let data = IndexData::build(&config(dir.path()), &|| false);
        assert_eq!(names(&data, "term"), vec!["terminal-notes.txt"]);
    }

    #[test]
    fn honours_entry_limit_and_depth() {
        let dir = tree();
        let mut cfg = config(dir.path());
        cfg.max_entries = 2;
        let data = IndexData::build(&cfg, &|| false);
        assert!(data.limit_reached);
        assert_eq!(data.live_entries(), 2);

        let mut cfg = config(dir.path());
        cfg.max_depth = 1;
        let data = IndexData::build(&cfg, &|| false);
        assert!(names(&data, "annual").is_empty(), "too deep");
        assert_eq!(names(&data, "documents"), vec!["Documents"]);
    }

    #[test]
    fn incremental_rescan_tracks_changes() {
        let dir = tree();
        let cfg = config(dir.path());
        let mut data = IndexData::build(&cfg, &|| false);
        let docs = dir.path().join("Documents");
        fs::write(docs.join("budget.xlsx"), b"x").unwrap();
        fs::create_dir_all(docs.join("Invoices/2026")).unwrap();
        fs::write(docs.join("Invoices/2026/invoice-17.pdf"), b"x").unwrap();
        fs::remove_file(docs.join("notes.md")).unwrap();
        let new_dirs = data.rescan(&docs.to_string_lossy(), &cfg);
        assert_eq!(new_dirs.len(), 2, "Invoices and 2026 walked");
        assert_eq!(names(&data, "budget"), vec!["budget.xlsx"]);
        assert_eq!(names(&data, "invoice-17"), vec!["invoice-17.pdf"]);
        assert!(names(&data, "notes").is_empty());

        fs::remove_dir_all(docs.join("Reports")).unwrap();
        data.rescan(&docs.to_string_lossy(), &cfg);
        assert!(names(&data, "annual").is_empty(), "subtree removed with its folder");
        assert!(names(&data, "reports").is_empty());
    }

    #[test]
    fn wildcard_and_path_exclusions() {
        assert!(wildcard_match("*.tmp", "a.tmp"));
        assert!(wildcard_match("cache*", "cache-v2"));
        assert!(wildcard_match("*cache*", "mycache2"));
        assert!(!wildcard_match("*.tmp", "a.tmpx"));
        assert!(wildcard_match("exact", "exact"));
        assert!(!wildcard_match("a*a", "a"));
        let videos = if cfg!(windows) { r"C:\Users\u\Videos" } else { "/home/u/Videos" };
        let cfg = IndexConfig {
            roots: vec![],
            exclusions: vec![videos.into()],
            include_hidden: false,
            max_entries: 1,
            max_depth: 1,
        };
        let sep = MAIN_SEPARATOR;
        assert!(cfg.is_excluded("Videos", videos));
        assert!(cfg.is_excluded("clip.mp4", &format!("{videos}{sep}clip.mp4")));
        assert!(!cfg.is_excluded("VideosOld", &format!("{videos}Old")));
    }
}
