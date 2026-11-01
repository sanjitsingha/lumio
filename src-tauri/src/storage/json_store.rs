use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// Writes `value` as pretty JSON using write-to-temp + rename, so a crash or
/// power loss never leaves a half-written settings file behind.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    write_atomic(path, &data)
}

pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = tmp_path(path);
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Reads a JSON file into a generic value. `Ok(None)` when the file does not exist.
pub fn read_json_value(path: &Path) -> std::io::Result<Option<Result<serde_json::Value, serde_json::Error>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Moves an unreadable file aside (`settings.corrupt-<unix>.json`) so the user's
/// data is never silently destroyed. Returns the backup location.
pub fn quarantine(path: &Path, label: &str) -> std::io::Result<PathBuf> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("data");
    let backup = path.with_file_name(format!("{stem}.{label}-{stamp}.json"));
    fs::rename(path, &backup)?;
    Ok(backup)
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_round_trips_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("value.json");
        write_json_atomic(&path, &serde_json::json!({ "a": 1 })).unwrap();
        let value = read_json_value(&path).unwrap().unwrap().unwrap();
        assert_eq!(value["a"], 1);
        assert!(!tmp_path(&path).exists());
    }

    #[test]
    fn missing_file_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_json_value(&dir.path().join("nope.json")).unwrap().is_none());
    }

    #[test]
    fn quarantine_moves_file_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{ not json").unwrap();
        let backup = quarantine(&path, "corrupt").unwrap();
        assert!(!path.exists());
        assert!(backup.exists());
        assert!(backup.file_name().unwrap().to_string_lossy().starts_with("settings.corrupt-"));
    }
}
