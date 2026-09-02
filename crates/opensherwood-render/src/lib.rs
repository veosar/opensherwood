//! Deterministic CPU compositor (ADR-0002). The framebuffer is the authoritative picture; presenters
//! only display it.
//!
//! Every public drawing function is total over its inputs: positions are `i32` and sizes `u32`
//! at the extremes of their types, and the arithmetic that combines them is done in `i64` (or
//! checked) so debug and release builds agree and nothing indexes out of bounds. A source buffer
//! shorter than its declared size is ignored, never read past; a framebuffer whose `rgba` is
//! shorter than `width * height * 4` (its fields are public and may be built by hand) takes a
//! single `put` only where its bytes exist, skips every area primitive and blit (their work is
//! then bounded by the bytes that exist, not by hostile dimensions) and encodes to an error,
//! never panics.

pub mod text;

use std::sync::Arc;

use opensherwood_core::{EntityKind, Fixed, World};

/// RGBA8 framebuffer, row-major, no padding. The fields are public for the presenters and the
/// UI; every write is bounds-checked against `rgba.len()`: `put` writes only a pixel that
/// exists, the area primitives and blits check [`Framebuffer::is_consistent`] first and skip a
/// short buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Pixels (`width * height * 4` bytes when consistent).
    pub rgba: Vec<u8>,
}

/// An RGBA colour.
pub type Color = [u8; 4];

/// A decoded background picture in map pixels (RGBA8) plus its foreground occluder masks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Background {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Pixels.
    pub rgba: Vec<u8>,
    /// Parts of the background drawn in front of sprites standing behind them.
    pub occluders: Vec<Occluder>,
}

/// A foreground mask (`docs/formats/rhp.md`, `FACE`): a 1-bit bitmap at a map position and a
/// depth line; a sprite whose feet are above the line (smaller y) is behind the mask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occluder {
    /// Map position of the mask.
    pub x: i32,
    /// Map position.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Rows of `ceil(width / 8)` bytes, MSB first, 1 = mask pixel.
    pub bits: Vec<u8>,
    /// Depth line in map pixels (two points); `None` = the mask's bottom edge.
    pub line: Option<((i32, i32), (i32, i32))>,
}

impl Occluder {
    fn stride(&self) -> usize {
        (self.width as usize).div_ceil(8)
    }

    /// Whether the mask covers map pixel `(mx, my)`. Total over any position and any mask
    /// geometry: a pixel outside the mask, or a mask row `bits` does not hold, is not covered.
    #[must_use]
    pub fn covers(&self, mx: i32, my: i32) -> bool {
        let (lx, ly) = (
            i64::from(mx) - i64::from(self.x),
            i64::from(my) - i64::from(self.y),
        );
        if lx < 0 || ly < 0 || lx >= i64::from(self.width) || ly >= i64::from(self.height) {
            return false;
        }
        // Both are below `u32::MAX` here, so they fit `usize` on every target.
        let (lx, ly) = (lx as usize, ly as usize);
        let idx = ly
            .checked_mul(self.stride())
            .and_then(|row| row.checked_add(lx >> 3));
        idx.and_then(|i| self.bits.get(i))
            .is_some_and(|b| b & (0x80 >> (lx & 7)) != 0)
    }

    /// y of the depth line at map x (clamped to the segment), or the mask bottom. The result is
    /// clamped to `i32` (a mask bottom beyond `i32::MAX` is as far down as anything can be).
    #[must_use]
    pub fn depth_y(&self, mx: i32) -> i32 {
        match self.line {
            Some(((x1, y1), (x2, y2))) => {
                if x1 == x2 {
                    y1.max(y2)
                } else {
                    let (x1, y1, x2, y2) =
                        (i64::from(x1), i64::from(y1), i64::from(x2), i64::from(y2));
                    let dx = x2 - x1;
                    let t = (i64::from(mx) - x1).clamp(0.min(dx), 0.max(dx));
                    // |t| and |y2 - y1| are below 2^32: their product needs `i128`.
                    let y = i128::from(y1) + i128::from(t) * i128::from(y2 - y1) / i128::from(dx);
                    // The interpolation stays between y1 and y2, both `i32`.
                    y.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
                }
            }
            None => to_i32(i64::from(self.y) + i64::from(self.height)),
        }
    }
}

/// Intersection of two ranges (empty when they do not overlap).
fn intersect(a: std::ops::Range<i64>, b: std::ops::Range<i64>) -> std::ops::Range<i64> {
    let start = a.start.max(b.start);
    start..a.end.min(b.end).max(start)
}

/// Clamp an `i64` coordinate into `i32` (positions past the `i32` range are off every buffer).
fn to_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Bytes an RGBA8 buffer of `w x h` pixels needs, `None` when the product overflows `usize`.
fn rgba_len(w: u32, h: u32) -> Option<usize> {
    (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(4))
}

/// Source pixel range `0..n` (as `i64`) that lands inside `0..extent` when drawn at `pos`.
fn visible_span(pos: i64, n: u32, extent: u32) -> std::ops::Range<i64> {
    let start = (-pos).max(0);
    let end = i64::from(n).min(i64::from(extent) - pos);
    start..end.max(start)
}

impl Framebuffer {
    /// Largest framebuffer dimension (a 4096x4096 RGBA buffer is 64 MiB).
    pub const MAX_DIMENSION: u32 = 4096;

    /// Allocate a black, opaque buffer (dimensions are clamped to `1..=MAX_DIMENSION`).
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let width = width.clamp(1, Self::MAX_DIMENSION);
        let height = height.clamp(1, Self::MAX_DIMENSION);
        let mut rgba = vec![0; width as usize * height as usize * 4];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Self {
            width,
            height,
            rgba,
        }
    }

    /// Whether `rgba` holds at least the `width * height * 4` bytes the dimensions declare.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        rgba_len(self.width, self.height).is_some_and(|n| self.rgba.len() >= n)
    }

    /// The pixel bytes (the declared area; empty when the buffer is inconsistent).
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        match rgba_len(self.width, self.height) {
            Some(n) if self.rgba.len() >= n => &self.rgba[..n],
            _ => &[],
        }
    }

    /// The pixel bytes for in-place edits (tints, fades); empty when the buffer is inconsistent.
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        match rgba_len(self.width, self.height) {
            Some(n) if self.rgba.len() >= n => &mut self.rgba[..n],
            _ => &mut [],
        }
    }

    /// Fill with a colour.
    pub fn clear(&mut self, c: Color) {
        for px in self.rgba.chunks_exact_mut(4) {
            px.copy_from_slice(&c);
        }
    }

    /// Set one pixel (ignored outside the buffer or when that pixel's bytes do not exist).
    pub fn put(&mut self, x: i32, y: i32, c: Color) {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return;
        };
        if x >= self.width || y >= self.height {
            return;
        }
        let i = (u128::from(y) * u128::from(self.width) + u128::from(x)) * 4;
        if let Ok(i) = usize::try_from(i)
            && let Some(px) = self.rgba.get_mut(i..i + 4)
        {
            px.copy_from_slice(&c);
        }
    }

    /// Axis-aligned filled rectangle (inclusive of `x0,y0`, exclusive of `x1,y1`).
    pub fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        if !self.is_consistent() {
            return;
        }
        let clamp = |v: i32, extent: u32| to_i32(i64::from(v).clamp(0, i64::from(extent)));
        let (x0, x1) = (clamp(x0, self.width), clamp(x1, self.width));
        let (y0, y1) = (clamp(y0, self.height), clamp(y1, self.height));
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x, y, c);
            }
        }
    }

    /// Filled disc (radius clamped to the buffer size; off-screen parts are skipped).
    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        if !self.is_consistent() {
            return;
        }
        let r = r.clamp(0, Self::MAX_DIMENSION as i32);
        let rr = i64::from(r) * i64::from(r);
        for y in Self::clip_range(cy, r, self.height) {
            for x in Self::clip_range(cx, r, self.width) {
                let (dx, dy) = (i64::from(x) - i64::from(cx), i64::from(y) - i64::from(cy));
                if dx * dx + dy * dy <= rr {
                    self.put(x, y, c);
                }
            }
        }
    }

    /// One-pixel circle outline.
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        if !self.is_consistent() {
            return;
        }
        let r = r.clamp(0, Self::MAX_DIMENSION as i32);
        let rr = i64::from(r) * i64::from(r);
        let inner = i64::from(r - 1) * i64::from(r - 1);
        for y in Self::clip_range(cy, r, self.height) {
            for x in Self::clip_range(cx, r, self.width) {
                let (dx, dy) = (i64::from(x) - i64::from(cx), i64::from(y) - i64::from(cy));
                let d = dx * dx + dy * dy;
                if d <= rr && d > inner {
                    self.put(x, y, c);
                }
            }
        }
    }

    /// Pixel range `centre - r ..= centre + r` clipped to `0..extent` (empty when `extent` is 0).
    fn clip_range(centre: i32, r: i32, extent: u32) -> std::ops::RangeInclusive<i32> {
        let lo = centre.saturating_sub(r).max(0);
        let hi = to_i32((i64::from(centre) + i64::from(r)).min(i64::from(extent) - 1));
        lo..=hi
    }

    /// Bresenham line, clipped to the buffer first (Liang-Barsky) so huge coordinates cost nothing.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        if !self.is_consistent() {
            return;
        }
        let Some((x0, y0, x1, y1)) = self.clip_line(x0, y0, x1, y1) else {
            return;
        };
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.put(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Clip a segment to the buffer rectangle; `None` when nothing is visible.
    fn clip_line(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> Option<(i32, i32, i32, i32)> {
        let (xmin, ymin) = (0i64, 0i64);
        let (xmax, ymax) = (i64::from(self.width) - 1, i64::from(self.height) - 1);
        let (x0, y0, x1, y1) = (i64::from(x0), i64::from(y0), i64::from(x1), i64::from(y1));
        let (dx, dy) = (x1 - x0, y1 - y0);
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for (p, q) in [
            (-dx, x0 - xmin),
            (dx, xmax - x0),
            (-dy, y0 - ymin),
            (dy, ymax - y0),
        ] {
            if p == 0 {
                if q < 0 {
                    return None;
                }
                continue;
            }
            let r = q as f64 / p as f64;
            if p < 0 {
                if r > t1 {
                    return None;
                }
                t0 = t0.max(r);
            } else {
                if r < t0 {
                    return None;
                }
                t1 = t1.min(r);
            }
        }
        if t0 > t1 {
            return None;
        }
        let px = |t: f64, a: i64, d: i64| (a as f64 + t * d as f64).round() as i32;
        Some((
            px(t0, x0, dx),
            px(t0, y0, dy),
            px(t1, x0, dx),
            px(t1, y0, dy),
        ))
    }

    /// Copy an opaque RGBA region of `src` (source rectangle at `sx,sy`) to `dx,dy`, clipped.
    /// Ignored when `src` is shorter than `src_w * src_h * 4` bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn blit_region(
        &mut self,
        src: &[u8],
        src_w: u32,
        src_h: u32,
        sx: i32,
        sy: i32,
        dx: i32,
        dy: i32,
        w: u32,
        h: u32,
    ) {
        if rgba_len(src_w, src_h).is_none_or(|n| src.len() < n) || !self.is_consistent() {
            return;
        }
        let (sx, sy, dx, dy) = (i64::from(sx), i64::from(sy), i64::from(dx), i64::from(dy));
        // Rows and columns of the `w x h` rectangle that lie inside both the source and the buffer.
        let rows = intersect(visible_span(sy, h, src_h), visible_span(dy, h, self.height));
        let cols = intersect(visible_span(sx, w, src_w), visible_span(dx, w, self.width));
        if rows.is_empty() || cols.is_empty() {
            return;
        }
        let n = ((cols.end - cols.start) * 4) as usize;
        let (src_w, fb_w) = (i64::from(src_w), i64::from(self.width));
        for row in rows {
            // In range on both sides, so the products are below the checked buffer lengths.
            let si = (((sy + row) * src_w + sx + cols.start) * 4) as usize;
            let di = (((dy + row) * fb_w + dx + cols.start) * 4) as usize;
            self.rgba[di..di + n].copy_from_slice(&src[si..si + n]);
        }
    }

    /// Like `blit_rgba`, but a destination pixel is written only when `visible(x, y)` holds for its
    /// screen position (used to hide the parts of a character standing behind an occluder).
    pub fn blit_rgba_masked(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        rgba: &[u8],
        visible: impl Fn(i32, i32) -> bool,
    ) {
        self.blit_rgba_with(x, y, w, h, rgba, Some(&visible));
    }

    /// Blit an RGBA image: alpha 0 skipped, alpha 255 copied, other alphas blended (integer math).
    /// Ignored when `rgba` is shorter than `w * h * 4`.
    pub fn blit_rgba(&mut self, x: i32, y: i32, w: u32, h: u32, rgba: &[u8]) {
        self.blit_rgba_with(x, y, w, h, rgba, None);
    }

    fn blit_rgba_with(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        rgba: &[u8],
        visible: Option<&dyn Fn(i32, i32) -> bool>,
    ) {
        if rgba_len(w, h).is_none_or(|n| rgba.len() < n) || !self.is_consistent() {
            return;
        }
        let (x, y) = (i64::from(x), i64::from(y));
        let rows = visible_span(y, h, self.height);
        let cols = visible_span(x, w, self.width);
        let (w, fb_w) = (i64::from(w), i64::from(self.width));
        for sy in rows {
            let dy = y + sy;
            for sx in cols.clone() {
                let dx = x + sx;
                let si = ((sy * w + sx) * 4) as usize;
                let a = u32::from(rgba[si + 3]);
                if a == 0 {
                    continue;
                }
                // `dx` and `dy` lie inside the buffer, so they fit `i32`.
                if visible.is_some_and(|v| !v(dx as i32, dy as i32)) {
                    continue;
                }
                let di = ((dy * fb_w + dx) * 4) as usize;
                if a == 255 {
                    self.rgba[di..di + 3].copy_from_slice(&rgba[si..si + 3]);
                } else {
                    for c in 0..3 {
                        let s = u32::from(rgba[si + c]);
                        let d = u32::from(self.rgba[di + c]);
                        self.rgba[di + c] = ((s * a + d * (255 - a)) / 255) as u8;
                    }
                }
                self.rgba[di + 3] = 255;
            }
        }
    }

    /// BLAKE3 of dimensions and pixels.
    #[must_use]
    pub fn hash(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(&self.width.to_le_bytes());
        h.update(&self.height.to_le_bytes());
        h.update(&self.rgba);
        h.finalize().to_hex().to_string()
    }

    /// Encode as PNG (an inconsistent buffer is an encoding error, never a panic).
    pub fn encode_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width, self.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header()?;
            w.write_image_data(&self.rgba)?;
        }
        Ok(out)
    }
}

pub use text::FontAtlas;

/// A decoded sprite frame (RGBA8, alpha 0 = transparent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteFrame {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Pixels.
    pub rgba: Vec<u8>,
}

/// Supplies decoded sprite frames by bank index (the app implements it over the sprite bank).
pub trait SpriteSource {
    /// Frame by index, or `None` if it cannot be provided.
    fn frame(&mut self, index: u32) -> Option<Arc<SpriteFrame>>;
}

/// A source with no sprites (synthetic scenarios).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoSprites;

impl SpriteSource for NoSprites {
    fn frame(&mut self, _index: u32) -> Option<Arc<SpriteFrame>> {
        None
    }
}

/// Something that shows a framebuffer (a window, a test sink).
pub trait Presenter {
    /// Present a frame.
    fn present(&mut self, frame: &Framebuffer);
}

/// Palette for the synthetic scenario.
pub mod palette {
    use super::Color;
    /// Ground.
    pub const GROUND: Color = [34, 68, 34, 255];
    /// Obstacle.
    pub const OBSTACLE: Color = [90, 90, 90, 255];
    /// Player.
    pub const PLAYER: Color = [40, 200, 60, 255];
    /// Guard.
    pub const GUARD: Color = [200, 60, 40, 255];
    /// Selection ring.
    pub const SELECTION: Color = [255, 255, 255, 255];
    /// Goal.
    pub const GOAL: Color = [230, 200, 40, 255];
    /// Target marker.
    pub const TARGET: Color = [120, 200, 255, 255];
    /// Pointer.
    pub const POINTER: Color = [255, 255, 0, 255];
    /// Outside the map.
    pub const VOID: Color = [0, 0, 0, 255];
}

/// Render a world into a new framebuffer at its logical viewport size, with an optional background
/// and sprites from `sprites` for entities that carry animation state.
#[must_use]
pub fn render(
    world: &World,
    background: Option<&Background>,
    sprites: &mut dyn SpriteSource,
) -> Framebuffer {
    let mut fb = Framebuffer::new(world.viewport.0, world.viewport.1);
    let (cx, cy) = world.camera;
    match background {
        Some(bg) => {
            fb.clear(palette::VOID);
            fb.blit_region(
                &bg.rgba, bg.width, bg.height, cx, cy, 0, 0, fb.width, fb.height,
            );
        }
        None => fb.clear(palette::GROUND),
    }
    // Viewport position of a map coordinate; the difference is formed in `i64` and clamped, so a
    // hostile camera or position lands off-screen instead of overflowing.
    let px = |f: Fixed, c: i32| to_i32(i64::from(f.round()) - i64::from(c));
    fb.circle(
        px(world.goal.0, cx),
        px(world.goal.1, cy),
        16,
        palette::GOAL,
    );
    // Ground markers first (target lines, selection circles), then sprites in depth order.
    for e in world.entities.iter().filter(|e| e.alive && e.active) {
        if e.kind == EntityKind::Obstacle {
            let (hw, hh) = (e.patrol[0].0, e.patrol[0].1);
            fb.fill_rect(
                px(e.x - hw, cx),
                px(e.y - hh, cy),
                px(e.x + hw, cx),
                px(e.y + hh, cy),
                palette::OBSTACLE,
            );
            continue;
        }
        if let Some((tx, ty)) = e.target {
            fb.line(
                px(e.x, cx),
                px(e.y, cy),
                px(tx, cx),
                px(ty, cy),
                palette::TARGET,
            );
        }
        if world.selected == Some(e.id) {
            fb.circle(
                px(e.x, cx),
                px(e.y, cy),
                e.size.round().saturating_add(3),
                palette::SELECTION,
            );
        }
    }
    // Painter's order: characters lower on the map (larger feet y) are nearer the viewer and drawn
    // last; ties are broken by entity index so the order is deterministic.
    let mut order: Vec<(i32, usize)> = world
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive && e.active && e.kind != EntityKind::Obstacle)
        .map(|(i, e)| (e.y.round(), i))
        .collect();
    order.sort_unstable();
    for (_, i) in order {
        let e = &world.entities[i];
        let c = if e.kind == EntityKind::Player {
            palette::PLAYER
        } else {
            palette::GUARD
        };
        let sprite = e
            .anim
            .as_ref()
            .and_then(|a| a.current(&world.catalog))
            .and_then(|spec| sprites.frame(spec.frame).map(|f| (spec, f)));
        let Some((spec, frame)) = sprite else {
            fb.fill_circle(px(e.x, cx), px(e.y, cy), e.size.round(), c);
            continue;
        };
        let x = to_i32(i64::from(px(e.x, cx)) + i64::from(spec.offset_x));
        let y = to_i32(i64::from(px(e.y, cy)) + i64::from(spec.offset_y));
        let (fx, fy) = (e.x.round(), e.y.round());
        // Occluders the character stands behind and whose mask overlaps the sprite rectangle
        // (rectangle arithmetic in `i64`: sizes are `u32`, positions any `i32`).
        let (x0, y0) = (i64::from(x), i64::from(y));
        let (x1, y1) = (x0 + i64::from(frame.width), y0 + i64::from(frame.height));
        let behind: Vec<&Occluder> = background
            .map(|bg| {
                bg.occluders
                    .iter()
                    .filter(|o| fy < o.depth_y(fx))
                    .filter(|o| {
                        let (ox0, oy0) = (
                            i64::from(o.x) - i64::from(cx),
                            i64::from(o.y) - i64::from(cy),
                        );
                        let (ox1, oy1) = (ox0 + i64::from(o.width), oy0 + i64::from(o.height));
                        x0 < ox1 && x1 > ox0 && y0 < oy1 && y1 > oy0
                    })
                    .collect()
            })
            .unwrap_or_default();
        if behind.is_empty() {
            fb.blit_rgba(x, y, frame.width, frame.height, &frame.rgba);
        } else {
            fb.blit_rgba_masked(x, y, frame.width, frame.height, &frame.rgba, |sx, sy| {
                let (mx, my) = (
                    to_i32(i64::from(sx) + i64::from(cx)),
                    to_i32(i64::from(sy) + i64::from(cy)),
                );
                !behind.iter().any(|o| o.covers(mx, my))
            });
        }
    }
    let (mx, my) = (
        Fixed::from_raw(world.pointer.0).round(),
        Fixed::from_raw(world.pointer.1).round(),
    );
    fb.line(
        mx.saturating_sub(4),
        my,
        mx.saturating_add(4),
        my,
        palette::POINTER,
    );
    fb.line(
        mx,
        my.saturating_sub(4),
        mx,
        my.saturating_add(4),
        palette::POINTER,
    );
    fb
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensherwood_core::Scenario;

    #[test]
    fn rendering_is_deterministic_and_png_encodes() {
        let w = World::new(Scenario::Synthetic("corridor".into()), 1).unwrap();
        let a = render(&w, None, &mut NoSprites);
        let b = render(&w, None, &mut NoSprites);
        assert_eq!(a.hash(), b.hash());
        assert_eq!(a.rgba.len(), 640 * 480 * 4);
        let png = a.encode_png().unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn overlapping_actors_compose_in_depth_order_with_occluders() {
        use opensherwood_core::{AnimSet, AnimState, FrameSpec};
        // Two 8x8 sprites: a red one whose feet are at y = 20 (behind the occluder line at y = 30)
        // and a blue one at y = 40 (in front). Their rectangles overlap the occluder's mask.
        struct Src;
        impl SpriteSource for Src {
            fn frame(&mut self, index: u32) -> Option<Arc<SpriteFrame>> {
                let c = if index == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
                Some(Arc::new(SpriteFrame {
                    width: 8,
                    height: 8,
                    rgba: c.repeat(64),
                }))
            }
        }
        let mut w = World::new(Scenario::Synthetic("corridor".into()), 1).unwrap();
        w.entities.truncate(2);
        let mut set = |i: usize, frame: u32, x: i32, y: i32| {
            let e = &mut w.entities[i];
            e.kind = EntityKind::Guard;
            e.x = Fixed::from_int(x);
            e.y = Fixed::from_int(y);
            e.target = None;
            e.anim = Some(AnimState::new(format!("s{i}"), 0));
            let mut anim = AnimSet::default();
            anim.animations.push(vec![FrameSpec {
                frame,
                duration: 1,
                offset_x: -4,
                offset_y: -8,
            }]);
            w.catalog.sets.insert(format!("s{i}"), anim);
        };
        set(0, 1, 20, 40); // blue, in front, listed first
        set(1, 0, 20, 20); // red, behind
        w.selected = None;
        let mut bg = Background {
            width: 64,
            height: 64,
            rgba: [0, 255, 0, 255].repeat(64 * 64),
            occluders: vec![Occluder {
                x: 10,
                y: 10,
                width: 20,
                height: 30,
                bits: vec![0xFF; 3 * 30],
                line: Some(((10, 30), (30, 30))),
            }],
        };
        bg.occluders[0].bits.truncate(3 * 30);
        let fb = render(&w, Some(&bg), &mut Src);
        let px = |x: i32, y: i32| {
            let i = ((y as u32 * fb.width + x as u32) * 4) as usize;
            [fb.rgba[i], fb.rgba[i + 1], fb.rgba[i + 2]]
        };
        // Red sprite covers (16..24, 12..20): fully inside the mask -> background shows.
        assert_eq!(px(18, 15), [0, 255, 0]);
        // Blue sprite covers (16..24, 32..40): overlaps the mask rows 32..40 but stands in front.
        assert_eq!(px(18, 35), [0, 0, 255]);
        // Depth order: move red (listed second) below blue so their rectangles overlap; red now has
        // the larger feet y and must be drawn on top, whatever the slot order.
        w.entities[1].y = Fixed::from_int(44);
        w.entities[1].x = Fixed::from_int(50);
        w.entities[0].x = Fixed::from_int(50);
        let fb2 = render(&w, Some(&bg), &mut Src);
        let px2 = |x: i32, y: i32| {
            let i = ((y as u32 * fb2.width + x as u32) * 4) as usize;
            [fb2.rgba[i], fb2.rgba[i + 1], fb2.rgba[i + 2]]
        };
        assert_eq!(px2(50, 38), [255, 0, 0]); // overlap rows 36..40: red on top
        assert_eq!(px2(50, 33), [0, 0, 255]); // only blue
        // Swapping the feet order swaps the winner.
        w.entities[1].y = Fixed::from_int(36);
        let fb3 = render(&w, Some(&bg), &mut Src);
        let i = ((34u32 * fb3.width + 50) * 4) as usize;
        assert_eq!(&fb3.rgba[i..i + 3], &[0, 0, 255]);
    }

    #[test]
    fn drawing_outside_is_ignored() {
        let mut fb = Framebuffer::new(4, 4);
        fb.put(-1, 0, [1, 1, 1, 255]);
        fb.put(4, 4, [1, 1, 1, 255]);
        fb.fill_rect(-10, -10, 100, 100, [7, 7, 7, 255]);
        assert!(fb.rgba.chunks_exact(4).all(|p| p == [7, 7, 7, 255]));
    }

    #[test]
    fn occluder_math_is_total_at_extremes() {
        let ext = [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX];
        let sizes = [0u32, 1, 7, 8, 9, u32::MAX - 1, u32::MAX];
        for &x in &ext {
            for &y in &ext {
                for &w in &sizes {
                    for &h in &sizes {
                        let o = Occluder {
                            x,
                            y,
                            width: w,
                            height: h,
                            bits: vec![0xFF; 16],
                            line: Some(((x, y), (y, x))),
                        };
                        for &mx in &ext {
                            for &my in &ext {
                                let _ = o.covers(mx, my);
                            }
                            let _ = o.depth_y(mx);
                        }
                        let bottom = Occluder { line: None, ..o };
                        for &mx in &ext {
                            let d = bottom.depth_y(mx);
                            assert_eq!(
                                i64::from(d),
                                (i64::from(y) + i64::from(h))
                                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                            );
                        }
                    }
                }
            }
        }
        // Coverage reads only the bytes the mask has: a huge declared size with a short `bits`.
        let sparse = Occluder {
            x: 0,
            y: 0,
            width: u32::MAX,
            height: u32::MAX,
            bits: vec![0x80],
            line: None,
        };
        assert!(sparse.covers(0, 0));
        assert!(!sparse.covers(1, 0));
        assert!(!sparse.covers(i32::MAX, i32::MAX));
        // The depth line is interpolated exactly between its ends, clamped beyond them.
        let line = Occluder {
            line: Some(((i32::MIN, i32::MIN), (i32::MAX, i32::MAX))),
            ..sparse
        };
        assert_eq!(line.depth_y(i32::MIN), i32::MIN);
        assert_eq!(line.depth_y(i32::MAX), i32::MAX);
        assert_eq!(line.depth_y(0), 0);
        let vertical = Occluder {
            line: Some(((5, i32::MAX), (5, i32::MIN))),
            ..line
        };
        assert_eq!(vertical.depth_y(i32::MIN), i32::MAX);
    }

    #[test]
    fn blits_are_total_at_extremes() {
        let ext = [
            i32::MIN,
            i32::MIN + 1,
            -5,
            -1,
            0,
            1,
            3,
            i32::MAX - 1,
            i32::MAX,
        ];
        let sizes = [0u32, 1, 4, 5, u32::MAX - 1, u32::MAX];
        let src: Vec<u8> = [9, 8, 7, 255].repeat(16);
        let mut fb = Framebuffer::new(4, 4);
        for &x in &ext {
            for &y in &ext {
                for &w in &sizes {
                    for &h in &sizes {
                        // Oversized declarations are ignored (the buffer is too short), small
                        // ones are clipped; nothing panics.
                        fb.blit_rgba(x, y, w, h, &src);
                        fb.blit_rgba_masked(x, y, w, h, &src, |_, _| true);
                        fb.blit_region(&src, w, h, x, y, x, y, w, h);
                        fb.blit_region(&src, 4, 4, x, y, x, y, w, h);
                        fb.blit_region(&src, 4, 4, x, y, 0, 0, w, h);
                        fb.blit_region(&src, w, h, 0, 0, x, y, 4, 4);
                        fb.fill_circle(x, y, 2, [1, 1, 1, 255]);
                        fb.circle(x, y, i32::MAX, [1, 1, 1, 255]);
                        fb.fill_rect(x, y, y, x, [1, 1, 1, 255]);
                        fb.line(x, y, y, x, [1, 1, 1, 255]);
                        fb.put(x, y, [1, 1, 1, 255]);
                    }
                }
            }
        }
        // A blit that lands lands exactly, whatever the extreme it was offset from.
        let mut fb = Framebuffer::new(4, 4);
        fb.blit_rgba(3, 3, 4, 4, &src);
        assert_eq!(
            &fb.rgba[(3 * 4 + 3) * 4..(3 * 4 + 3) * 4 + 4],
            &[9, 8, 7, 255]
        );
        assert_eq!(&fb.rgba[0..4], &[0, 0, 0, 255]);
        fb.blit_rgba(-3, -3, 4, 4, &src);
        assert_eq!(&fb.rgba[0..4], &[9, 8, 7, 255]);
        assert_eq!(&fb.rgba[4..8], &[0, 0, 0, 255]);
        // A masked blit sees buffer positions, never source ones.
        let mut seen = std::cell::RefCell::new(Vec::new());
        fb.blit_rgba_masked(-3, -3, 4, 4, &src, |x, y| {
            seen.borrow_mut().push((x, y));
            true
        });
        assert_eq!(seen.get_mut().as_slice(), &[(0, 0)]);
        // A mask with no room in `bits` never covers; alpha 0 pixels are skipped without asking.
        let clear = [0u8; 64];
        fb.clear([2, 2, 2, 255]);
        fb.blit_rgba_masked(0, 0, 4, 4, &clear, |_, _| {
            panic!("asked for a transparent pixel")
        });
        assert!(fb.rgba.chunks_exact(4).all(|p| p == [2, 2, 2, 255]));
    }

    #[test]
    fn short_buffers_draw_nothing_and_never_panic() {
        let src: Vec<u8> = [9, 8, 7, 255].repeat(16);
        let mut short = Framebuffer {
            width: 4,
            height: 4,
            rgba: vec![0; 4 * 4 * 4 - 1],
        };
        assert!(!short.is_consistent());
        assert!(short.pixels().is_empty() && short.pixels_mut().is_empty());
        // `put` writes a pixel that exists and skips the missing last one.
        short.put(0, 0, [1, 1, 1, 255]);
        short.put(3, 3, [1, 1, 1, 255]);
        assert_eq!(&short.rgba[..4], &[1, 1, 1, 255]);
        assert_eq!(&short.rgba[60..], &[0, 0, 0]);
        // Area primitives and blits need the whole declared area: a short buffer is left alone.
        let before = short.rgba.clone();
        short.fill_rect(0, 0, 4, 4, [2, 2, 2, 255]);
        short.fill_circle(2, 2, 3, [2, 2, 2, 255]);
        short.circle(2, 2, 3, [2, 2, 2, 255]);
        short.line(0, 0, 3, 3, [2, 2, 2, 255]);
        short.blit_rgba(0, 0, 4, 4, &src);
        short.blit_rgba_masked(0, 0, 4, 4, &src, |_, _| true);
        short.blit_region(&src, 4, 4, 0, 0, 0, 0, 4, 4);
        assert_eq!(short.rgba, before);
        assert!(short.encode_png().is_err());
        let _ = short.hash();
        // `clear` and the accessors work on the bytes that exist.
        short.clear([2, 2, 2, 255]);
        assert!(
            short.rgba[..60]
                .chunks_exact(4)
                .all(|p| p == [2, 2, 2, 255])
        );
        let mut empty = Framebuffer {
            width: u32::MAX,
            height: u32::MAX,
            rgba: Vec::new(),
        };
        empty.put(0, 0, [1, 1, 1, 255]);
        empty.put(i32::MAX, i32::MAX, [1, 1, 1, 255]);
        empty.blit_rgba(0, 0, 4, 4, &src);
        empty.fill_rect(i32::MIN, i32::MIN, i32::MAX, i32::MAX, [1, 1, 1, 255]);
        empty.fill_circle(0, 0, i32::MAX, [1, 1, 1, 255]);
        empty.circle(0, 0, i32::MAX, [1, 1, 1, 255]);
        empty.line(i32::MIN, i32::MIN, i32::MAX, i32::MAX, [1, 1, 1, 255]);
        assert!(empty.rgba.is_empty());
        // Dimensions beyond `i32` with a real (tiny) buffer: nothing is touched, nothing loops.
        let mut wide = Framebuffer {
            width: u32::MAX,
            height: 1,
            rgba: vec![0; 16],
        };
        wide.fill_rect(0, 0, 4, 1, [1, 1, 1, 255]);
        wide.line(0, 0, 3, 0, [1, 1, 1, 255]);
        wide.put(1, 0, [1, 1, 1, 255]);
        assert_eq!(&wide.rgba[4..8], &[1, 1, 1, 255]);
        assert_eq!(&wide.rgba[..4], &[0, 0, 0, 0]);
        // A consistent buffer exposes exactly its area.
        let mut fb = Framebuffer::new(4, 4);
        assert_eq!(fb.pixels().len(), 64);
        for px in fb.pixels_mut().chunks_exact_mut(4) {
            px[1] = 200;
        }
        assert!(fb.rgba.chunks_exact(4).all(|p| p[1] == 200));
    }

    #[test]
    fn blit_region_clips_on_every_side() {
        let src: Vec<u8> = (0..(3 * 3)).flat_map(|i| [i as u8, 0, 0, 255]).collect();
        let mut fb = Framebuffer::new(4, 4);
        fb.blit_region(&src, 3, 3, -1, -1, 2, 2, 4, 4);
        // Source pixel (0,0) lands at (3,3); (2,2) would land at (5,5): clipped.
        assert_eq!(
            &fb.rgba[(3 * 4 + 3) * 4..(3 * 4 + 3) * 4 + 4],
            &[0, 0, 0, 255]
        );
        fb.blit_region(&src, 3, 3, 1, 1, 0, 0, 4, 4);
        assert_eq!(&fb.rgba[0..4], &[4, 0, 0, 255]);
        assert_eq!(&fb.rgba[5 * 4..5 * 4 + 4], &[8, 0, 0, 255]);
    }
}
