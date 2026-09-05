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
    /// Delete the selected save.
    Delete,
    /// Cancel / back.
    Cancel,
    /// Options sub-screens and their buttons.
    Graphics,
    Sounds,
    Shortcuts,
    Back,
    Ok,
    /// Shortcut sets (display only).
    DefaultSet1,
    DefaultSet2,
    UserSet,
    /// Select player screen.
    Select,
    New,
    Rename,
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
            MenuAction::Yes | MenuAction::Ok => 15,
            MenuAction::No | MenuAction::Cancel => 16,
            MenuAction::Delete => 8,
            MenuAction::Graphics => 18,
            MenuAction::Sounds => 19,
            MenuAction::Shortcuts => 20,
            MenuAction::Back => 17,
            MenuAction::DefaultSet1 => 21,
            MenuAction::DefaultSet2 => 22,
            MenuAction::UserSet => 23,
            MenuAction::Select => 5,
            MenuAction::New => 6,
            MenuAction::Rename => 7,
        }
    }

    /// Entries that do something in this build; the others are drawn on the disabled plate.
    fn implemented(self) -> bool {
        !matches!(self, MenuAction::ShowMovies)
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
            MenuAction::Delete => "delete",
            MenuAction::Cancel => "cancel",
            MenuAction::Graphics => "graphics",
            MenuAction::Sounds => "sounds",
            MenuAction::Shortcuts => "shortcuts",
            MenuAction::Back => "back",
            MenuAction::Ok => "ok",
            MenuAction::DefaultSet1 => "default set 1",
            MenuAction::DefaultSet2 => "default set 2",
            MenuAction::UserSet => "user defined",
            MenuAction::Select => "select",
            MenuAction::New => "new",
            MenuAction::Rename => "rename",
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
            selected: false,
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
    /// Gold load seal (`BTTN` 277, 44x46) and restart seal (`BTTN` 278) of the lost page.
    pub seal_load: Vec<SpriteFrame>,
    pub seal_restart: Vec<SpriteFrame>,
    /// Arrow cursor (`PIC` 284).
    pub cursor: Option<SpriteFrame>,
    /// Credits background (`PIC` 309, 1024x768) and text strip (`PIC` 308, 400x7659).
    pub credits_background: Option<SpriteFrame>,
    pub credits_strip: Option<SpriteFrame>,
    /// Dungeon background of the load / save screens (`PIC` 189, 1024x512).
    pub dungeon_background: Option<SpriteFrame>,
    /// Forest background of the options screen (`PIC` 186) and the sunlit forest of the graphics /
    /// sound options (`PIC` 188).
    pub forest_background: Option<SpriteFrame>,
    pub sunlit_background: Option<SpriteFrame>,
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
    /// Towers, zoom (`BTTN` 4 above `BTTN` 5).
    pub towers: Option<SpriteFrame>,
    pub towers_far: Option<SpriteFrame>,
    /// Standing figure (`BTTN` 3) above the kneeling one (`BTTN` 2).
    pub stand: Option<SpriteFrame>,
    pub kneel: Option<SpriteFrame>,
    /// Plan scroll (`BTTN` 1).
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

/// Profile summary shown left of the buttons; also the persisted profile record (`profiles.json`
/// under the artifact directory, a modern replacement for the original's `Profiles` file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct ProfileSummary {
    /// Name.
    pub name: String,
    /// Difficulty: 0 easy, 1 medium, 2 hard.
    pub difficulty: u8,
    /// Money (signed: the scripts' money is an `i32`).
    pub money: i32,
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

/// The select player screen (`ui-flow.md` 5): ten rows at x = 227..639, y = 225 + 41 k, 23 px high, with
/// the profile name at x = 236 and `<difficulty> / <progress> %` right-aligned to x = 628; the selected row
/// is orange. Buttons Select (row 3) / New (row 4) / Rename (row 5) / Delete (row 6). New and Rename open
/// the vertical parchment with a name field and three difficulty seals.
#[derive(Debug)]
pub struct SelectPlayerScreen {
    column: ButtonColumn,
    /// Profiles in stored order.
    pub profiles: Vec<ProfileSummary>,
    /// Selected row.
    pub selected: Option<usize>,
    /// The parchment editor (new or rename), if open.
    pub editor: Option<ProfileEditor>,
    strings: Vec<String>,
    pointer: (i32, i32),
}

/// The new / rename parchment.
#[derive(Debug, Clone)]
pub struct ProfileEditor {
    /// Editing an existing row, or creating a new profile.
    pub rename_of: Option<usize>,
    /// Name being typed.
    pub name: String,
    /// Difficulty 0..=2.
    pub difficulty: u8,
}

pub const PROFILE_ROWS: usize = 10;
/// Longest profile name (the editor's limit).
pub const PROFILE_NAME_MAX: usize = 16;
const PROFILE_ROW_Y0: i32 = 225;
const PROFILE_ROW_PITCH: i32 = 41;
const PROFILE_ROW_H: i32 = 23;
const PROFILE_LIST_X: (i32, i32) = (227, 639);
/// Difficulty seals of the parchment (centres, 41x44 plates).
const SEAL_EASY: (i32, i32, i32, i32) = (424, 406, 41, 44);
const SEAL_MEDIUM: (i32, i32, i32, i32) = (484, 406, 41, 44);
const SEAL_HARD: (i32, i32, i32, i32) = (560, 406, 41, 44);
const SEAL_EDIT_OK: (i32, i32, i32, i32) = (460, 520, 41, 44);
const SEAL_EDIT_CANCEL: (i32, i32, i32, i32) = (520, 520, 41, 44);
/// Name field of the parchment.
const PROFILE_NAME_FIELD: (i32, i32, i32, i32) = (316, 290, 400, 22);

/// Outcome of the select player screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectOutcome {
    /// Use the profile at this index.
    Select(usize),
    /// The profile list changed (create / rename / delete): persist it.
    Changed,
    /// Leave (Escape does not leave this screen in the original; `Select` is the way out, but a
    /// harness or a player with no profiles needs an exit, so Escape leaves when nothing is edited).
    Leave,
}

impl SelectPlayerScreen {
    /// New screen over the stored profiles, with `selected` preselected.
    #[must_use]
    pub fn new(profiles: Vec<ProfileSummary>, selected: Option<usize>, strings: &[String]) -> Self {
        Self {
            column: ButtonColumn::new(
                &[
                    MenuAction::Select,
                    MenuAction::New,
                    MenuAction::Rename,
                    MenuAction::Delete,
                ],
                3,
                strings,
            ),
            profiles,
            selected,
            editor: None,
            strings: strings.to_vec(),
            pointer: (0, 0),
        }
    }

    fn row_rect(i: usize) -> (i32, i32, i32, i32) {
        (
            PROFILE_LIST_X.0,
            PROFILE_ROW_Y0 + i as i32 * PROFILE_ROW_PITCH,
            PROFILE_LIST_X.1 - PROFILE_LIST_X.0,
            PROFILE_ROW_H,
        )
    }

    fn handle_editor(&mut self, event: InputEvent) -> Option<SelectOutcome> {
        let ed = self.editor.as_mut()?;
        match event {
            InputEvent::KeyDown {
                key: Key::Letter(c),
            } if ed.name.len() < 16 => ed.name.push(c),
            InputEvent::KeyDown { key: Key::Digit(d) } if ed.name.len() < 16 => {
                ed.name.push(char::from(b'0' + d.min(9)));
            }
            InputEvent::KeyDown {
                key: Key::Backspace,
            } => {
                ed.name.pop();
            }
            // An inline rename (the row turned into an edit field, `ui-flow.md` 5) ends with
            // Enter (commit) or Escape (cancel); which key the original commits with is not
            // captured, so Enter is the engine's choice. The New player parchment ends only through
            // its seals, as observed (`ui-flow.md` 2.2).
            InputEvent::KeyDown { key: Key::Enter } if ed.rename_of.is_some() => {
                let ed = self.editor.take()?;
                let name: String = ed.name.trim().chars().take(PROFILE_NAME_MAX).collect();
                if name.is_empty() {
                    return None;
                }
                if let Some(p) = ed.rename_of.and_then(|i| self.profiles.get_mut(i)) {
                    p.name = name;
                }
                return Some(SelectOutcome::Changed);
            }
            InputEvent::KeyDown { key: Key::Escape } if ed.rename_of.is_some() => {
                self.editor = None;
            }
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } if ed.rename_of.is_none() => {
                let (px, py) = self.pointer;
                if hit(SEAL_EASY, px, py) {
                    ed.difficulty = 0;
                } else if hit(SEAL_MEDIUM, px, py) {
                    ed.difficulty = 1;
                } else if hit(SEAL_HARD, px, py) {
                    ed.difficulty = 2;
                } else if hit(SEAL_EDIT_CANCEL, px, py) {
                    self.editor = None;
                } else if hit(SEAL_EDIT_OK, px, py) {
                    let name: String = ed.name.trim().chars().take(16).collect();
                    if name.is_empty() {
                        return None;
                    }
                    let ed = self.editor.take()?;
                    match ed.rename_of {
                        Some(i) => {
                            if let Some(p) = self.profiles.get_mut(i) {
                                p.name = name;
                                p.difficulty = ed.difficulty;
                            }
                        }
                        None => {
                            if self.profiles.len() < PROFILE_ROWS {
                                self.profiles.push(ProfileSummary {
                                    name,
                                    difficulty: ed.difficulty,
                                    ..ProfileSummary::default()
                                });
                                self.selected = Some(self.profiles.len() - 1);
                            }
                        }
                    }
                    return Some(SelectOutcome::Changed);
                }
            }
            _ => {}
        }
        None
    }

    /// Apply input.
    pub fn handle(&mut self, event: InputEvent) -> Option<SelectOutcome> {
        if self.editor.is_some() {
            return self.handle_editor(event);
        }
        match event {
            InputEvent::KeyDown { key: Key::Escape } => return Some(SelectOutcome::Leave),
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } => {
                let (px, py) = self.pointer;
                if let Some(i) = (0..self.profiles.len().min(PROFILE_ROWS))
                    .find(|&i| hit(Self::row_rect(i), px, py))
                {
                    self.selected = Some(i);
                }
            }
            _ => {}
        }
        let chosen = self.column.handle(event)?;
        match chosen {
            MenuAction::Select => self.selected.map(SelectOutcome::Select),
            MenuAction::New => {
                if self.profiles.len() < PROFILE_ROWS {
                    self.editor = Some(ProfileEditor {
                        rename_of: None,
                        name: String::new(),
                        difficulty: 1,
                    });
                }
                None
            }
            MenuAction::Rename => {
                let i = self.selected?;
                let p = self.profiles.get(i)?;
                self.editor = Some(ProfileEditor {
                    rename_of: Some(i),
                    name: p.name.clone(),
                    difficulty: p.difficulty,
                });
                None
            }
            MenuAction::Delete => {
                let i = self.selected?;
                if i < self.profiles.len() {
                    self.profiles.remove(i);
                    self.selected = if self.profiles.is_empty() {
                        None
                    } else {
                        Some(i.min(self.profiles.len() - 1))
                    };
                    return Some(SelectOutcome::Changed);
                }
                None
            }
            _ => None,
        }
    }

    /// State for `observe`: buttons, rows (`row:<name>`), and the editor's seals when open.
    #[must_use]
    pub fn state(&self) -> MenuState {
        let mut items = items_to_protocol(&self.column.items);
        let renaming = self.editor.as_ref().and_then(|e| e.rename_of);
        for (i, p) in self.profiles.iter().take(PROFILE_ROWS).enumerate() {
            let r = Self::row_rect(i);
            // Rows are identified by index (names are not unique); every row reacts to a click.
            items.push(UiItem {
                action: format!("row:{i}"),
                label: match &self.editor {
                    Some(ed) if renaming == Some(i) => ed.name.clone(),
                    _ => p.name.clone(),
                },
                rect: [r.0, r.1, r.2, r.3],
                enabled: true,
                selected: self.selected == Some(i),
            });
        }
        if let Some(ed) = self.editor.as_ref().filter(|e| e.rename_of.is_none()) {
            for (name, r) in [
                ("easy", SEAL_EASY),
                ("medium", SEAL_MEDIUM),
                ("hard", SEAL_HARD),
                ("yes", SEAL_EDIT_OK),
                ("no", SEAL_EDIT_CANCEL),
            ] {
                items.push(UiItem {
                    action: name.into(),
                    label: ed.name.clone(),
                    rect: [r.0, r.1, r.2, r.3],
                    enabled: true,
                    selected: false,
                });
            }
        }
        // `hovered` is the pointer's element in the complete item array: a button of the column
        // or, after them, a row.
        let buttons = self.column.items.len();
        let (px, py) = self.pointer;
        let hovered = (0..self.profiles.len().min(PROFILE_ROWS))
            .find(|&i| hit(Self::row_rect(i), px, py))
            .map(|i| buttons + i)
            .or_else(|| self.column.items.iter().position(|it| hit(it.rect, px, py)));
        MenuState {
            screen: match &self.editor {
                Some(ed) if ed.rename_of.is_some() => "rename_player".into(),
                Some(_) => "new_player".into(),
                None => "select_player".into(),
            },
            items,
            hovered,
            page: None,
        }
    }

    /// Render the frame.
    #[must_use]
    pub fn render(&self, assets: Option<&UiAssets>) -> Framebuffer {
        let mut fb = Framebuffer::new(MENU_FRAME.0, MENU_FRAME.1);
        fb.clear([0, 0, 0, 255]);
        // The forest background (`PIC` 186) as observed for Select player (`ui-flow.md` 1).
        if let Some(bg) = assets.and_then(|a| a.forest_background.as_ref()) {
            fb.blit_rgba(0, BG_Y, bg.width, bg.height, &bg.rgba);
        }
        let font = assets.and_then(|a| a.font_text.as_ref());
        let s = &self.strings;
        let renaming = self.editor.as_ref().and_then(|e| e.rename_of);
        for (i, p) in self.profiles.iter().take(PROFILE_ROWS).enumerate() {
            let (x, y, w, h) = Self::row_rect(i);
            let colour = if self.selected == Some(i) {
                ORANGE
            } else {
                [90, 90, 90, 255]
            };
            fb.fill_rect(x, y, x + w, y + h, colour);
            if let Some(f) = font {
                // The row being renamed is an edit field with a caret.
                let shown = match &self.editor {
                    Some(ed) if renaming == Some(i) => format!("{}_", ed.name),
                    _ => p.name.clone(),
                };
                f.draw(&mut fb, &shown, 236, y + 4);
                let d = usize::from(p.difficulty.min(2));
                let right = format!(
                    "{} / {} %",
                    text(s, t::DIFFICULTY_NAMES + d, ["easy", "medium", "hard"][d]),
                    p.progress
                );
                let tw = f.measure(&right);
                f.draw(&mut fb, &right, 628 - tw, y + 4);
            }
        }
        self.column.draw(&mut fb, assets);
        if let (Some(ed), Some(a)) = (
            self.editor.as_ref().filter(|e| e.rename_of.is_none()),
            assets,
        ) {
            if let Some(p) = &a.parchment {
                fb.blit_rgba(264, 148, p.width, p.height, &p.rgba);
            }
            if let Some(f) = a.font_title.as_ref() {
                f.draw_centered(&mut fb, text(s, 30, "new player"), 512, 200);
            }
            if let Some(f) = a.font_debrief.as_ref().or(font) {
                f.draw(&mut fb, text(s, 61, "name"), 316, 262);
                let (x, y, w, h) = PROFILE_NAME_FIELD;
                fb.fill_rect(x, y, x + w, y + h, [60, 40, 20, 255]);
                fb.fill_rect(x + 1, y + 1, x + w - 1, y + h - 1, [240, 225, 190, 255]);
                f.draw(&mut fb, &format!("{}_", ed.name), x + 6, y + 3);
                f.draw(&mut fb, text(s, t::DIFFICULTY, "difficulty"), 316, 370);
                for (d, r) in [SEAL_EASY, SEAL_MEDIUM, SEAL_HARD].into_iter().enumerate() {
                    let seal = if ed.difficulty == d as u8 {
                        a.seal_ok.get(2).or(a.seal_ok.first())
                    } else {
                        a.seal_ok.first()
                    };
                    if let Some(sl) = seal {
                        fb.blit_rgba(r.0, r.1, sl.width, sl.height, &sl.rgba);
                    }
                    f.draw_centered(
                        &mut fb,
                        text(s, t::DIFFICULTY_NAMES + d, ["easy", "medium", "hard"][d]),
                        r.0 + r.2 / 2,
                        r.1 + r.3 + 2,
                    );
                }
                if let Some(sl) = a.seal_ok.get(1).or(a.seal_ok.first()) {
                    fb.blit_rgba(
                        SEAL_EDIT_OK.0,
                        SEAL_EDIT_OK.1,
                        sl.width,
                        sl.height,
                        &sl.rgba,
                    );
                }
                if let Some(sl) = a.seal_cancel.get(1).or(a.seal_cancel.first()) {
                    fb.blit_rgba(
                        SEAL_EDIT_CANCEL.0,
                        SEAL_EDIT_CANCEL.1,
                        sl.width,
                        sl.height,
                        &sl.rgba,
                    );
                }
            }
        }
        draw_pointer(
            &mut fb,
            self.pointer,
            assets.and_then(|a| a.cursor.as_ref()),
        );
        fb
    }
}

/// Player settings (modern additions kept in the session's artifact directory; the original stores
/// them in the profile and `Configuration/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Settings {
    /// Document format of `settings.json` (1).
    #[serde(default = "settings_format")]
    pub format: u32,
    /// Aspect ratio choice: 0 = 4:3, 1 = 16:9, 2 = 16:10 (the patched build's graphics options).
    pub aspect: u8,
    /// The four effect toggles (alpha view cones, transparent shadows, effect animations,
    /// background animations).
    pub effects: [bool; 4],
    /// Sound mode 0 stereo / 1 three-dimensional (unavailable), quality 0 high / 1 low.
    pub sound_mode: u8,
    pub sound_quality: u8,
    /// Volumes 0..=10: effects, dialogue, music, comments; comment frequency 0..=10.
    pub volumes: [u8; 4],
    pub comment_frequency: u8,
    /// Shortcut set: 0 default 1, 1 default 2, 2 user defined (display only).
    pub shortcut_set: u8,
}

fn settings_format() -> u32 {
    1
}

impl Settings {
    /// Every field within its documented range (a read document is clamped, never trusted).
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.format = 1;
        self.aspect = self.aspect.min(2);
        self.sound_mode = self.sound_mode.min(1);
        self.sound_quality = self.sound_quality.min(1);
        for v in &mut self.volumes {
            *v = (*v).min(10);
        }
        self.comment_frequency = self.comment_frequency.min(10);
        self.shortcut_set = self.shortcut_set.min(2);
        self
    }
}

impl ProfileSummary {
    /// A read profile within the documented ranges, or `None` when its name is unusable.
    #[must_use]
    pub fn sanitized(mut self) -> Option<Self> {
        let name: String = self
            .name
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(PROFILE_NAME_MAX)
            .collect();
        if name.is_empty() {
            return None;
        }
        self.name = name;
        self.difficulty = self.difficulty.min(2);
        self.spared_lives = self.spared_lives.min(100);
        self.progress = self.progress.min(100);
        self.game_length = self.game_length.chars().take(32).collect();
        Some(self)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: 1,
            aspect: 0,
            effects: [true; 4],
            sound_mode: 0,
            sound_quality: 0,
            volumes: [10; 4],
            comment_frequency: 6,
            shortcut_set: 0,
        }
    }
}

/// Which options screen is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsPage {
    /// The options menu (graphics / sounds / shortcuts / back).
    Main,
    /// Graphical options.
    Graphics,
    /// Sound options.
    Sounds,
    /// Shortcuts table.
    Shortcuts,
}

/// Outcome of the options screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionsOutcome {
    /// Leave the options (back to the caller).
    Back,
    /// Apply the edited settings.
    Apply(Settings),
}

/// An option bar: rectangle, label string index, selected.
type Bar = ((i32, i32, i32, i32), usize, bool);

/// Bar geometry of the options pages (`ui-flow.md` 4.1 / 4.2): x = 227..640, 26 px high.
const OPT_BAR_X: i32 = 227;
const OPT_BAR_W: i32 = 413;
const OPT_BAR_H: i32 = 26;
/// Slider cells: 10 cells of 27 px at x = 226 + 42 * i, 14 px high.
const SLIDER_X: i32 = 226;
const SLIDER_PITCH: i32 = 42;
const SLIDER_CELL_W: i32 = 27;
const SLIDER_H: i32 = 14;
const ORANGE: [u8; 4] = [214, 120, 30, 255];
const TEAL: [u8; 4] = [40, 110, 110, 255];
const TEAL_DIM: [u8; 4] = [60, 80, 80, 255];

/// The options screens (`ui-flow.md` 4): a button column per page, option bars and sliders edited in
/// place; OK applies, Cancel / Escape discards the page's edits.
#[derive(Debug)]
pub struct OptionsScreen {
    /// Current page.
    pub page: OptionsPage,
    column: ButtonColumn,
    /// Settings as applied so far.
    pub applied: Settings,
    /// Settings being edited on the current page.
    pub edit: Settings,
    strings: Vec<String>,
    pointer: (i32, i32),
}

impl OptionsScreen {
    /// New options screen over the current settings.
    #[must_use]
    pub fn new(settings: Settings, strings: &[String]) -> Self {
        let mut s = Self {
            page: OptionsPage::Main,
            column: ButtonColumn::new(&[], 0, strings),
            applied: settings.clone(),
            edit: settings,
            strings: strings.to_vec(),
            pointer: (0, 0),
        };
        s.open(OptionsPage::Main);
        s
    }

    fn open(&mut self, page: OptionsPage) {
        self.page = page;
        self.edit = self.applied.clone();
        self.column = match page {
            OptionsPage::Main => ButtonColumn::new(
                &[
                    MenuAction::Graphics,
                    MenuAction::Sounds,
                    MenuAction::Shortcuts,
                    MenuAction::Back,
                ],
                3,
                &self.strings,
            ),
            OptionsPage::Graphics | OptionsPage::Sounds => {
                ButtonColumn::new(&[MenuAction::Ok, MenuAction::Cancel], 5, &self.strings)
            }
            OptionsPage::Shortcuts => ButtonColumn::new(
                &[
                    MenuAction::Ok,
                    MenuAction::DefaultSet1,
                    MenuAction::DefaultSet2,
                    MenuAction::UserSet,
                    MenuAction::Cancel,
                ],
                2,
                &self.strings,
            ),
        };
    }

    /// Option bars of the current page: (rect, label string index, selected).
    fn bars(&self) -> Vec<Bar> {
        let bar = |y: i32| (OPT_BAR_X, y, OPT_BAR_W, OPT_BAR_H);
        match self.page {
            OptionsPage::Graphics => {
                let mut v = vec![
                    (bar(249), 43, self.edit.aspect == 0),
                    (bar(290), 44, self.edit.aspect == 1),
                    (bar(331), 45, self.edit.aspect == 2),
                ];
                for (i, y) in [400, 441, 482, 523].into_iter().enumerate() {
                    v.push((bar(y), 47 + i, self.edit.effects[i]));
                }
                v
            }
            OptionsPage::Sounds => vec![
                (bar(220), 51, self.edit.sound_mode == 0),
                (bar(261), 53, self.edit.sound_mode == 1),
                (bar(320), 54, self.edit.sound_quality == 0),
                (bar(361), 55, self.edit.sound_quality == 1),
            ],
            _ => Vec::new(),
        }
    }

    /// Sliders of the sound page: (track y, label string index, value).
    fn sliders(&self) -> Vec<(i32, usize, u8)> {
        if self.page != OptionsPage::Sounds {
            return Vec::new();
        }
        vec![
            (433, 56, self.edit.volumes[0]),
            (473, 57, self.edit.volumes[1]),
            (513, 58, self.edit.volumes[2]),
            (553, 59, self.edit.volumes[3]),
            (593, 60, self.edit.comment_frequency),
        ]
    }

    fn click_bar(&mut self, index: usize) {
        match self.page {
            OptionsPage::Graphics => match index {
                0..=2 => self.edit.aspect = index as u8,
                3..=6 => self.edit.effects[index - 3] = !self.edit.effects[index - 3],
                _ => {}
            },
            // Bar 1 (three-dimensional sound) is unavailable, greyed out as observed.
            OptionsPage::Sounds => match index {
                0 => self.edit.sound_mode = 0,
                2 => self.edit.sound_quality = 0,
                3 => self.edit.sound_quality = 1,
                _ => {}
            },
            _ => {}
        }
    }

    fn click_slider(&mut self, index: usize, cell: u8) {
        let v = cell.min(10);
        match index {
            0..=3 => self.edit.volumes[index] = v,
            4 => self.edit.comment_frequency = v,
            _ => {}
        }
    }

    /// Apply input.
    pub fn handle(&mut self, event: InputEvent) -> Option<OptionsOutcome> {
        match event {
            InputEvent::KeyDown { key: Key::Escape } => {
                // Escape leaves the graphics page (= Cancel) and the options menu; the sound and
                // shortcut pages need their buttons (`ui-flow.md` 2.2).
                return match self.page {
                    OptionsPage::Main => Some(OptionsOutcome::Back),
                    OptionsPage::Graphics => {
                        self.open(OptionsPage::Main);
                        None
                    }
                    _ => None,
                };
            }
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } => {
                let (px, py) = self.pointer;
                if let Some(i) = self.bars().iter().position(|(r, _, _)| hit(*r, px, py)) {
                    self.click_bar(i);
                    return None;
                }
                for (i, (y, _, _)) in self.sliders().iter().enumerate() {
                    // Cells 1..=10; the empty position left of the first cell is 0 (mute). Where
                    // the original puts its zero is not captured yet.
                    for cell in 0..=10 {
                        let r = (
                            SLIDER_X + SLIDER_PITCH * (cell - 1),
                            *y,
                            SLIDER_CELL_W,
                            SLIDER_H,
                        );
                        if hit(r, px, py) {
                            self.click_slider(i, cell as u8);
                            return None;
                        }
                    }
                }
            }
            _ => {}
        }
        let chosen = self.column.handle(event)?;
        match (self.page, chosen) {
            (OptionsPage::Main, MenuAction::Graphics) => self.open(OptionsPage::Graphics),
            (OptionsPage::Main, MenuAction::Sounds) => self.open(OptionsPage::Sounds),
            (OptionsPage::Main, MenuAction::Shortcuts) => self.open(OptionsPage::Shortcuts),
            (OptionsPage::Main, MenuAction::Back) => return Some(OptionsOutcome::Back),
            (_, MenuAction::Ok) => {
                self.applied = self.edit.clone();
                let applied = self.applied.clone();
                self.open(OptionsPage::Main);
                return Some(OptionsOutcome::Apply(applied));
            }
            (_, MenuAction::Cancel) => self.open(OptionsPage::Main),
            (OptionsPage::Shortcuts, MenuAction::DefaultSet1) => self.edit.shortcut_set = 0,
            (OptionsPage::Shortcuts, MenuAction::DefaultSet2) => self.edit.shortcut_set = 1,
            (OptionsPage::Shortcuts, MenuAction::UserSet) => self.edit.shortcut_set = 2,
            _ => {}
        }
        None
    }

    /// State for `observe`: the buttons, then the bars (`bar:<n>`, every one clickable, `selected` =
    /// on) and the sliders (`slider:<n>`).
    #[must_use]
    pub fn state(&self) -> MenuState {
        let mut items = items_to_protocol(&self.column.items);
        for (i, (r, label, on)) in self.bars().iter().enumerate() {
            items.push(UiItem {
                action: format!("bar:{i}"),
                label: text(&self.strings, *label, "option").to_string(),
                rect: [r.0, r.1, r.2, r.3],
                enabled: true,
                selected: *on,
            });
        }
        for (i, (y, label, value)) in self.sliders().iter().enumerate() {
            items.push(UiItem {
                action: format!("slider:{i}"),
                label: format!("{} {value}", text(&self.strings, *label, "volume")),
                rect: [SLIDER_X, *y, SLIDER_PITCH * 10, SLIDER_H],
                enabled: true,
                selected: false,
            });
        }
        MenuState {
            screen: match self.page {
                OptionsPage::Main => "options",
                OptionsPage::Graphics => "options_graphics",
                OptionsPage::Sounds => "options_sounds",
                OptionsPage::Shortcuts => "options_shortcuts",
            }
            .into(),
            items,
            hovered: self.column.hovered,
            page: None,
        }
    }

    /// Render the frame.
    #[must_use]
    pub fn render(&self, assets: Option<&UiAssets>) -> Framebuffer {
        let mut fb = Framebuffer::new(MENU_FRAME.0, MENU_FRAME.1);
        fb.clear([0, 0, 0, 255]);
        let bg = assets.and_then(|a| match self.page {
            OptionsPage::Main => a.forest_background.as_ref(),
            OptionsPage::Shortcuts => a.dungeon_background.as_ref(),
            _ => a.sunlit_background.as_ref(),
        });
        if let Some(bg) = bg {
            fb.blit_rgba(0, BG_Y, bg.width, bg.height, &bg.rgba);
        }
        let title_font = assets.and_then(|a| a.font_title.as_ref());
        let font = assets.and_then(|a| a.font_text.as_ref());
        let s = &self.strings;
        let title = match self.page {
            OptionsPage::Main => text(s, 27, "options"),
            OptionsPage::Graphics => text(s, 28, "graphics"),
            OptionsPage::Sounds => text(s, 29, "sounds"),
            OptionsPage::Shortcuts => text(s, 20, "shortcuts"),
        };
        if let Some(f) = title_font {
            f.draw_centered(&mut fb, title, 442, 158 - f.height() as i32 / 2);
        }
        if let Some(f) = font {
            match self.page {
                OptionsPage::Main => {
                    // The original prints the processor and memory here; the engine prints its own.
                    f.draw_centered(&mut fb, "OpenSherwood", 442, 254 - f.height() as i32 / 2);
                    f.draw_centered(
                        &mut fb,
                        concat!("v", env!("CARGO_PKG_VERSION")),
                        442,
                        274 - f.height() as i32 / 2,
                    );
                }
                OptionsPage::Graphics => {
                    f.draw(
                        &mut fb,
                        text(s, 42, "resolution"),
                        OPT_BAR_X,
                        233 - f.height() as i32,
                    );
                    f.draw(
                        &mut fb,
                        text(s, 46, "effects"),
                        OPT_BAR_X,
                        383 - f.height() as i32,
                    );
                }
                OptionsPage::Shortcuts => {
                    let mut y = 161;
                    for (i, (name, key)) in
                        shortcut_lines(self.edit.shortcut_set).iter().enumerate()
                    {
                        let _ = i;
                        f.draw(&mut fb, name, 226, y);
                        let w = f.measure(key);
                        f.draw(&mut fb, key, 590 - w, y);
                        y += 15;
                    }
                }
                OptionsPage::Sounds => {}
            }
            for (r, label, on) in self.bars() {
                fb.fill_rect(
                    r.0,
                    r.1,
                    r.0 + r.2,
                    r.1 + r.3,
                    if on { ORANGE } else { TEAL },
                );
                f.draw_centered(&mut fb, text(s, label, "option"), r.0 + r.2 / 2, r.1 + 5);
            }
            for (y, label, value) in self.sliders() {
                f.draw(
                    &mut fb,
                    text(s, label, "volume"),
                    SLIDER_X,
                    y - 9 - f.height() as i32 + 4,
                );
                for cell in 0..10 {
                    let x = SLIDER_X + SLIDER_PITCH * cell;
                    let filled = (cell as u8) < value;
                    fb.fill_rect(
                        x,
                        y,
                        x + SLIDER_CELL_W,
                        y + SLIDER_H,
                        if filled { ORANGE } else { TEAL_DIM },
                    );
                }
            }
        }
        self.column.draw(&mut fb, assets);
        draw_pointer(
            &mut fb,
            self.pointer,
            assets.and_then(|a| a.cursor.as_ref()),
        );
        fb
    }
}

/// The shortcut table by function (the action names are the engine's words; `ui-flow.md` 4.3).
fn shortcut_lines(set: u8) -> Vec<(String, String)> {
    let default2 = set == 1;
    let k = |a: &str, b: &str| if default2 { b } else { a }.to_string();
    vec![
        (
            "zoom in / out".to_string(),
            k("num + / num -", "num + / num -"),
        ),
        ("scroll".to_string(), "arrow keys".to_string()),
        ("minimap".to_string(), k(";", "num *")),
        ("select character 1..5".to_string(), k("1..5", "num 1..5")),
        ("select all / none".to_string(), k("q / d", "num 6 / num 0")),
        (
            "crouch / stand".to_string(),
            k("c / s", "page down / page up"),
        ),
        (
            "go behind (modifier)".to_string(),
            k("left shift", "right shift"),
        ),
        ("outlines".to_string(), "caps lock".to_string()),
        (
            "action 1 / 2 / 3".to_string(),
            k("g / h / j", "num 7 / 8 / 9"),
        ),
        (
            "move during action (modifier)".to_string(),
            k("left ctrl", "right ctrl"),
        ),
        ("save quick action".to_string(), k("a", "return")),
        ("start quick actions".to_string(), "space".to_string()),
        ("clear quick actions".to_string(), "backspace".to_string()),
        ("field of vision".to_string(), k("alt", "alt gr")),
        ("quick save / quick load".to_string(), "F1 / F5".to_string()),
    ]
}

/// One entry of the load / save list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SaveEntry {
    /// File name without extension (`quick`, `auto-0`, a typed name).
    pub name: String,
    /// World tick stored in the file.
    pub world_tick: u64,
}

/// The load / save screen (`ui-flow.md` 6): the dungeon background, a list of saves at x = 227..610 from
/// y = 170 (rows of `SAVE_ROW_H` px, the selected one orange), on the save screen a name field at the bottom
/// of the list, and the button column Load-or-Save (row 4) / Delete (row 5) / Cancel (row 6). Row geometry
/// of the list is the engine's (the original's rows were not captured); the column rows are the observed ones.
#[derive(Debug)]
pub struct SaveScreen {
    column: ButtonColumn,
    /// Whether this is the save screen (name field, Save button) or the load screen.
    pub saving: bool,
    /// Entries, newest first.
    pub entries: Vec<SaveEntry>,
    /// Selected entry.
    pub selected: Option<usize>,
    /// Name being typed (save screen).
    pub name: String,
    pointer: (i32, i32),
}

/// Height of a list row.
pub const SAVE_ROW_H: i32 = 30;
/// List rectangle.
const SAVE_LIST: (i32, i32, i32, i32) = (227, 170, 383, 360);
/// Name field rectangle (save screen).
const SAVE_NAME_FIELD: (i32, i32, i32, i32) = (227, 545, 383, 26);
/// Most rows that fit the list.
const SAVE_ROWS: usize = 12;

/// The outcome of the screen's input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    /// Load the named save.
    Load(String),
    /// Write a save with this name.
    Save(String),
    /// Delete the named save.
    Delete(String),
    /// Leave the screen.
    Cancel,
}

impl SaveScreen {
    /// New screen over `entries` (newest first); `default_name` fills the save field.
    #[must_use]
    pub fn new(
        saving: bool,
        entries: Vec<SaveEntry>,
        default_name: String,
        strings: &[String],
    ) -> Self {
        let first = if saving {
            MenuAction::Save
        } else {
            MenuAction::Load
        };
        Self {
            column: ButtonColumn::new(&[first, MenuAction::Delete, MenuAction::Cancel], 4, strings),
            saving,
            entries,
            selected: None,
            name: default_name,
            pointer: (0, 0),
        }
    }

    /// Apply input.
    pub fn handle(&mut self, event: InputEvent) -> Option<SaveOutcome> {
        match event {
            InputEvent::KeyDown { key: Key::Escape } => return Some(SaveOutcome::Cancel),
            InputEvent::KeyDown {
                key: Key::Letter(c),
            } if self.saving && self.name.len() < 24 => {
                self.name.push(c);
                return None;
            }
            InputEvent::KeyDown { key: Key::Digit(d) } if self.saving && self.name.len() < 24 => {
                self.name.push(char::from(b'0' + d.min(9)));
                return None;
            }
            InputEvent::KeyDown {
                key: Key::Backspace,
            } if self.saving => {
                self.name.pop();
                return None;
            }
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } if hit(SAVE_LIST, self.pointer.0, self.pointer.1) => {
                let row = ((self.pointer.1 - SAVE_LIST.1) / SAVE_ROW_H) as usize;
                if row < self.entries.len().min(SAVE_ROWS) {
                    self.selected = Some(row);
                    if self.saving {
                        self.name = self.entries[row].name.clone();
                    }
                }
            }
            _ => {}
        }
        let chosen = self.column.handle(event)?;
        let selected_name = self
            .selected
            .and_then(|i| self.entries.get(i))
            .map(|e| e.name.clone());
        match chosen {
            MenuAction::Cancel => Some(SaveOutcome::Cancel),
            MenuAction::Delete => selected_name.map(SaveOutcome::Delete),
            MenuAction::Load => selected_name.map(SaveOutcome::Load),
            MenuAction::Save => {
                let name: String = self
                    .name
                    .trim()
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                (!name.is_empty()).then_some(SaveOutcome::Save(name))
            }
            _ => None,
        }
    }

    /// State for `observe`: the column's buttons plus one item per list row (`action` = `load`).
    #[must_use]
    pub fn state(&self) -> MenuState {
        let mut items = items_to_protocol(&self.column.items);
        for (i, e) in self.entries.iter().take(SAVE_ROWS).enumerate() {
            items.push(UiItem {
                action: format!("row:{}", e.name),
                label: format!("{} (tick {})", e.name, e.world_tick),
                rect: [
                    SAVE_LIST.0,
                    SAVE_LIST.1 + i as i32 * SAVE_ROW_H,
                    SAVE_LIST.2,
                    SAVE_ROW_H,
                ],
                enabled: true,
                selected: false,
            });
        }
        MenuState {
            screen: if self.saving {
                "save".into()
            } else {
                "load".into()
            },
            items,
            hovered: self.selected,
            page: None,
        }
    }

    /// Render the frame.
    #[must_use]
    pub fn render(&self, assets: Option<&UiAssets>) -> Framebuffer {
        let mut fb = Framebuffer::new(MENU_FRAME.0, MENU_FRAME.1);
        fb.clear([0, 0, 0, 255]);
        if let Some(bg) = assets.and_then(|a| a.dungeon_background.as_ref()) {
            fb.blit_rgba(0, BG_Y, bg.width, bg.height, &bg.rgba);
        }
        let font = assets.and_then(|a| a.font_text.as_ref());
        for (i, e) in self.entries.iter().take(SAVE_ROWS).enumerate() {
            let y = SAVE_LIST.1 + i as i32 * SAVE_ROW_H;
            let colour = if self.selected == Some(i) {
                [214, 120, 30, 255]
            } else {
                [40, 80, 80, 255]
            };
            fb.fill_rect(
                SAVE_LIST.0,
                y + 2,
                SAVE_LIST.0 + SAVE_LIST.2,
                y + SAVE_ROW_H - 2,
                colour,
            );
            if let Some(f) = font {
                f.draw(&mut fb, &e.name, SAVE_LIST.0 + 8, y + 7);
                let t = format!("{}", e.world_tick);
                let w = f.measure(&t);
                f.draw(&mut fb, &t, SAVE_LIST.0 + SAVE_LIST.2 - 8 - w, y + 7);
            }
        }
        if self.saving {
            let (x, y, w, h) = SAVE_NAME_FIELD;
            fb.fill_rect(x, y, x + w, y + h, [20, 20, 20, 255]);
            fb.fill_rect(x + 1, y + 1, x + w - 1, y + h - 1, [222, 200, 156, 255]);
            if let Some(f) = font {
                f.draw(&mut fb, &format!("{}_", self.name), x + 6, y + 5);
            }
        }
        self.column.draw(&mut fb, assets);
        draw_pointer(
            &mut fb,
            self.pointer,
            assets.and_then(|a| a.cursor.as_ref()),
        );
        fb
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

/// Seals of the lost page (`combat-measurements.md` 4): restart centred (333,556), load (388,556),
/// OK (517,547); rectangles of the 44x46 gold seals (`BTTN` 278 / 277) and the 41x44 V seal around
/// those centres.
const SEAL_LOST_RESTART: (i32, i32, i32, i32) = (311, 533, 44, 46);
const SEAL_LOST_LOAD: (i32, i32, i32, i32) = (366, 533, 44, 46);
const SEAL_LOST_OK: (i32, i32, i32, i32) = (497, 525, 41, 44);

/// What the player chose on the lost page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LostOutcome {
    /// Restart the mission (the original goes to the briefing).
    Restart,
    /// Open the load screen.
    Load,
    /// Leave to the main menu.
    Ok,
}

/// The lost page (`combat-measurements.md` 4): the paused, green-tinted world with the HUD, the
/// vertical parchment at (264,148) with the level's lost text from y 244, and three seals at the
/// bottom edge: the gold restart (`BTTN` 278, double chevron) and load (`BTTN` 277, folder) seals and
/// the blue V.
#[derive(Debug)]
pub struct LostPage {
    /// The lost debriefing text.
    pub text: String,
    pointer: (i32, i32),
}

impl LostPage {
    /// New page over `text`.
    #[must_use]
    pub fn new(text: String) -> Self {
        Self {
            text,
            pointer: (0, 0),
        }
    }

    /// Apply input.
    pub fn handle(&mut self, event: InputEvent) -> Option<LostOutcome> {
        match event {
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
                None
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } => {
                let (px, py) = self.pointer;
                if hit(SEAL_LOST_RESTART, px, py) {
                    Some(LostOutcome::Restart)
                } else if hit(SEAL_LOST_LOAD, px, py) {
                    Some(LostOutcome::Load)
                } else if hit(SEAL_LOST_OK, px, py) {
                    Some(LostOutcome::Ok)
                } else {
                    None
                }
            }
            // Keys: not observed on the original's lost page; Enter confirms like the other pages.
            InputEvent::KeyDown {
                key: Key::Enter | Key::Space,
            } => Some(LostOutcome::Ok),
            _ => None,
        }
    }

    /// State for `observe` (the page has no variable state beyond its text).
    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn state(&self) -> MenuState {
        MenuState {
            screen: "lost".into(),
            items: vec![
                UiItem {
                    action: "restart".into(),
                    label: "restart".into(),
                    rect: [
                        SEAL_LOST_RESTART.0,
                        SEAL_LOST_RESTART.1,
                        SEAL_LOST_RESTART.2,
                        SEAL_LOST_RESTART.3,
                    ],
                    enabled: true,
                    selected: false,
                },
                UiItem {
                    action: "load".into(),
                    label: "load".into(),
                    rect: [
                        SEAL_LOST_LOAD.0,
                        SEAL_LOST_LOAD.1,
                        SEAL_LOST_LOAD.2,
                        SEAL_LOST_LOAD.3,
                    ],
                    enabled: true,
                    selected: false,
                },
                UiItem {
                    action: "ok".into(),
                    label: "ok".into(),
                    rect: [
                        SEAL_LOST_OK.0,
                        SEAL_LOST_OK.1,
                        SEAL_LOST_OK.2,
                        SEAL_LOST_OK.3,
                    ],
                    enabled: true,
                    selected: false,
                },
            ],
            hovered: None,
            page: None,
        }
    }

    /// Draw the page over the (paused) scene.
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
                SEAL_LOST_OK.0,
                SEAL_LOST_OK.1,
                seal.width,
                seal.height,
                &seal.rgba,
            );
        }
        for (r, seals) in [
            (SEAL_LOST_RESTART, &a.seal_restart),
            (SEAL_LOST_LOAD, &a.seal_load),
        ] {
            if let Some(seal) = seals.get(1).or(seals.first()) {
                scene.blit_rgba(r.0, r.1, seal.width, seal.height, &seal.rgba);
            }
        }
        if let Some(font) = a.font_debrief.as_ref() {
            let mut y = 244;
            for line in wrap(font, &self.text, 400) {
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
    pub money: i32,
    /// Clover charms.
    pub clover: u32,
    /// Selected hero's name lines.
    pub hero_name: Vec<String>,
    /// The selected hero's arrows (the counter under the bow icon; `Entity::arrows`).
    pub arrows: i32,
    /// The selected hero's purses (the counter under the purse icon; `Entity::purses`).
    pub purses: i32,
}

/// HUD widget rectangles (x, y, w, h) at 1024x768, located by template matching (see `draw_hud`).
pub mod hud_rects {
    /// Robin's eyes in the leaves (decoration).
    pub const EYES: (i32, i32, i32, i32) = (924, 0, 74, 60);
    /// Towers: zoom levels.
    pub const TOWERS: (i32, i32, i32, i32) = (998, 0, 26, 100);
    /// Map scroll: minimap.
    pub const MAP_SCROLL: (i32, i32, i32, i32) = (941, 38, 61, 52);
    /// Standing figure: stand up.
    pub const STAND: (i32, i32, i32, i32) = (1, 661, 43, 62);
    /// Kneeling figure: crouch.
    pub const KNEEL: (i32, i32, i32, i32) = (0, 721, 43, 45);
    /// Plan scroll (quick actions).
    pub const PLAN: (i32, i32, i32, i32) = (964, 701, 43, 41);
    /// Portrait parchment.
    pub const PORTRAIT: (i32, i32, i32, i32) = (70, 640, 220, 110);
    /// Top-left of the arrow counter of the portrait's action row (`combat-measurements.md`
    /// 1.1, observed: the bow icon at (100,715), the fist at (135,715), the purse at
    /// (165,715), the counters below them; the icons are not drawn yet, the exact offset of
    /// the counters under them is not measured).
    pub const ARROW_COUNTER: (i32, i32) = (96, 728);
    /// Top-left of the purse counter: below the purse icon.
    pub const PURSE_COUNTER: (i32, i32) = (161, 728);
}

/// What a click on the HUD does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudAction {
    /// Crouch the selected character (kneel icon).
    Crouch,
    /// Stand the selected character up (standing figure).
    Stand,
    /// Toggle the mini-map (map scroll).
    Map,
    /// A widget without behaviour yet: the click is consumed, not passed to the world.
    Consumed,
}

/// The HUD element under a logical-pixel position, if any (`ui-flow.md` 9.3).
#[must_use]
pub fn hud_hit(x: i32, y: i32) -> Option<HudAction> {
    use hud_rects as r;
    if hit(r::KNEEL, x, y) {
        Some(HudAction::Crouch)
    } else if hit(r::STAND, x, y) {
        Some(HudAction::Stand)
    } else if hit(r::MAP_SCROLL, x, y) {
        Some(HudAction::Map)
    } else if [r::EYES, r::TOWERS, r::PLAN, r::PORTRAIT]
        .iter()
        .any(|&rc| hit(rc, x, y))
    {
        Some(HudAction::Consumed)
    } else {
        None
    }
}

/// Draw the HUD over the scene (`ui-flow.md` 9.3). Widget positions were located by template matching the
/// decoded pictures in the original's pause capture (2026-09-02, correlation 0.93-0.998): eyes (924,0),
/// map scroll (941,38), towers (998,0) and (998,46), standing figure (1,661), kneeling figure (0,721),
/// plan scroll (964,701), portrait face (83,657). The portrait frame picture is not identified yet, so a
/// small scroll stands in for it.
pub fn draw_hud(scene: &mut Framebuffer, assets: &UiAssets, hud: &HudState) {
    let h = &assets.hud;
    for (pic, x, y) in &h.foliage {
        scene.blit_rgba(*x, *y, pic.width, pic.height, &pic.rgba);
    }
    let place = |scene: &mut Framebuffer, pic: &Option<SpriteFrame>, x: i32, y: i32| {
        if let Some(p) = pic {
            scene.blit_rgba(x, y, p.width, p.height, &p.rgba);
        }
    };
    place(scene, &h.eyes, 924, 0);
    place(scene, &h.towers, 998, 0);
    place(scene, &h.towers_far, 998, 46);
    place(scene, &h.map_scroll, 941, 38);
    place(scene, &h.stand, 1, 661);
    place(scene, &h.kneel, 0, 721);
    place(scene, &h.plan, 964, 701);
    place(scene, &h.portrait_scroll, 70, 640);
    place(scene, &h.portrait, 83, 657);
    if let Some(font) = &assets.font_text {
        let s = &assets.strings;
        let money = text(s, t::MONEY_FORMAT, "money: %i").replace("%i", &hud.money.to_string());
        let clover = text(s, t::CLOVER_FORMAT, "clover: %i").replace("%i", &hud.clover.to_string());
        font.draw(scene, &money, 4, 4);
        font.draw(scene, &clover, 4, 20);
        for (i, line) in hud.hero_name.iter().enumerate() {
            font.draw(scene, line, 140, 665 + 18 * i as i32);
        }
        // The portrait's two counters ("0" and "0" at a mission's start, `ui-flow.md` 9.3
        // element 4): arrows under the bow icon, purses under the purse icon.
        let (ax, ay) = hud_rects::ARROW_COUNTER;
        font.draw(scene, &hud.arrows.to_string(), ax, ay);
        let (px, py) = hud_rects::PURSE_COUNTER;
        font.draw(scene, &hud.purses.to_string(), px, py);
    }
}

/// A non-blocking script text (native 202) over the running world: the small scroll (`PIC` 133,
/// 220x100) centred near the top with the text wrapped inside. Layout is the engine's (the original's
/// presentation of these hints is not observed yet).
pub fn draw_notice(scene: &mut Framebuffer, assets: &UiAssets, text: &str) {
    let Some(font) = assets.font_debrief.as_ref().or(assets.font_text.as_ref()) else {
        return;
    };
    let lines = wrap(font, text, 400);
    let height = (lines.len() as i32) * (font.height() as i32 + 2) + 24;
    let (w, h) = (440, height.max(60));
    let (x, y) = ((MENU_FRAME.0 as i32 - w) / 2, 70);
    scene.fill_rect(x, y, x + w, y + h, [40, 30, 20, 255]);
    scene.fill_rect(x + 2, y + 2, x + w - 2, y + h - 2, [222, 200, 156, 255]);
    let mut ty = y + 12;
    for line in lines {
        font.draw_centered(scene, &line, MENU_FRAME.0 as i32 / 2, ty);
        ty += font.height() as i32 + 2;
    }
}

/// The level's mini-map picture (`Data/Levels/<ambiance>/<map>.min`, 225x183 in every level).
#[derive(Debug, Clone)]
pub struct Minimap {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Opaque RGBA pixels.
    pub rgba: Vec<u8>,
}

/// Where the mini-map scroll sits and where its map picture is (`combat-measurements.md` 5:
/// the scroll spans x 718..940, y 92..283; the map picture 204x155 at (728,112)).
pub const MINIMAP_SCROLL_POS: (i32, i32) = (718, 92);
/// The map picture inside the scroll (x, y, w, h).
pub const MINIMAP_AREA: (i32, i32, i32, i32) = (728, 112, 204, 155);

/// The field-of-vision overlay (`h01-measurements-2.md` 6): while Alt is held and the pointer rests
/// on a soldier, his view cone is drawn from his feet: the sector of the core's cone (half-angle
/// `VIEW_CONE_HALF_ANGLE_256`, reach `VIEW_RANGE` along x compressed by `VIEW_Y_COMPRESSION` along y,
/// the exact region the perception tests), as a yellow outline. The original's fill and line style
/// are not measured (one frame showed the sector's extent), so the outline is the engine's choice;
/// the pointer's yellow shape under Alt is not drawn.
pub fn draw_view_cone(scene: &mut Framebuffer, world: &opensherwood_core::World) {
    use opensherwood_core::ai::{
        VIEW_CONE_HALF_ANGLE_256, VIEW_RANGE, VIEW_Y_COMPRESSION, cos256, sin256,
    };
    use opensherwood_core::{EntityKind, Key};
    if !world.keys_down.contains(&Key::Alt) {
        return;
    }
    let Some(id) = world.actor_at_pointer() else {
        return;
    };
    let Some(e) = world
        .entities
        .iter()
        .find(|e| e.id == id && e.alive && e.active && e.kind == EntityKind::Guard)
    else {
        return;
    };
    let (cx, cy) = world.camera;
    let (fx, fy) = (e.x.round() - cx, e.y.round() - cy);
    let colour = [255, 230, 90, 255];
    let (rx, ry) = (
        f64::from(VIEW_RANGE),
        f64::from(VIEW_RANGE) * f64::from(VIEW_Y_COMPRESSION.0) / f64::from(VIEW_Y_COMPRESSION.1),
    );
    // A point of the boundary at cone angle `a` (256 units): the ray from the apex until it leaves
    // the ellipse.
    let boundary = |a: i32| -> (i32, i32) {
        let (c, s) = (f64::from(cos256(a)) / 256.0, f64::from(sin256(a)) / 256.0);
        let r = 1.0 / ((c / rx).powi(2) + (s / ry).powi(2)).sqrt();
        (fx + (c * r).round() as i32, fy + (s * r).round() as i32)
    };
    let mut plot = |x: i32, y: i32| {
        if x >= 0 && y >= 0 && x < MENU_FRAME.0 as i32 && y < MENU_FRAME.1 as i32 {
            scene.fill_rect(x, y, x + 1, y + 1, colour);
        }
    };
    let mut line = |(x0, y0): (i32, i32), (x1, y1): (i32, i32)| {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=steps {
            plot(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps);
        }
    };
    let (lo, hi) = (
        e.facing256 - VIEW_CONE_HALF_ANGLE_256,
        e.facing256 + VIEW_CONE_HALF_ANGLE_256,
    );
    line((fx, fy), boundary(lo));
    line((fx, fy), boundary(hi));
    let mut prev = boundary(lo);
    for a in lo + 1..=hi {
        let p = boundary(a);
        line(prev, p);
        prev = p;
    }
}

/// Marker colours of the mini-map (`h01-measurements-2.md` 5).
mod minimap_palette {
    /// The player characters' oval.
    pub const PLAYER: [u8; 4] = [164, 251, 82, 255];
    /// An identified enemy's oval.
    pub const ENEMY: [u8; 4] = [255, 0, 0, 255];
    /// An unidentified character's oval.
    pub const UNKNOWN: [u8; 4] = [176, 176, 176, 255];
    /// The outline of the ovals and of the camera rectangle.
    pub const OUTLINE: [u8; 4] = [0, 0, 0, 255];
    /// A pick-up cross.
    pub const CROSS: [u8; 4] = [255, 220, 40, 255];
    /// The cross's centre.
    pub const CROSS_CENTRE: [u8; 4] = [255, 255, 255, 255];
}

/// Distance (map px) within which a soldier counts as identified on the mini-map: the original
/// shows the guards near the hero in red once he was close (`h01-measurements-2.md` 5); the exact
/// rule (a sighting, a distance, memory) is not measured, so this is the engine's reading.
const MINIMAP_IDENTIFY_RANGE: i32 = 400;

/// The mini-map overlay (`ui-flow.md` 9.3 element 2, `combat-measurements.md` 5 and
/// `h01-measurements-2.md` 5): the map scroll widget or the `;` key toggles it, a right click does
/// not close it; the world keeps running underneath. The picture is a parchment scroll with the map
/// painted inside (its corners hold the UI colour key), drawn at the observed position; over its
/// map area (15 map px per picture px on the first mission's map, the map's size over the area's in
/// general) the camera's view is a black-outlined rectangle, every living character a 2x4 oval
/// with a black outline (green for the player characters, red for identified enemies, grey for the
/// rest), and every active pick-up (scroll or item) a 5x5 yellow cross with a white centre.
pub fn draw_minimap(scene: &mut Framebuffer, m: &Minimap, world: &opensherwood_core::World) {
    use opensherwood_core::EntityKind;
    let (x, y) = MINIMAP_SCROLL_POS;
    scene.blit_rgba(x, y, m.width, m.height, &m.rgba);
    let map = world.map_size;
    if map.0 == 0 || map.1 == 0 {
        return;
    }
    let (ax, ay, aw, ah) = MINIMAP_AREA;
    let sx = |v: i32| ax + (i64::from(v) * i64::from(aw) / i64::from(map.0)) as i32;
    let sy = |v: i32| ay + (i64::from(v) * i64::from(ah) / i64::from(map.1)) as i32;
    let inside = |px: i32, py: i32| px >= ax && px < ax + aw && py >= ay && py < ay + ah;
    // Pick-ups first (under the characters): active scrolls and items.
    if let Some(vm) = world.vm.as_ref() {
        let mut crosses: Vec<(i32, i32)> = vm
            .items()
            .into_iter()
            .filter(|it| it.active)
            .map(|it| (it.x, it.y))
            .collect();
        crosses.extend(
            vm.program
                .elements
                .iter()
                .enumerate()
                .filter_map(|(i, el)| match el {
                    opensherwood_core::vm::Element::Scroll { x, y }
                        if i32::try_from(i).is_ok_and(|h| vm.element_active(h)) =>
                    {
                        Some((*x, *y))
                    }
                    _ => None,
                }),
        );
        for (mx, my) in crosses {
            let (cx, cy) = (sx(mx), sy(my));
            if !inside(cx, cy) {
                continue;
            }
            scene.fill_rect(cx - 2, cy, cx + 3, cy + 1, minimap_palette::CROSS);
            scene.fill_rect(cx, cy - 2, cx + 1, cy + 3, minimap_palette::CROSS);
            scene.fill_rect(cx, cy, cx + 1, cy + 1, minimap_palette::CROSS_CENTRE);
        }
    }
    let players: Vec<(i32, i32)> = world
        .entities
        .iter()
        .filter(|e| e.active && e.alive && e.kind == EntityKind::Player)
        .map(|e| (e.x.round(), e.y.round()))
        .collect();
    for e in world
        .entities
        .iter()
        .filter(|e| e.active && e.alive && e.kind != EntityKind::Obstacle)
    {
        let (mx, my) = (e.x.round(), e.y.round());
        let (cx, cy) = (sx(mx), sy(my));
        if !inside(cx, cy) {
            continue;
        }
        let fill = match e.kind {
            EntityKind::Player => minimap_palette::PLAYER,
            EntityKind::Guard
                if e.team == opensherwood_core::Team::Enemy
                    && players.iter().any(|&(px, py)| {
                        let (dx, dy) = (i64::from(px - mx), i64::from(py - my));
                        dx * dx + dy * dy <= i64::from(MINIMAP_IDENTIFY_RANGE).pow(2)
                    }) =>
            {
                minimap_palette::ENEMY
            }
            _ => minimap_palette::UNKNOWN,
        };
        // A 2x4 oval with a 1 px outline (4x6 with the outline).
        scene.fill_rect(cx - 2, cy - 3, cx + 2, cy + 3, minimap_palette::OUTLINE);
        scene.fill_rect(cx - 1, cy - 2, cx + 1, cy + 2, fill);
    }
    let camera = world.camera;
    let view = world.viewport;
    let (x0, y0) = (sx(camera.0).max(ax), sy(camera.1).max(ay));
    let (x1, y1) = (
        sx(camera.0.saturating_add(view.0 as i32)).min(ax + aw),
        sy(camera.1.saturating_add(view.1 as i32)).min(ay + ah),
    );
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let c = minimap_palette::OUTLINE;
    scene.fill_rect(x0, y0, x1, y0 + 1, c);
    scene.fill_rect(x0, y1 - 1, x1, y1, c);
    scene.fill_rect(x0, y0, x0 + 1, y1, c);
    scene.fill_rect(x1 - 1, y0, x1, y1, c);
}

/// The `observe.ui` state while the mini-map overlay is open (no items: it has no buttons).
#[must_use]
pub fn minimap_state() -> opensherwood_protocol::UiState {
    opensherwood_protocol::UiState {
        screen: "minimap".into(),
        items: Vec::new(),
        hovered: None,
        page: None,
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

/// The original's paused / briefing scene is a green-tinted luminance image. Fitted on 2026-09-02 against
/// the analyst's capture of the first briefing (same camera, parchment and HUD masked, 500k pixels):
/// `out = lum * (0.124, 0.429, 0.287)` for (r, g, b) with `lum = 0.299 r + 0.587 g + 0.114 b`, residual
/// 4-6 levels per channel (a per-channel scale fits worse, 5-7). Offsets of a few levels are ignored.
fn tint_green(fb: &mut Framebuffer) {
    for px in fb.rgba.chunks_exact_mut(4) {
        let lum = (77 * u32::from(px[0]) + 150 * u32::from(px[1]) + 29 * u32::from(px[2])) >> 8;
        px[0] = ((lum * 32) >> 8) as u8;
        px[1] = ((lum * 110) >> 8) as u8;
        px[2] = ((lum * 73) >> 8) as u8;
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
