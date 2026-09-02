//! Screens outside the simulation, laid out like the original at 1024x768
//! (`docs/original/ui-flow.md`): the main menu (background picture, 3-state plate buttons, retail fonts,
//! profile summary) and the mission briefing parchment shown over the paused, green-tinted scene.
//! Driven by the same canonical input events as the world so the harness can click through them.

use opensherwood_core::{Button, Fixed, InputEvent, Key};
use opensherwood_render::{FontAtlas, Framebuffer, SpriteFrame};
use serde::Serialize;

/// Logical frame of every menu screen.
pub const MENU_FRAME: (u32, u32) = (1024, 768);
/// Button column: x, width, first row y, row pitch, plate height (`ui-flow.md` 2.1).
const BTN_X: i32 = 664;
const BTN_W: i32 = 168;
// `ui-flow.md` lists 345; measuring the plate's top edge on the original's capture gives 339.
const BTN_ROW0_Y: i32 = 339;
const BTN_PITCH: i32 = 41;
const BTN_H: i32 = 39;
/// Background picture position inside the frame.
const BG_Y: i32 = 128;

/// Menu entries of the main menu, top to bottom (rows 0..6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuAction {
    /// Start the campaign (first mission).
    Play,
    /// Load a saved game (not implemented).
    Load,
    /// Profile selection (not implemented).
    SelectPlayer,
    /// Options (not implemented).
    Options,
    /// Movies (not implemented).
    ShowMovies,
    /// Credits (not implemented).
    Credits,
    /// Quit.
    Exit,
}

impl MenuAction {
    const ALL: [MenuAction; 7] = [
        MenuAction::Play,
        MenuAction::Load,
        MenuAction::SelectPlayer,
        MenuAction::Options,
        MenuAction::ShowMovies,
        MenuAction::Credits,
        MenuAction::Exit,
    ];

    /// Index of the entry's label in the interface string table (`ui_assets::texts::INTERFACE`).
    fn label_index(self) -> usize {
        match self {
            MenuAction::Play => 0,
            MenuAction::Load => 10,
            MenuAction::SelectPlayer => 1,
            MenuAction::Options => 12,
            MenuAction::ShowMovies => 2,
            MenuAction::Credits => 3,
            MenuAction::Exit => 4,
        }
    }

    /// Neutral identifier shown when the player's string table is unavailable (synthetic runs).
    fn fallback_label(self) -> &'static str {
        match self {
            MenuAction::Play => "play",
            MenuAction::Load => "load",
            MenuAction::SelectPlayer => "select player",
            MenuAction::Options => "options",
            MenuAction::ShowMovies => "movies",
            MenuAction::Credits => "credits",
            MenuAction::Exit => "exit",
        }
    }
}

/// One clickable line.
#[derive(Debug, Clone, Serialize)]
pub struct MenuItem {
    /// Action.
    pub action: MenuAction,
    /// Label.
    pub label: String,
    /// Rectangle in logical pixels (x, y, w, h).
    pub rect: (i32, i32, i32, i32),
    /// Whether the entry reacts.
    pub enabled: bool,
}

/// Pictures and fonts the menus need, decoded from the player's `DEFAULT.RES` and font files.
pub struct UiAssets {
    /// Main menu background (`PIC` 187, 1024x512).
    pub menu_background: Option<SpriteFrame>,
    /// Button plate states (`BTTN` 190): disabled, normal, hovered.
    pub button: Vec<SpriteFrame>,
    /// Vertical parchment (`PIC` 147) for briefings.
    pub parchment: Option<SpriteFrame>,
    /// Blue V seal (`BTTN` 145) states.
    pub seal_ok: Vec<SpriteFrame>,
    /// Arrow cursor (`PIC` 284).
    pub cursor: Option<SpriteFrame>,
    /// Fonts.
    pub font_button: Option<FontAtlas>,
    /// Disabled button face.
    pub font_button_disabled: Option<FontAtlas>,
    /// Screen titles / profile name.
    pub font_title: Option<FontAtlas>,
    /// Menu text (profile summary).
    pub font_text: Option<FontAtlas>,
    /// Briefing parchment text.
    pub font_debrief: Option<FontAtlas>,
    /// Interface strings (`Level.res` TEXT 1000507), indexed as in `ui_assets::texts`.
    pub strings: Vec<String>,
}

/// Interface string by index, or a neutral fallback.
fn text<'a>(strings: &'a [String], index: usize, fallback: &'a str) -> &'a str {
    strings.get(index).map_or(fallback, String::as_str)
}

impl std::fmt::Debug for UiAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiAssets")
            .field("menu_background", &self.menu_background.is_some())
            .field("button_states", &self.button.len())
            .finish_non_exhaustive()
    }
}

/// Profile summary shown left of the buttons.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileSummary {
    /// Name.
    pub name: String,
    /// Difficulty: 0 easy, 1 medium, 2 hard.
    pub difficulty: u8,
    /// Money.
    pub money: u32,
    /// Score.
    pub score: u32,
    /// Spared lives percentage.
    pub spared_lives: u32,
    /// Progress percentage.
    pub progress: u32,
    /// Game length text.
    pub game_length: String,
}

impl Default for ProfileSummary {
    fn default() -> Self {
        Self {
            name: "Player".into(),
            difficulty: 1,
            money: 100,
            score: 0,
            spared_lives: 0,
            progress: 0,
            game_length: "00:00".into(),
        }
    }
}

/// The main menu.
#[derive(Debug)]
pub struct MainMenu {
    items: Vec<MenuItem>,
    hovered: Option<usize>,
    pointer: (i32, i32),
    pending: Option<MenuAction>,
    /// Profile summary.
    pub profile: ProfileSummary,
}

/// Observation of a screen for the harness.
#[derive(Debug, Clone, Serialize)]
pub struct MenuState {
    /// Screen name (`main_menu`, `briefing`).
    pub screen: String,
    /// Items.
    pub items: Vec<MenuItem>,
    /// Hovered item index.
    pub hovered: Option<usize>,
    /// Briefing page (1-based) and page count when on the briefing screen.
    pub page: Option<(usize, usize)>,
}

impl MainMenu {
    /// Build the menu with the original's button column; labels come from the interface string table.
    #[must_use]
    pub fn new(profile: ProfileSummary, strings: &[String]) -> Self {
        let items = MenuAction::ALL
            .iter()
            .enumerate()
            .map(|(row, &action)| MenuItem {
                action,
                label: text(strings, action.label_index(), action.fallback_label()).to_string(),
                rect: (BTN_X, BTN_ROW0_Y + row as i32 * BTN_PITCH, BTN_W, BTN_H),
                // The original shows every entry enabled; unimplemented ones only log.
                enabled: true,
            })
            .collect();
        Self {
            items,
            hovered: None,
            pointer: (0, 0),
            pending: None,
            profile,
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<usize> {
        self.items.iter().position(|it| {
            let (ix, iy, w, h) = it.rect;
            (ix..ix + w).contains(&x) && (iy..iy + h).contains(&y)
        })
    }

    /// Apply input; returns an action when one was chosen.
    pub fn handle(&mut self, event: InputEvent) -> Option<MenuAction> {
        match event {
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
                self.hovered = self.hit(self.pointer.0, self.pointer.1);
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } => {
                if let Some(i) = self.hit(self.pointer.0, self.pointer.1)
                    && self.items[i].enabled
                {
                    self.pending = Some(self.items[i].action);
                }
            }
            InputEvent::KeyDown { key } => match key {
                Key::Up => {
                    let n = self.items.len();
                    self.hovered = Some(self.hovered.map_or(n - 1, |i| (i + n - 1) % n));
                }
                Key::Down => {
                    self.hovered = Some(self.hovered.map_or(0, |i| (i + 1) % self.items.len()));
                }
                Key::Enter | Key::Space => {
                    if let Some(i) = self.hovered
                        && self.items[i].enabled
                    {
                        self.pending = Some(self.items[i].action);
                    }
                }
                Key::Escape => self.pending = Some(MenuAction::Exit),
                _ => {}
            },
            _ => {}
        }
        self.pending.take()
    }

    /// Current state for `observe`.
    #[must_use]
    pub fn state(&self) -> MenuState {
        MenuState {
            screen: "main_menu".into(),
            items: self.items.clone(),
            hovered: self.hovered,
            page: None,
        }
    }

    /// Render the menu frame.
    #[must_use]
    pub fn render(&self, assets: Option<&UiAssets>) -> Framebuffer {
        let mut fb = Framebuffer::new(MENU_FRAME.0, MENU_FRAME.1);
        fb.clear([0, 0, 0, 255]);
        let Some(a) = assets else {
            for (i, it) in self.items.iter().enumerate() {
                let c = if self.hovered == Some(i) {
                    [220, 140, 40, 255]
                } else {
                    [40, 110, 110, 255]
                };
                fb.fill_rect(
                    it.rect.0,
                    it.rect.1,
                    it.rect.0 + it.rect.2,
                    it.rect.1 + it.rect.3,
                    c,
                );
            }
            draw_pointer(&mut fb, self.pointer, None);
            return fb;
        };
        if let Some(bg) = &a.menu_background {
            fb.blit_rgba(0, BG_Y, bg.width, bg.height, &bg.rgba);
        }
        for (i, it) in self.items.iter().enumerate() {
            let state = if !it.enabled {
                0
            } else if self.hovered == Some(i) {
                2
            } else {
                1
            };
            if let Some(plate) = a.button.get(state.min(a.button.len().saturating_sub(1))) {
                fb.blit_rgba(it.rect.0, it.rect.1, plate.width, plate.height, &plate.rgba);
            }
            let font = if it.enabled {
                a.font_button.as_ref()
            } else {
                a.font_button_disabled.as_ref().or(a.font_button.as_ref())
            };
            if let Some(font) = font {
                let cx = it.rect.0 + it.rect.2 / 2;
                let cy = it.rect.1 + 19 - font.height() as i32 / 2;
                font.draw_centered(&mut fb, &it.label, cx, cy);
            }
        }
        // Profile summary, centred at x = 432 (ui-flow.md section 3).
        if let Some(title) = &a.font_title {
            title.draw_centered(
                &mut fb,
                &self.profile.name,
                432,
                254 - title.height() as i32 / 2,
            );
        }
        if let Some(font) = &a.font_text {
            use crate::ui_assets::texts as t;
            let s = &a.strings;
            let p = &self.profile;
            let difficulty = text(
                s,
                t::DIFFICULTY_NAMES + usize::from(p.difficulty.min(2)),
                ["easy", "medium", "hard"][usize::from(p.difficulty.min(2))],
            );
            let lines = [
                format!("{} : {difficulty}", text(s, t::DIFFICULTY, "difficulty")),
                text(s, t::MONEY_FORMAT, "money: %i").replace("%i", &p.money.to_string()),
                format!("{} : {}", text(s, t::SCORE, "score"), p.score),
                format!(
                    "{} : {} %",
                    text(s, t::SPARED_LIVES, "spared lives"),
                    p.spared_lives
                ),
                format!("{} : {} %", text(s, t::PROGRESS, "progress"), p.progress),
                format!(
                    "{} : {}",
                    text(s, t::GAME_LENGTH, "game length"),
                    p.game_length
                ),
            ];
            for (k, line) in lines.iter().enumerate() {
                let y = 278 + 20 * k as i32 - font.height() as i32 / 2;
                font.draw_centered(&mut fb, line, 432, y);
            }
        }
        draw_pointer(&mut fb, self.pointer, a.cursor.as_ref());
        fb
    }
}

/// The mission briefing: parchment pages over the paused scene.
#[derive(Debug)]
pub struct Briefing {
    /// Pages of text.
    pub pages: Vec<String>,
    /// Current page.
    pub page: usize,
    pointer: (i32, i32),
    done: bool,
}

/// Seal button rectangle (`ui-flow.md` 9.2: blue V at (508,552), 41x44).
const SEAL_RECT: (i32, i32, i32, i32) = (488, 530, 41, 44);

impl Briefing {
    /// New briefing over `pages`.
    #[must_use]
    pub fn new(pages: Vec<String>) -> Self {
        Self {
            pages,
            page: 0,
            pointer: (0, 0),
            done: false,
        }
    }

    /// Apply input; returns true when the last page was confirmed.
    pub fn handle(&mut self, event: InputEvent) -> bool {
        let mut next = false;
        match event {
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } => {
                let (x, y, w, h) = SEAL_RECT;
                next = (x..x + w).contains(&self.pointer.0) && (y..y + h).contains(&self.pointer.1);
            }
            InputEvent::KeyDown {
                key: Key::Enter | Key::Space,
            } => next = true,
            _ => {}
        }
        if next {
            if self.page + 1 < self.pages.len() {
                self.page += 1;
            } else {
                self.done = true;
            }
        }
        self.done
    }

    /// State for `observe`.
    #[must_use]
    pub fn state(&self) -> MenuState {
        MenuState {
            screen: "briefing".into(),
            items: vec![MenuItem {
                action: MenuAction::Play,
                label: "ok".into(),
                rect: SEAL_RECT,
                enabled: true,
            }],
            hovered: None,
            page: Some((self.page + 1, self.pages.len().max(1))),
        }
    }

    /// Draw the parchment over `scene` (which is tinted green like the paused original).
    pub fn render(&self, scene: &mut Framebuffer, assets: Option<&UiAssets>) {
        tint_green(scene);
        let Some(a) = assets else {
            scene.fill_rect(264, 148, 264 + 496, 148 + 463, [220, 200, 150, 255]);
            return;
        };
        if let Some(p) = &a.parchment {
            scene.blit_rgba(264, 148, p.width, p.height, &p.rgba);
        }
        if let Some(seal) = a.seal_ok.get(1).or(a.seal_ok.first()) {
            scene.blit_rgba(
                SEAL_RECT.0,
                SEAL_RECT.1,
                seal.width,
                seal.height,
                &seal.rgba,
            );
        }
        if let (Some(font), Some(text)) = (a.font_debrief.as_ref(), self.pages.get(self.page)) {
            let mut y = 205;
            for line in wrap(font, text, 400) {
                font.draw(scene, &line, 318, y);
                y += font.height() as i32 + 2;
            }
        }
        draw_pointer(scene, self.pointer, a.cursor.as_ref());
    }
}

/// Word-wrap `text` to `max_width` pixels with `font`.
fn wrap(font: &FontAtlas, text: &str, max_width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if font.measure(&candidate) > max_width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// The original pauses with a green tint: keep green, halve red and blue.
fn tint_green(fb: &mut Framebuffer) {
    for px in fb.rgba.chunks_exact_mut(4) {
        px[0] /= 2;
        px[2] /= 2;
    }
}

/// Draw the arrow cursor (hotspot at its top-left tip) or a cross when the picture is missing.
fn draw_pointer(fb: &mut Framebuffer, (mx, my): (i32, i32), cursor: Option<&SpriteFrame>) {
    if let Some(c) = cursor {
        fb.blit_rgba(mx, my, c.width, c.height, &c.rgba);
    } else {
        fb.line(mx - 4, my, mx + 4, my, [255, 255, 0, 255]);
        fb.line(mx, my - 4, mx, my + 4, [255, 255, 0, 255]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_rows_match_the_original_column() {
        let m = MainMenu::new(ProfileSummary::default(), &[]);
        assert_eq!(m.items.len(), 7);
        assert_eq!(m.items[0].label, "play");
        let named = MainMenu::new(ProfileSummary::default(), &["Go".to_string()]);
        assert_eq!(named.items[0].label, "Go");
        assert_eq!(m.items[0].rect, (664, 339, 168, 39));
        assert_eq!(m.items[6].rect, (664, 585, 168, 39));
        assert_eq!(m.items[6].action, MenuAction::Exit);
    }

    #[test]
    fn click_on_play_starts_and_keyboard_navigates() {
        let mut m = MainMenu::new(ProfileSummary::default(), &[]);
        assert_eq!(
            m.handle(InputEvent::PointerMove {
                x256: 748 * 256,
                y256: 364 * 256
            }),
            None
        );
        assert_eq!(m.hovered, Some(0));
        assert_eq!(
            m.handle(InputEvent::PointerDown {
                button: Button::Left
            }),
            Some(MenuAction::Play)
        );
        let mut m = MainMenu::new(ProfileSummary::default(), &[]);
        m.handle(InputEvent::KeyDown { key: Key::Up });
        assert_eq!(m.hovered, Some(6));
        assert_eq!(
            m.handle(InputEvent::KeyDown { key: Key::Enter }),
            Some(MenuAction::Exit)
        );
        // Between two plates nothing is hovered.
        let mut m = MainMenu::new(ProfileSummary::default(), &[]);
        m.handle(InputEvent::PointerMove {
            x256: 748 * 256,
            y256: 379 * 256,
        });
        assert_eq!(m.hovered, None);
    }

    #[test]
    fn briefing_pages_advance_with_enter_and_seal() {
        let mut b = Briefing::new(vec!["one".into(), "two".into()]);
        assert!(!b.handle(InputEvent::KeyDown { key: Key::Enter }));
        assert_eq!(b.page, 1);
        b.handle(InputEvent::PointerMove {
            x256: 508 * 256,
            y256: 552 * 256,
        });
        assert!(b.handle(InputEvent::PointerDown {
            button: Button::Left
        }));
        let fb = MainMenu::new(ProfileSummary::default(), &[]).render(None);
        assert_eq!((fb.width, fb.height), MENU_FRAME);
    }
}
