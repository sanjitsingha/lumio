//! Exchange rates for Spotlight's currency answers. Lummo's website holds the
//! service's key and asks it once a day; the app downloads that result at most
//! once a day and keeps it, so every conversion is answered from this file and
//! works offline.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::storage::json_store;

const RATES_URL: &str = "https://lummo.space/api/rates";
const TIMEOUT: Duration = Duration::from_secs(15);
/// Saved rates are replaced once they are this old.
const MAX_AGE_SECS: u64 = 24 * 3600;
/// Wakes up at least this often, because a sleeping computer stops the timer.
const CHECK_EVERY_SECS: u64 = 3600;
const RETRY: Duration = Duration::from_secs(15 * 60);
/// Fewer currencies than this is a broken answer, not a small list.
const MIN_CURRENCIES: usize = 20;

static RATES: RwLock<Option<Rates>> = RwLock::new(None);

/// Units of each currency per one `base`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rates {
    base: String,
    /// When the service updated these rates, in seconds since 1970.
    updated_at: u64,
    rates: HashMap<String, f64>,
    /// When this computer downloaded them, in seconds since 1970.
    #[serde(default)]
    fetched_at: u64,
}

impl Rates {
    fn is_valid(&self) -> bool {
        self.rates.len() >= MIN_CURRENCIES
            && self.rates.get(&self.base) == Some(&1.0)
            && self.rates.iter().all(|(code, rate)| {
                code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase()) && rate.is_finite() && *rate > 0.0
            })
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Seconds until rates downloaded at `fetched_at` are due; a clock that moved back makes them due now.
fn seconds_until_due(fetched_at: u64, now: u64) -> u64 {
    if fetched_at > now {
        return 0;
    }
    (fetched_at + MAX_AGE_SECS).saturating_sub(now)
}

fn load(path: &Path) -> Option<Rates> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<Rates>(&bytes).ok().filter(Rates::is_valid)
}

/// A development build can be pointed at a local copy of the website.
fn rates_url() -> String {
    #[cfg(debug_assertions)]
    if let Some(url) = std::env::var("LUMMO_RATES_URL").ok().filter(|u| !u.is_empty()) {
        return url;
    }
    RATES_URL.to_string()
}

async fn download() -> Result<Rates, String> {
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(concat!("Lummo/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(rates_url()).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("the rates server answered {}", response.status()));
    }
    let rates: Rates = response.json().await.map_err(|e| e.to_string())?;
    if rates.is_valid() {
        Ok(rates)
    } else {
        Err("the rates server sent invalid rates".into())
    }
}

fn current() -> Option<Rates> {
    RATES.read().unwrap_or_else(|e| e.into_inner()).clone()
}

fn keep_up_to_date(path: PathBuf) {
    loop {
        let now = unix_now();
        let due = current().map_or(0, |rates| seconds_until_due(rates.fetched_at, now));
        if due > 0 {
            std::thread::sleep(Duration::from_secs(due.min(CHECK_EVERY_SECS)));
            continue;
        }
        match tauri::async_runtime::block_on(download()) {
            Ok(mut rates) => {
                rates.fetched_at = now;
                if let Err(e) = json_store::write_json_atomic(&path, &rates) {
                    log::warn!("could not keep the exchange rates: {e}");
                }
                log::info!("exchange rates updated ({} currencies)", rates.rates.len());
                *RATES.write().unwrap_or_else(|e| e.into_inner()) = Some(rates);
            }
            Err(e) => {
                log::warn!("exchange rate update failed: {e}");
                std::thread::sleep(RETRY);
            }
        }
    }
}

/// Loads the saved rates and keeps them up to date in the background.
pub fn start(app: &AppHandle) {
    let path = app.state::<AppState>().paths.data_dir.join("currency-rates.json");
    *RATES.write().unwrap_or_else(|e| e.into_inner()) = load(&path);
    let spawned = std::thread::Builder::new().name("lummo-rates".into()).spawn(move || keep_up_to_date(path));
    if let Err(e) = spawned {
        log::error!("could not start exchange rate updates: {e}");
    }
}

pub fn rates() -> Option<Rates> {
    current()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Rates {
        let mut rates: HashMap<String, f64> =
            (0..MIN_CURRENCIES as u8).map(|i| (format!("AA{}", (b'A' + i) as char), 2.0)).collect();
        rates.insert("USD".into(), 1.0);
        Rates { base: "USD".into(), updated_at: 10, rates, fetched_at: 20 }
    }

    #[test]
    fn accepts_only_sensible_rates() {
        assert!(sample().is_valid());
        let broken = |change: fn(&mut Rates)| {
            let mut rates = sample();
            change(&mut rates);
            rates.is_valid()
        };
        assert!(!broken(|r| r.rates.retain(|code, _| code == "USD")));
        assert!(!broken(|r| {
            r.rates.insert("USD".into(), 2.0);
        }));
        assert!(!broken(|r| {
            r.rates.insert("INR".into(), 0.0);
        }));
        assert!(!broken(|r| {
            r.rates.insert("INR".into(), f64::NAN);
        }));
        assert!(!broken(|r| {
            r.rates.insert("inr".into(), 90.0);
        }));
    }

    #[test]
    fn downloads_at_most_once_a_day() {
        assert_eq!(seconds_until_due(1000, 1000), MAX_AGE_SECS);
        assert_eq!(seconds_until_due(1000, 1000 + MAX_AGE_SECS - 5), 5);
        assert_eq!(seconds_until_due(1000, 1000 + MAX_AGE_SECS), 0);
        assert_eq!(seconds_until_due(1000, 1000 + 2 * MAX_AGE_SECS), 0);
        assert_eq!(seconds_until_due(5000, 1000), 0);
    }

    #[test]
    fn saved_rates_survive_a_restart_and_bad_files_do_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("currency-rates.json");
        assert_eq!(load(&path), None);

        json_store::write_json_atomic(&path, &sample()).unwrap();
        assert_eq!(load(&path), Some(sample()));

        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(load(&path), None);
    }
}
