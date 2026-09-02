//! Engine session: world + assets + RPC method dispatch, shared by headless and window modes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use std::sync::Arc;

use opensherwood_assets::{GameDir, SpriteBank};
use opensherwood_core::{
    AnimSet, Catalog, EntityKind, FrameSpec, InputEvent, Key, MapInfo, Scenario, Snapshot, World,
};

use crate::mission;
use crate::ui::{Briefing, HudState, MainMenu, MenuAction, PauseMenu, ProfileSummary, UiAssets};
use crate::ui_assets;
use opensherwood_core::Geometry;
use opensherwood_protocol::{
    CaptureParams, CaptureResult, HelloResult, ObserveParams, ObserveResult, PROTOCOL_VERSION,
    Replay, ReplayCheckpoint, ReplayEvent, ReplayHeader, ReplayPlayParams, ReplayPlayResult,
    ReplayStartParams, ReplayStopResult, ResetParams, RestoreParams, RngStreamInit, RpcError,
    SnapshotResult, StepParams, StepResult, replay_limits,
};
use opensherwood_render::Occluder;
use opensherwood_render::{Background, Framebuffer, NoSprites, SpriteFrame, SpriteSource, render};
use serde_json::{Value, json};

/// Result of an RPC method.
pub type RpcResult = Result<Value, RpcError>;

/// Sprite bank adapter for the renderer.
struct Sprites {
    bank: SpriteBank,
}

impl SpriteSource for Sprites {
    fn frame(&mut self, index: u32) -> Option<Arc<SpriteFrame>> {
        let img = self.bank.frame(index).ok()?;
        Some(Arc::new(SpriteFrame {
            width: img.width,
            height: img.height,
            rgba: img.rgba.clone(),
        }))
    }
}

/// Build the core's animation set from a parsed profile using the documented block layout
/// (`docs/formats/sprite-animations.md`): 16-direction blocks per action; idle = action 0,
/// walk = action 6. Falls back to the first animations when the profile has no table.
fn anim_set_from_profile(profile: &opensherwood_formats::rhs::Profile) -> AnimSet {
    use opensherwood_formats::anim_table::{AnimationTable, Direction};
    // Frame anchors are relative to a canvas whose origin (the entity position) is the sequence's
    // `origin_x/origin_y` (150,150 for characters); see docs/formats/sprites.md.
    let animations: Vec<Vec<FrameSpec>> = profile
        .sequences
        .iter()
        .flat_map(|s| {
            let (ox, oy) = (s.origin_x as i32, s.origin_y as i32);
            s.animations.iter().map(move |a| {
                a.frames
                    .iter()
                    .map(|f| FrameSpec {
                        frame: f.frame,
                        duration: (f.duration & 0xFFFF).max(1),
                        offset_x: i32::from(f.anchor_x) - ox,
                        offset_y: i32::from(f.anchor_y) - oy,
                    })
                    .collect()
            })
        })
        .collect();
    let n = animations.len().max(1) as u32;
    let mut idle = [0u32; 8];
    let mut walk = [0u32; 8];
    let table = AnimationTable::from_profile(profile);
    for (o, (i, w)) in idle.iter_mut().zip(walk.iter_mut()).enumerate() {
        let dir = Direction::from_octant(o);
        *i = table
            .as_ref()
            .and_then(|t| t.idle(dir))
            .map_or(o as u32 % n, |a| a as u32);
        *w = table
            .as_ref()
            .and_then(|t| t.walk(dir))
            .map_or((8 + o as u32) % n, |a| a as u32);
    }
    AnimSet {
        animations,
        idle,
        walk,
    }
}

/// One engine instance.
pub struct Session {
    game: Option<GameDir>,
    artifacts: PathBuf,
    /// The world, if a scenario is loaded.
    pub world: Option<World>,
    background: Option<Background>,
    sprites: Option<Sprites>,
    snapshots: BTreeMap<String, Snapshot>,
    next_snapshot: u64,
    frame: Option<Framebuffer>,
    /// Window input waiting to be applied by the next `step` (controlled window mode).
    queued_input: Vec<InputEvent>,
    /// Replay being recorded, if any.
    recording: Option<Recording>,
    /// Audio output (window mode only; `None` when muted, headless or unavailable).
    audio: Option<opensherwood_audio::Audio>,
    /// Active screen.
    screen: Screen,
    /// Menu pictures and fonts (loaded on first use).
    ui_assets: Option<UiAssets>,
    /// The window should close (Exit chosen in the menu).
    pub exit_requested: bool,
    /// Current objective text (pause menu).
    objective: String,
    /// HUD values.
    hud: HudState,
}

/// What the player is looking at.
#[derive(Debug)]
enum Screen {
    /// The world is played directly.
    World,
    /// Main menu.
    Menu(MainMenu),
    /// Briefing parchment over the paused mission.
    Briefing(Briefing),
    /// Pause menu over the paused mission.
    Pause(PauseMenu),
}

/// The mission `Play!` starts with a fresh profile (`docs/original/campaign-flow.md`).
pub const FIRST_MISSION: &str = "H01_Lin_VL";

/// Tick rate used by replays and the window (ticks per second).
pub const TICK_RATE: (u32, u32) = (60, 1);

struct Recording {
    replay: Replay,
    checkpoint_every: u64,
    /// Set when a recording quota (`replay_limits`) was hit outside an RPC `step` (window mode);
    /// nothing more is recorded and `replay.stop` reports the error instead of a replay.
    failed: Option<String>,
}

/// Limits that keep a hostile client from exhausting memory or time. Recording quotas are the
/// `replay_limits` of the protocol crate: the recorder never produces a replay its parser rejects.
pub mod limits {
    /// Most ticks in one `step`.
    pub const MAX_TICKS: u32 = 100_000;
    /// Most ticks one `replay.play` simulates synchronously (about 4.6 hours at 60 Hz; the
    /// replay format itself allows 2^24). Longer replays are rejected before the session is reset.
    pub const MAX_REPLAY_PLAY_TICKS: u64 = 1_000_000;
    /// Most ticks per `step` when per-tick hashes are requested.
    pub const MAX_HASHED_TICKS: u32 = 10_000;
    /// Most events in one `step`.
    pub const MAX_EVENTS: usize = 100_000;
    /// Snapshot handles kept (oldest are dropped).
    pub const MAX_SNAPSHOTS: usize = 32;
    /// Most queued window input events.
    pub const MAX_QUEUED_INPUT: usize = 10_000;
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("world", &self.world.as_ref().map(|w| w.tick))
            .finish_non_exhaustive()
    }
}

fn params<T: serde::de::DeserializeOwned + Default>(p: Option<Value>) -> Result<T, RpcError> {
    match p {
        None => Ok(T::default()),
        Some(v) => serde_json::from_value(v)
            .map_err(|e| RpcError::new(RpcError::INVALID_PARAMS, e.to_string())),
    }
}

fn params_required<T: serde::de::DeserializeOwned>(p: Option<Value>) -> Result<T, RpcError> {
    let v = p.ok_or_else(|| RpcError::new(RpcError::INVALID_PARAMS, "params required"))?;
    serde_json::from_value(v).map_err(|e| RpcError::new(RpcError::INVALID_PARAMS, e.to_string()))
}

fn ok<T: serde::Serialize>(v: T) -> RpcResult {
    serde_json::to_value(v).map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))
}

fn engine_err(msg: impl Into<String>) -> RpcError {
    RpcError::new(RpcError::ENGINE, msg)
}

impl Session {
    /// Create a session (no world yet).
    #[must_use]
    pub fn new(game: Option<GameDir>, artifacts: PathBuf) -> Self {
        Self {
            game,
            artifacts,
            world: None,
            background: None,
            sprites: None,
            snapshots: BTreeMap::new(),
            next_snapshot: 0,
            frame: None,
            queued_input: Vec::new(),
            recording: None,
            audio: None,
            screen: Screen::World,
            ui_assets: None,
            exit_requested: false,
            objective: String::new(),
            hud: HudState::default(),
        }
    }

    fn ui_assets(&mut self) -> Option<&UiAssets> {
        if self.ui_assets.is_none()
            && let Some(game) = self.game.as_ref()
        {
            self.ui_assets = Some(ui_assets::load(game));
        }
        self.ui_assets.as_ref()
    }

    /// Open the main menu.
    fn open_menu(&mut self) {
        let _ = self.ui_assets();
        let strings = self
            .ui_assets
            .as_ref()
            .map_or(&[][..], |a| a.strings.as_slice());
        self.screen = Screen::Menu(MainMenu::new(ProfileSummary::default(), strings));
        self.frame = None;
        self.start_scenario_music();
    }

    /// `Play!`: load the first mission behind its briefing parchment.
    fn start_campaign(&mut self) -> Result<(), String> {
        self.reset(Scenario::Mission(FIRST_MISSION.into()), 0)?;
        let pages = self
            .game
            .as_ref()
            .map(|g| ui_assets::level_texts(g, ui_assets::texts::FIRST_MISSION_BRIEFING))
            .unwrap_or_default();
        // Strings 0..2 are the briefing pages; the rest are in-mission tutorial popups.
        let pages: Vec<String> = pages
            .into_iter()
            .take(ui_assets::texts::FIRST_MISSION_BRIEFING_PAGES)
            .collect();
        self.screen = if pages.is_empty() {
            Screen::World
        } else {
            Screen::Briefing(Briefing::new(pages))
        };
        self.objective = self
            .game
            .as_ref()
            .and_then(|g| {
                ui_assets::level_texts(g, ui_assets::texts::FIRST_MISSION_OBJECTIVES)
                    .into_iter()
                    .next()
            })
            .unwrap_or_default();
        self.hud = HudState {
            money: 100,
            clover: 0,
            hero_name: self.hero_name_lines(),
        };
        self.frame = None;
        Ok(())
    }

    /// The selected hero's name, split in two lines like the original's portrait ("Robin" / "Hood").
    fn hero_name_lines(&self) -> Vec<String> {
        let name = self
            .world
            .as_ref()
            .and_then(|w| w.entities.iter().find(|e| e.kind == EntityKind::Player))
            .and_then(|e| e.anim.as_ref().map(|a| a.set.clone()))
            .unwrap_or_default();
        // Profile names are CamelCase ("RobinHood"); split at the case changes.
        let mut lines: Vec<String> = Vec::new();
        for ch in name.chars() {
            if ch.is_uppercase() || lines.is_empty() {
                lines.push(ch.to_string());
            } else if let Some(last) = lines.last_mut() {
                last.push(ch);
            }
        }
        lines
    }

    /// Snapshots and replays describe a directly played world (ADR-0004, "Screens and the world").
    fn require_world_screen(&self) -> Result<(), RpcError> {
        if matches!(self.screen, Screen::World) {
            Ok(())
        } else {
            Err(engine_err(
                "screen shown: dismiss the menu, briefing or pause screen first",
            ))
        }
    }

    /// Escape in a mission opens the pause menu.
    fn open_pause(&mut self) {
        let _ = self.ui_assets();
        let strings = self
            .ui_assets
            .as_ref()
            .map_or(&[][..], |a| a.strings.as_slice());
        self.screen = Screen::Pause(PauseMenu::new(self.objective.clone(), strings));
        self.frame = None;
    }

    /// Advance one tick: menus consume the events without ticking the world; the briefing pauses
    /// the world; otherwise the world steps.
    pub fn advance(&mut self, events: &[InputEvent]) {
        match &mut self.screen {
            Screen::Menu(menu) => {
                let mut chosen = None;
                for e in events {
                    if let Some(a) = menu.handle(*e) {
                        chosen = Some(a);
                        break;
                    }
                }
                self.frame = None;
                match chosen {
                    Some(MenuAction::Play) => {
                        if let Err(e) = self.start_campaign() {
                            // Stay on the menu rather than leaving the player with nothing.
                            eprintln!("opensherwood: cannot start the campaign: {e}");
                            self.open_menu();
                        }
                    }
                    Some(MenuAction::Exit) => self.exit_requested = true,
                    Some(other) => eprintln!("opensherwood: menu action {other:?} not implemented"),
                    None => {}
                }
            }
            Screen::Briefing(b) => {
                let mut done = false;
                for e in events {
                    if b.handle(*e) {
                        done = true;
                        break;
                    }
                }
                self.frame = None;
                if done {
                    self.screen = Screen::World;
                }
            }
            Screen::Pause(p) => {
                let mut chosen = None;
                for e in events {
                    if let Some(a) = p.handle(*e) {
                        chosen = Some(a);
                        break;
                    }
                }
                self.frame = None;
                match chosen {
                    Some(MenuAction::Continue) => self.screen = Screen::World,
                    Some(MenuAction::Restart) => {
                        if let Err(e) = self.start_campaign() {
                            eprintln!("opensherwood: cannot restart: {e}");
                        }
                    }
                    Some(MenuAction::Quit) => {
                        if let Err(e) = self.reset(Scenario::Menu("main".into()), 0) {
                            eprintln!("opensherwood: cannot open the menu: {e}");
                        }
                    }
                    Some(other) => {
                        eprintln!("opensherwood: pause action {other:?} not implemented");
                    }
                    None => {}
                }
            }
            Screen::World => {
                let in_mission = matches!(
                    self.world.as_ref().map(|w| &w.scenario),
                    Some(Scenario::Mission(_))
                );
                let escape_at = events.iter().position(|e| {
                    in_mission && matches!(e, InputEvent::KeyDown { key: Key::Escape })
                });
                match escape_at {
                    // Escape wins the tick: the world does not advance and the tick's other events
                    // are dropped, so the pause always lands on the state the player saw.
                    Some(_) => self.open_pause(),
                    None => self.step_recorded(events),
                }
            }
        }
    }

    /// Screen state for `observe`.
    fn ui_state(&self) -> Option<opensherwood_protocol::UiState> {
        match &self.screen {
            Screen::World => None,
            Screen::Menu(m) => Some(m.state()),
            Screen::Briefing(b) => Some(b.state()),
            Screen::Pause(p) => Some(p.state()),
        }
    }

    /// Open (or skip) the audio device. Called once by window mode.
    pub fn set_audio(&mut self, mute: bool) {
        self.audio = if mute {
            None
        } else {
            match opensherwood_audio::Audio::open() {
                Ok(a) => Some(a),
                Err(e) => {
                    eprintln!("opensherwood: audio disabled: {e}");
                    None
                }
            }
        };
    }

    /// Start the music that belongs to the current scenario (retail track names by map).
    fn start_scenario_music(&mut self) {
        let Some(audio) = self.audio.as_mut() else {
            return;
        };
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let track = match self.world.as_ref().map(|w| &w.scenario) {
            Some(Scenario::Mission(_)) => {
                // Mission music states are not modelled yet; the menu theme must not carry on.
                audio.stop_music();
                return;
            }
            Some(Scenario::MapView { map, ambiance }) => {
                let base = match map.to_lowercase().as_str() {
                    "sherwood" => "Sherwood".to_string(),
                    m if m.starts_with("croisement") => "Cross_Amb".to_string(),
                    m => {
                        let city = match m {
                            "nottingham" => "Nottingham",
                            "lincoln" => "Lincoln",
                            "york" => "York",
                            "derby" => "Derby",
                            "leicester" => "Leicester",
                            _ => return,
                        };
                        let suffix = if ambiance.eq_ignore_ascii_case("day") {
                            "D"
                        } else {
                            "NF"
                        };
                        format!("{city}_{suffix}")
                    }
                };
                format!("Data/Musics/{base}.wav")
            }
            _ => "Data/Musics/Menu.wav".to_string(),
        };
        match game.read(&track) {
            Ok(bytes) => {
                if let Err(e) = audio.play_music(bytes, true) {
                    eprintln!("opensherwood: music {track}: {e}");
                }
            }
            Err(e) => eprintln!("opensherwood: music {track}: {e}"),
        }
    }

    /// Fingerprint of the loaded game content (`None` without game data). A file that cannot be
    /// hashed is an error, never a partial fingerprint.
    fn content_fingerprint(&self) -> Result<Option<String>, RpcError> {
        self.game
            .as_ref()
            .map(GameDir::fingerprint)
            .transpose()
            .map_err(|e| RpcError::new(RpcError::INTERNAL, format!("content fingerprint: {e}")))
    }

    fn replay_header(&self, world: &World) -> Result<ReplayHeader, RpcError> {
        let fingerprint = match world.scenario {
            Scenario::Synthetic(_) => None,
            _ => self.content_fingerprint()?,
        };
        Ok(ReplayHeader {
            replay_version: 1,
            protocol: PROTOCOL_VERSION,
            ruleset: opensherwood_core::RULESET_VERSION,
            content_fingerprint: fingerprint,
            scenario: world.scenario.clone(),
            viewport: world.viewport,
            tick_rate: TICK_RATE,
            hash_schema: opensherwood_core::hash::HASH_SCHEMA_VERSION,
            seed: world.seed,
            rng_streams: [(
                "gameplay".to_string(),
                RngStreamInit {
                    algorithm: opensherwood_core::rng::Rng::ALGORITHM.to_string(),
                    seed: world.rng.seed,
                    stream: world.rng.stream,
                },
            )]
            .into_iter()
            .collect(),
        })
    }

    /// Reject a `step` that would push the active recording over the `replay_limits` quotas
    /// (event count, checkpoint count, highest tick). Checked before anything is mutated so a
    /// refused step leaves both the world and the recording untouched.
    fn check_recording_quota(&self, ticks: u32, new_events: usize) -> Result<(), RpcError> {
        let (Some(rec), Some(world)) = (self.recording.as_ref(), self.world.as_ref()) else {
            return Ok(());
        };
        let quota = |what: &str| {
            RpcError::new(
                RpcError::INVALID_PARAMS,
                format!("step would exceed the replay recording quota ({what}); call replay.stop"),
            )
        };
        if world.tick + u64::from(ticks) > replay_limits::MAX_TICK {
            return Err(quota(&format!("tick {}", replay_limits::MAX_TICK)));
        }
        if rec.replay.events.len() + new_events > replay_limits::MAX_EVENTS {
            return Err(quota(&format!("{} events", replay_limits::MAX_EVENTS)));
        }
        let planned = u64::from(ticks)
            .checked_div(rec.checkpoint_every)
            .map_or(0, |n| n + 1);
        // `replay.stop` may append one final checkpoint: keep room for it.
        if rec.replay.checkpoints.len() as u64 + planned + 1 > replay_limits::MAX_CHECKPOINTS as u64
        {
            return Err(quota(&format!(
                "{} checkpoints",
                replay_limits::MAX_CHECKPOINTS
            )));
        }
        Ok(())
    }

    /// Run one tick, recording it if a replay is being recorded. Recording stops (and the
    /// recording is marked failed) at the `replay_limits` quotas; RPC `step` refuses such a step
    /// up front through [`Session::check_recording_quota`], this is the backstop for window mode.
    fn step_recorded(&mut self, events: &[InputEvent]) {
        let Some(world) = self.world.as_mut() else {
            return;
        };
        let tick = world.tick;
        if let Some(rec) = self.recording.as_mut()
            && rec.failed.is_none()
        {
            if tick >= replay_limits::MAX_TICK {
                rec.failed = Some(format!("tick quota {} reached", replay_limits::MAX_TICK));
            } else if rec.replay.events.len() + events.len() > replay_limits::MAX_EVENTS {
                rec.failed = Some(format!(
                    "event quota {} exceeded at tick {tick}",
                    replay_limits::MAX_EVENTS
                ));
            } else {
                for (i, e) in events.iter().enumerate() {
                    rec.replay.events.push(ReplayEvent {
                        tick,
                        sequence: i as u32,
                        event: *e,
                        intent: None,
                    });
                }
            }
        }
        world.step(events);
        if let Some(rec) = self.recording.as_mut()
            && rec.failed.is_none()
            && rec.checkpoint_every > 0
            && world.tick.is_multiple_of(rec.checkpoint_every)
        {
            // Keep room for the final checkpoint `replay.stop` appends.
            if rec.replay.checkpoints.len() + 1 >= replay_limits::MAX_CHECKPOINTS {
                rec.failed = Some(format!(
                    "checkpoint quota {} exceeded at tick {}",
                    replay_limits::MAX_CHECKPOINTS,
                    world.tick
                ));
            } else {
                rec.replay.checkpoints.push(ReplayCheckpoint {
                    tick: world.tick,
                    hashes: world.hashes(),
                });
            }
        }
    }

    /// Queue window input for the next `step` (controlled window mode).
    pub fn queue_input(&mut self, events: Vec<InputEvent>) {
        self.queued_input.extend(events);
        if self.queued_input.len() > limits::MAX_QUEUED_INPUT {
            let excess = self.queued_input.len() - limits::MAX_QUEUED_INPUT;
            self.queued_input.drain(..excess);
        }
    }

    /// Parse a `--scenario` argument.
    pub fn parse_scenario(text: &str) -> Result<Scenario, String> {
        let mut parts = text.split(':');
        match parts.next() {
            Some("map") => {
                let map = parts
                    .next()
                    .ok_or("map scenario needs a name: map:<name>[:<ambiance>]")?;
                let ambiance = parts.next().unwrap_or("Day");
                Ok(Scenario::MapView {
                    map: map.to_string(),
                    ambiance: ambiance.to_string(),
                })
            }
            Some("mission") => Ok(Scenario::Mission(parts.next().unwrap_or("").to_string())),
            Some("menu") => Ok(Scenario::Menu(parts.next().unwrap_or("main").to_string())),
            Some(name) => Ok(Scenario::Synthetic(name.to_string())),
            None => Err("empty scenario".into()),
        }
    }

    /// Decode a retail background and the map's geometry (occluders for drawing, walkable area for
    /// movement).
    fn load_map(
        game: &GameDir,
        map: &str,
        ambiance: &str,
    ) -> Result<(Background, Geometry), String> {
        let logical = format!("Data/Levels/{ambiance}/{map}.map");
        let data = game.read(&logical).map_err(|e| e.to_string())?;
        let img = opensherwood_formats::image_blob::parse_file(&data)
            .map_err(|e| format!("{logical}: {e}"))?;
        let mut background = Background {
            width: u32::from(img.width),
            height: u32::from(img.height),
            rgba: img.to_rgba8_565(),
            occluders: Vec::new(),
        };
        let mut geometry = Geometry::default();
        let rhp_path = format!("Data/Levels/{map}.rhp");
        match game.read(&rhp_path) {
            Ok(bytes) => match opensherwood_formats::rhp::parse(&bytes) {
                Ok(rhp) => {
                    background.occluders = rhp
                        .faces
                        .iter()
                        .map(|f| Occluder {
                            x: i32::from(f.x),
                            y: i32::from(f.y),
                            width: u32::from(f.width),
                            height: u32::from(f.height),
                            bits: f.mask.clone(),
                            line: f.lines.first().and_then(|l| {
                                (l.points.len() >= 2).then(|| {
                                    (
                                        (i32::from(l.points[0].x), i32::from(l.points[0].y)),
                                        (i32::from(l.points[1].x), i32::from(l.points[1].y)),
                                    )
                                })
                            }),
                        })
                        .collect();
                    geometry.boundary = rhp
                        .stat
                        .boundary
                        .iter()
                        .map(|p| (i32::from(p.x), i32::from(p.y)))
                        .collect();
                    geometry.obstacles = rhp
                        .stat
                        .obstacles
                        .iter()
                        .map(|o| {
                            o.polygon
                                .points
                                .iter()
                                .map(|p| (i32::from(p.x), i32::from(p.y)))
                                .collect()
                        })
                        .collect();
                }
                Err(e) => eprintln!("opensherwood: {rhp_path}: {e}"),
            },
            Err(e) => eprintln!("opensherwood: {rhp_path}: {e}"),
        }
        Ok((background, geometry))
    }

    /// Open the sprite bank once and build a catalog for the given profiles.
    fn load_catalog(&mut self, profiles: &[String]) -> Catalog {
        let mut catalog = Catalog::default();
        let Some(game) = self.game.as_ref() else {
            return catalog;
        };
        if self.sprites.is_none() {
            match SpriteBank::open(game) {
                Ok(bank) => self.sprites = Some(Sprites { bank }),
                Err(e) => eprintln!("opensherwood: sprite bank unavailable: {e}"),
            }
        }
        if self.sprites.is_none() {
            return catalog;
        }
        for name in profiles {
            match SpriteBank::load_profile(game, name) {
                Ok(profile) => {
                    catalog
                        .sets
                        .insert(name.clone(), anim_set_from_profile(&profile));
                }
                Err(e) => eprintln!("opensherwood: profile {name}: {e}"),
            }
        }
        catalog
    }

    /// Load a scenario (what `reset` does).
    pub fn reset(&mut self, scenario: Scenario, seed: u64) -> Result<(), String> {
        if let Scenario::Menu(name) = &scenario {
            if name != "main" {
                return Err(format!("unknown menu '{name}'"));
            }
            self.world = None;
            self.background = None;
            self.snapshots.clear();
            self.queued_input.clear();
            self.recording = None;
            self.open_menu();
            return Ok(());
        }
        self.screen = Screen::World;
        let (world, background) = match &scenario {
            Scenario::MapView { map, ambiance } => {
                let game = self
                    .game
                    .as_ref()
                    .ok_or("map scenarios need a game directory")?;
                let (bg, geometry) = Self::load_map(game, map, ambiance)?;
                let info = MapInfo {
                    width: bg.width,
                    height: bg.height,
                };
                let mut world = World::new_map_view(scenario, seed, info)?;
                world.set_geometry(geometry);
                let catalog =
                    self.load_catalog(&["RobinHood".to_string(), "Soldier A00".to_string()]);
                if !catalog.sets.is_empty() {
                    world.attach_catalog(catalog, Some("RobinHood"), Some("Soldier A00"));
                }
                (world, Some(bg))
            }
            Scenario::Mission(name) => {
                let game = self.game.as_ref().ok_or("missions need a game directory")?;
                let (mission_file, map) = mission::load(game, name)?;
                let ambiance = "Day";
                let (bg, geometry) = Self::load_map(game, &map, ambiance)?;
                let info = MapInfo {
                    width: bg.width,
                    height: bg.height,
                };
                let (spec, profiles) = mission::build_spec(&mission_file, info, geometry);
                let mut world = World::new_mission(scenario, seed, &spec)?;
                let catalog = self.load_catalog(&profiles);
                if !catalog.sets.is_empty() {
                    world.attach_catalog(catalog, None, None);
                }
                (world, Some(bg))
            }
            Scenario::Synthetic(_) | Scenario::Menu(_) => (World::new(scenario, seed)?, None),
        };
        self.world = Some(world);
        self.background = background;
        self.frame = None;
        self.snapshots.clear();
        self.queued_input.clear();
        self.recording = None;
        self.start_scenario_music();
        Ok(())
    }

    /// Advance one tick with the given events (window mode).
    pub fn tick(&mut self, events: &[InputEvent]) {
        self.advance(events);
    }

    /// Resolve a relative artifact path (rejecting absolute paths and `..`).
    fn artifact_path(&self, rel: &str) -> Result<PathBuf, RpcError> {
        if rel.contains("..") || PathBuf::from(rel).is_absolute() {
            return Err(RpcError::new(
                RpcError::INVALID_PARAMS,
                "path must be relative",
            ));
        }
        let path = self.artifacts.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
        }
        Ok(path)
    }

    /// The current frame, rendering it if needed.
    pub fn frame(&mut self) -> Option<&Framebuffer> {
        if self.frame.is_none() {
            let frame = match &self.screen {
                Screen::Menu(_) => {
                    let _ = self.ui_assets();
                    let Screen::Menu(menu) = &self.screen else {
                        unreachable!()
                    };
                    menu.render(self.ui_assets.as_ref())
                }
                Screen::Briefing(_) | Screen::Pause(_) | Screen::World => {
                    let in_mission = matches!(
                        self.world.as_ref().map(|w| &w.scenario),
                        Some(Scenario::Mission(_))
                    );
                    if in_mission {
                        let _ = self.ui_assets();
                    }
                    let world = self.world.as_ref()?;
                    let mut frame = match self.sprites.as_mut() {
                        Some(s) => render(world, self.background.as_ref(), s),
                        None => render(world, self.background.as_ref(), &mut NoSprites),
                    };
                    if in_mission && let Some(a) = self.ui_assets.as_ref() {
                        crate::ui::draw_hud(&mut frame, a, &self.hud);
                    }
                    match &self.screen {
                        Screen::Briefing(b) => b.render(&mut frame, self.ui_assets.as_ref()),
                        Screen::Pause(p) => p.render(&mut frame, self.ui_assets.as_ref()),
                        _ => {}
                    }
                    frame
                }
            };
            self.frame = Some(frame);
        }
        self.frame.as_ref()
    }

    fn world(&mut self) -> Result<&mut World, RpcError> {
        self.world
            .as_mut()
            .ok_or_else(|| engine_err("no world loaded; call reset first"))
    }

    /// Dispatch one RPC method.
    pub fn dispatch(&mut self, method: &str, p: Option<Value>) -> RpcResult {
        match method {
            "hello" => ok(HelloResult {
                protocol: PROTOCOL_VERSION,
                build: env!("CARGO_PKG_VERSION").to_string(),
                ruleset: opensherwood_core::RULESET_VERSION,
                capabilities: vec![
                    "synthetic".into(),
                    "capture".into(),
                    "snapshot".into(),
                    "map_view".into(),
                    "mission".into(),
                    "replay".into(),
                    "menu".into(),
                ],
                content_fingerprint: self.content_fingerprint()?,
            }),
            "reset" => {
                let p: ResetParams = params_required(p)?;
                self.reset(p.scenario, p.seed).map_err(engine_err)?;
                let ui = self.ui_state();
                match self.world.as_ref() {
                    Some(world) => {
                        ok(json!({ "tick": world.tick, "hashes": world.hashes(), "ui": ui }))
                    }
                    None => ok(json!({ "tick": 0, "hashes": {}, "ui": ui })),
                }
            }
            "step" => {
                let p: StepParams = params_required(p)?;
                if p.ticks == 0 || p.ticks > limits::MAX_TICKS {
                    return Err(RpcError::new(
                        RpcError::INVALID_PARAMS,
                        format!("ticks must be in 1..={}", limits::MAX_TICKS),
                    ));
                }
                if p.hash_every_tick && p.ticks > limits::MAX_HASHED_TICKS {
                    return Err(RpcError::new(
                        RpcError::INVALID_PARAMS,
                        format!(
                            "hash_every_tick allows at most {} ticks",
                            limits::MAX_HASHED_TICKS
                        ),
                    ));
                }
                if p.events.len() > limits::MAX_EVENTS {
                    return Err(RpcError::new(
                        RpcError::INVALID_PARAMS,
                        format!("at most {} events per step", limits::MAX_EVENTS),
                    ));
                }
                if p.events.iter().any(|e| e.tick_offset >= p.ticks) {
                    return Err(RpcError::new(
                        RpcError::INVALID_PARAMS,
                        "event tick_offset beyond ticks",
                    ));
                }
                let mut events = p.events;
                events.sort_by_key(|e| (e.tick_offset, e.sequence));
                self.check_recording_quota(p.ticks, events.len() + self.queued_input.len())?;
                let queued = std::mem::take(&mut self.queued_input);
                if self.world.is_none() && matches!(self.screen, Screen::World) {
                    return Err(engine_err("no world loaded; call reset first"));
                }
                let mut per_tick = Vec::new();
                let mut cursor = 0usize;
                let mut tick_events: Vec<InputEvent> = Vec::new();
                for offset in 0..p.ticks {
                    tick_events.clear();
                    if offset == 0 {
                        tick_events.extend(queued.iter().copied());
                    }
                    while cursor < events.len() && events[cursor].tick_offset == offset {
                        tick_events.push(events[cursor].event);
                        cursor += 1;
                    }
                    self.advance(&tick_events);
                    if p.hash_every_tick
                        && let Some(w) = self.world.as_ref()
                    {
                        per_tick.push(w.hashes());
                    }
                }
                self.frame = None;
                let (tick, hashes) = self
                    .world
                    .as_ref()
                    .map_or((0, opensherwood_core::Hashes::default()), |w| {
                        (w.tick, w.hashes())
                    });
                ok(StepResult {
                    tick,
                    hashes,
                    per_tick,
                })
            }
            "observe" => {
                let p: ObserveParams = params(p)?;
                let ui = self.ui_state();
                ok(ObserveResult {
                    observation: self.world.as_ref().map(|w| w.observe(p.entities)),
                    hashes: self
                        .world
                        .as_ref()
                        .map_or_else(opensherwood_core::Hashes::default, World::hashes),
                    ui,
                })
            }
            "snapshot" => {
                self.require_world_screen()?;
                let world = self.world()?;
                let snapshot = world.snapshot();
                let hashes = world.hashes();
                self.next_snapshot += 1;
                // Zero-padded so lexicographic order in the map is insertion (FIFO) order.
                let id = format!("snap-{:012}", self.next_snapshot);
                while self.snapshots.len() >= limits::MAX_SNAPSHOTS {
                    let oldest = self.snapshots.keys().next().cloned();
                    if let Some(k) = oldest {
                        self.snapshots.remove(&k);
                    }
                }
                self.snapshots.insert(id.clone(), snapshot.clone());
                ok(SnapshotResult {
                    id,
                    snapshot,
                    hashes,
                })
            }
            "restore" => {
                self.require_world_screen()?;
                let p: RestoreParams = params_required(p)?;
                let snap = match (p.snapshot, p.id) {
                    (Some(s), _) => s,
                    (None, Some(id)) => self.snapshots.get(&id).cloned().ok_or_else(|| {
                        RpcError::new(RpcError::INVALID_PARAMS, format!("unknown snapshot {id}"))
                    })?,
                    (None, None) => {
                        return Err(RpcError::new(
                            RpcError::INVALID_PARAMS,
                            "id or snapshot required",
                        ));
                    }
                };
                // Validate first (never install an invalid world), then make sure the session's
                // assets (background, catalog) belong to the snapshot's scenario.
                snap.world.validate().map_err(engine_err)?;
                let same_scenario = self
                    .world
                    .as_ref()
                    .is_some_and(|w| w.scenario == snap.world.scenario);
                if !same_scenario {
                    self.reset(snap.world.scenario.clone(), snap.world.seed)
                        .map_err(engine_err)?;
                }
                self.frame = None;
                let world = self.world()?;
                world.restore(&snap).map_err(engine_err)?;
                ok(json!({ "tick": world.tick, "hashes": world.hashes() }))
            }
            "capture" => {
                let p: CaptureParams = params(p)?;
                let artifacts = self.artifacts.clone();
                let frame = self
                    .frame()
                    .ok_or_else(|| engine_err("nothing to capture; call reset first"))?;
                let mut written = None;
                if let Some(rel) = p.path {
                    if rel.contains("..") || PathBuf::from(&rel).is_absolute() {
                        return Err(RpcError::new(
                            RpcError::INVALID_PARAMS,
                            "path must be relative",
                        ));
                    }
                    let path = artifacts.join(rel);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
                    }
                    let png = frame
                        .encode_png()
                        .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
                    std::fs::write(&path, png)
                        .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
                    written = Some(path.to_string_lossy().to_string());
                }
                ok(CaptureResult {
                    hash: frame.hash(),
                    width: frame.width,
                    height: frame.height,
                    path: written,
                })
            }
            "replay.start" => {
                self.require_world_screen()?;
                let p: ReplayStartParams = params(p)?;
                let world = self
                    .world
                    .as_ref()
                    .ok_or_else(|| engine_err("no world loaded; call reset first"))?;
                if world.tick != 0 {
                    return Err(engine_err(
                        "replay recording must start at tick 0 (call reset first)",
                    ));
                }
                let header = self.replay_header(world)?;
                let mut replay = Replay {
                    header,
                    events: Vec::new(),
                    checkpoints: Vec::new(),
                };
                replay.checkpoints.push(ReplayCheckpoint {
                    tick: 0,
                    hashes: world.hashes(),
                });
                self.recording = Some(Recording {
                    replay,
                    checkpoint_every: p.checkpoint_every,
                    failed: None,
                });
                ok(json!({ "recording": true }))
            }
            "replay.stop" => {
                let p: CaptureParams = params(p)?;
                let mut rec = self
                    .recording
                    .take()
                    .ok_or_else(|| engine_err("no replay is being recorded"))?;
                if let Some(why) = rec.failed {
                    return Err(engine_err(format!("recording discarded: {why}")));
                }
                let world = self.world()?;
                if rec
                    .replay
                    .checkpoints
                    .last()
                    .is_none_or(|c| c.tick != world.tick)
                {
                    rec.replay.checkpoints.push(ReplayCheckpoint {
                        tick: world.tick,
                        hashes: world.hashes(),
                    });
                }
                // Checkpoint at tick 0 duplicates the header's guarantees; keep it only if alone.
                if rec.replay.checkpoints.len() > 1 && rec.replay.checkpoints[0].tick == 0 {
                    rec.replay.checkpoints.remove(0);
                }
                let jsonl = rec.replay.to_jsonl();
                let mut written = None;
                if let Some(rel) = p.path {
                    let path = self.artifact_path(&rel)?;
                    std::fs::write(&path, &jsonl)
                        .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
                    written = Some(path.to_string_lossy().to_string());
                }
                ok(ReplayStopResult {
                    events: rec.replay.events.len(),
                    checkpoints: rec.replay.checkpoints.len(),
                    jsonl,
                    path: written,
                })
            }
            "replay.play" => {
                self.require_world_screen()?;
                let p: ReplayPlayParams = params(p)?;
                let text = match (p.jsonl, p.path) {
                    (Some(t), _) => t,
                    (None, Some(rel)) => {
                        let path = self.artifact_path(&rel)?;
                        // Size first: the parser's cap must never be reached by reading the file.
                        let len = std::fs::metadata(&path)
                            .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?
                            .len();
                        if len > replay_limits::MAX_BYTES as u64 {
                            return Err(RpcError::new(
                                RpcError::INVALID_PARAMS,
                                format!(
                                    "replay file is {len} bytes; at most {} are accepted",
                                    replay_limits::MAX_BYTES
                                ),
                            ));
                        }
                        std::fs::read_to_string(&path)
                            .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?
                    }
                    (None, None) => {
                        return Err(RpcError::new(
                            RpcError::INVALID_PARAMS,
                            "jsonl or path required",
                        ));
                    }
                };
                let replay = Replay::from_jsonl(&text)
                    .map_err(|e| RpcError::new(RpcError::INVALID_PARAMS, e))?;
                let last = replay.last_tick();
                if last > limits::MAX_REPLAY_PLAY_TICKS {
                    return Err(RpcError::new(
                        RpcError::INVALID_PARAMS,
                        format!(
                            "replay runs to tick {last}; at most {} ticks are played in one call",
                            limits::MAX_REPLAY_PLAY_TICKS
                        ),
                    ));
                }
                if let Some(fp) = &replay.header.content_fingerprint
                    && self.content_fingerprint()?.as_ref() != Some(fp)
                {
                    return Err(engine_err(
                        "replay was recorded with different game content",
                    ));
                }
                self.reset(replay.header.scenario.clone(), replay.header.seed)
                    .map_err(engine_err)?;
                let mut events = replay.events.iter().peekable();
                let mut checkpoints = replay.checkpoints.iter().peekable();
                let mut checkpoints_ok = 0usize;
                let mut first_divergence = None;
                let mut tick_events: Vec<InputEvent> = Vec::new();
                for tick in 0..last {
                    tick_events.clear();
                    while let Some(e) = events.peek()
                        && e.tick == tick
                    {
                        tick_events.push(e.event);
                        events.next();
                    }
                    self.step_recorded(&tick_events);
                    let hashes = self.world()?.hashes();
                    while let Some(c) = checkpoints.peek()
                        && c.tick <= tick + 1
                    {
                        if c.tick == tick + 1 {
                            let diff = c.hashes.diff(&hashes);
                            if diff.is_empty() {
                                checkpoints_ok += 1;
                            } else if first_divergence.is_none() {
                                first_divergence = Some((c.tick, diff));
                            }
                        }
                        checkpoints.next();
                    }
                    if first_divergence.is_some() && p.stop_on_divergence {
                        break;
                    }
                }
                self.frame = None;
                let world = self.world()?;
                ok(ReplayPlayResult {
                    ticks: world.tick,
                    checkpoints_ok,
                    first_divergence,
                    hashes: world.hashes(),
                })
            }
            "debug.nav" => {
                #[derive(serde::Deserialize)]
                struct P {
                    x: i32,
                    y: i32,
                    #[serde(default)]
                    to: Option<(i32, i32)>,
                }
                let p: P = params_required(p)?;
                let world = self.world()?;
                world.ensure_nav();
                let nav = world.nav.as_ref().expect("ensured");
                let cell = nav.cell_of(p.x, p.y);
                let path = p.to.map(|(tx, ty)| {
                    let goal = nav.cell_of(tx, ty);
                    nav.find_path(cell, goal).map(|cells| cells.len())
                });
                ok(json!({
                    "geometry_walkable": world.geometry.is_walkable(p.x, p.y),
                    "cell": cell,
                    "cell_walkable": nav.walkable(cell),
                    "nearest_walkable": nav.nearest_walkable(cell, 8),
                    "grid": [nav.width, nav.height],
                    "boundary_points": world.geometry.boundary.len(),
                    "obstacles": world.geometry.obstacles.len(),
                    "path_cells": path,
                }))
            }
            "shutdown" => ok(json!({ "ok": true })),
            _ => Err(RpcError::new(
                RpcError::METHOD_NOT_FOUND,
                format!("unknown method '{method}'"),
            )),
        }
    }
}

#[cfg(test)]
mod replay_limit_tests {
    use super::*;

    fn session(name: &str) -> (Session, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "opensherwood-replay-limits-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (Session::new(None, dir.clone()), dir)
    }

    fn corridor(s: &mut Session) {
        s.dispatch(
            "reset",
            Some(json!({ "scenario": { "synthetic": "corridor" }, "seed": 1 })),
        )
        .unwrap();
    }

    #[test]
    fn step_refuses_to_exceed_the_recording_quotas() {
        let (mut s, _dir) = session("quota");
        corridor(&mut s);
        s.dispatch("replay.start", Some(json!({ "checkpoint_every": 1 })))
            .unwrap();
        // 100,000 checkpoints would exceed the format's 65,536: refused before anything moves.
        let err = s
            .dispatch("step", Some(json!({ "ticks": limits::MAX_TICKS })))
            .unwrap_err();
        assert_eq!(err.code, RpcError::INVALID_PARAMS);
        assert!(err.message.contains("checkpoints"), "{}", err.message);
        assert_eq!(s.world.as_ref().unwrap().tick, 0);
        assert_eq!(s.recording.as_ref().unwrap().replay.checkpoints.len(), 1);
        // A step within the quota records normally.
        s.dispatch("step", Some(json!({ "ticks": 10 }))).unwrap();
        // Event quota: pretend the recording already holds the maximum.
        let rec = s.recording.as_mut().unwrap();
        let filler = ReplayEvent {
            tick: 0,
            sequence: 0,
            event: InputEvent::PointerMove { x256: 0, y256: 0 },
            intent: None,
        };
        rec.replay
            .events
            .resize(replay_limits::MAX_EVENTS, filler.clone());
        let err = s
            .dispatch(
                "step",
                Some(json!({
                    "ticks": 1,
                    "events": [{ "tick_offset": 0, "sequence": 0, "kind": "pointer_move", "x256": 0, "y256": 0 }]
                })),
            )
            .unwrap_err();
        assert!(err.message.contains("events"), "{}", err.message);
        assert_eq!(s.world.as_ref().unwrap().tick, 10);
        // The window-mode backstop: stepping directly past the quota marks the recording failed
        // and `replay.stop` reports it instead of returning a replay the parser would reject.
        s.step_recorded(&[filler.event]);
        assert!(s.recording.as_ref().unwrap().failed.is_some());
        let err = s.dispatch("replay.stop", Some(json!({}))).unwrap_err();
        assert!(err.message.contains("discarded"), "{}", err.message);
        assert!(s.recording.is_none());
    }

    #[test]
    fn play_rejects_long_replays_and_oversized_files() {
        let (mut s, dir) = session("play");
        corridor(&mut s);
        s.dispatch("replay.start", Some(json!({ "checkpoint_every": 0 })))
            .unwrap();
        s.dispatch("step", Some(json!({ "ticks": 5 }))).unwrap();
        let stopped = s.dispatch("replay.stop", Some(json!({}))).unwrap();
        let jsonl = stopped["jsonl"].as_str().unwrap();
        // Move the final checkpoint far beyond the playback budget (still a valid replay).
        let long: String = jsonl
            .lines()
            .map(|line| {
                let mut v: Value = serde_json::from_str(line).unwrap();
                if v["type"] == "checkpoint" {
                    v["tick"] = json!(limits::MAX_REPLAY_PLAY_TICKS + 1);
                }
                v.to_string() + "\n"
            })
            .collect();
        let err = s
            .dispatch("replay.play", Some(json!({ "jsonl": long })))
            .unwrap_err();
        assert_eq!(err.code, RpcError::INVALID_PARAMS);
        assert!(err.message.contains("ticks are played"), "{}", err.message);
        // Rejected before the session was reset: the world is still at tick 5.
        assert_eq!(s.world.as_ref().unwrap().tick, 5);
        // A file above the parser cap is refused by its size, before it is read.
        std::fs::create_dir_all(dir.join("replays")).unwrap();
        let huge = dir.join("replays/huge.jsonl");
        std::fs::File::create(&huge)
            .unwrap()
            .set_len(replay_limits::MAX_BYTES as u64 + 1)
            .unwrap();
        let err = s
            .dispatch("replay.play", Some(json!({ "path": "replays/huge.jsonl" })))
            .unwrap_err();
        assert_eq!(err.code, RpcError::INVALID_PARAMS);
        assert!(err.message.contains("bytes"), "{}", err.message);
        assert_eq!(s.world.as_ref().unwrap().tick, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
