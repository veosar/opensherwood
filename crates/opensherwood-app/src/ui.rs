//! Screens outside the simulation: the main menu (retail background, retail fonts, our layout until
//! `docs/original/ui-flow.md` fixes the exact positions). Driven by the same canonical input events
//! as the world so the harness can click through it and capture it.

use opensherwood_core::{Button, Fixed, InputEvent, Key};
use opensherwood_render::{Background, FontAtlas, Framebuffer};
use serde::Serialize;

/// Menu entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuAction {
    /// Start the campaign (currently: the tutorial map view).
    NewGame,
    /// Load a saved game (not implemented).
    LoadGame,
    /// Options (not implemented).
    Options,
    /// Quit.
    Quit,
}

impl MenuAction {
    fn label(self) -> &'static str {
        match self {
            MenuAction::NewGame => "New Game",
            MenuAction::LoadGame => "Load Game",
            MenuAction::Options => "Options",
            MenuAction::Quit => "Quit",
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
}

/// Fonts used by the menu (retail bitmap fonts).
pub struct MenuFonts {
    /// Enabled button face.
    pub button: FontAtlas,
    /// Focused / hovered face (the disabled face is used as the highlight until the original's
    /// hover behaviour is documented).
    pub button_hover: FontAtlas,
    /// Title face.
    pub title: FontAtlas,
}

impl std::fmt::Debug for MenuFonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MenuFonts")
    }
}

/// The main menu.
#[derive(Debug)]
pub struct MainMenu {
    background: Option<Background>,
    fonts: Option<MenuFonts>,
    items: Vec<MenuItem>,
    hovered: Option<usize>,
    pointer: (i32, i32),
    pending: Option<MenuAction>,
}

/// Observation of the menu for the harness.
#[derive(Debug, Clone, Serialize)]
pub struct MenuState {
    /// Items.
    pub items: Vec<MenuItem>,
    /// Hovered item index.
    pub hovered: Option<usize>,
}

impl MainMenu {
    /// Build the menu for a viewport.
    #[must_use]
    pub fn new(viewport: (u32, u32), background: Option<Background>, fonts: Option<MenuFonts>) -> Self {
        let (w, h) = (viewport.0 as i32, viewport.1 as i32);
        let line = fonts.as_ref().map_or(24, |f| f.button.height() as i32 + 6);
        let actions = [
            MenuAction::NewGame,
            MenuAction::LoadGame,
            MenuAction::Options,
            MenuAction::Quit,
        ];
        let top = h / 2 - (actions.len() as i32 * line) / 2 + h / 8;
        let items = actions
            .iter()
            .enumerate()
            .map(|(i, &action)| {
                let label = action.label().to_string();
                let tw = fonts
                    .as_ref()
                    .map_or(label.len() as i32 * 8, |f| f.button.measure(&label));
                MenuItem {
                    action,
                    label,
                    rect: (w / 2 - tw / 2 - 8, top + i as i32 * line, tw + 16, line),
                }
            })
            .collect();
        Self {
            background,
            fonts,
            items,
            hovered: None,
            pointer: (0, 0),
            pending: None,
        }
    }

    /// Apply input; returns an action when one was chosen.
    pub fn handle(&mut self, event: InputEvent) -> Option<MenuAction> {
        match event {
            InputEvent::PointerMove { x256, y256 } => {
                self.pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
                self.hovered = self.items.iter().position(|it| {
                    let (x, y, w, h) = it.rect;
                    (x..x + w).contains(&self.pointer.0) && (y..y + h).contains(&self.pointer.1)
                });
            }
            InputEvent::PointerDown {
                button: Button::Left,
            } => {
                if let Some(i) = self.hovered {
                    self.pending = Some(self.items[i].action);
                }
            }
            InputEvent::KeyDown { key } => match key {
                Key::Up => {
                    self.hovered = Some(self.hovered.map_or(0, |i| (i + self.items.len() - 1) % self.items.len()));
                }
                Key::Down => {
                    self.hovered = Some(self.hovered.map_or(0, |i| (i + 1) % self.items.len()));
                }
                Key::Enter | Key::Space => {
                    if let Some(i) = self.hovered {
                        self.pending = Some(self.items[i].action);
                    }
                }
                Key::Escape => self.pending = Some(MenuAction::Quit),
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
            items: self.items.clone(),
            hovered: self.hovered,
        }
    }

    /// Render into a framebuffer of the given size.
    #[must_use]
    pub fn render(&self, viewport: (u32, u32)) -> Framebuffer {
        let mut fb = Framebuffer::new(viewport.0, viewport.1);
        match &self.background {
            Some(bg) => {
                // Scale the background to the viewport with nearest sampling (the retail menu
                // background is 1024x768; the logical viewport may be smaller).
                for y in 0..fb.height {
                    let sy = (u64::from(y) * u64::from(bg.height) / u64::from(fb.height)) as usize;
                    for x in 0..fb.width {
                        let sx = (u64::from(x) * u64::from(bg.width) / u64::from(fb.width)) as usize;
                        let si = (sy * bg.width as usize + sx) * 4;
                        let di = ((y * fb.width + x) * 4) as usize;
                        fb.rgba[di..di + 4].copy_from_slice(&bg.rgba[si..si + 4]);
                    }
                }
            }
            None => fb.clear([20, 30, 20, 255]),
        }
        if let Some(fonts) = &self.fonts {
            fonts
                .title
                .draw_centered(&mut fb, "OpenSherwood", fb.width as i32 / 2, fb.height as i32 / 8);
            for (i, it) in self.items.iter().enumerate() {
                let font = if self.hovered == Some(i) {
                    &fonts.button_hover
                } else {
                    &fonts.button
                };
                font.draw(&mut fb, &it.label, it.rect.0 + 8, it.rect.1 + 3);
            }
        } else {
            for (i, it) in self.items.iter().enumerate() {
                let c = if self.hovered == Some(i) {
                    [255, 255, 255, 255]
                } else {
                    [180, 180, 180, 255]
                };
                fb.fill_rect(it.rect.0, it.rect.1, it.rect.0 + it.rect.2, it.rect.1 + it.rect.3, c);
            }
        }
        let (mx, my) = self.pointer;
        fb.line(mx - 4, my, mx + 4, my, [255, 255, 0, 255]);
        fb.line(mx, my - 4, mx, my + 4, [255, 255, 0, 255]);
        fb
    }
}
