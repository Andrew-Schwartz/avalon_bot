use std::ffi::OsStr;
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
/// # Errors
///
/// Errors if the the file at [`path`](path) isn't one of the supported image types (which are png,
/// jpg, and gif), or if [`std::fs::read`](std::fs::read) fails
pub fn hash_image<P: AsRef<Path>>(path: P) -> Result<String, ImageHashError> {
    let path = path.as_ref();
    let image = path.extension()
        .and_then(OsStr::to_str)
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

pub trait TryRemove<T> {
    fn try_remove(&mut self, index: usize) -> Option<T>;
}

impl<T> TryRemove<T> for Vec<T> {
    fn try_remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len() {
            None
        } else {
            Some(self.remove(index))
        }
    }
}

pub fn array_try_from_iter<T, Iterable, F, Error, const N: usize>(
    iterable: Iterable,
    mut not_enough_elements: F,
) -> Result<[T; N], Error>
    where
        Iterable: IntoIterator<Item=Result<T, Error>>,
    // really should be able to be `FnOnce` but try_array_init's signature can't show that
        F: FnMut(usize) -> Error,
{
    array_init::try_array_init({
        let mut iterator = iterable.into_iter();
        move |i| {
            match iterator.next() {
                Some(a) => a,
                None => Err(not_enough_elements(i)),
            }
        }
    })
}
