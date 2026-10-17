//! WordCraft metafiles: reads Windows Metafiles (WMF) and Enhanced Metafiles (EMF) into a neutral
//! vector model of filled and stroked paths and RGBA bitmaps, in a picture frame with y down.
//!
//! The reader is pure Rust over byte slices (no file system), so it builds for wasm. Every read is
//! bounds-checked and every count is capped; hostile input yields a shorter picture or an [`Error`],
//! never a panic.
//!
//! Records follow MS-WMF and MS-EMF. Records outside the supported set are skipped silently.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod bytes;
mod canvas;
mod dib;
mod emf;
mod geom;
mod place;
mod wmf;

#[cfg(test)]
mod tests;

use bytes::Bytes;
use canvas::Raw;
use geom::{Op, P};
use kurbo::{Affine, BezPath};
pub use place::{MAX_BITMAP_SIDE, PlacedItem};

/// Items kept from one file. Past the cap the parse stops and keeps what it has.
pub(crate) const MAX_ITEMS: usize = 200_000;
/// Object table slots (WMF handles, EMF object indices).
pub(crate) const MAX_OBJECTS: usize = 4096;
/// Largest bitmap in pixels (about 40 megapixels, 160 MB of RGBA).
pub(crate) const MAX_PIXELS: usize = 40_000_000;
/// Logical fonts kept from one file.
pub(crate) const MAX_FONTS: usize = 4096;
/// DC save-stack depth.
pub(crate) const MAX_STACK: usize = 1024;
/// Points in one open path bracket.
pub(crate) const MAX_PATH_OPS: usize = 2_000_000;
/// Bytes of path ops and pixels kept from one file (about 256 MB).
pub(crate) const MAX_BYTES: usize = 256 * 1024 * 1024;

/// A parsed metafile. Items use coordinates in `0..width` and `0..height`, y down.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    /// Size of the drawing in its own units: points for placeable WMF and EMF (from the frame), device
    /// or logical units otherwise.
    pub width: f32,
    pub height: f32,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A filled and/or stroked path, in picture coordinates.
    Path {
        segs: Vec<Seg>,
        fill: Option<[u8; 4]>,
        /// Colour and width in picture units.
        stroke: Option<([u8; 4], f32)>,
        even_odd: bool,
    },
    /// A bitmap stretched into `rect` = `[x, y, w, h]` (picture units), RGBA8 rows top-down.
    Bitmap { rect: [f32; 4], width: u32, height: u32, rgba: Vec<u8> },
    /// Text drawn by the file. Renderers draw it after [`Picture::outline_text`] has turned it into a path.
    Text(TextItem),
}

/// A metafile text record in picture units: the reference point, the size (`cell`: a cell height,
/// em plus internal leading), the baseline angle (radians, counterclockwise on the page), the GDI
/// alignment flags and, when the file gives them, the advance of each character.
#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub cell: bool,
    pub angle: f32,
    pub color: [u8; 4],
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    pub align: u32,
    pub text: String,
    pub advances: Option<Vec<f32>>,
}

/// Text as outlines in ems (1.0 = the font size), y down, origin at the start of the baseline, with
/// its advance and the font's ascent and descent in ems: what a font library gives
/// [`Picture::outline_text`].
pub struct GlyphRun {
    pub path: BezPath,
    pub advance: f64,
    pub ascent: f64,
    pub descent: f64,
}

impl Picture {
    /// Turns text items into filled paths with `outline(family, bold, italic, text)`, placing them by
    /// their alignment, angle and advances. Text the outliner gives nothing for is dropped.
    pub fn outline_text(&mut self, outline: impl Fn(&str, bool, bool, &str) -> Option<GlyphRun>) {
        for item in &mut self.items {
            let Item::Text(t) = item else { continue };
            *item = text_path(t, &outline).unwrap_or(Item::Path { segs: Vec::new(), fill: None, stroke: None, even_odd: false });
        }
        self.items.retain(|it| !matches!(it, Item::Path { segs, .. } if segs.is_empty()));
    }
}

fn text_path(t: &TextItem, outline: &impl Fn(&str, bool, bool, &str) -> Option<GlyphRun>) -> Option<Item> {
    let size = f64::from(t.size);
    let whole = outline(&t.family, t.bold, t.italic, &t.text)?;
    // A cell height includes the internal leading: the em is the cell over ascent plus descent.
    let em = if t.cell { size / (whole.ascent + whole.descent).max(0.1) } else { size };
    let (path, advance) = match &t.advances {
        // The file's advances place each character where it was laid out, whatever font draws it.
        Some(adv) if adv.len() == t.text.chars().count() => {
            let mut p = BezPath::new();
            let mut x = 0.0;
            for (c, a) in t.text.chars().zip(adv) {
                let g = outline(&t.family, t.bold, t.italic, c.encode_utf8(&mut [0; 4]))?;
                p.extend((Affine::translate((x, 0.0)) * Affine::scale(em) * g.path).elements().iter().copied());
                x += f64::from(*a);
            }
            (p, x)
        }
        _ => (Affine::scale(em) * whole.path, whole.advance * em),
    };
    let dx = match t.align & canvas::TA_HORIZONTAL {
        6 => -advance / 2.0,
        2 => -advance,
        _ => 0.0,
    };
    let dy = match t.align & canvas::TA_VERTICAL {
        24 => 0.0,
        8 => -whole.descent * em,
        _ => whole.ascent * em,
    };
    let tf = Affine::translate((f64::from(t.x), f64::from(t.y))) * Affine::rotate(-f64::from(t.angle)) * Affine::translate((dx, dy));
    let segs = segs_of(&(tf * path));
    Some(Item::Path { segs, fill: Some(t.color), stroke: None, even_odd: false })
}

/// A kurbo path as segments; quadratic curves (TrueType outlines) become the equivalent cubics.
fn segs_of(path: &BezPath) -> Vec<Seg> {
    let pt = |p: kurbo::Point| Some((f(p.x)?, f(p.y)?));
    let mut out = Vec::new();
    let (mut start, mut cur) = (kurbo::Point::ZERO, kurbo::Point::ZERO);
    for e in path.elements() {
        let seg = match *e {
            kurbo::PathEl::MoveTo(a) => {
                (start, cur) = (a, a);
                pt(a).map(|(x, y)| Seg::Move(x, y))
            }
            kurbo::PathEl::LineTo(a) => {
                cur = a;
                pt(a).map(|(x, y)| Seg::Line(x, y))
            }
            kurbo::PathEl::QuadTo(q, b) => {
                let (c1, c2) = (cur + (q - cur) * (2.0 / 3.0), b + (q - b) * (2.0 / 3.0));
                cur = b;
                pt(c1).zip(pt(c2)).zip(pt(b)).map(|(((x1, y1), (x2, y2)), (x3, y3))| Seg::Cubic(x1, y1, x2, y2, x3, y3))
            }
            kurbo::PathEl::CurveTo(a, c, b) => {
                cur = b;
                pt(a).zip(pt(c)).zip(pt(b)).map(|(((x1, y1), (x2, y2)), (x3, y3))| Seg::Cubic(x1, y1, x2, y2, x3, y3))
            }
            kurbo::PathEl::ClosePath => {
                cur = start;
                Some(Seg::Close)
            }
        };
        out.extend(seg);
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a WMF or EMF file")]
    NotMetafile,
    #[error("metafile header is truncated or invalid")]
    BadHeader,
    #[error("metafile has no readable records")]
    NoRecords,
}

/// The picture frame in device units (`rect` = `[x, y, w, h]`) and, when the file states one, its
/// size in picture units.
pub(crate) struct Frame {
    pub rect: [f64; 4],
    pub size: Option<(f64, f64)>,
}

/// Parses a WMF (placeable or plain) or EMF file.
pub fn parse(bytes: &[u8]) -> Result<Picture, Error> {
    let b = Bytes(bytes);
    let (cv, frame) = if emf::is_emf(b) {
        emf::parse(b)?
    } else if wmf::is_wmf(b) {
        wmf::parse(b)?
    } else {
        return Err(Error::NotMetafile);
    };
    Ok(build(cv.items, frame))
}

/// True when the bytes start like a WMF (placeable or plain header) or an EMF.
pub fn is_metafile(bytes: &[u8]) -> bool {
    let b = Bytes(bytes);
    emf::is_emf(b) || wmf::is_wmf(b)
}

/// Maps device-space output into the picture frame. Without a usable frame the frame is the bounding
/// box of the drawing, so the origin is still the top-left of what was drawn.
fn build(raw: Vec<Raw>, frame: Option<Frame>) -> Picture {
    let frame = frame.filter(|f| f.rect.iter().all(|v| v.is_finite()) && f.rect[2] > 0.0 && f.rect[3] > 0.0);
    let (rect, size) = match frame {
        Some(f) => (f.rect, f.size.unwrap_or((f.rect[2], f.rect[3]))),
        None => match bbox(&raw) {
            Some([x0, y0, x1, y1]) => ([x0, y0, x1 - x0, y1 - y0], (x1 - x0, y1 - y0)),
            None => return Picture { width: 0.0, height: 0.0, items: Vec::new() },
        },
    };
    let sx = if rect[2] > 0.0 { size.0 / rect[2] } else { 1.0 };
    let sy = if rect[3] > 0.0 { size.1 / rect[3] } else { 1.0 };
    let tf = move |p: P| ((p.0 - rect[0]) * sx, (p.1 - rect[1]) * sy);
    let stroke_scale = (sx + sy) / 2.0;
    // No stroke is wider than the picture's diagonal: anything wider is a mapping artefact.
    let diag = size.0.hypot(size.1);
    let items = raw.into_iter().filter_map(|r| match r {
        Raw::Path { ops, fill, stroke, even_odd } => {
            let segs = ops.iter().map(|op| seg(op, tf)).collect::<Option<Vec<Seg>>>()?;
            if segs.len() < 2 {
                return None;
            }
            let stroke = stroke.and_then(|(c, w)| Some((c, f((w * stroke_scale).min(diag))?)));
            Some(Item::Path { segs, fill, stroke, even_odd })
        }
        Raw::Bitmap { rect: r, img } => {
            let (x, y) = tf((r[0], r[1]));
            let rect = [f(x)?, f(y)?, f(r[2] * sx)?, f(r[3] * sy)?];
            Some(Item::Bitmap { rect, width: img.w as u32, height: img.h as u32, rgba: img.px })
        }
        Raw::Text { at, size, cell, angle, color, font, align, text, dx } => {
            let (x, y) = tf(at);
            // Text stays upright whatever the y direction of the mapping; sizes and advances scale.
            let k = (sx.abs() + sy.abs()) / 2.0;
            let advances = dx.and_then(|d| d.iter().map(|v| f(v * sx.signum() * k)).collect::<Option<Vec<_>>>());
            Some(Item::Text(TextItem {
                x: f(x)?,
                y: f(y)?,
                size: f(size * k)?,
                cell,
                angle: f(angle)?,
                color,
                family: font.family,
                bold: font.bold,
                italic: font.italic,
                align,
                text,
                advances,
            }))
        }
    });
    let items = items.collect();
    Picture { width: f(size.0).unwrap_or(0.0), height: f(size.1).unwrap_or(0.0), items }
}

fn f(v: f64) -> Option<f32> {
    let v = v as f32;
    v.is_finite().then_some(v)
}

fn seg(op: &Op, tf: impl Fn(P) -> P) -> Option<Seg> {
    let pt = |p: P| {
        let (x, y) = tf(p);
        Some((f(x)?, f(y)?))
    };
    Some(match *op {
        Op::M(p) => {
            let (x, y) = pt(p)?;
            Seg::Move(x, y)
        }
        Op::L(p) => {
            let (x, y) = pt(p)?;
            Seg::Line(x, y)
        }
        Op::C(a, b, c) => {
            let (x1, y1) = pt(a)?;
            let (x2, y2) = pt(b)?;
            let (x3, y3) = pt(c)?;
            Seg::Cubic(x1, y1, x2, y2, x3, y3)
        }
        Op::Z => Seg::Close,
    })
}

/// Bounding box `[x0, y0, x1, y1]` over the finite points of the output.
fn bbox(raw: &[Raw]) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    let mut grow = |p: P| {
        if p.0.is_finite() && p.1.is_finite() {
            b = Some(match b {
                None => [p.0, p.1, p.0, p.1],
                Some([x0, y0, x1, y1]) => [x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1)],
            });
        }
    };
    for r in raw {
        match r {
            Raw::Path { ops, .. } => {
                for op in ops {
                    match *op {
                        Op::M(p) | Op::L(p) => grow(p),
                        Op::C(a, c, d) => {
                            grow(a);
                            grow(c);
                            grow(d);
                        }
                        Op::Z => {}
                    }
                }
            }
            Raw::Bitmap { rect, .. } => {
                grow((rect[0], rect[1]));
                grow((rect[0] + rect[2], rect[1] + rect[3]));
            }
            Raw::Text { at, .. } => grow(*at),
        }
    }
    b
}
