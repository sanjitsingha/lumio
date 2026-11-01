//! Persists the file index so search works immediately after login while a
//! fresh scan runs in the background.
//!
//! Format (UTF-8 lines, tab separated, `\\`, `\t`, `\n` escaped):
//! ```text
//! LUMIO-FILE-INDEX 1 <config fingerprint>
//! R <root path>
//! D <parent dir index> <name>      (folder; becomes the next dir index)
//! F <parent dir index> <name>      (file)
//! ```

use std::fmt::Write as _;
use std::path::Path;

use super::index::{join, Entry, IndexData, FLAG_DIR};
use crate::search::fuzzy::fold;
use crate::storage::json_store;

const MAGIC: &str = "LUMIO-FILE-INDEX 1";

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

pub fn save(data: &IndexData, fingerprint: u64, path: &Path) -> std::io::Result<()> {
    let mut out = String::with_capacity(data.entries.len() * 32);
    let _ = writeln!(out, "{MAGIC} {fingerprint}");
    // Dir indices in the cache are renumbered in write order: roots, then
    // folders in the order their entries appear.
    let mut remap = vec![u32::MAX; data.dirs.len()];
    let mut next = 0u32;
    for (i, root) in data.dirs.iter().take(data.roots).enumerate() {
        remap[i] = next;
        next += 1;
        out.push_str("R\t");
        escape(root, &mut out);
        out.push('\n');
    }
    for entry in data.entries.iter().filter(|e| !e.is_deleted()) {
        let parent = remap[entry.parent as usize];
        if parent == u32::MAX {
            continue; // parent folder was removed
        }
        if entry.is_dir() {
            let full = data.full_path(entry);
            if let Some(&idx) = data.dir_lookup.get(full.as_str()) {
                remap[idx as usize] = next;
                next += 1;
            } else {
                continue;
            }
        }
        out.push(if entry.is_dir() { 'D' } else { 'F' });
        let _ = write!(out, "\t{parent}\t");
        escape(&entry.name, &mut out);
        out.push('\n');
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    json_store::write_atomic(path, out.as_bytes())
}

/// Loads a cache written for the same configuration. `None` if missing,
/// stale or damaged (the index is then rebuilt from disk).
pub fn load(path: &Path, fingerprint: u64) -> Option<IndexData> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header = lines.next()?;
    let stored: u64 = header.strip_prefix(MAGIC)?.trim().parse().ok()?;
    if stored != fingerprint {
        log::info!("file index cache was built with different settings; ignoring it");
        return None;
    }
    let mut data = IndexData::default();
    for line in lines {
        let mut parts = line.splitn(3, '\t');
        let kind = parts.next()?;
        match kind {
            "R" => {
                data.add_dir(unescape(parts.next()?));
                data.roots = data.dirs.len();
            }
            "D" | "F" => {
                let parent: u32 = parts.next()?.parse().ok()?;
                let name = unescape(parts.next()?);
                let dir_path = data.dirs.get(parent as usize)?.to_string();
                let is_dir = kind == "D";
                if is_dir {
                    data.add_dir(join(&dir_path, &name));
                }
                data.entries.push(Entry {
                    parent,
                    folded: fold(&name).into(),
                    name: name.into(),
                    flags: if is_dir { FLAG_DIR } else { 0 },
                });
            }
            _ => return None,
        }
    }
    Some(data)
}

#[cfg(test)]
mod tests {
    use super::super::index::IndexConfig;
    use super::*;
    use crate::search::fuzzy::Query;

    #[test]
    fn escaping_round_trips() {
        let mut out = String::new();
        let nasty = "tab\there\\back\nline";
        escape(nasty, &mut out);
        assert!(!out.contains('\t') && !out.contains('\n'));
        assert_eq!(unescape(&out), nasty);
    }

    #[test]
    fn cache_round_trips_index() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/deep file.txt"), b"x").unwrap();
        std::fs::write(root.join("top.md"), b"x").unwrap();
        let cfg = IndexConfig { roots: vec![root.clone()], exclusions: vec![], include_hidden: false, max_entries: 100, max_depth: 8 };
        let data = IndexData::build(&cfg, &|| false);
        let cache = dir.path().join("cache.txt");
        save(&data, cfg.fingerprint(), &cache).unwrap();

        assert!(load(&cache, cfg.fingerprint() ^ 1).is_none(), "fingerprint mismatch");
        let loaded = load(&cache, cfg.fingerprint()).unwrap();
        assert_eq!(loaded.live_entries(), data.live_entries());
        let hit = loaded.search(&Query::new("deep file"), true, true, 5);
        let full = loaded.full_path(&loaded.entries[hit[0].0]);
        assert_eq!(Path::new(&full), root.join("a").join("b").join("deep file.txt"));
    }

    #[test]
    fn damaged_cache_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache.txt");
        std::fs::write(&cache, format!("{MAGIC} 7\nF\t99\tghost\n")).unwrap();
        assert!(load(&cache, 7).is_none());
        std::fs::write(&cache, "garbage").unwrap();
        assert!(load(&cache, 7).is_none());
    }
}
