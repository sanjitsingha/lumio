//! freedesktop Icon Theme lookup (subset of the spec sufficient for app icons):
//! theme inheritance, size/scale matching, `hicolor` fallback and pixmaps.
//! <https://specifications.freedesktop.org/icon-theme-spec/latest/>

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
enum DirKind {
    Fixed,
    Scalable { min: u32, max: u32 },
    Threshold(u32),
}

#[derive(Debug, Clone)]
struct ThemeDir {
    subdir: String,
    size: u32,
    scale: u32,
    kind: DirKind,
}

impl ThemeDir {
    fn matches(&self, size: u32) -> bool {
        match self.kind {
            DirKind::Fixed => self.size == size,
            DirKind::Scalable { min, max } => (min..=max).contains(&size),
            DirKind::Threshold(t) => self.size.abs_diff(size) <= t,
        }
    }

    /// Distance per the spec's `DirectorySizeDistance`, preferring larger icons.
    fn distance(&self, size: u32) -> u32 {
        let (lo, hi) = match self.kind {
            DirKind::Fixed => (self.size, self.size),
            DirKind::Scalable { min, max } => (min, max),
            DirKind::Threshold(t) => (self.size.saturating_sub(t), self.size + t),
        };
        if size < lo {
            lo - size
        } else if size > hi {
            // smaller-than-requested icons look blurry; penalize them more
            (size - hi) * 2
        } else {
            0
        }
    }
}

#[derive(Debug, Clone)]
struct Theme {
    bases: Vec<PathBuf>,
    dirs: Vec<ThemeDir>,
}

pub struct IconResolver {
    chain: Vec<Theme>,
    pixmaps: Vec<PathBuf>,
}

/// `$XDG_DATA_HOME/icons`, `~/.icons`, each `$XDG_DATA_DIRS/icons`.
pub fn icon_base_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".icons"));
    }
    dirs.push(super::apps::data_home().join("icons"));
    dirs.extend(super::apps::data_dirs().into_iter().map(|d| d.join("icons")));
    dirs.dedup();
    dirs
}

fn parse_index(text: &str) -> (Vec<ThemeDir>, Vec<String>) {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current = String::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            current = line[1..line.len() - 1].to_string();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            sections.entry(current.clone()).or_default().insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let main = sections.get("Icon Theme").cloned().unwrap_or_default();
    let list = |key: &str| -> Vec<String> {
        main.get(key).map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
    };
    let inherits = list("Inherits");
    let mut names = list("Directories");
    names.extend(list("ScaledDirectories"));
    names.dedup();
    let dirs = names
        .into_iter()
        .filter_map(|subdir| {
            let s = sections.get(&subdir)?;
            let num = |k: &str| s.get(k).and_then(|v| v.parse::<u32>().ok());
            let size = num("Size")?;
            let kind = match s.get("Type").map(String::as_str).unwrap_or("Threshold") {
                "Fixed" => DirKind::Fixed,
                "Scalable" => DirKind::Scalable { min: num("MinSize").unwrap_or(size), max: num("MaxSize").unwrap_or(size) },
                _ => DirKind::Threshold(num("Threshold").unwrap_or(2)),
            };
            Some(ThemeDir { subdir, size, scale: num("Scale").unwrap_or(1), kind })
        })
        .collect();
    (dirs, inherits)
}

impl IconResolver {
    pub fn new(theme: &str, bases: &[PathBuf]) -> Self {
        let mut chain = Vec::new();
        let mut queue = vec![theme.to_string()];
        let mut seen = Vec::new();
        while let Some(name) = queue.pop() {
            if seen.contains(&name) || seen.len() > 12 {
                continue;
            }
            seen.push(name.clone());
            let theme_bases: Vec<PathBuf> = bases.iter().map(|b| b.join(&name)).filter(|p| p.is_dir()).collect();
            let Some(index) = theme_bases.iter().find_map(|b| std::fs::read_to_string(b.join("index.theme")).ok()) else {
                continue;
            };
            let (dirs, inherits) = parse_index(&index);
            chain.push(Theme { bases: theme_bases, dirs });
            // depth-first in declaration order
            for parent in inherits.into_iter().rev() {
                queue.push(parent);
            }
        }
        if !seen.iter().any(|s| s == "hicolor") {
            let hicolor = IconResolver::new("hicolor", bases);
            chain.extend(hicolor.chain);
        }
        let mut pixmaps = vec![PathBuf::from("/usr/share/pixmaps")];
        pixmaps.extend(super::apps::data_dirs().into_iter().map(|d| d.join("pixmaps")));
        pixmaps.dedup();
        Self { chain, pixmaps }
    }

    /// Finds the best file for icon `name` near `size` px at integer `scale`.
    pub fn lookup(&self, name: &str, size: u32, scale: u32) -> Option<PathBuf> {
        let path = Path::new(name);
        if path.is_absolute() {
            return path.is_file().then(|| path.to_path_buf());
        }
        let name = name.strip_suffix(".png").or_else(|| name.strip_suffix(".svg")).unwrap_or(name);
        for theme in &self.chain {
            if let Some(found) = Self::lookup_in(theme, name, size, scale) {
                return Some(found);
            }
        }
        for dir in &self.pixmaps {
            for ext in ["svg", "png"] {
                let candidate = dir.join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn lookup_in(theme: &Theme, name: &str, size: u32, scale: u32) -> Option<PathBuf> {
        let mut best: Option<(u32, PathBuf)> = None;
        for dir in &theme.dirs {
            let scale_penalty = if dir.scale == scale { 0 } else { 100 };
            let distance = if dir.matches(size) && dir.scale == scale { 0 } else { dir.distance(size) + scale_penalty };
            if best.as_ref().is_some_and(|(d, _)| *d <= distance) {
                continue;
            }
            for base in &theme.bases {
                for ext in ["svg", "png"] {
                    let candidate = base.join(&dir.subdir).join(format!("{name}.{ext}"));
                    if candidate.is_file() {
                        best = Some((distance, candidate));
                        break;
                    }
                }
                if best.as_ref().is_some_and(|(d, _)| *d == distance) {
                    break;
                }
            }
            if best.as_ref().is_some_and(|(d, _)| *d == 0) {
                break;
            }
        }
        best.map(|(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_theme(base: &Path, name: &str, index: &str, icons: &[&str]) {
        let root = base.join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("index.theme"), index).unwrap();
        for icon in icons {
            let p = root.join(icon);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"<svg/>").unwrap();
        }
    }

    #[test]
    fn resolves_with_inheritance_and_size_preference() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().to_path_buf();
        write_theme(
            &base,
            "child",
            "[Icon Theme]\nName=Child\nInherits=hicolor\nDirectories=apps/16,apps/64\n\n[apps/16]\nSize=16\nType=Fixed\n\n[apps/64]\nSize=64\nType=Fixed\n",
            &["apps/16/editor.png", "apps/64/editor.png"],
        );
        write_theme(
            &base,
            "hicolor",
            "[Icon Theme]\nName=Hicolor\nDirectories=scalable/apps,48x48/apps\n\n[scalable/apps]\nSize=128\nType=Scalable\nMinSize=8\nMaxSize=512\n\n[48x48/apps]\nSize=48\nType=Threshold\n",
            &["scalable/apps/viewer.svg", "48x48/apps/legacy.png"],
        );
        let resolver = IconResolver::new("child", std::slice::from_ref(&base));
        assert_eq!(resolver.lookup("editor", 64, 1).unwrap(), base.join("child/apps/64/editor.png"));
        assert_eq!(resolver.lookup("editor", 60, 1).unwrap(), base.join("child/apps/64/editor.png"), "closest larger size");
        assert_eq!(resolver.lookup("viewer", 64, 1).unwrap(), base.join("hicolor/scalable/apps/viewer.svg"), "inherited");
        assert_eq!(resolver.lookup("legacy.png", 48, 1).unwrap(), base.join("hicolor/48x48/apps/legacy.png"));
        assert!(resolver.lookup("does-not-exist-anywhere", 64, 1).is_none());
    }

    #[test]
    fn absolute_paths_are_used_directly() {
        let tmp = tempfile::tempdir().unwrap();
        let icon = tmp.path().join("custom.png");
        fs::write(&icon, b"png").unwrap();
        let resolver = IconResolver::new("none", &[]);
        assert_eq!(resolver.lookup(icon.to_str().unwrap(), 64, 1), Some(icon));
        assert_eq!(resolver.lookup("/no/such/icon.png", 64, 1), None);
    }

    #[test]
    fn distance_prefers_larger_over_smaller() {
        // equally far away in pixels, but upscaling a smaller icon looks worse
        let bigger = ThemeDir { subdir: String::new(), size: 72, scale: 1, kind: DirKind::Fixed };
        let smaller = ThemeDir { subdir: String::new(), size: 56, scale: 1, kind: DirKind::Fixed };
        assert!(bigger.distance(64) < smaller.distance(64));
        let scalable = ThemeDir { subdir: String::new(), size: 128, scale: 1, kind: DirKind::Scalable { min: 8, max: 512 } };
        assert!(scalable.matches(64) && scalable.distance(64) == 0);
    }
}
