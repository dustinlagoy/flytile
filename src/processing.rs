use serde_json;

#[derive(Debug, Clone, Copy)]
pub struct ProcessingError;

impl std::error::Error for ProcessingError {}
impl std::fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "processing error")
    }
}
impl From<std::io::Error> for ProcessingError {
    fn from(_: std::io::Error) -> Self {
        ProcessingError
    }
}
impl From<std::env::VarError> for ProcessingError {
    fn from(_: std::env::VarError) -> Self {
        ProcessingError
    }
}
impl From<serde_json::Error> for ProcessingError {
    fn from(_: serde_json::Error) -> Self {
        ProcessingError
    }
}
impl From<reqwest::Error> for ProcessingError {
    fn from(_: reqwest::Error) -> Self {
        ProcessingError
    }
}
impl From<image::ImageError> for ProcessingError {
    fn from(_: image::ImageError) -> Self {
        ProcessingError
    }
}
impl From<ab_glyph::InvalidFont> for ProcessingError {
    fn from(_: ab_glyph::InvalidFont) -> Self {
        ProcessingError
    }
}
