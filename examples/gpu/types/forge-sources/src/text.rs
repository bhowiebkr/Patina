//! Text: lines laid out with ab_glyph and drawn as coverage. The raster spans two canvases of
//! 512 points around the block's anchor, so a node can move the block anywhere on its image.

use std::path::Path;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};

use crate::{Pixels, Raster, SourceError, read};

/// Points across one canvas: the node's image.
pub const CANVAS_POINTS: f32 = 512.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub struct TextSpec<'a> {
    pub text: &'a str,
    /// A TrueType/OpenType file; `None` takes the built-in Ubuntu Light.
    pub font: Option<&'a Path>,
    pub size_pt: f32,
    /// Lines line up on the anchor's left, centre or right; the block is centred vertically.
    pub align: Align,
    /// Raster side in pixels; it covers `2 * CANVAS_POINTS`.
    pub side: u32,
}

fn load_font(path: Option<&Path>) -> Result<FontArc, SourceError> {
    match path {
        None => FontArc::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT)
            .map_err(|e| SourceError::Font(e.to_string())),
        Some(path) => FontArc::try_from_vec(read(path)?).map_err(|e| SourceError::Decode {
            path: path.to_owned(),
            msg: e.to_string(),
        }),
    }
}

/// Coverage in every channel, 8-bit, `side` square with the block's anchor at the centre.
pub fn text(spec: &TextSpec) -> Result<Raster, SourceError> {
    text_with_font(spec, None)
}

pub fn text_with_font(spec: &TextSpec, bytes: Option<&[u8]>) -> Result<Raster, SourceError> {
    let side = spec.side.max(1);
    let font = match bytes {
        Some(data) => {
            FontArc::try_from_vec(data.to_vec()).map_err(|e| SourceError::Font(e.to_string()))?
        }
        None => load_font(spec.font)?,
    };
    let px_per_pt = side as f32 / (2.0 * CANVAS_POINTS);
    // The size is the em in canvas points (ab_glyph's pt_to_px_scale would add a 96/72 DPI
    // factor); PxScale wants the ascent-to-descent height.
    let em = spec.size_pt.max(0.0) * px_per_pt;
    let scale = match font.units_per_em() {
        Some(units) => PxScale::from(em * font.height_unscaled() / units),
        None => PxScale::from(em),
    };
    let scaled = font.as_scaled(scale);
    let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();
    let lines: Vec<&str> = spec.text.lines().collect();
    let block = lines.len() as f32 * line_height - scaled.line_gap();
    let centre = side as f32 * 0.5;
    let mut coverage = vec![0.0f32; (side * side) as usize];
    for (row, line) in lines.iter().enumerate() {
        let mut width = 0.0;
        let mut prev = None;
        for c in line.chars() {
            let id = scaled.glyph_id(c);
            if let Some(p) = prev {
                width += scaled.kern(p, id);
            }
            width += scaled.h_advance(id);
            prev = Some(id);
        }
        let mut x = centre
            - match spec.align {
                Align::Left => 0.0,
                Align::Center => width * 0.5,
                Align::Right => width,
            };
        let baseline = centre - block * 0.5 + row as f32 * line_height + scaled.ascent();
        let mut prev = None;
        for c in line.chars() {
            let id = scaled.glyph_id(c);
            if let Some(p) = prev {
                x += scaled.kern(p, id);
            }
            prev = Some(id);
            let glyph = id.with_scale_and_position(scale, point(x, baseline));
            x += scaled.h_advance(id);
            let Some(outline) = font.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, c| {
                let px = bounds.min.x as i64 + i64::from(gx);
                let py = bounds.min.y as i64 + i64::from(gy);
                if (0..i64::from(side)).contains(&px) && (0..i64::from(side)).contains(&py) {
                    let i = (py as u32 * side + px as u32) as usize;
                    coverage[i] = (coverage[i] + c).min(1.0);
                }
            });
        }
    }
    let pixels = coverage
        .iter()
        .flat_map(|c| [(c * 255.0 + 0.5) as u8; 4])
        .collect();
    Ok(Raster {
        width: side,
        height: side,
        pixels: Pixels::U8(pixels),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn centred_text_covers_the_middle() {
        let spec = TextSpec {
            text: "Hi",
            font: None,
            size_pt: 64.0,
            align: Align::Center,
            side: 256,
        };
        let r = text(&spec).unwrap();
        let Pixels::U8(px) = r.pixels else { panic!() };
        let inked = |x0: u32, x1: u32| {
            (0..256u32)
                .flat_map(|y| (x0..x1).map(move |x| (y * 256 + x) as usize * 4))
                .filter(|&i| px[i] > 128)
                .count()
        };
        assert!(inked(112, 144) > 20);
        assert_eq!(inked(0, 64), 0);
    }
}
