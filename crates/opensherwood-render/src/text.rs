//! Text drawing with the game's bitmap fonts (`docs/formats/fonts.md`).

use std::collections::HashMap;

use opensherwood_formats::font::BitmapFont;

use crate::{Framebuffer, SpriteFrame};

/// A bitmap font with its glyphs pre-converted to RGBA.
#[derive(Debug)]
pub struct FontAtlas {
    font: BitmapFont,
    glyphs: HashMap<char, SpriteFrame>,
}

impl FontAtlas {
    /// Convert every glyph once.
    #[must_use]
    pub fn new(font: BitmapFont) -> Self {
        let mut glyphs = HashMap::new();
        for g in &font.glyphs {
            if let Some(ch) = char::from_u32(u32::from(g.code)) {
                let img = font.glyph_rgba(g);
                glyphs.insert(
                    ch,
                    SpriteFrame {
                        width: img.width,
                        height: img.height,
                        rgba: img.pixels,
                    },
                );
            }
        }
        Self { font, glyphs }
    }

    /// Line height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.font.height()
    }

    /// Width of a string in pixels (same advance rule as [`FontAtlas::draw`]).
    #[must_use]
    pub fn measure(&self, text: &str) -> i32 {
        let mut pen = 0i32;
        for ch in text.chars() {
            match self.font.glyph(ch) {
                Some(g) => pen += g.x_adjust + self.font.advance(g),
                None => pen += self.space_advance(),
            }
        }
        pen
    }

    fn space_advance(&self) -> i32 {
        self.font
            .glyph(' ')
            .map_or(self.height() as i32 / 3, |g| self.font.advance(g).max(2))
    }

    /// Draw `text` with its top-left corner at `(x, y)`; returns the pen x after the last glyph.
    pub fn draw(&self, fb: &mut Framebuffer, text: &str, x: i32, y: i32) -> i32 {
        let mut pen = x;
        for ch in text.chars() {
            let Some(g) = self.font.glyph(ch) else {
                pen += self.space_advance();
                continue;
            };
            pen += g.x_adjust;
            // The space glyph aliases other cells in some faces: advance only.
            if ch != ' '
                && let Some(frame) = self.glyphs.get(&ch)
            {
                fb.blit_rgba(pen, y, frame.width, frame.height, &frame.rgba);
            }
            pen += self.font.advance(g);
        }
        pen
    }

    /// Draw centred on `cx`.
    pub fn draw_centered(&self, fb: &mut Framebuffer, text: &str, cx: i32, y: i32) {
        let w = self.measure(text);
        self.draw(fb, text, cx - w / 2, y);
    }
}
