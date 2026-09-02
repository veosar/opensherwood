//! Deterministic CPU compositor (ADR-0002). The framebuffer is the authoritative picture; presenters
//! only display it.

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

/// A decoded background picture in map pixels (RGBA8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Background {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Pixels.
    pub rgba: Vec<u8>,
}

impl Framebuffer {
    /// Allocate a black, opaque buffer.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let mut rgba = vec![0; (width * height * 4) as usize];
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

    /// Filled disc.
    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        for y in -r..=r {
            for x in -r..=r {
                if x * x + y * y <= r * r {
                    self.put(cx + x, cy + y, c);
                }
            }
        }
    }

    /// One-pixel circle outline.
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        for y in -r..=r {
            for x in -r..=r {
                let d = x * x + y * y;
                if d <= r * r && d > (r - 1) * (r - 1) {
                    self.put(cx + x, cy + y, c);
                }
            }
        }
    }

    /// Bresenham line.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
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

    /// Blit an RGBA image (alpha 0 = skip, otherwise opaque copy).
    pub fn blit_rgba(&mut self, x: i32, y: i32, w: u32, h: u32, rgba: &[u8]) {
        for sy in 0..h {
            for sx in 0..w {
                let i = ((sy * w + sx) * 4) as usize;
                if rgba[i + 3] != 0 {
                    self.put(
                        x + sx as i32,
                        y + sy as i32,
                        [rgba[i], rgba[i + 1], rgba[i + 2], 255],
                    );
                }
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

/// Render a world into a new framebuffer at its logical viewport size, with an optional background.
#[must_use]
pub fn render(world: &World, background: Option<&Background>) -> Framebuffer {
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
                fb.fill_circle(px(e.x, cx), px(e.y, cy), e.size.round(), c);
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
    let (mx, my) = (
        Fixed::from_raw(world.pointer.0).round(),
        Fixed::from_raw(world.pointer.1).round(),
    );
    fb.line(mx - 4, my, mx + 4, my, palette::POINTER);
    fb.line(mx, my - 4, mx, my + 4, palette::POINTER);
    fb
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensherwood_core::Scenario;

    #[test]
    fn rendering_is_deterministic_and_png_encodes() {
        let w = World::new(Scenario::Synthetic("corridor".into()), 1).unwrap();
        let a = render(&w, None);
        let b = render(&w, None);
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
