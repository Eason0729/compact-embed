#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::{
    borrow::Cow,
    io::{self, Read as _},
    path::{Component, Path},
};

mod mime;
pub use mime::MimeType;

#[cfg(feature = "derive")]
pub use compact_embed_derive::Embed;

#[cfg(feature = "derive")]
#[doc(hidden)]
pub use compact_embed_derive::__embed_data;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encoding {
    Identity,
    Zstd,
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub data: Cow<'static, [u8]>,
    pub encoding: Encoding,
    pub original_size: u64,
    pub mime_type: MimeType,
}

impl Asset {
    pub fn reader(&self) -> io::Result<Box<dyn io::Read + '_>> {
        match self.encoding {
            Encoding::Identity => Ok(Box::new(io::Cursor::new(self.data.as_ref()))),
            #[cfg(feature = "decode")]
            Encoding::Zstd => Ok(Box::new(zstd::stream::read::Decoder::new(
                self.data.as_ref(),
            )?)),
            #[cfg(not(feature = "decode"))]
            Encoding::Zstd => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "enable compact-embed's decode feature to read Zstd assets",
            )),
        }
    }

    pub fn decoded(&self) -> io::Result<Cow<'_, [u8]>> {
        if self.encoding == Encoding::Identity {
            return Ok(Cow::Borrowed(self.data.as_ref()));
        }
        let mut bytes = Vec::new();
        self.reader()?
            .take(self.original_size.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != self.original_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "decoded asset length differs from embedded metadata",
            ));
        }
        Ok(Cow::Owned(bytes))
    }
}

pub trait Embed {
    fn get(path: &str) -> io::Result<Option<Asset>>;
}

/// Call from the consumer's build script to rebuild when the asset directory changes.
pub fn track(folder: impl AsRef<Path>) {
    let path = folder.as_ref();
    println!("cargo:rerun-if-changed={}", path.display());
}

#[doc(hidden)]
pub fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[doc(hidden)]
pub fn dynamic_asset(root: &str, path: &str, maximum_file_bytes: u64) -> io::Result<Option<Asset>> {
    if !valid_path(path) {
        return Ok(None);
    }
    let mut absolute = Path::new(root).to_path_buf();
    reject_symlink(&absolute)?;
    for component in Path::new(path).components() {
        absolute.push(component);
        match std::fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "compact-embed rejects symlinks",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    }
    let metadata = std::fs::metadata(&absolute)?;
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > maximum_file_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "asset exceeds max_file_bytes",
        ));
    }
    let mut data = Vec::new();
    std::fs::File::open(absolute)?
        .take(maximum_file_bytes.saturating_add(1))
        .read_to_end(&mut data)?;
    if data.len() as u64 > maximum_file_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "asset exceeds max_file_bytes",
        ));
    }
    Ok(Some(Asset {
        original_size: data.len() as u64,
        data: Cow::Owned(data),
        encoding: Encoding::Identity,
        mime_type: mime_type(path),
    }))
}

fn reject_symlink(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "compact-embed rejects symlinks",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[doc(hidden)]
pub fn mime_type(path: &str) -> MimeType {
    MimeType::from_extension(
        &Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_ascii_lowercase(),
    )
}
