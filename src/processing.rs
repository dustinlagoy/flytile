use serde_json;

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
    fn from(_: std::io::Error) -> Self {
        ProcessingError {
            message: format!("io"),
        }
    }
}
impl From<std::env::VarError> for ProcessingError {
    fn from(_: std::env::VarError) -> Self {
        ProcessingError {
            message: format!("env"),
        }
    }
}
impl From<serde_json::Error> for ProcessingError {
    fn from(_: serde_json::Error) -> Self {
        ProcessingError {
            message: format!("serde"),
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
    fn from(_: image::ImageError) -> Self {
        ProcessingError {
            message: format!("image"),
        }
    }
}
impl From<ab_glyph::InvalidFont> for ProcessingError {
    fn from(_: ab_glyph::InvalidFont) -> Self {
        ProcessingError {
            message: format!("io"),
        }
    }
}
