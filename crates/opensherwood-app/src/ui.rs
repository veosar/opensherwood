//! Screens outside the simulation, laid out like the original at 1024x768
//! (`docs/original/ui-flow.md`): the main menu and pause menu (background picture, 3-state plate buttons,
//! retail fonts and strings), confirmation dialogs, the mission briefing parchment over the paused,
//! green-tinted scene, and the in-mission HUD. Driven by the same canonical input events as the world so the
//! harness can click through them. All text comes from the player's files; the fallbacks are neutral
//! identifiers for runs without game data.

use opensherwood_core::{Button, Fixed, InputEvent, Key};
pub use opensherwood_protocol::{UiItem, UiState as MenuState};
use opensherwood_render::{FontAtlas, Framebuffer, SpriteFrame};
use serde::Serialize;

use crate::ui_assets::texts as t;

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
/// Seal button rectangles (`ui-flow.md` 2.3, 9.2): 41x44 plates.
const SEAL_BRIEFING: (i32, i32, i32, i32) = (488, 530, 41, 44);
const SEAL_YES: (i32, i32, i32, i32) = (463, 411, 41, 44);
const SEAL_NO: (i32, i32, i32, i32) = (521, 411, 41, 44);
/// Horizontal dialog scroll (`PIC` 38, 400x200) position.
const DIALOG_POS: (i32, i32) = (312, 288);

/// Everything a menu entry or dialog seal can do.
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
    /// Quit the program.
    Exit,
    /// Resume the mission.
    Continue,
    /// Save (not implemented).
    Save,
    /// Restart the mission.
    Restart,
    /// Leave the mission for the main menu.
    Quit,
    /// Confirm a dialog.
    Yes,
    /// Cancel a dialog.
    No,
}

impl MenuAction {
    const MAIN: [MenuAction; 7] = [
        MenuAction::Play,
        MenuAction::Load,
        MenuAction::SelectPlayer,
        MenuAction::Options,
        MenuAction::ShowMovies,
        MenuAction::Credits,
        MenuAction::Exit,
    ];
    const PAUSE: [MenuAction; 6] = [
        MenuAction::Continue,
        MenuAction::Load,
        MenuAction::Save,
        MenuAction::Options,
        MenuAction::Restart,
        MenuAction::Quit,
    ];

    /// Index of the entry's label in the interface string table (`ui_assets::texts::INTERFACE`).
    fn label_index(self) -> usize {
        match self {
            MenuAction::Play => 0,
            MenuAction::SelectPlayer => 1,
            MenuAction::ShowMovies => 2,
            MenuAction::Credits => 3,
            MenuAction::Exit => 4,
            MenuAction::Continue => 9,
            MenuAction::Load => 10,
            MenuAction::Save => 11,
            MenuAction::Options => 12,
            MenuAction::Restart => 13,
            MenuAction::Quit => 14,
            MenuAction::Yes => 15,
            MenuAction::No => 16,
        }
    }

    /// Entries that do something in this build; the others are drawn on the disabled plate.
    fn implemented(self) -> bool {
        !matches!(
            self,
            MenuAction::Load
                | MenuAction::SelectPlayer
                | MenuAction::Options
                | MenuAction::ShowMovies
                | MenuAction::Save
        )
    }

    /// Protocol identifier (`snake_case` of the variant).
    fn id(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
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
            MenuAction::Continue => "continue",
            MenuAction::Save => "save",
            MenuAction::Restart => "restart",
            MenuAction::Quit => "quit",
            MenuAction::Yes => "yes",
            MenuAction::No => "no",
        }
    }
}

/// One clickable element.
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

impl MenuItem {
    fn to_protocol(&self) -> UiItem {
        UiItem {
            action: self.action.id(),
            label: self.label.clone(),
            rect: [self.rect.0, self.rect.1, self.rect.2, self.rect.3],
            enabled: self.enabled,
        }
    }
}

fn items_to_protocol(items: &[MenuItem]) -> Vec<UiItem> {
    items.iter().map(MenuItem::to_protocol).collect()
}

/// Pictures and fonts the menus need, decoded from the player's `DEFAULT.RES` and font files.
pub struct UiAssets {
    /// Main menu background (`PIC` 187, 1024x512).
    pub menu_background: Option<SpriteFrame>,
    /// Button plate states (`BTTN` 190): disabled, normal, hovered.
    pub button: Vec<SpriteFrame>,
    /// Vertical parchment (`PIC` 147) for briefings.
    pub parchment: Option<SpriteFrame>,
    /// Horizontal dialog scroll (`PIC` 38).
    pub dialog: Option<SpriteFrame>,
    /// Blue V seal (`BTTN` 145) states.
    pub seal_ok: Vec<SpriteFrame>,
    /// Red X seal (`BTTN` 146) states.
    pub seal_cancel: Vec<SpriteFrame>,
    /// Arrow cursor (`PIC` 284).
    pub cursor: Option<SpriteFrame>,
    /// Credits background (`PIC` 309, 1024x768) and text strip (`PIC` 308, 400x7659).
    pub credits_background: Option<SpriteFrame>,
    pub credits_strip: Option<SpriteFrame>,
    /// HUD pictures, see `HudAssets`.
    pub hud: HudAssets,
    /// Fonts.
    pub font_button: Option<FontAtlas>,
    /// Disabled button face.
    pub font_button_disabled: Option<FontAtlas>,
    /// Screen titles / profile name.
    pub font_title: Option<FontAtlas>,
    /// Menu text (profile summary, HUD counters).
    pub font_text: Option<FontAtlas>,
    /// Briefing parchment text.
    pub font_debrief: Option<FontAtlas>,
    /// Objective line of the pause menu.
    pub font_objective: Option<FontAtlas>,
    /// Interface strings (`Level.res` TEXT 1000507), indexed as in `ui_assets::texts`.
    pub strings: Vec<String>,
}

/// HUD pictures (`ui-flow.md` 9.3 and 10; positions are matched by eye to the original's captures).
#[derive(Default)]
pub struct HudAssets {
    /// Foliage pictures with their positions (bushes, strips, corners).
    pub foliage: Vec<(SpriteFrame, i32, i32)>,
    /// Robin's eyes in the leaves (`BTTN` 60).
    pub eyes: Option<SpriteFrame>,
    /// Map scroll (`BTTN` 61).
    pub map_scroll: Option<SpriteFrame>,
    /// Towers, zoom (`BTTN` 4).
    pub towers: Option<SpriteFrame>,
    /// Standing figure (`BTTN` 3).
    pub stand: Option<SpriteFrame>,
    /// Plan scroll (`BTTN` 251).
    pub plan: Option<SpriteFrame>,
    /// Hero portrait face (`PIC` 136).
    pub portrait: Option<SpriteFrame>,
    /// Small scroll behind the portrait (`PIC` 133; the original's frame is not identified).
    pub portrait_scroll: Option<SpriteFrame>,
}

impl std::fmt::Debug for UiAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiAssets")
            .field("menu_background", &self.menu_background.is_some())
            .field("button_states", &self.button.len())
            .field("strings", &self.strings.len())
            .finish_non_exhaustive()
    }
}

/// Interface string by index, or a neutral fallback.
fn text<'a>(strings: &'a [String], index: usize, fallback: &'a str) -> &'a str {
    strings.get(index).map_or(fallback, String::as_str)
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

/// The shared button column with pointer and keyboard handling.
#[derive(Debug)]
struct ButtonColumn {
    items: Vec<MenuItem>,
    hovered: Option<usize>,
    pointer: (i32, i32),
}

impl ButtonColumn {
    fn new(actions: &[MenuAction], first_row: usize, strings: &[String]) -> Self {
        let items = actions
            .iter()
            .enumerate()
            .map(|(i, &action)| MenuItem {
                action,
                label: text(strings, action.label_index(), action.fallback_label()).to_string(),
                rect: (
                    BTN_X,
                    BTN_ROW0_Y + (first_row + i) as i32 * BTN_PITCH,
                    BTN_W,
                    BTN_H,
                ),
                // The original shows every entry enabled; entries this build cannot serve are drawn
                // on the disabled plate so the screen does not promise more than it does.
                enabled: action.implemented(),
            })
            .collect();
        Self {
            items,
            hovered: None,
            pointer: (0, 0),
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<usize> {
        self.items.iter().position(|it| hit(it.rect, x, y))
    }

    /// Apply input; returns the chosen action.
    fn handle(&mut self, event: InputEvent) -> Option<MenuAction> {
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
                    return Some(self.items[i].action);
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
                        return Some(self.items[i].action);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn draw(&self, fb: &mut Framebuffer, assets: Option<&UiAssets>) {
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
            return;
        };
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
                font.draw_centered(fb, &it.label, cx, cy);
            }
        }
    }
}

fn hit((x, y, w, h): (i32, i32, i32, i32), px: i32, py: i32) -> bool {
    (x..x + w).contains(&px) && (y..y + h).contains(&py)
}

/// A yes/no dialog on the horizontal scroll (`ui-flow.md` 2.3).
#[derive(Debug)]
pub struct Confirm {
    question: String,
    items: Vec<MenuItem>,
    pointer: (i32, i32),
}

impl Confirm {
    fn new(question_index: usize, fallback: &str, strings: &[String]) -> Self {
        Self {
            question: text(strings, question_index, fallback).to_string(),
            items: vec![
                MenuItem {
                    action: MenuAction::Yes,
                    label: text(strings, MenuAction::Yes.label_index(), "yes").to_string(),
                    rect: SEAL_YES,
                    enabled: true,
                },
                MenuItem {
                    action: MenuAction::No,
                    label: text(strings, MenuAction::No.label_index(), "no").to_string(),
                    rect: SEAL_NO,
                    enabled: true,
                },
            ],
            pointer: (0, 0),
        }
    }

    /// Returns `Some(true)` for yes, `Some(false)` for no.
    fn handle(&mut self, event: InputEvent) -> Option<bool> {
        match event {
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
                None
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } => {
                if hit(SEAL_YES, self.pointer.0, self.pointer.1) {
                    Some(true)
                } else if hit(SEAL_NO, self.pointer.0, self.pointer.1) {
                    Some(false)
                } else {
                    None
                }
            }
            InputEvent::KeyDown {
                key: Key::Enter | Key::Space,
            } => Some(true),
            InputEvent::KeyDown { key: Key::Escape } => Some(false),
            _ => None,
        }
    }

    fn draw(&self, fb: &mut Framebuffer, assets: Option<&UiAssets>) {
        let Some(a) = assets else {
            fb.fill_rect(
                DIALOG_POS.0,
                DIALOG_POS.1,
                DIALOG_POS.0 + 400,
                DIALOG_POS.1 + 200,
                [220, 200, 150, 255],
            );
            return;
        };
        if let Some(p) = &a.dialog {
            fb.blit_rgba(DIALOG_POS.0, DIALOG_POS.1, p.width, p.height, &p.rgba);
        }
        if let Some(font) = a.font_debrief.as_ref().or(a.font_text.as_ref()) {
            let mut y = DIALOG_POS.1 + 45;
            for line in wrap(font, &self.question, 340) {
                font.draw_centered(fb, &line, 512, y);
                y += font.height() as i32 + 2;
            }
        }
        for (seal, rect) in [(&a.seal_ok, SEAL_YES), (&a.seal_cancel, SEAL_NO)] {
            let hovered = hit(rect, self.pointer.0, self.pointer.1);
            let state = if hovered { 2 } else { 1 };
            if let Some(s) = seal.get(state.min(seal.len().saturating_sub(1))) {
                fb.blit_rgba(rect.0, rect.1, s.width, s.height, &s.rgba);
            }
        }
    }

    fn state(&self) -> MenuState {
        MenuState {
            screen: "dialog".into(),
            items: items_to_protocol(&self.items),
            hovered: None,
            page: None,
        }
    }
}

/// The main menu.
#[derive(Debug)]
pub struct MainMenu {
    column: ButtonColumn,
    confirm: Option<Confirm>,
    strings: Vec<String>,
    /// Profile summary.
    pub profile: ProfileSummary,
}

impl MainMenu {
    /// Build the menu with the original's button column; labels come from the interface string table.
    #[must_use]
    pub fn new(profile: ProfileSummary, strings: &[String]) -> Self {
        Self {
            column: ButtonColumn::new(&MenuAction::MAIN, 0, strings),
            confirm: None,
            strings: strings.to_vec(),
            profile,
        }
    }

    /// Apply input; returns an action when one was chosen (`Exit` only after confirmation).
    pub fn handle(&mut self, event: InputEvent) -> Option<MenuAction> {
        if let Some(c) = self.confirm.as_mut() {
            return match c.handle(event) {
                Some(true) => {
                    self.confirm = None;
                    Some(MenuAction::Exit)
                }
                Some(false) => {
                    self.confirm = None;
                    None
                }
                None => None,
            };
        }
        let chosen = match event {
            InputEvent::KeyDown { key: Key::Escape } => Some(MenuAction::Exit),
            e => self.column.handle(e),
        };
        match chosen {
            Some(MenuAction::Exit) => {
                self.confirm = Some(Confirm::new(t::CONFIRM_QUIT, "quit?", &self.strings));
                None
            }
            other => other,
        }
    }

    /// Current state for `observe`.
    #[must_use]
    pub fn state(&self) -> MenuState {
        if let Some(c) = &self.confirm {
            return c.state();
        }
        MenuState {
            screen: "main_menu".into(),
            items: items_to_protocol(&self.column.items),
            hovered: self.column.hovered,
            page: None,
        }
    }

    /// Render the menu frame.
    #[must_use]
    pub fn render(&self, assets: Option<&UiAssets>) -> Framebuffer {
        let mut fb = Framebuffer::new(MENU_FRAME.0, MENU_FRAME.1);
        fb.clear([0, 0, 0, 255]);
        if let Some(bg) = assets.and_then(|a| a.menu_background.as_ref()) {
            fb.blit_rgba(0, BG_Y, bg.width, bg.height, &bg.rgba);
        }
        self.column.draw(&mut fb, assets);
        if let Some(a) = assets {
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
                let s = &a.strings;
                let p = &self.profile;
                let d = usize::from(p.difficulty.min(2));
                let difficulty = text(s, t::DIFFICULTY_NAMES + d, ["easy", "medium", "hard"][d]);
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
        }
        if let Some(c) = &self.confirm {
            c.draw(&mut fb, assets);
        }
        let pointer = self
            .confirm
            .as_ref()
            .map_or(self.column.pointer, |c| c.pointer);
        draw_pointer(&mut fb, pointer, assets.and_then(|a| a.cursor.as_ref()));
        fb
    }
}

/// The pause menu over the green-tinted, paused mission (`ui-flow.md` 9.5).
#[derive(Debug)]
pub struct PauseMenu {
    column: ButtonColumn,
    confirm: Option<Confirm>,
    strings: Vec<String>,
    objective: String,
}

impl PauseMenu {
    /// New pause menu showing the current objective.
    #[must_use]
    pub fn new(objective: String, strings: &[String]) -> Self {
        Self {
            column: ButtonColumn::new(&MenuAction::PAUSE, 1, strings),
            confirm: None,
            strings: strings.to_vec(),
            objective,
        }
    }

    /// Apply input; `Quit` is returned only after confirmation, Escape continues.
    pub fn handle(&mut self, event: InputEvent) -> Option<MenuAction> {
        if let Some(c) = self.confirm.as_mut() {
            return match c.handle(event) {
                Some(true) => {
                    self.confirm = None;
                    Some(MenuAction::Quit)
                }
                Some(false) => {
                    self.confirm = None;
                    None
                }
                None => None,
            };
        }
        let chosen = match event {
            InputEvent::KeyDown { key: Key::Escape } => Some(MenuAction::Continue),
            e => self.column.handle(e),
        };
        match chosen {
            Some(MenuAction::Quit) => {
                self.confirm = Some(Confirm::new(t::CONFIRM_LEAVE, "leave?", &self.strings));
                None
            }
            other => other,
        }
    }

    /// Current state for `observe`.
    #[must_use]
    pub fn state(&self) -> MenuState {
        if let Some(c) = &self.confirm {
            return c.state();
        }
        MenuState {
            screen: "pause_menu".into(),
            items: items_to_protocol(&self.column.items),
            hovered: self.column.hovered,
            page: None,
        }
    }

    /// Draw over the (already rendered) scene.
    pub fn render(&self, scene: &mut Framebuffer, assets: Option<&UiAssets>) {
        tint_green(scene);
        if let Some(font) = assets.and_then(|a| a.font_objective.as_ref().or(a.font_text.as_ref()))
        {
            font.draw(scene, &self.objective, 210, 150 - font.height() as i32 / 2);
        }
        self.column.draw(scene, assets);
        if let Some(c) = &self.confirm {
            c.draw(scene, assets);
        }
        let pointer = self
            .confirm
            .as_ref()
            .map_or(self.column.pointer, |c| c.pointer);
        draw_pointer(scene, pointer, assets.and_then(|a| a.cursor.as_ref()));
    }
}

/// The credits: the text strip scrolls up over the dark forest (`ui-flow.md` 8) at about 20 px/s;
/// Escape, Enter or a click returns to the main menu.
#[derive(Debug)]
pub struct Credits {
    /// Scroll position in pixels times `tick_rate` (exact integer accumulation; 0 = the strip's top edge
    /// at the bottom of the frame).
    offset_num: i64,
    /// Ticks per second, to turn the observed speed into a per-tick step.
    tick_rate: u32,
}

impl Credits {
    /// Observed scroll speed in pixels per second.
    pub const SPEED_PX_PER_S: i64 = 20;

    /// New credits screen.
    #[must_use]
    pub fn new(tick_rate: u32) -> Self {
        Self {
            offset_num: 0,
            tick_rate: tick_rate.max(1),
        }
    }

    /// Advance one tick.
    pub fn tick(&mut self) {
        self.offset_num += Self::SPEED_PX_PER_S;
    }

    /// Whether an input event leaves the screen.
    #[must_use]
    pub fn leaves(event: InputEvent) -> bool {
        matches!(
            event,
            InputEvent::KeyDown {
                key: Key::Escape | Key::Enter | Key::Space
            } | InputEvent::PointerDown { .. }
        )
    }

    /// Current scroll position in pixels.
    #[must_use]
    pub fn offset(&self) -> i32 {
        (self.offset_num / i64::from(self.tick_rate)) as i32
    }

    /// State for `observe`.
    #[must_use]
    pub fn state(&self) -> MenuState {
        MenuState {
            screen: "credits".into(),
            items: Vec::new(),
            hovered: None,
            page: Some([self.offset().max(0) as usize, 0]),
        }
    }

    /// Render the frame.
    #[must_use]
    pub fn render(&self, assets: Option<&UiAssets>) -> Framebuffer {
        let mut fb = Framebuffer::new(MENU_FRAME.0, MENU_FRAME.1);
        fb.clear([0, 0, 0, 255]);
        let Some(a) = assets else {
            fb.fill_rect(
                312,
                768 - self.offset(),
                712,
                768 - self.offset() + 400,
                [200, 200, 200, 255],
            );
            return fb;
        };
        if let Some(bg) = &a.credits_background {
            fb.blit_rgba(0, 0, bg.width, bg.height, &bg.rgba);
        }
        if let Some(strip) = &a.credits_strip {
            let x = (MENU_FRAME.0 as i32 - strip.width as i32) / 2;
            fb.blit_rgba(
                x,
                MENU_FRAME.1 as i32 - self.offset(),
                strip.width,
                strip.height,
                &strip.rgba,
            );
        }
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
            } => next = hit(SEAL_BRIEFING, self.pointer.0, self.pointer.1),
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
            items: items_to_protocol(&[MenuItem {
                action: MenuAction::Yes,
                label: "ok".into(),
                rect: SEAL_BRIEFING,
                enabled: true,
            }]),
            hovered: None,
            page: Some([self.page + 1, self.pages.len().max(1)]),
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
                SEAL_BRIEFING.0,
                SEAL_BRIEFING.1,
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

/// In-mission HUD values.
#[derive(Debug, Clone, Default)]
pub struct HudState {
    /// Campaign money.
    pub money: u32,
    /// Clover charms.
    pub clover: u32,
    /// Selected hero's name lines.
    pub hero_name: Vec<String>,
}

/// Draw the HUD over the scene (`ui-flow.md` 9.3). Positions matched by eye to the original's captures;
/// the portrait frame picture is not identified yet, so a small scroll stands in for it.
pub fn draw_hud(scene: &mut Framebuffer, assets: &UiAssets, hud: &HudState) {
    let h = &assets.hud;
    for (pic, x, y) in &h.foliage {
        scene.blit_rgba(*x, *y, pic.width, pic.height, &pic.rgba);
    }
    if let Some(p) = &h.eyes {
        scene.blit_rgba(950, 0, p.width, p.height, &p.rgba);
    }
    if let Some(p) = &h.towers {
        scene.blit_rgba(998, 8, p.width, p.height, &p.rgba);
    }
    if let Some(p) = &h.map_scroll {
        scene.blit_rgba(935, 40, p.width, p.height, &p.rgba);
    }
    if let Some(p) = &h.stand {
        scene.blit_rgba(5, 690, p.width, p.height, &p.rgba);
    }
    if let Some(p) = &h.plan {
        scene.blit_rgba(950, 700, p.width, p.height, &p.rgba);
    }
    if let Some(p) = &h.portrait_scroll {
        scene.blit_rgba(70, 640, p.width, p.height, &p.rgba);
    }
    if let Some(p) = &h.portrait {
        scene.blit_rgba(88, 660, p.width, p.height, &p.rgba);
    }
    if let Some(font) = &assets.font_text {
        let s = &assets.strings;
        let money = text(s, t::MONEY_FORMAT, "money: %i").replace("%i", &hud.money.to_string());
        let clover = text(s, t::CLOVER_FORMAT, "clover: %i").replace("%i", &hud.clover.to_string());
        font.draw(scene, &money, 4, 4);
        font.draw(scene, &clover, 4, 20);
        for (i, line) in hud.hero_name.iter().enumerate() {
            font.draw(scene, line, 140, 665 + 18 * i as i32);
        }
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

    fn mv(x: i32, y: i32) -> InputEvent {
        InputEvent::PointerMove {
            x256: x * 256,
            y256: y * 256,
        }
    }

    fn click() -> InputEvent {
        InputEvent::PointerDown {
            button: Button::Left,
        }
    }

    #[test]
    fn menu_rows_match_the_original_column() {
        let m = MainMenu::new(ProfileSummary::default(), &[]);
        assert_eq!(m.column.items.len(), 7);
        assert_eq!(m.column.items[0].label, "play");
        assert_eq!(m.column.items[0].rect, (664, 339, 168, 39));
        assert_eq!(m.column.items[6].rect, (664, 585, 168, 39));
        assert_eq!(m.column.items[6].action, MenuAction::Exit);
        let named = MainMenu::new(ProfileSummary::default(), &["Go".to_string()]);
        assert_eq!(named.column.items[0].label, "Go");
        let p = PauseMenu::new(String::new(), &[]);
        assert_eq!(p.column.items[0].rect.1, 380);
        assert_eq!(p.column.items[5].action, MenuAction::Quit);
    }

    #[test]
    fn click_on_play_starts_and_keyboard_navigates() {
        let mut m = MainMenu::new(ProfileSummary::default(), &[]);
        assert_eq!(m.handle(mv(748, 364)), None);
        assert_eq!(m.column.hovered, Some(0));
        assert_eq!(m.handle(click()), Some(MenuAction::Play));
        let mut m = MainMenu::new(ProfileSummary::default(), &[]);
        m.handle(InputEvent::KeyDown { key: Key::Up });
        assert_eq!(m.column.hovered, Some(6));
        // Exit asks for confirmation: Enter on the dialog confirms.
        assert_eq!(m.handle(InputEvent::KeyDown { key: Key::Enter }), None);
        assert_eq!(m.state().screen, "dialog");
        assert_eq!(
            m.handle(InputEvent::KeyDown { key: Key::Enter }),
            Some(MenuAction::Exit)
        );
        // Between two plates nothing is hovered.
        let mut m = MainMenu::new(ProfileSummary::default(), &[]);
        m.handle(mv(748, 379));
        assert_eq!(m.column.hovered, None);
    }

    #[test]
    fn pause_menu_quits_only_after_confirmation() {
        let mut p = PauseMenu::new("goal".into(), &[]);
        p.handle(mv(748, 600));
        assert_eq!(p.column.hovered, Some(5));
        assert_eq!(p.handle(click()), None);
        assert_eq!(p.state().screen, "dialog");
        // The red X cancels.
        p.handle(mv(541, 433));
        assert_eq!(p.handle(click()), None);
        assert_eq!(p.state().screen, "pause_menu");
        // Escape resumes.
        assert_eq!(
            p.handle(InputEvent::KeyDown { key: Key::Escape }),
            Some(MenuAction::Continue)
        );
        // Quit via the blue V.
        p.handle(mv(748, 600));
        p.handle(click());
        p.handle(mv(483, 433));
        assert_eq!(p.handle(click()), Some(MenuAction::Quit));
    }

    #[test]
    fn credits_scroll_at_the_observed_speed_and_leave_on_escape() {
        let mut c = Credits::new(60);
        for _ in 0..60 {
            c.tick();
        }
        assert_eq!(c.offset(), 20);
        assert!(!Credits::leaves(mv(1, 1)));
        assert!(Credits::leaves(InputEvent::KeyDown { key: Key::Escape }));
        let fb = c.render(None);
        assert_eq!((fb.width, fb.height), MENU_FRAME);
    }

    #[test]
    fn briefing_pages_advance_with_enter_and_seal() {
        let mut b = Briefing::new(vec!["one".into(), "two".into()]);
        assert!(!b.handle(InputEvent::KeyDown { key: Key::Enter }));
        assert_eq!(b.page, 1);
        b.handle(mv(508, 552));
        assert!(b.handle(click()));
        let fb = MainMenu::new(ProfileSummary::default(), &[]).render(None);
        assert_eq!((fb.width, fb.height), MENU_FRAME);
    }
}
