use std::io;
use std::path::Path;

use thiserror::Error;

use ImageHashError::*;

#[derive(Debug, Error)]
pub enum ImageHashError {
    #[error("Unsupported error type. Only `png`, `jpeg`, and `gif` are supported by Discord.")]
    FileType,
    #[error("IO error {0}")]
    Io(io::Error),
}

// todo fix this, `base64::encode` is way too long
//  actually I was trying to put this where `icon_url` should be, have to test on some POST/PATCH or
//  some method
pub fn hash_image<P: AsRef<Path>>(path: P) -> Result<String, ImageHashError> {
    let path = path.as_ref();
    let image = path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| match ext {
            "jpg" | "jpeg" => Some("jpeg"),
            "png" => Some("png"),
            "gif" => Some("gif"),
            _ => None,
        });
    if let Some(image) = image {
        match std::fs::read(path) {
            Ok(file) => {
                Ok(format!("data:image/{};base64,{}", image, base64::encode(&file)))
            }
            Err(e) => Err(Io(e)),
        }
    } else {
        Err(FileType)
    }
}