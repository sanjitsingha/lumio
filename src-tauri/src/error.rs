use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Errors returned to the frontend. Every variant serializes to
/// `{ "code": "...", "message": "..." }` so the UI can show a friendly
/// message and branch on the code.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{message}")]
    Shortcut { code: &'static str, message: String },
    #[error("{0}")]
    Launch(String),
    #[error("{0}")]
    Platform(String),
    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Window error: {0}")]
    Tauri(#[from] tauri::Error),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Invalid(_) => "invalid",
            AppError::NotFound(_) => "not-found",
            AppError::Unsupported(_) => "unsupported",
            AppError::Shortcut { code, .. } => code,
            AppError::Launch(_) => "launch-failed",
            AppError::Platform(_) => "platform",
            AppError::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => "permission-denied",
            AppError::Io(_) => "io",
            AppError::Json(_) => "invalid-data",
            AppError::Tauri(_) => "window",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_message() {
        let err = AppError::Shortcut { code: "conflict", message: "Alt+Space is taken".into() };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "conflict");
        assert_eq!(json["message"], "Alt+Space is taken");
    }

    #[test]
    fn permission_denied_has_dedicated_code() {
        let err = AppError::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert_eq!(err.code(), "permission-denied");
    }
}
