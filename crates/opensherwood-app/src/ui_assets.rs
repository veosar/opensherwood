//! Loading the pictures, fonts and texts the menus use from the player's files
//! (`docs/original/ui-flow.md` section 10 for the resource ids).

use opensherwood_assets::GameDir;
use opensherwood_formats::font;
use opensherwood_formats::image_blob::Image16;
use opensherwood_formats::sres;
use opensherwood_render::{FontAtlas, SpriteFrame};

use crate::ui::{HudAssets, UiAssets};

/// Resource ids in `DEFAULT.RES`.
pub mod ids {
    /// Main menu background.
    pub const MENU_BACKGROUND: u32 = 187;
    /// Menu button plate (3 states).
    pub const MENU_BUTTON: u32 = 190;
    /// Vertical parchment.
    pub const PARCHMENT: u32 = 147;
    /// Blue V seal.
    pub const SEAL_OK: u32 = 145;
    /// Red X seal.
    pub const SEAL_CANCEL: u32 = 146;
    /// Horizontal dialog scroll.
    pub const DIALOG: u32 = 38;
    /// HUD: foliage (bottom-left bush, bottom-right bush, bottom strips, top corners).
    pub const FOLIAGE_LEFT: u32 = 48;
    pub const FOLIAGE_RIGHT: u32 = 49;
    pub const FOLIAGE_STRIP_A: u32 = 50;
    pub const FOLIAGE_STRIP_B: u32 = 51;
    pub const CORNER_LEFT: u32 = 46;
    pub const CORNER_RIGHT: u32 = 47;
    /// HUD widgets.
    pub const EYES: u32 = 60;
    pub const MAP_SCROLL: u32 = 61;
    pub const TOWERS: u32 = 4;
    pub const STAND: u32 = 3;
    pub const PLAN: u32 = 251;
    /// Portrait faces start here (136 = the hero of the first mission).
    pub const PORTRAIT_FIRST: u32 = 136;
    /// Small scroll used behind the portrait until the original's frame is identified.
    pub const SMALL_SCROLL: u32 = 133;
    /// Plain arrow cursor.
    pub const CURSOR: u32 = 284;
    /// Credits background (full frame) and scrolling text strip.
    pub const CREDITS_BACKGROUND: u32 = 309;
    pub const CREDITS_STRIP: u32 = 308;
}

/// Text ids in `Level.res` (`docs/original/campaign-flow.md`).
pub mod texts {
    /// Briefing pages of the first mission.
    pub const FIRST_MISSION_BRIEFING: u32 = 1_000_105;
    /// Number of briefing pages at the start of that entry (the rest are tutorial popups).
    pub const FIRST_MISSION_BRIEFING_PAGES: usize = 3;
    /// Short briefings (objectives) of the first mission; string 0 is the initial objective.
    pub const FIRST_MISSION_OBJECTIVES: u32 = 1_000_283;
    /// Interface string table (menu labels, dialog wording, option names).
    pub const INTERFACE: u32 = 1_000_507;
    /// Question of the in-mission leave confirmation.
    pub const CONFIRM_LEAVE: usize = 31;
    /// Question of the main menu quit confirmation: the original's wording lives elsewhere (not located
    /// yet, `docs/original/ui-flow.md` open questions); the leave question is used meanwhile.
    pub const CONFIRM_QUIT: usize = 31;
    /// `Clover: %i` of the HUD.
    pub const CLOVER_FORMAT: usize = 245;
    /// Indices into `INTERFACE` used by the menus (observed in the retail English table).
    pub const DIFFICULTY_NAMES: usize = 34; // easy, medium, hard
    pub const MONEY_FORMAT: usize = 63; // contains `%i`
    pub const SCORE: usize = 64;
    pub const SPARED_LIVES: usize = 65;
    pub const PROGRESS: usize = 66;
    pub const DIFFICULTY: usize = 67;
    pub const GAME_LENGTH: usize = 258;
}

/// Bright green (`0x07C0` in RGB565), the transparent key of UI widgets (`docs/formats/image-blob.md`,
/// "UI colour key": the plate corners and the parchment margins hold it and the original composites them
/// transparent, as the engine's menu matches `menu_main.png` pixel for pixel outside the text).
const UI_COLOUR_KEY: u16 = 0x07C0;

/// Decode a UI picture: RGB565 to RGBA8; with `keyed`, the colour key becomes transparent. Unlike the
/// sprite preview converter no shadow key is applied.
fn frame_with(img: &Image16, keyed: bool) -> SpriteFrame {
    let mut rgba = Vec::with_capacity(img.pixels.len() * 4);
    for &p in &img.pixels {
        if keyed && p == UI_COLOUR_KEY {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let r = ((p >> 11) & 0x1F) as u8;
        let g = ((p >> 5) & 0x3F) as u8;
        let b = (p & 0x1F) as u8;
        rgba.extend_from_slice(&[
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
            255,
        ]);
    }
    SpriteFrame {
        width: u32::from(img.width),
        height: u32::from(img.height),
        rgba,
    }
}

/// Widgets, parchments, cursors and HUD pieces: keyed.
fn frame(img: &Image16) -> SpriteFrame {
    frame_with(img, true)
}

/// Full-width backgrounds: opaque, every pixel is content.
fn opaque(img: &Image16) -> SpriteFrame {
    frame_with(img, false)
}

fn load_font(game: &GameDir, name: &str) -> Option<FontAtlas> {
    let data = game.read(&format!("Data/Interface/Fonts/{name}")).ok()?;
    let f = font::parse_bitmap(&data)
        .map_err(|e| eprintln!("opensherwood: font {name}: {e}"))
        .ok()?;
    Some(FontAtlas::new(f))
}

/// Load everything the menus need. Missing pieces are logged and left `None`.
pub fn load(game: &GameDir) -> UiAssets {
    let archive = game.read("Data/Interface/Default.res").ok().and_then(|d| {
        sres::parse(&d)
            .map_err(|e| eprintln!("opensherwood: DEFAULT.RES: {e}"))
            .ok()
    });
    let pic_with = |id: u32, keyed: bool| -> Option<SpriteFrame> {
        let a = archive.as_ref()?;
        let conv = if keyed { frame } else { opaque };
        match &a.get(id)?.body {
            sres::Body::Picture(p) => Some(conv(p)),
            sres::Body::PictureCollection(v) => v.first().map(conv),
            _ => None,
        }
    };
    let pic = |id: u32| pic_with(id, true);
    let widget = |id: u32| -> Vec<SpriteFrame> {
        let Some(a) = archive.as_ref() else {
            return Vec::new();
        };
        match a.get(id).map(|e| &e.body) {
            Some(sres::Body::Widget { pictures, .. }) => pictures.iter().map(frame).collect(),
            _ => Vec::new(),
        }
    };
    let place = |id: u32, x: i32, y: i32| pic(id).map(|p| (p, x, y));
    let hud = HudAssets {
        foliage: [
            place(ids::FOLIAGE_LEFT, 0, 603),
            place(ids::FOLIAGE_RIGHT, 704, 603),
            place(ids::FOLIAGE_STRIP_A, 0, 658),
            place(ids::FOLIAGE_STRIP_B, 290, 658),
            place(ids::CORNER_LEFT, 0, 0),
            place(ids::CORNER_RIGHT, 978, 0),
        ]
        .into_iter()
        .flatten()
        .collect(),
        eyes: widget(ids::EYES).into_iter().next(),
        map_scroll: widget(ids::MAP_SCROLL).into_iter().next(),
        towers: widget(ids::TOWERS).into_iter().next(),
        stand: widget(ids::STAND).into_iter().next(),
        plan: widget(ids::PLAN).into_iter().next(),
        portrait: pic(ids::PORTRAIT_FIRST),
        portrait_scroll: pic(ids::SMALL_SCROLL),
    };
    UiAssets {
        menu_background: pic_with(ids::MENU_BACKGROUND, false),
        button: widget(ids::MENU_BUTTON),
        parchment: pic(ids::PARCHMENT),
        dialog: pic(ids::DIALOG),
        seal_ok: widget(ids::SEAL_OK),
        seal_cancel: widget(ids::SEAL_CANCEL),
        cursor: pic(ids::CURSOR),
        credits_background: pic_with(ids::CREDITS_BACKGROUND, false),
        credits_strip: pic(ids::CREDITS_STRIP),
        hud,
        font_button: load_font(game, "MenuButtonEnabled.bfn"),
        font_button_disabled: load_font(game, "MenuButtonDisabled.bfn"),
        font_title: load_font(game, "Title.bfn"),
        font_text: load_font(game, "tooltips.bfn"),
        font_debrief: load_font(game, "Debrief.bfn"),
        font_objective: load_font(game, "ShortBriefingActive.bfn"),
        strings: level_texts(game, texts::INTERFACE),
    }
}

/// Strings of a `Level.res` TEXT entry.
pub fn level_texts(game: &GameDir, id: u32) -> Vec<String> {
    let Ok(data) = game.read("Data/Text/Level.res") else {
        return Vec::new();
    };
    let Ok(a) = sres::parse(&data) else {
        return Vec::new();
    };
    match a.get(id).map(|e| &e.body) {
        Some(sres::Body::Text(v)) => v.clone(),
        _ => Vec::new(),
    }
}
