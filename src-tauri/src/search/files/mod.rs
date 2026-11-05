//! File and folder search backed by a background-built name index that is
//! kept current with native change notifications (inotify on Linux,
//! ReadDirectoryChangesW on Windows) and cached on disk.

pub mod cache;
pub mod index;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::apps::AppIcon;
use super::types::{file_category, ExecuteMode, IndexStatus, ResultIcon, ResultKind, SearchResult};
use super::{hex_key, ExecutedItem, SearchContext, SearchProvider};
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::storage::history::now_secs;
use crate::storage::settings::SpotlightSettings;
use index::{IndexConfig, IndexData};

pub const MAX_ENTRIES: usize = 250_000;
pub const MAX_DEPTH: usize = 14;
/// inotify watches are a per-user kernel resource; stay well below common limits.
const MAX_WATCHES: usize = 24_000;
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(900);
const REFRESH_WHEN_WATCHING: Duration = Duration::from_secs(6 * 3600);
const REFRESH_WITHOUT_WATCHING: Duration = Duration::from_secs(30 * 60);
const MAX_CACHED_ICONS: usize = 512;
/// Types whose files each carry their own icon (programs, shortcuts, icons).
const OWN_ICON_EXTENSIONS: &[&str] = &["exe", "lnk", "url", "ico", "cur", "ani", "scr", "cpl", "msc", "appref-ms"];

enum Msg {
    Rebuild,
    Fs(notify::Result<notify::Event>),
    Shutdown,
}

#[derive(Default)]
struct Status {
    indexing: bool,
    ready: bool,
    last_indexed: Option<u64>,
    last_duration_ms: Option<u64>,
    watching: bool,
    error: Option<String>,
}

pub struct FileIndex {
    data: RwLock<IndexData>,
    config: Mutex<Option<IndexConfig>>,
    status: Mutex<Status>,
    tx: Mutex<Option<Sender<Msg>>>,
    /// Bumped to cancel an in-progress build when settings change.
    generation: AtomicU64,
    cache_path: PathBuf,
    home: Option<PathBuf>,
    enabled: AtomicBool,
    /// Rendered icons by [`icon_key`], so each file type is drawn once.
    icons: Mutex<HashMap<String, AppIcon>>,
}

impl FileIndex {
    pub fn new(cache_path: PathBuf, home: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            data: RwLock::default(),
            config: Mutex::new(None),
            status: Mutex::default(),
            tx: Mutex::new(None),
            generation: AtomicU64::new(0),
            cache_path,
            home,
            enabled: AtomicBool::new(false),
            icons: Mutex::default(),
        })
    }

    /// Builds the indexing configuration from settings. `private_dirs` are
    /// Lummo's own data folders, always excluded to avoid feedback loops.
    pub fn config_from(settings: &SpotlightSettings, home: Option<&Path>, private_dirs: &[PathBuf]) -> IndexConfig {
        let mut exclusions = settings.exclusions.clone();
        exclusions.extend(private_dirs.iter().map(|p| p.to_string_lossy().into_owned()));
        let roots = if settings.index_roots.is_empty() {
            home.map(|h| vec![h.to_path_buf()]).unwrap_or_default()
        } else {
            settings.index_roots.iter().map(PathBuf::from).collect()
        };
        IndexConfig { roots, exclusions, include_hidden: settings.index_hidden, max_entries: MAX_ENTRIES, max_depth: MAX_DEPTH }
    }

    /// Starts (or reconfigures) background indexing.
    pub fn configure(self: &Arc<Self>, config: IndexConfig, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        let changed = {
            let mut current = self.config.lock().unwrap_or_else(|e| e.into_inner());
            let changed = current.as_ref() != Some(&config);
            *current = Some(config);
            changed
        };
        if !enabled {
            self.stop_worker();
            *self.data.write().unwrap_or_else(|e| e.into_inner()) = IndexData::default();
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            *status = Status::default();
            return;
        }
        let mut tx = self.tx.lock().unwrap_or_else(|e| e.into_inner());
        match tx.as_ref() {
            Some(sender) if changed => {
                self.generation.fetch_add(1, Ordering::SeqCst);
                let _ = sender.send(Msg::Rebuild);
            }
            Some(_) => {}
            None => {
                let (sender, receiver) = mpsc::channel();
                *tx = Some(sender.clone());
                let this = Arc::clone(self);
                let spawned = std::thread::Builder::new()
                    .name("lummo-file-index".into())
                    .spawn(move || this.worker(sender, receiver));
                if let Err(e) = spawned {
                    log::error!("could not start the file indexer: {e}");
                    *tx = None;
                    self.status.lock().unwrap_or_else(|e| e.into_inner()).error = Some(format!("Indexer failed to start: {e}"));
                }
            }
        }
    }

    pub fn rebuild(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(tx) = self.tx.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = tx.send(Msg::Rebuild);
        }
    }

    pub fn clear_cache(&self) {
        if let Err(e) = std::fs::remove_file(&self.cache_path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("could not delete file index cache: {e}");
            }
        }
        self.icons.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Icon for the `lummo-icon` protocol. Only indexed files are handed to
    /// the system to draw; a type drawn before is answered from the cache.
    pub fn icon(&self, path: &str) -> Option<AppIcon> {
        let key = icon_key(Path::new(path));
        if let Some(icon) = self.icons.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return Some(icon.clone());
        }
        let is_dir = self.data.read().unwrap_or_else(|e| e.into_inner()).lookup(path)?;
        if is_dir {
            return None;
        }
        let icon = platform::apps::file_icon(Path::new(path));
        // a failure may be this file's alone (deleted, unreadable), so it is not remembered
        if !matches!(icon, AppIcon::None) {
            let mut icons = self.icons.lock().unwrap_or_else(|e| e.into_inner());
            if icons.len() >= MAX_CACHED_ICONS {
                icons.clear();
            }
            icons.insert(key, icon.clone());
        }
        Some(icon)
    }

    pub fn stop_worker(&self) {
        if let Some(tx) = self.tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
            self.generation.fetch_add(1, Ordering::SeqCst);
            let _ = tx.send(Msg::Shutdown);
        }
    }

    pub fn status(&self) -> IndexStatus {
        let (files, folders) = self.data.read().unwrap_or_else(|e| e.into_inner()).counts();
        let limit_reached = self.data.read().unwrap_or_else(|e| e.into_inner()).limit_reached;
        let s = self.status.lock().unwrap_or_else(|e| e.into_inner());
        IndexStatus {
            apps: 0,
            files,
            folders,
            indexing: s.indexing,
            ready: s.ready,
            last_indexed: s.last_indexed,
            last_duration_ms: s.last_duration_ms,
            limit_reached,
            watching: s.watching,
            error: s.error.clone(),
        }
    }

    fn current_config(&self) -> Option<IndexConfig> {
        self.config.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn worker(self: Arc<Self>, tx: Sender<Msg>, rx: mpsc::Receiver<Msg>) {
        let Some(config) = self.current_config() else { return };
        if let Some(cached) = cache::load(&self.cache_path, config.fingerprint()) {
            log::info!("loaded {} cached file index entries", cached.live_entries());
            *self.data.write().unwrap_or_else(|e| e.into_inner()) = cached;
            self.status.lock().unwrap_or_else(|e| e.into_inner()).ready = true;
        }
        let mut watcher = self.full_build(&tx);
        let mut last_full = Instant::now();
        let mut pending: HashSet<PathBuf> = HashSet::new();
        let mut deadline: Option<Instant> = None;
        loop {
            let refresh_every =
                if self.status.lock().unwrap_or_else(|e| e.into_inner()).watching { REFRESH_WHEN_WATCHING } else { REFRESH_WITHOUT_WATCHING };
            let wait = deadline
                .map(|d| d.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|| refresh_every.saturating_sub(last_full.elapsed()))
                .max(Duration::from_millis(50));
            match rx.recv_timeout(wait) {
                Ok(Msg::Rebuild) => {
                    pending.clear();
                    deadline = None;
                    drop(watcher.take());
                    watcher = self.full_build(&tx);
                    last_full = Instant::now();
                }
                Ok(Msg::Fs(Ok(event))) => {
                    if event.need_rescan() {
                        log::info!("file watcher overflowed; scheduling a full rescan");
                        let _ = tx.send(Msg::Rebuild);
                        continue;
                    }
                    if matches!(event.kind, notify::EventKind::Access(_)) {
                        continue;
                    }
                    pending.extend(event.paths);
                    deadline.get_or_insert_with(|| Instant::now() + CHANGE_DEBOUNCE);
                }
                Ok(Msg::Fs(Err(e))) => log::warn!("file watcher error: {e}"),
                Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    if deadline.is_some_and(|d| Instant::now() >= d) {
                        deadline = None;
                        let paths: Vec<PathBuf> = pending.drain().collect();
                        self.apply_changes(&paths, watcher.as_mut());
                    } else if deadline.is_none() && last_full.elapsed() >= refresh_every {
                        drop(watcher.take());
                        watcher = self.full_build(&tx);
                        last_full = Instant::now();
                    }
                }
            }
        }
        log::info!("file indexer stopped");
    }

    /// Rebuilds from disk, swaps the index in, saves the cache and installs watchers.
    fn full_build(&self, tx: &Sender<Msg>) -> Option<RecommendedWatcher> {
        let config = self.current_config()?;
        let generation = self.generation.load(Ordering::SeqCst);
        self.status.lock().unwrap_or_else(|e| e.into_inner()).indexing = true;
        let started = Instant::now();
        let built = IndexData::build(&config, &|| self.generation.load(Ordering::SeqCst) != generation);
        let cancelled = self.generation.load(Ordering::SeqCst) != generation;
        let elapsed = started.elapsed();
        if cancelled {
            log::info!("file index build cancelled after {elapsed:?} (settings changed)");
            let _ = tx.send(Msg::Rebuild);
            self.status.lock().unwrap_or_else(|e| e.into_inner()).indexing = false;
            return None;
        }
        let (files, folders) = built.counts();
        log::info!(
            "indexed {files} files and {folders} folders in {elapsed:?}{}{}",
            if built.limit_reached { " (entry limit reached)" } else { "" },
            if built.unreadable > 0 { format!(", {} folders unreadable", built.unreadable) } else { String::new() }
        );
        if let Err(e) = cache::save(&built, config.fingerprint(), &self.cache_path) {
            log::warn!("could not write file index cache: {e}");
        }
        let dirs: Vec<String> = built.dir_lookup.keys().map(|d| d.to_string()).collect();
        *self.data.write().unwrap_or_else(|e| e.into_inner()) = built;
        let watcher = self.install_watcher(tx, &config, &dirs);
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        status.indexing = false;
        status.ready = true;
        status.last_indexed = Some(now_secs());
        status.last_duration_ms = Some(elapsed.as_millis() as u64);
        status.error = None;
        watcher
    }

    fn install_watcher(&self, tx: &Sender<Msg>, config: &IndexConfig, dirs: &[String]) -> Option<RecommendedWatcher> {
        let sender = tx.clone();
        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = sender.send(Msg::Fs(res));
        }) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("file change notifications unavailable ({e}); falling back to periodic rescans");
                self.status.lock().unwrap_or_else(|e| e.into_inner()).watching = false;
                return None;
            }
        };
        let complete = if cfg!(windows) {
            // ReadDirectoryChangesW watches whole trees cheaply; events are filtered later.
            config.roots.iter().all(|root| watcher.watch(root, RecursiveMode::Recursive).is_ok())
        } else {
            // inotify: one non-recursive watch per indexed folder so excluded trees cost nothing.
            let mut ok = dirs.len() <= MAX_WATCHES;
            for dir in dirs.iter().take(MAX_WATCHES) {
                if let Err(e) = watcher.watch(Path::new(dir), RecursiveMode::NonRecursive) {
                    log::warn!("stopped adding file watches at {dir}: {e}");
                    ok = false;
                    break;
                }
            }
            ok
        };
        if !complete {
            log::info!("file watching is partial; the index will also refresh every 30 minutes");
        }
        self.status.lock().unwrap_or_else(|e| e.into_inner()).watching = complete;
        Some(watcher)
    }

    fn apply_changes(&self, paths: &[PathBuf], watcher: Option<&mut RecommendedWatcher>) {
        let Some(config) = self.current_config() else { return };
        let mut dirs: HashSet<String> = HashSet::new();
        for path in paths {
            if config.path_is_filtered(path) {
                continue;
            }
            dirs.insert(path.to_string_lossy().into_owned());
            if let Some(parent) = path.parent() {
                dirs.insert(parent.to_string_lossy().into_owned());
            }
        }
        if dirs.is_empty() {
            return;
        }
        if dirs.len() > 400 {
            log::info!("{} folders changed at once; rebuilding the index", dirs.len());
            self.rebuild();
            return;
        }
        let new_dirs: Vec<String> = {
            let mut data = self.data.write().unwrap_or_else(|e| e.into_inner());
            let mut added = Vec::new();
            for dir in &dirs {
                for idx in data.rescan(dir, &config) {
                    added.push(data.dirs[idx as usize].to_string());
                }
            }
            data.compact_if_needed();
            added
        };
        if cfg!(not(windows)) {
            if let Some(w) = watcher {
                for dir in &new_dirs {
                    let _ = w.watch(Path::new(dir), RecursiveMode::NonRecursive);
                }
            }
        }
        log::debug!("applied file changes in {} folders ({} new)", dirs.len(), new_dirs.len());
    }

    /// Validates that `path` belongs to the index (and still exists on disk).
    fn verify(&self, path: &str) -> AppResult<bool> {
        let indexed = self.data.read().unwrap_or_else(|e| e.into_inner()).lookup(path);
        let Some(is_dir) = indexed else {
            return Err(AppError::NotFound("This item is not part of the search index.".into()));
        };
        if !Path::new(path).exists() {
            return Err(AppError::NotFound("This item no longer exists.".into()));
        }
        Ok(is_dir)
    }

    fn result_for(&self, path: &str, is_dir: bool, score: f32) -> SearchResult {
        let p = Path::new(path);
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string());
        let parent = p.parent().map(|d| self.display_path(d)).unwrap_or_default();
        let ext = p.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let executable = !is_dir && platform::launch::is_executable(p);
        SearchResult {
            id: format!("file:{path}"),
            kind: if is_dir { ResultKind::Folder } else { ResultKind::File },
            title: name,
            subtitle: parent,
            icon: if is_dir {
                ResultIcon::Symbol { name: "folder".into() }
            } else {
                ResultIcon::File { category: file_category(&ext), extension: ext.to_lowercase(), key: hex_key(path) }
            },
            score,
            reveal: true,
            note: executable.then(|| "Programs open in the file manager for safety".into()),
            actions: Vec::new(),
        }
    }

    /// `~/Documents` style path for subtitles.
    fn display_path(&self, path: &Path) -> String {
        if let Some(home) = &self.home {
            if let Ok(rest) = path.strip_prefix(home) {
                let rest = rest.to_string_lossy();
                return if rest.is_empty() { "~".into() } else { format!("~{}{rest}", std::path::MAIN_SEPARATOR) };
            }
        }
        path.to_string_lossy().into_owned()
    }
}

/// Files of one type share an icon (`.pdf`); programs, shortcuts and files
/// without an extension are drawn one by one.
fn icon_key(path: &Path) -> String {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext.is_empty() || OWN_ICON_EXTENSIONS.contains(&ext.as_str()) {
        path.to_string_lossy().into_owned()
    } else {
        format!(".{ext}")
    }
}

pub struct FilesProvider {
    index: Arc<FileIndex>,
}

impl FilesProvider {
    pub fn new(index: Arc<FileIndex>) -> Self {
        Self { index }
    }
}

impl SearchProvider for FilesProvider {
    fn id(&self) -> &'static str {
        "file"
    }

    fn enabled(&self, settings: &SpotlightSettings) -> bool {
        settings.search_files || settings.search_folders
    }

    fn search(&self, ctx: &SearchContext<'_>, out: &mut Vec<SearchResult>) {
        if !self.index.enabled.load(Ordering::SeqCst) {
            return;
        }
        let hits: Vec<(String, bool, u32)> = {
            let data = self.index.data.read().unwrap_or_else(|e| e.into_inner());
            data.search(ctx.query, ctx.settings.search_files, ctx.settings.search_folders, ctx.limit)
                .into_iter()
                .map(|(i, score)| {
                    let entry = &data.entries[i];
                    (data.full_path(entry), entry.is_dir(), score)
                })
                .collect()
        };
        out.extend(hits.into_iter().map(|(path, is_dir, score)| self.index.result_for(&path, is_dir, score as f32)));
    }

    fn execute(&self, path: &str, mode: ExecuteMode) -> AppResult<ExecutedItem> {
        let is_dir = self.index.verify(path)?;
        let p = Path::new(path);
        let reveal = mode == ExecuteMode::Reveal || (!is_dir && platform::launch::is_executable(p));
        let outcome = if reveal { platform::launch::reveal_path(p) } else { platform::launch::open_path(p) };
        outcome.map_err(|e| {
            log::error!("could not open {path}: {e}");
            AppError::Launch(format!("Could not open “{}”: {e}", p.file_name().unwrap_or_default().to_string_lossy()))
        })?;
        let result = self.index.result_for(path, is_dir, 0.0);
        Ok(ExecutedItem { kind: result.kind, title: result.title, subtitle: result.subtitle })
    }

    fn resolve(&self, path: &str) -> Option<SearchResult> {
        let is_dir = self.index.verify(path).ok()?;
        Some(self.index.result_for(path, is_dir, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_shared_by_type_unless_each_file_has_its_own() {
        let key = |p: &str| icon_key(Path::new(p));
        assert_eq!(key("/home/a/Report.PDF"), ".pdf");
        assert_eq!(key("/home/b/poster.psd"), key("/tmp/other.psd"));
        assert_eq!(key("/home/a/setup.exe"), "/home/a/setup.exe");
        assert_eq!(key("/home/a/Makefile"), "/home/a/Makefile");
    }

    #[test]
    fn only_indexed_files_get_icons() {
        let tmp = tempfile::tempdir().unwrap();
        let index = FileIndex::new(tmp.path().join("cache.bin"), None);
        let outside = tmp.path().join("not-indexed.pdf");
        std::fs::write(&outside, b"%PDF").unwrap();
        assert!(index.icon(&outside.to_string_lossy()).is_none());
    }
}
