//! CPU rasterisers for the image-source nodes (Bitmap, Text, SVG). Each returns an RGBA
//! `Raster`, top row first, that the node engine uploads and its kernel resamples. Kept in its
//! own crate so the decoders build once and never with the engine.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod svg;
mod text;

use std::path::{Path, PathBuf};

pub use svg::{svg, svg_bytes};
pub use text::{Align, TextSpec, text, text_with_font};

/// RGBA pixels, top row first: 8-bit values as stored (sRGB-encoded) or floats for HDR and
/// 16-bit files.
pub enum Pixels {
    U8(Vec<u8>),
    F32(Vec<f32>),
}

pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels,
}

/// Header inspection before allocating image pixels. Original encoded bytes remain unchanged.
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub linear_color: bool,
}
pub fn image_info(bytes: &[u8], path: &Path) -> Result<ImageInfo, SourceError> {
    let decode = |msg: String| SourceError::Decode {
        path: path.to_owned(),
        msg,
    };
    if bytes.len() > 256 * 1024 * 1024 {
        return Err(decode("Image exceeds 256 MiB".into()));
    }
    let format = image::ImageFormat::from_path(path)
        .or_else(|_| image::guess_format(bytes))
        .map_err(|e| decode(e.to_string()))?;
    let reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let linear_color = matches!(
        reader.format(),
        Some(image::ImageFormat::Hdr | image::ImageFormat::OpenExr)
    );
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| decode(e.to_string()))?;
    if width == 0
        || height == 0
        || width.max(height) > 16384
        || u64::from(width) * u64::from(height) > 64 * 1024 * 1024
    {
        return Err(decode(
            "Image dimensions must be nonzero, at most 16384 per side and 64 megapixels".into(),
        ));
    }
    Ok(ImageInfo {
        width,
        height,
        linear_color,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("{}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}: {msg}", path.display())]
    Decode { path: PathBuf, msg: String },
    #[error("built-in font: {0}")]
    Font(String),
    #[error("no image size")]
    Empty,
}

fn read(path: &Path) -> Result<Vec<u8>, SourceError> {
    std::fs::read(path).map_err(|source| SourceError::Read {
        path: path.to_owned(),
        source,
    })
}

/// Decodes an image file, shrunk to fit `max_side` (keeping its aspect).
pub fn bitmap(path: &Path, max_side: u32) -> Result<Raster, SourceError> {
    bitmap_bytes(&read(path)?, path, max_side)
}

pub fn bitmap_bytes(bytes: &[u8], path: &Path, max_side: u32) -> Result<Raster, SourceError> {
    image_info(bytes, path)?;
    let decode = |msg: String| SourceError::Decode {
        path: path.to_owned(),
        msg,
    };
    let format = image::ImageFormat::from_path(path)
        .or_else(|_| image::guess_format(bytes))
        .map_err(|e| decode(e.to_string()))?;
    let mut img =
        image::load_from_memory_with_format(bytes, format).map_err(|e| decode(e.to_string()))?;
    if img.width() > max_side || img.height() > max_side {
        img = img.thumbnail(max_side, max_side);
    }
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err(SourceError::Empty);
    }
    let eight = matches!(
        img.color(),
        image::ColorType::L8
            | image::ColorType::La8
            | image::ColorType::Rgb8
            | image::ColorType::Rgba8
    );
    let pixels = if eight {
        Pixels::U8(img.into_rgba8().into_raw())
    } else {
        Pixels::F32(img.into_rgba32f().into_raw())
    };
    Ok(Raster {
        width,
        height,
        pixels,
    })
}
