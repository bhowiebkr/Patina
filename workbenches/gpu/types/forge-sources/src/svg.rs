//! SVG: drawn with resvg, its view box fitted into the square raster (aspect kept, centred).
//! Built without text or embedded raster support.

use std::path::Path;

use resvg::{tiny_skia, usvg};

use crate::{Pixels, Raster, SourceError, read};

/// Premultiplied 8-bit RGBA, `side` square.
pub fn svg(path: &Path, side: u32) -> Result<Raster, SourceError> {
    svg_bytes(&read(path)?, path, side)
}

pub fn svg_bytes(bytes: &[u8], path: &Path, side: u32) -> Result<Raster, SourceError> {
    let decode = |msg: String| SourceError::Decode {
        path: path.to_owned(),
        msg,
    };
    let side = side.max(1);
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())
        .map_err(|e| decode(e.to_string()))?;
    let size = tree.size();
    let scale = side as f32 / size.width().max(size.height());
    let tx = (side as f32 - size.width() * scale) * 0.5;
    let ty = (side as f32 - size.height() * scale) * 0.5;
    let mut pixmap = tiny_skia::Pixmap::new(side, side).ok_or(SourceError::Empty)?;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, tx, ty);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(Raster {
        width: side,
        height: side,
        pixels: Pixels::U8(pixmap.take()),
    })
}
