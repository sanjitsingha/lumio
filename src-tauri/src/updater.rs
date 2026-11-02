//! In-app updates from signed GitHub releases.
//!
//! Releases are built by `.github/workflows/release.yml`, which uploads the
//! installers, their minisign signatures and a `latest.json` manifest. Lummo
//! reads `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`
//! (derived from `repository` in Cargo.toml), downloads the artifact matching
//! how it was installed (NSIS `.exe`, `.deb` or AppImage), verifies it against
//! the public key embedded in `tauri.conf.json`, installs it and restarts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::events;
use crate::storage::history::now_secs;

/// Overrides the release feed, e.g. `http://127.0.0.1:8765/latest.json` for
/// local testing (plain http is only accepted by debug builds).
pub const ENDPOINT_ENV: &str = "LUMMO_UPDATE_ENDPOINT";
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading { downloaded: u64, total: Option<u64> },
    /// Downloaded and signature-verified; ready to install.
    Ready,
    Installing,
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current_version: String,
    /// A release feed is known (repository set or override given).
    pub configured: bool,
    pub feed: Option<String>,
    pub release_page: Option<String>,
    pub can_install: bool,
    /// Why installing is impossible, or what installing involves.
    pub install_note: Option<String>,
    pub phase: Phase,
    pub latest: Option<Release>,
    pub last_checked: Option<u64>,
}

/// `https://github.com/<owner>/<repo>` (optionally `.git` / trailing slash).
pub fn parse_github_repo(url: &str) -> Option<(String, String)> {
    let rest = url.trim().strip_prefix("https://github.com/")?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let valid = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    (parts.next().is_none() && valid(owner) && valid(repo)).then(|| (owner.to_string(), repo.to_string()))
}

fn repository() -> Option<(String, String)> {
    parse_github_repo(env!("CARGO_PKG_REPOSITORY"))
}

pub fn feed_url() -> Option<Url> {
    if let Some(custom) = std::env::var(ENDPOINT_ENV).ok().filter(|v| !v.is_empty()) {
        return match Url::parse(&custom) {
            Ok(url) => Some(url),
            Err(e) => {
                log::error!("ignoring invalid {ENDPOINT_ENV}: {e}");
                None
            }
        };
    }
    let (owner, repo) = repository()?;
    Url::parse(&format!("https://github.com/{owner}/{repo}/releases/latest/download/latest.json")).ok()
}

pub fn release_page() -> Option<String> {
    repository().map(|(owner, repo)| format!("https://github.com/{owner}/{repo}/releases/latest"))
}

/// Whether this binary can replace itself, based on how it was packaged.
fn install_support() -> (bool, Option<String>) {
    use tauri::utils::config::BundleType;
    if cfg!(debug_assertions) {
        return (
            false,
            Some(
                "This is a development build: updates can be checked and downloaded here, but they install only into \
                 an installed copy of Lummo (.exe installer, .deb or AppImage)."
                    .into(),
            ),
        );
    }
    match tauri::utils::platform::bundle_type() {
        Some(BundleType::Nsis) | Some(BundleType::Msi) => (true, None),
        Some(BundleType::AppImage) => (true, None),
        Some(BundleType::Deb) | Some(BundleType::Rpm) => {
            (true, Some("Installing asks for your administrator password (system package).".into()))
        }
        _ => (
            false,
            Some("This copy of Lummo was not installed from a package, so it cannot update itself. Download the \
                  latest release instead."
                .into()),
        ),
    }
}

pub struct UpdateService {
    status: Mutex<UpdateStatus>,
    pending: Mutex<Option<Update>>,
    downloaded: Mutex<Option<Vec<u8>>>,
    busy: AtomicBool,
    notified_version: Mutex<Option<String>>,
}

/// Clears the busy flag however the operation ends.
struct BusyGuard<'a>(&'a AtomicBool);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl UpdateService {
    pub fn new(current_version: String) -> Self {
        let (can_install, install_note) = install_support();
        let feed = feed_url();
        Self {
            status: Mutex::new(UpdateStatus {
                current_version,
                configured: feed.is_some(),
                feed: feed.map(|u| u.to_string()),
                release_page: release_page(),
                can_install,
                install_note,
                phase: Phase::Idle,
                latest: None,
                last_checked: None,
            }),
            pending: Mutex::default(),
            downloaded: Mutex::default(),
            busy: AtomicBool::new(false),
            notified_version: Mutex::default(),
        }
    }

    pub fn status(&self) -> UpdateStatus {
        self.status.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set_phase(&self, app: &AppHandle, phase: Phase) {
        let snapshot = {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            status.phase = phase;
            status.clone()
        };
        let _ = app.emit(events::UPDATE_STATUS, snapshot);
    }

    fn begin(&self) -> AppResult<BusyGuard<'_>> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(AppError::Invalid("An update operation is already running.".into()));
        }
        Ok(BusyGuard(&self.busy))
    }

    fn fail(&self, app: &AppHandle, message: String) -> AppError {
        log::error!("update failed: {message}");
        self.set_phase(app, Phase::Error { message: message.clone() });
        AppError::Platform(message)
    }

    /// Asks the release feed whether a newer version exists.
    pub async fn check(&self, app: &AppHandle) -> AppResult<UpdateStatus> {
        let url = feed_url().ok_or_else(|| {
            AppError::Unsupported(
                "Updates are not configured for this build: set `repository` in src-tauri/Cargo.toml.".into(),
            )
        })?;
        let _busy = self.begin()?;
        self.set_phase(app, Phase::Checking);
        let handle = app.clone();
        let updater = app
            .updater_builder()
            .endpoints(vec![url.clone()])
            .and_then(|b| {
                b.timeout(Duration::from_secs(30))
                    .on_before_exit(move || {
                        // the Windows installer replaces Lummo right after this
                        if let Some(state) = handle.try_state::<AppState>() {
                            state.shortcuts.shutdown();
                        }
                    })
                    .build()
            })
            .map_err(|e| self.fail(app, format!("Could not prepare the update check: {e}")))?;
        log::info!("checking for updates at {url}");
        let result = updater.check().await;
        let found = match result {
            Ok(found) => found,
            Err(e) => return Err(self.fail(app, format!("Could not check for updates: {e}"))),
        };
        *self.downloaded.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let phase = {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            status.last_checked = Some(now_secs());
            status.latest = found.as_ref().map(|u| Release {
                version: u.version.clone(),
                notes: u.body.clone().filter(|b| !b.trim().is_empty()),
                date: u.date.map(|d| d.date().to_string()),
            });
            if let Some(u) = &found {
                log::info!("update available: {} -> {}", u.current_version, u.version);
                Phase::Available
            } else {
                log::info!("Lummo is up to date");
                Phase::UpToDate
            }
        };
        *self.pending.lock().unwrap_or_else(|e| e.into_inner()) = found;
        self.set_phase(app, phase);
        Ok(self.status())
    }

    /// Downloads the pending update and verifies its signature.
    pub async fn download(&self, app: &AppHandle) -> AppResult<()> {
        let update = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| AppError::Invalid("No update is available. Check for updates first.".into()))?;
        let _busy = self.begin()?;
        self.set_phase(app, Phase::Downloading { downloaded: 0, total: None });
        let mut downloaded: u64 = 0;
        let mut last_emit = Instant::now();
        let progress_app = app.clone();
        let result = update
            .download(
                |chunk, total| {
                    downloaded += chunk as u64;
                    if last_emit.elapsed() >= PROGRESS_EVENT_INTERVAL || total.is_some_and(|t| downloaded >= t) {
                        last_emit = Instant::now();
                        if let Some(state) = progress_app.try_state::<AppState>() {
                            state.updates.set_phase(&progress_app, Phase::Downloading { downloaded, total });
                        }
                    }
                },
                || {},
            )
            .await;
        match result {
            Ok(bytes) => {
                log::info!("downloaded and verified update {} ({} bytes)", update.version, bytes.len());
                *self.downloaded.lock().unwrap_or_else(|e| e.into_inner()) = Some(bytes);
                self.set_phase(app, Phase::Ready);
                Ok(())
            }
            Err(e) => Err(self.fail(app, format!("The download failed or its signature is invalid: {e}"))),
        }
    }

    /// Installs the downloaded update and restarts Lummo.
    pub async fn install(&self, app: &AppHandle) -> AppResult<()> {
        let (can_install, note) = install_support();
        if !can_install {
            return Err(AppError::Unsupported(note.unwrap_or_default()));
        }
        if self.downloaded.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            self.download(app).await?;
        }
        let update = self.pending.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let bytes = self.downloaded.lock().unwrap_or_else(|e| e.into_inner()).take();
        let (Some(update), Some(bytes)) = (update, bytes) else {
            return Err(AppError::Invalid("No downloaded update to install.".into()));
        };
        let _busy = self.begin()?;
        self.set_phase(app, Phase::Installing);
        log::info!("installing update {}", update.version);
        // dpkg via pkexec (password prompt) and the Windows installer block.
        let outcome = tauri::async_runtime::spawn_blocking(move || update.install(bytes))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));
        if let Err(e) = outcome {
            return Err(self.fail(app, format!("The update could not be installed: {e}")));
        }
        log::info!("update installed; restarting");
        relaunch_after_update(app);
        Ok(())
    }

    fn should_notify(&self, version: &str) -> bool {
        let mut last = self.notified_version.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_deref() == Some(version) {
            return false;
        }
        *last = Some(version.to_string());
        true
    }
}

/// Starts the new version once this process has exited, so the single-
/// instance check does not hand the launch back to the old process.
fn relaunch_after_update(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    {
        let exe = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from).or_else(|| {
            std::env::current_exe().ok().map(|p| {
                // the replaced binary shows up as "/usr/bin/lummo (deleted)"
                let s = p.to_string_lossy();
                std::path::PathBuf::from(s.strip_suffix(" (deleted)").unwrap_or(&s).to_string())
            })
        });
        match exe {
            Some(exe) => {
                if let Err(e) = std::process::Command::new(&exe)
                    .arg(crate::cli::AFTER_UPDATE_FLAG)
                    .arg(std::process::id().to_string())
                    .spawn()
                {
                    log::error!("could not start the updated Lummo ({}): {e}", exe.display());
                }
            }
            None => log::error!("could not determine Lummo's executable to restart"),
        }
    }
    // On Windows the installer has already been started and restarts Lummo itself.
    app.exit(0);
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("could not show a notification: {e}");
    }
}

/// Background checks: shortly after startup, then every few hours.
pub fn start_background_checks(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("lummo-updates".into()).spawn(move || {
        std::thread::sleep(FIRST_CHECK_DELAY);
        loop {
            let Some(state) = app.try_state::<AppState>() else { return };
            let general = state.settings().general;
            if state.updates.status().configured {
                let found = tauri::async_runtime::block_on(state.updates.check(&app));
                if let Ok(status) = found {
                    if let (Phase::Available, Some(latest)) = (&status.phase, &status.latest) {
                        if general.auto_install_updates && status.can_install {
                            notify(&app, "Updating Lummo", &format!("Installing version {}…", latest.version));
                            if let Err(e) = tauri::async_runtime::block_on(state.updates.install(&app)) {
                                log::error!("automatic update failed: {e}");
                            }
                        } else if state.updates.should_notify(&latest.version) {
                            notify(
                                &app,
                                &format!("Lummo {} is available", latest.version),
                                "Open Lummo Settings → About to install it.",
                            );
                        }
                    }
                }
            }
            std::thread::sleep(CHECK_INTERVAL);
        }
    });
    if let Err(e) = spawned {
        log::error!("could not start background update checks: {e}");
    }
}

/// Used by `lummo --after-update <pid>`: wait until the previous process has exited.
pub fn wait_for_previous_instance(pid: u32) {
    #[cfg(target_os = "linux")]
    {
        let proc_dir = std::path::PathBuf::from(format!("/proc/{pid}"));
        let deadline = Instant::now() + Duration::from_secs(15);
        while proc_dir.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = pid;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_repository_urls() {
        assert_eq!(parse_github_repo("https://github.com/acme/lummo"), Some(("acme".into(), "lummo".into())));
        assert_eq!(parse_github_repo("https://github.com/acme/lummo.git"), Some(("acme".into(), "lummo".into())));
        assert_eq!(parse_github_repo("https://github.com/acme/lummo/"), Some(("acme".into(), "lummo".into())));
        assert_eq!(parse_github_repo("https://github.com/acme"), None);
        assert_eq!(parse_github_repo("https://github.com/acme/lummo/tree/main"), None);
        assert_eq!(parse_github_repo("https://gitlab.com/acme/lummo"), None);
        assert_eq!(parse_github_repo("https://github.com/ac me/lummo"), None);
        assert_eq!(parse_github_repo(""), None);
    }

    #[test]
    fn phases_serialize_with_a_state_tag() {
        let v = serde_json::to_value(Phase::Downloading { downloaded: 10, total: Some(100) }).unwrap();
        assert_eq!(v["state"], "downloading");
        assert_eq!(v["total"], 100);
        assert_eq!(serde_json::to_value(Phase::UpToDate).unwrap()["state"], "upToDate");
    }

    #[test]
    fn debug_builds_never_install() {
        if cfg!(debug_assertions) {
            let (can_install, note) = install_support();
            assert!(!can_install);
            assert!(note.unwrap().contains("development build"));
        }
    }
}
