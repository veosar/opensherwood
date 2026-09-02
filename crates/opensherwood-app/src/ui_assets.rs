//! Loading the pictures, fonts and texts the menus use from the player's files
//! (`docs/original/ui-flow.md` section 10 for the resource ids).

use opensherwood_assets::GameDir;
use opensherwood_formats::font;
use opensherwood_formats::image_blob::Image16;
use opensherwood_formats::sres;
use opensherwood_render::{FontAtlas, SpriteFrame};

use crate::ui::UiAssets;

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
    /// Plain arrow cursor.
    pub const CURSOR: u32 = 284;
}

/// Text ids in `Level.res` (`docs/original/campaign-flow.md`).
pub mod texts {
    /// Briefing pages of the first mission.
    pub const FIRST_MISSION_BRIEFING: u32 = 1_000_105;
    /// Number of briefing pages at the start of that entry (the rest are tutorial popups).
    pub const FIRST_MISSION_BRIEFING_PAGES: usize = 3;
    /// Interface string table (menu labels, dialog wording, option names).
    pub const INTERFACE: u32 = 1_000_507;
    /// Indices into `INTERFACE` used by the menus (observed in the retail English table).
    pub const DIFFICULTY_NAMES: usize = 34; // easy, medium, hard
    pub const MONEY_FORMAT: usize = 63; // contains `%i`
    pub const SCORE: usize = 64;
    pub const SPARED_LIVES: usize = 65;
    pub const PROGRESS: usize = 66;
    pub const DIFFICULTY: usize = 67;
    pub const GAME_LENGTH: usize = 258;
}

fn frame(img: &Image16) -> SpriteFrame {
    SpriteFrame {
        width: u32::from(img.width),
        height: u32::from(img.height),
        rgba: keyed(img),
    }
}

/// UI pictures use bright green (`0x07C0`) as the transparent key, like sprites.
fn keyed(img: &Image16) -> Vec<u8> {
    opensherwood_formats::sprite_decode::to_rgba8_keyed(img)
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
    let pic = |id: u32| -> Option<SpriteFrame> {
        let a = archive.as_ref()?;
        match &a.get(id)?.body {
            sres::Body::Picture(p) => Some(frame(p)),
            sres::Body::PictureCollection(v) => v.first().map(frame),
            _ => None,
        }
    };
    let widget = |id: u32| -> Vec<SpriteFrame> {
        let Some(a) = archive.as_ref() else {
            return Vec::new();
        };
        match a.get(id).map(|e| &e.body) {
            Some(sres::Body::Widget { pictures, .. }) => pictures.iter().map(frame).collect(),
            _ => Vec::new(),
        }
    };
    UiAssets {
        menu_background: pic(ids::MENU_BACKGROUND),
        button: widget(ids::MENU_BUTTON),
        parchment: pic(ids::PARCHMENT),
        seal_ok: widget(ids::SEAL_OK),
        cursor: pic(ids::CURSOR),
        font_button: load_font(game, "MenuButtonEnabled.bfn"),
        font_button_disabled: load_font(game, "MenuButtonDisabled.bfn"),
        font_title: load_font(game, "Title.bfn"),
        font_text: load_font(game, "tooltips.bfn"),
        font_debrief: load_font(game, "Debrief.bfn"),
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
