use serde_json;

pub type ProcessingResult<T> = std::result::Result<T, ProcessingError>;

#[derive(Debug, Clone)]
pub struct ProcessingError {
    message: String,
}

impl ProcessingError {
    pub fn new(message: &str) -> Self {
        ProcessingError {
            message: message.into(),
        }
    }
}

impl std::error::Error for ProcessingError {}
impl std::fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "processing error: {}", self.message)
    }
}
impl From<std::io::Error> for ProcessingError {
    fn from(error: std::io::Error) -> Self {
        ProcessingError {
            message: format!("io: {}", error),
        }
    }
}
impl From<std::env::VarError> for ProcessingError {
    fn from(error: std::env::VarError) -> Self {
        ProcessingError {
            message: format!("env: {}", error),
        }
    }
}
impl From<serde_json::Error> for ProcessingError {
    fn from(error: serde_json::Error) -> Self {
        ProcessingError {
            message: format!("serde: {}", error),
        }
    }
}
impl From<reqwest::Error> for ProcessingError {
    fn from(error: reqwest::Error) -> Self {
        ProcessingError {
            message: format!("reqwest: {}", error),
        }
    }
}
impl From<image::ImageError> for ProcessingError {
    fn from(error: image::ImageError) -> Self {
        ProcessingError {
            message: format!("image: {}", error),
        }
    }
}
impl From<ab_glyph::InvalidFont> for ProcessingError {
    fn from(error: ab_glyph::InvalidFont) -> Self {
        ProcessingError {
            message: format!("font: {}", error),
        }
    }
}
