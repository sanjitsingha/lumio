use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultKind {
    App,
    Action,
    Folder,
    File,
    /// "Search Google for …", always listed last.
    Web,
}

impl ResultKind {
    /// Relative weight applied to match scores so apps win close calls.
    pub fn weight(self) -> f32 {
        match self {
            ResultKind::App => 1.0,
            ResultKind::Action => 0.97,
            ResultKind::Folder => 0.86,
            ResultKind::File => 0.84,
            // placed after the ranking, see `engine::finish`
            ResultKind::Web => 0.0,
        }
    }

    pub fn section(self) -> (&'static str, &'static str) {
        match self {
            ResultKind::App => ("apps", "Applications"),
            ResultKind::Action => ("actions", "Quick Actions"),
            ResultKind::Folder => ("folders", "Folders"),
            ResultKind::File => ("files", "Files"),
            ResultKind::Web => ("web", "Web"),
        }
    }
}

/// How the UI should draw a result's icon.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ResultIcon {
    /// Served by the `lummo-icon` protocol at `/app/<key>`.
    App { key: String },
    /// A Lucide icon name rendered by the frontend.
    Symbol { name: String },
    /// The system's icon for the file's type, served at `/file/<key>`; the
    /// glyph for `category` (document, image, ...) stands in when there is none.
    File { category: &'static str, extension: String, key: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// `<provider>:<local id>`; only ids issued by the backend can be executed.
    pub id: String,
    pub kind: ResultKind,
    pub title: String,
    pub subtitle: String,
    pub icon: ResultIcon,
    pub score: f32,
    /// Secondary action available with Ctrl+Enter (e.g. "Show in Folder").
    pub reveal: bool,
    /// Short note shown on the selected row.
    pub note: Option<String>,
    /// The app's own actions (e.g. "New Private Window"), shown below it while it is selected.
    pub actions: Vec<ResultAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultAction {
    /// Executable result id, issued by the backend like any other.
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub apps: usize,
    pub files: usize,
    pub folders: usize,
    pub indexing: bool,
    pub ready: bool,
    pub last_indexed: Option<u64>,
    pub last_duration_ms: Option<u64>,
    pub limit_reached: bool,
    pub watching: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub request_id: u64,
    pub query: String,
    pub sections: Vec<Section>,
    pub total: usize,
    pub elapsed_ms: f64,
    pub suggestion: Option<String>,
    pub index: IndexStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecuteMode {
    Open,
    Reveal,
}

/// Maps a file extension to a generic category used for its glyph.
pub fn file_category(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "pdf" => "pdf",
        "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "pages" | "tex" => "document",
        "xls" | "xlsx" | "ods" | "csv" | "tsv" | "numbers" => "spreadsheet",
        "ppt" | "pptx" | "odp" | "key" => "presentation",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "tif" | "tiff" | "heic" | "avif" | "ico" | "raw" => "image",
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "m4v" => "video",
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" | "wma" => "audio",
        "zip" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "7z" | "rar" | "zst" | "deb" | "rpm" | "iso" => "archive",
        "rs" | "js" | "jsx" | "ts" | "tsx" | "py" | "c" | "h" | "cpp" | "hpp" | "go" | "java" | "kt" | "rb" | "php"
        | "cs" | "swift" | "html" | "css" | "scss" | "json" | "toml" | "yaml" | "yml" | "xml" | "sh" | "ps1" | "vala"
        | "lua" | "sql" => "code",
        "exe" | "msi" | "bat" | "cmd" | "com" | "appimage" | "run" | "bin" | "desktop" | "lnk" => "executable",
        _ => "generic",
    }
}
