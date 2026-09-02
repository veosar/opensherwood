//! Deterministic CPU compositor (ADR-0002). The framebuffer is the authoritative picture; presenters
//! only display it.

pub mod text;

use std::sync::Arc;

use opensherwood_core::{EntityKind, Fixed, World};

/// RGBA8 framebuffer, row-major, no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Pixels.
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

    /// Whether the mask covers map pixel `(mx, my)`.
    #[must_use]
    pub fn covers(&self, mx: i32, my: i32) -> bool {
        let (lx, ly) = (mx - self.x, my - self.y);
        if lx < 0 || ly < 0 || lx >= self.width as i32 || ly >= self.height as i32 {
            return false;
        }
        let idx = ly as usize * self.stride() + (lx as usize >> 3);
        self.bits
            .get(idx)
            .is_some_and(|b| b & (0x80 >> (lx & 7)) != 0)
    }

    /// y of the depth line at map x (clamped to the segment), or the mask bottom.
    #[must_use]
    pub fn depth_y(&self, mx: i32) -> i32 {
        match self.line {
            Some(((x1, y1), (x2, y2))) => {
                if x1 == x2 {
                    y1.max(y2)
                } else {
                    let t = ((mx - x1).clamp(0.min(x2 - x1), 0.max(x2 - x1))) as i64;
                    (i64::from(y1) + t * i64::from(y2 - y1) / i64::from(x2 - x1)) as i32
                }
            }
            None => self.y + self.height as i32,
        }
    }
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

    /// Fill with a colour.
    pub fn clear(&mut self, c: Color) {
        for px in self.rgba.chunks_exact_mut(4) {
            px.copy_from_slice(&c);
        }
    }

    /// Set one pixel (ignored outside the buffer).
    pub fn put(&mut self, x: i32, y: i32, c: Color) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = ((y as u32 * self.width + x as u32) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&c);
    }

    /// Axis-aligned filled rectangle (inclusive of `x0,y0`, exclusive of `x1,y1`).
    pub fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        let (x0, x1) = (
            x0.clamp(0, self.width as i32),
            x1.clamp(0, self.width as i32),
        );
        let (y0, y1) = (
            y0.clamp(0, self.height as i32),
            y1.clamp(0, self.height as i32),
        );
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x, y, c);
            }
        }
    }

    /// Filled disc (radius clamped to the buffer size; off-screen parts are skipped).
    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        let r = r.clamp(0, Self::MAX_DIMENSION as i32);
        let rr = i64::from(r) * i64::from(r);
        for y in Self::clip_range(cy, r, self.height) {
            for x in Self::clip_range(cx, r, self.width) {
                let (dx, dy) = (i64::from(x - cx), i64::from(y - cy));
                if dx * dx + dy * dy <= rr {
                    self.put(x, y, c);
                }
            }
        }
    }

    /// One-pixel circle outline.
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        let r = r.clamp(0, Self::MAX_DIMENSION as i32);
        let rr = i64::from(r) * i64::from(r);
        let inner = i64::from(r - 1) * i64::from(r - 1);
        for y in Self::clip_range(cy, r, self.height) {
            for x in Self::clip_range(cx, r, self.width) {
                let (dx, dy) = (i64::from(x - cx), i64::from(y - cy));
                let d = dx * dx + dy * dy;
                if d <= rr && d > inner {
                    self.put(x, y, c);
                }
            }
        }
    }

    /// Pixel range `centre - r ..= centre + r` clipped to `0..extent`.
    fn clip_range(centre: i32, r: i32, extent: u32) -> std::ops::RangeInclusive<i32> {
        let lo = centre.saturating_sub(r).max(0);
        let hi = centre.saturating_add(r).min(extent as i32 - 1);
        lo..=hi
    }

    /// Bresenham line, clipped to the buffer first (Liang-Barsky) so huge coordinates cost nothing.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
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
        if src.len() < src_w as usize * src_h as usize * 4 {
            return;
        }
        for row in 0..h as i32 {
            let syy = sy + row;
            let dyy = dy + row;
            if syy < 0 || syy >= src_h as i32 || dyy < 0 || dyy >= self.height as i32 {
                continue;
            }
            let x_start = (-sx).max(-dx).max(0);
            let x_end = (w as i32)
                .min(src_w as i32 - sx)
                .min(self.width as i32 - dx);
            if x_end <= x_start {
                continue;
            }
            let si = ((syy as u32 * src_w) as i32 + sx + x_start) as usize * 4;
            let di = ((dyy as u32 * self.width) as i32 + dx + x_start) as usize * 4;
            let n = (x_end - x_start) as usize * 4;
            self.rgba[di..di + n].copy_from_slice(&src[si..si + n]);
        }
    }

    /// Blit an RGBA image: alpha 0 skipped, alpha 255 copied, other alphas blended (integer math).
    /// Ignored when `rgba` is shorter than `w * h * 4`.
    pub fn blit_rgba(&mut self, x: i32, y: i32, w: u32, h: u32, rgba: &[u8]) {
        if rgba.len() < w as usize * h as usize * 4 {
            return;
        }
        for sy in 0..h {
            let dy = y + sy as i32;
            if dy < 0 || dy >= self.height as i32 {
                continue;
            }
            for sx in 0..w {
                let dx = x + sx as i32;
                if dx < 0 || dx >= self.width as i32 {
                    continue;
                }
                let i = ((sy * w + sx) * 4) as usize;
                let a = u32::from(rgba[i + 3]);
                if a == 0 {
                    continue;
                }
                let di = ((dy as u32 * self.width + dx as u32) * 4) as usize;
                if a == 255 {
                    self.rgba[di..di + 3].copy_from_slice(&rgba[i..i + 3]);
                } else {
                    for c in 0..3 {
                        let s = u32::from(rgba[i + c]);
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

    /// Encode as PNG.
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
    let px = |f: Fixed, c: i32| f.round() - c;
    fb.circle(
        px(world.goal.0, cx),
        px(world.goal.1, cy),
        16,
        palette::GOAL,
    );
    // Sprites drawn this frame: (feet map x, feet map y, screen rect) for occlusion.
    let mut drawn: Vec<DrawnSprite> = Vec::new();
    for e in &world.entities {
        if !e.alive {
            continue;
        }
        match e.kind {
            EntityKind::Obstacle => {
                let (hw, hh) = (e.patrol[0].0, e.patrol[0].1);
                fb.fill_rect(
                    px(e.x - hw, cx),
                    px(e.y - hh, cy),
                    px(e.x + hw, cx),
                    px(e.y + hh, cy),
                    palette::OBSTACLE,
                );
            }
            EntityKind::Player | EntityKind::Guard => {
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
                match sprite {
                    Some((spec, frame)) => {
                        let x = px(e.x, cx) + spec.offset_x;
                        let y = px(e.y, cy) + spec.offset_y;
                        fb.blit_rgba(x, y, frame.width, frame.height, &frame.rgba);
                        drawn.push((
                            e.x.round(),
                            e.y.round(),
                            (x, y, frame.width as i32, frame.height as i32),
                        ));
                    }
                    None => fb.fill_circle(px(e.x, cx), px(e.y, cy), e.size.round(), c),
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
                        e.size.round() + 3,
                        palette::SELECTION,
                    );
                }
            }
        }
    }
    if let Some(bg) = background {
        apply_occluders(&mut fb, bg, (cx, cy), &drawn);
    }
    if let Some(bg) = background {
        apply_occluders(&mut fb, bg, (cx, cy), &drawn);
    }
    let (mx, my) = (
        Fixed::from_raw(world.pointer.0).round(),
        Fixed::from_raw(world.pointer.1).round(),
    );
    fb.line(mx - 4, my, mx + 4, my, palette::POINTER);
    fb.line(mx, my - 4, mx, my + 4, palette::POINTER);
    fb
}

/// A sprite drawn this frame: feet position in map pixels and its screen rectangle.
type DrawnSprite = (i32, i32, (i32, i32, i32, i32));

/// Restore background pixels of every occluder over the sprites standing behind it.
fn apply_occluders(
    fb: &mut Framebuffer,
    bg: &Background,
    (cx, cy): (i32, i32),
    drawn: &[DrawnSprite],
) {
    for occ in &bg.occluders {
        let (ox0, oy0) = (occ.x - cx, occ.y - cy);
        let (ox1, oy1) = (ox0 + occ.width as i32, oy0 + occ.height as i32);
        for &(fx, fy, (sx, sy, sw, sh)) in drawn {
            if fy >= occ.depth_y(fx) {
                continue; // in front of the object
            }
            let (x0, y0) = (sx.max(ox0).max(0), sy.max(oy0).max(0));
            let (x1, y1) = (
                (sx + sw).min(ox1).min(fb.width as i32),
                (sy + sh).min(oy1).min(fb.height as i32),
            );
            for y in y0..y1 {
                for x in x0..x1 {
                    let (mx, my) = (x + cx, y + cy);
                    if occ.covers(mx, my)
                        && mx >= 0
                        && my >= 0
                        && (mx as u32) < bg.width
                        && (my as u32) < bg.height
                    {
                        let si = ((my as u32 * bg.width + mx as u32) * 4) as usize;
                        let di = ((y as u32 * fb.width + x as u32) * 4) as usize;
                        fb.rgba[di..di + 4].copy_from_slice(&bg.rgba[si..si + 4]);
                    }
                }
            }
        }
    }
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
    fn drawing_outside_is_ignored() {
        let mut fb = Framebuffer::new(4, 4);
        fb.put(-1, 0, [1, 1, 1, 255]);
        fb.put(4, 4, [1, 1, 1, 255]);
        fb.fill_rect(-10, -10, 100, 100, [7, 7, 7, 255]);
        assert!(fb.rgba.chunks_exact(4).all(|p| p == [7, 7, 7, 255]));
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
