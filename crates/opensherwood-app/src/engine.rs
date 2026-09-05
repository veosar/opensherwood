//! Engine session: world + assets + RPC method dispatch, shared by headless and window modes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use std::sync::Arc;

use opensherwood_assets::{GameDir, SpriteBank};
use opensherwood_core::{
    AnimSet, Catalog, EntityKind, Fixed, FrameSpec, InputEvent, Key, MapInfo, Scenario, Snapshot,
    World,
};

use crate::mission;
use crate::ui::{
    Briefing, Credits, HudState, MainMenu, MenuAction, OptionsOutcome, OptionsScreen, PauseMenu,
    ProfileSummary, SaveEntry, SaveOutcome, SaveScreen, SelectOutcome, SelectPlayerScreen,
    Settings, UiAssets,
};
use crate::ui_assets;
use opensherwood_core::Geometry;
use opensherwood_protocol::{
    CaptureParams, CaptureResult, HelloResult, ObserveParams, ObserveResult, PROTOCOL_VERSION,
    REPLAY_TIME_SESSION, Replay, ReplayCheckpoint, ReplayEvent, ReplayHeader, ReplayPlayParams,
    ReplayPlayResult, ReplayRecorder, ReplayStartParams, ReplayStopResult, ResetParams,
    RestoreParams, RngStreamInit, RpcError, SnapshotResult, StepParams, StepResult, replay_limits,
};
use opensherwood_render::Occluder;
use opensherwood_render::{Background, Framebuffer, NoSprites, SpriteFrame, SpriteSource, render};
use serde::{Deserialize, Serialize};
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
/// walk = action 6, run = action 7, crouched idle = 14, crouched walk (sneak) = 16, the alert
/// set 140 / 141 / 142 / 143 / 151, the fall set 41 / 44 / 47 / 48 / 49 and the knock-out blow
/// 123 (`docs/original/stealth-and-combat.md`). A profile without a block names the fallback
/// `AnimSet` documents (soldiers and civilians have no crouch set, civilians and heroes no
/// alert set, only Robin and the big man the blow). Falls back to the first animations when the
/// profile has no table. Frame timing follows `docs/formats/sprite-animations.md` "Reading
/// rules" on the measured animation clock (`opensherwood_core::anim`): the timing word's tick
/// half plus one is the frame's duration in table ticks, its signed high half the advance in
/// map pixels, from which the core derives the movement speeds.
fn anim_set_from_profile(profile: &opensherwood_formats::rhs::Profile) -> AnimSet {
    use opensherwood_formats::anim_table::{ActionId, AnimationTable, Direction, split_duration};
    // Action ids of the alert, fall and knock-out sets (`docs/formats/sprite-animations.md`,
    // "Combat, state and stealth ids").
    const ALERT_IDLE: ActionId = ActionId(140);
    const NOTICED: ActionId = ActionId(141);
    const ALARM: ActionId = ActionId(142);
    const ALERT_WALK: ActionId = ActionId(143);
    const ALERT_RUN: ActionId = ActionId(151);
    const LYING_BACK: ActionId = ActionId(48);
    const PUNCH: ActionId = ActionId(123);
    const FIGHT_IDLE: ActionId = ActionId(54);
    const STRIKE: ActionId = ActionId(59);
    const POWERFUL_BLOW: ActionId = ActionId(75);
    const FLINCH: ActionId = ActionId(104);
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
                    .map(|f| {
                        let (ticks, advance) = split_duration(f.duration);
                        FrameSpec {
                            frame: f.frame,
                            duration: u32::from(ticks) + 1,
                            advance: i32::from(advance),
                            offset_x: i32::from(f.anchor_x) - ox,
                            offset_y: i32::from(f.anchor_y) - oy,
                        }
                    })
                    .collect()
            })
        })
        .collect();
    let n = animations.len().max(1) as u32;
    let mut idle = [0u32; 8];
    let mut walk = [0u32; 8];
    let mut run = [0u32; 8];
    let mut crouch_idle = [0u32; 8];
    let mut crouch_walk = [0u32; 8];
    let mut alert_idle = [0u32; 8];
    let mut noticed = [0u32; 8];
    let mut alarm = [0u32; 8];
    let mut alert_walk = [0u32; 8];
    let mut alert_run = [0u32; 8];
    let mut knocked_down = [0u32; 8];
    let mut knocked_down_back = [0u32; 8];
    let mut lying = [0u32; 8];
    let mut lying_back = [0u32; 8];
    let mut get_up = [0u32; 8];
    let mut punch = [0u32; 8];
    let mut fight_idle = [0u32; 8];
    let mut strike = [0u32; 8];
    let mut powerful_blow = [0u32; 8];
    let mut flinch = [0u32; 8];
    let table = AnimationTable::from_profile(profile);
    for o in 0..8 {
        let dir = Direction::from_octant(o);
        let lookup = |f: fn(&AnimationTable, Direction) -> Option<usize>| {
            table.as_ref().and_then(|t| f(t, dir)).map(|a| a as u32)
        };
        let action = |id: ActionId| {
            table
                .as_ref()
                .and_then(|t| t.animation(id, dir))
                .map(|a| a as u32)
        };
        idle[o] = lookup(AnimationTable::idle).unwrap_or(o as u32 % n);
        walk[o] = lookup(AnimationTable::walk).unwrap_or((8 + o as u32) % n);
        run[o] = lookup(AnimationTable::run).unwrap_or(walk[o]);
        crouch_idle[o] = lookup(AnimationTable::crouch_idle).unwrap_or(idle[o]);
        crouch_walk[o] = lookup(AnimationTable::sneak).unwrap_or(walk[o]);
        alert_idle[o] = action(ALERT_IDLE).unwrap_or(idle[o]);
        noticed[o] = action(NOTICED).unwrap_or(idle[o]);
        alarm[o] = action(ALARM).unwrap_or(idle[o]);
        alert_walk[o] = action(ALERT_WALK).unwrap_or(walk[o]);
        alert_run[o] = action(ALERT_RUN).unwrap_or(run[o]);
        knocked_down[o] = action(ActionId::KNOCKED_DOWN).unwrap_or(idle[o]);
        knocked_down_back[o] = action(ActionId::KNOCKED_DOWN_BACK).unwrap_or(knocked_down[o]);
        lying[o] = action(ActionId::LYING).unwrap_or(knocked_down[o]);
        lying_back[o] = action(LYING_BACK).unwrap_or(lying[o]);
        get_up[o] = action(ActionId::GET_UP).unwrap_or(idle[o]);
        punch[o] = action(PUNCH).unwrap_or(idle[o]);
        fight_idle[o] = action(FIGHT_IDLE).unwrap_or(idle[o]);
        strike[o] = action(STRIKE).unwrap_or(fight_idle[o]);
        powerful_blow[o] = action(POWERFUL_BLOW).unwrap_or(strike[o]);
        flinch[o] = action(FLINCH).unwrap_or(fight_idle[o]);
    }
    AnimSet {
        animations,
        idle,
        walk,
        run,
        crouch_idle,
        crouch_walk,
        alert_idle,
        noticed,
        alarm,
        alert_walk,
        alert_run,
        knocked_down,
        knocked_down_back,
        lying,
        lying_back,
        get_up,
        punch,
        has_punch: table.as_ref().is_some_and(|t| t.has(PUNCH)),
        fight_idle,
        strike,
        powerful_blow,
        flinch,
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
    /// The session tick: the number of `advance` calls since the current world was installed (a
    /// `reset`, or Play! / Restart from a menu), whether a screen consumed the tick's events or
    /// the world stepped. Replay time (`REPLAY_TIME_SESSION`): events and checkpoints are keyed by
    /// it, so screens are part of the timeline and the world tick may lag behind it.
    elapsed: u64,
    /// Audio output (window mode only; `None` when muted, headless or unavailable).
    audio: Option<opensherwood_audio::Audio>,
    /// Active screen.
    screen: Screen,
    /// Menu pictures and fonts (loaded on first use).
    ui_assets: Option<UiAssets>,
    /// The window should close (Exit chosen in the menu).
    pub exit_requested: bool,
    /// Texts of the current mission's level text list (`.red` entry 4), indexed as the script does.
    mission_texts: Vec<String>,
    /// Short briefings (objectives) of the current mission (`.red` last entry).
    mission_objectives: Vec<String>,
    /// Debriefing texts shown when the mission is won / lost (`.red` entries before the last).
    debriefings_won: Vec<String>,
    debriefings_lost: Vec<String>,
    /// The mission's end has been shown (the debriefing parchment leads to the next mission or the menu).
    ended: bool,
    /// Starting money to use for the next mission load instead of the profile's (a `reset`
    /// parameter or a replay header); consumed by the load.
    money_override: Option<i32>,
    /// Starting money the current mission was loaded with (recorded in replay headers).
    starting_money: Option<i32>,
    /// The level's mini-map picture, when the level has one.
    minimap: Option<crate::ui::Minimap>,
    /// The mini-map overlay is open (session presentation state; the world keeps running).
    minimap_open: bool,
    /// The mission ended with a loss: the debriefing leads back to the menu, not onward.
    ended_lost: bool,
    /// The last failed profile / settings write (cleared by a successful one); in `observe`.
    persistence_error: Option<String>,
    /// Mission file that follows the current one in the campaign graph (`profile.cpf` level table:
    /// the level whose only prerequisite is the current level), if any.
    next_mission: Option<String>,
    /// A non-blocking script text (native 202) shown over the world, with its remaining ticks.
    notice: Option<(String, u32)>,
    /// A HUD press whose release has not arrived yet (swallowed when it does).
    hud_press_pending: bool,
    /// Next rolling auto-save slot.
    autosave_slot: u32,
    /// Player settings (loaded from `settings.json` under the artifact directory).
    settings: Settings,
    /// Profiles (`profiles.json` under the artifact directory) and the selected one.
    profiles: Vec<ProfileSummary>,
    profile_index: usize,
    /// Scenario and seed of the world that is installed (for Restart).
    current: Option<(Scenario, u64, Option<i32>)>,
    /// HUD values.
    hud: HudState,
    /// Unknown-native policy for mission scripts (`--lenient-natives`).
    lenient_natives: bool,
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
    /// Debriefing parchment at the end of a mission; dismissing it returns to the menu.
    Debriefing(Briefing),
    /// Load / save screen; `from_pause` says where Cancel returns.
    Saves {
        screen: SaveScreen,
        from_pause: bool,
    },
    /// Options screens; `from_pause` says where Back returns.
    Options {
        screen: OptionsScreen,
        from_pause: bool,
    },
    /// Select player screen.
    SelectPlayer(SelectPlayerScreen),
    /// Credits.
    Credits(Credits),
    /// The lost page over the paused world (restart / load / OK seals).
    Lost(crate::ui::LostPage),
}

/// The mission `Play!` starts with a fresh profile (`docs/original/campaign-flow.md`).
pub const FIRST_MISSION: &str = "H01_Lin_VL";
/// How long a non-blocking script text stays on screen (5 s at 60 ticks; the original's timing is
/// not observed yet).
const NOTICE_TICKS: u32 = 300;
/// World ticks between rolling auto saves (one minute at 60 Hz) and the number of slots kept.
const AUTOSAVE_TICKS: u64 = 3600;
const AUTOSAVE_SLOTS: u32 = 5;
/// Save file format version (bumped with the snapshot envelope).
const SAVE_FORMAT: u32 = 1;

/// A save file: the snapshot envelope plus bookkeeping.
#[derive(Debug, Serialize, Deserialize)]
struct SaveFile {
    format: u32,
    world_tick: u64,
    snapshot: Snapshot,
}

/// Tick rate used by replays and the window (ticks per second).
pub const TICK_RATE: (u32, u32) = (60, 1);

struct Recording {
    /// The replay so far; the recorder accounts the serialised bytes of every line it accepts
    /// against `replay_limits::MAX_BYTES` (with the final checkpoint reserved) and refuses,
    /// without mutating, anything the parser would reject.
    recorder: ReplayRecorder,
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
    /// Largest save file accepted (the snapshot cap of the protocol).
    pub const MAX_SAVE_BYTES: usize = 64 * 1024 * 1024;
    pub const MAX_QUEUED_INPUT: usize = 10_000;
}

/// Whether a missing or malformed retail dependency (map geometry, profile table, a sprite
/// profile the mission references, the sprite bank) degrades to a logged default instead of
/// failing the scenario load: `OPENSHERWOOD_LENIENT_ASSETS=1` (`docs/build.md`). Off by default:
/// a retail scenario either loads what the mission needs or reports what is missing.
#[must_use]
pub fn lenient_assets() -> bool {
    std::env::var_os("OPENSHERWOOD_LENIENT_ASSETS").is_some_and(|v| v == "1")
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
            elapsed: 0,
            audio: None,
            screen: Screen::World,
            ui_assets: None,
            exit_requested: false,
            mission_texts: Vec::new(),
            mission_objectives: Vec::new(),
            debriefings_won: Vec::new(),
            debriefings_lost: Vec::new(),
            ended: false,
            money_override: None,
            starting_money: None,
            minimap: None,
            minimap_open: false,
            ended_lost: false,
            persistence_error: None,
            next_mission: None,
            notice: None,
            hud_press_pending: false,
            autosave_slot: 0,
            settings: Settings::default(),
            profiles: vec![ProfileSummary::default()],
            profile_index: 0,
            current: None,
            hud: HudState::default(),
            lenient_natives: false,
        }
    }

    /// Select the unknown-native policy of mission scripts (see `opensherwood_core::natives`).
    pub fn set_lenient_natives(&mut self, lenient: bool) {
        self.lenient_natives = lenient;
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
        let profile = self.profile();
        self.screen = Screen::Menu(MainMenu::new(profile, strings));
        self.frame = None;
        self.start_scenario_music();
    }

    /// `Play!`: load the first mission; its script then shows the briefing pages (`sync_text_screen`).
    fn start_campaign(&mut self) -> Result<(), String> {
        self.reset(Scenario::Mission(FIRST_MISSION.into()), 0)
    }

    /// Texts and objectives of a mission: the profile table maps the mission file to its level code,
    /// and `Text/RHLevel<code>.red` lists the text ids (`docs/formats/red.md`: entry 4 = the text list,
    /// the last entry = the short briefings). Missing pieces leave the lists empty.
    fn load_mission_texts(&mut self, name: &str) {
        self.mission_texts.clear();
        self.mission_objectives.clear();
        self.debriefings_won.clear();
        self.debriefings_lost.clear();
        self.ended = false;
        self.minimap_open = false;
        self.ended_lost = false;
        self.next_mission = None;
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let table = game
            .read("Data/Configuration/profile.cpf")
            .ok()
            .and_then(|d| opensherwood_formats::cpf::parse(&d).ok());
        let Some(code) = table.as_ref().and_then(|t| {
            t.levels
                .iter()
                .find(|l| {
                    let f = l.mission_file.trim_end_matches(".rhm");
                    f.eq_ignore_ascii_case(name) || l.mission_file.eq_ignore_ascii_case(name)
                })
                .map(|l| l.code.clone())
        }) else {
            eprintln!("opensherwood: no level code for mission {name}; no texts");
            return;
        };
        // Campaign graph (`docs/formats/profile.md`, medium confidence): a level is available after
        // the levels in its `after` list. The successor launched automatically is the level whose
        // only prerequisite is this one, first in table order (mission 1 -> the first secondary).
        self.next_mission = table.as_ref().and_then(|t| {
            t.levels
                .iter()
                .find(|l| l.after.len() == 1 && l.after[0] == code && !l.mission_file.is_empty())
                .map(|l| l.mission_file.trim_end_matches(".rhm").to_string())
        });
        let Ok(red) = game.read(&format!("Data/Text/RHLevel{code}.red")) else {
            eprintln!("opensherwood: no text index for level {code}");
            return;
        };
        let ids: Vec<u32> = red
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        if let Some(&texts) = ids.get(4) {
            self.mission_texts = ui_assets::level_texts(game, texts);
        }
        if let Some(&short) = ids.last() {
            self.mission_objectives = ui_assets::level_texts(game, short);
        }
        // Tail of the list: `n_won, won id, n_lost, lost id, n_short, short id`.
        if ids.len() >= 6 {
            self.debriefings_won = ui_assets::level_texts(game, ids[ids.len() - 5]);
            self.debriefings_lost = ui_assets::level_texts(game, ids[ids.len() - 3]);
        }
    }

    /// When the script reports the mission won, show the debriefing parchment once; dismissing it
    /// returns to the main menu (campaign progression comes later).
    fn sync_mission_end(&mut self) {
        if self.ended || !matches!(self.screen, Screen::World) {
            return;
        }
        let Some(world) = self.world.as_ref() else {
            return;
        };
        // The hero's death is the loss whether or not the script's `CheckVictoryCondition`
        // reports it (`combat-measurements.md` 4: the lost page within a third of a second).
        let hero_dead = world.hero_dead;
        let Some(vm) = world.vm.as_ref() else {
            return;
        };
        // A loss takes precedence: the script sets it on death or failure and no win follows it.
        let lost = vm.mission_lost || hero_dead;
        if !vm.mission_won && !lost {
            return;
        }
        let variant = vm
            .debriefing
            .and_then(|d| usize::try_from(d).ok())
            .unwrap_or(0);
        let (texts, fallback) = if lost {
            (&self.debriefings_lost, "[mission lost]")
        } else {
            (&self.debriefings_won, "[mission won]")
        };
        let text = texts
            .get(variant)
            .or_else(|| texts.first())
            .cloned()
            .unwrap_or_else(|| fallback.to_string());
        self.ended = true;
        self.ended_lost = lost;
        if lost {
            // The lost page follows the death within a third of a second in the original
            // (`combat-measurements.md` 4): here on the same tick.
            let _ = self.ui_assets();
            self.screen = Screen::Lost(crate::ui::LostPage::new(text));
            self.frame = None;
            return;
        }
        // Campaign money: a won mission's money becomes the profile's (the loss keeps the
        // profile's). Whether the original also keeps money from a lost mission is not observed.
        if !lost {
            let money = vm.money;
            if let Some(p) = self.profiles.get_mut(self.profile_index) {
                p.money = money;
                self.save_profiles();
            }
        }
        let _ = self.ui_assets();
        self.screen = Screen::Debriefing(Briefing::new(vec![text]));
        self.frame = None;
    }

    /// The current primary objective (first primary one not accomplished), for the pause menu.
    fn current_objective(&self) -> String {
        self.world
            .as_ref()
            .and_then(|w| w.vm.as_ref())
            .and_then(|vm| {
                vm.objectives
                    .iter()
                    .filter(|o| o.primary && !o.done)
                    .chain(vm.objectives.iter().filter(|o| !o.done))
                    .next()
                    .map(|o| o.index)
            })
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| self.mission_objectives.get(i).cloned())
            .unwrap_or_default()
    }

    /// While the world is played and the script has a text pending: a blocking page (native 203)
    /// goes on the parchment and waits for the player, a non-blocking one (native 202) becomes a
    /// notice drawn over the running world for a few seconds (the original's presentation of 202 is
    /// not observed yet) and is dismissed in the VM at once.
    fn sync_text_screen(&mut self) {
        if !matches!(self.screen, Screen::World) {
            return;
        }
        let Some((k, blocking)) = self
            .world
            .as_ref()
            .and_then(|w| w.vm.as_ref())
            .and_then(|vm| {
                vm.pending_text_requests()
                    .first()
                    .map(|t| (t.text, t.blocking))
            })
        else {
            return;
        };
        let text = usize::try_from(k)
            .ok()
            .and_then(|i| self.mission_texts.get(i).cloned())
            .unwrap_or_else(|| format!("[text {k}]"));
        let _ = self.ui_assets();
        if blocking {
            self.screen = Screen::Briefing(Briefing::new(vec![text]));
        } else {
            self.notice = Some((text, NOTICE_TICKS));
            if let Some(w) = self.world.as_mut() {
                w.vm_dismiss_text();
            }
        }
        self.frame = None;
    }

    /// Dismiss the script's current text page: closes the parchment if it shows one, lets the VM's
    /// sequence continue and shows the next page if there is one. Returns whether a page was pending.
    fn dismiss_text(&mut self) -> bool {
        if matches!(self.screen, Screen::Briefing(_)) {
            self.screen = Screen::World;
        }
        let dismissed = self.world.as_mut().is_some_and(World::vm_dismiss_text);
        self.frame = None;
        self.sync_text_screen();
        dismissed
    }

    /// The session tick (replay time): `advance` calls since the current world was installed.
    #[must_use]
    pub fn session_tick(&self) -> u64 {
        self.elapsed
    }

    /// Input queued by the window that the session has not consumed (controlled mode ended).
    pub fn take_queued_input(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.queued_input)
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

    /// Restore a snapshot transactionally: the envelope (versions, content identity) is checked,
    /// then the world is validated and restored into a temporary when the scenario changes (so the
    /// session's assets, background and catalog, belong to the snapshot's scenario). The session is
    /// touched only once everything succeeded; a failed restore leaves world, background, screen and
    /// snapshot handles as they were.
    fn restore_snapshot(&mut self, snap: &Snapshot) -> Result<(), RpcError> {
        snap.check_versions().map_err(engine_err)?;
        let expected = self.scenario_content(&snap.world.scenario)?;
        snap.check_content(expected.as_deref())
            .map_err(engine_err)?;
        // The navigation grid is derived state the core rebuilds on restore; its cost (cells,
        // polygons, scan-conversion work) is bounded here, before any world is touched, so a
        // hostile snapshot cannot make the rebuild exhaust time or memory.
        opensherwood_core::NavGrid::check_budget(
            &snap.world.geometry,
            snap.world.map_size.0,
            snap.world.map_size.1,
        )
        .map_err(|e| engine_err(format!("navigation: {e}")))?;
        let same_scenario = self
            .world
            .as_ref()
            .is_some_and(|w| w.scenario == snap.world.scenario);
        if same_scenario {
            // `World::restore` validates against the attached catalog and only then replaces the
            // state and rebuilds the navigation grid. It runs on a copy so the session's world is
            // replaced only once the derived state exists.
            let world = self.world()?;
            let mut candidate = world.clone();
            candidate.restore(snap).map_err(engine_err)?;
            *world = candidate;
        } else {
            let (mut world, background) = self
                .load_scenario(snap.world.scenario.clone(), snap.world.seed)
                .map_err(engine_err)?;
            world.restore(snap).map_err(engine_err)?;
            self.install(world, background);
        }
        self.frame = None;
        Ok(())
    }

    /// Directory of the session's save files (under the artifact directory).
    fn saves_dir(&self) -> PathBuf {
        self.artifacts.join("saves")
    }

    /// The save files, newest first (by modification time).
    fn list_saves(&self) -> Vec<SaveEntry> {
        let Ok(dir) = std::fs::read_dir(self.saves_dir()) else {
            return Vec::new();
        };
        let mut found: Vec<(std::time::SystemTime, SaveEntry)> = Vec::new();
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            if meta.len() as usize > limits::MAX_SAVE_BYTES {
                continue;
            }
            // Only the bookkeeping is needed for the list: read the head of the file.
            let world_tick = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str::<SaveFile>(&t).ok())
                .map_or(0, |f| f.world_tick);
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            found.push((
                modified,
                SaveEntry {
                    name: name.to_string(),
                    world_tick,
                },
            ));
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        found.into_iter().map(|(_, e)| e).collect()
    }

    /// Open the load (or save) screen from the main menu or the pause menu.
    fn open_saves(&mut self, saving: bool, from_pause: bool) {
        let _ = self.ui_assets();
        let strings: Vec<String> = self
            .ui_assets
            .as_ref()
            .map(|a| a.strings.clone())
            .unwrap_or_default();
        let entries = self.list_saves();
        let default_name = format!("save-{}", entries.len() + 1);
        self.screen = Screen::Saves {
            screen: SaveScreen::new(saving, entries, default_name, &strings),
            from_pause,
        };
        self.frame = None;
    }

    /// The profile in use.
    fn profile(&self) -> ProfileSummary {
        self.profiles
            .get(self.profile_index)
            .cloned()
            .unwrap_or_default()
    }

    /// Open the select player screen.
    fn open_select_player(&mut self) {
        let _ = self.ui_assets();
        let strings: Vec<String> = self
            .ui_assets
            .as_ref()
            .map(|a| a.strings.clone())
            .unwrap_or_default();
        let selected = (!self.profiles.is_empty()).then_some(self.profile_index);
        self.screen = Screen::SelectPlayer(SelectPlayerScreen::new(
            self.profiles.clone(),
            selected,
            &strings,
        ));
        self.frame = None;
    }

    /// Write `profiles.json` (`{"format": 1, "selected": i, "profiles": [...]}`).
    fn save_profiles(&mut self) {
        let path = self.artifacts.join("profiles.json");
        let doc = json!({ "format": 1, "selected": self.profile_index, "profiles": self.profiles });
        let result = serde_json::to_string_pretty(&doc)
            .map_err(|e| e.to_string())
            .and_then(|text| write_atomic(&path, &text).map_err(|e| e.to_string()));
        self.note_persistence("profiles", &path, result);
    }

    /// Log a persistence failure and keep it for `observe` (`persistence_error`), so a failed
    /// write is visible to the player and the harness, not only in the log.
    fn note_persistence(&mut self, what: &str, path: &Path, result: Result<(), String>) {
        match result {
            Ok(()) => self.persistence_error = None,
            Err(e) => {
                let msg = format!("{what} {}: {e}", path.display());
                eprintln!("opensherwood: {msg}");
                self.persistence_error = Some(msg);
            }
        }
    }

    /// Read `profiles.json` if present (with `load_settings`, at startup). A malformed or
    /// oversized document is ignored (logged); each profile is clamped to its documented ranges,
    /// unusable ones dropped, and at most `PROFILE_ROWS` are kept.
    fn load_profiles(&mut self) {
        let path = self.artifacts.join("profiles.json");
        let Some(text) = read_bounded(&path, PERSIST_MAX_BYTES) else {
            return;
        };
        let doc = match serde_json::from_str::<Value>(&text) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("opensherwood: profiles {}: {e}; ignored", path.display());
                return;
            }
        };
        // The version envelope is required: exactly the integer 1.
        if doc["format"].as_u64() != Some(1) {
            eprintln!(
                "opensherwood: profiles {}: missing or unknown format; ignored",
                path.display()
            );
            return;
        }
        let list: Vec<ProfileSummary> = doc["profiles"]
            .as_array()
            .into_iter()
            .flatten()
            .take(crate::ui::PROFILE_ROWS)
            .filter_map(|v| serde_json::from_value::<ProfileSummary>(v.clone()).ok())
            .filter_map(ProfileSummary::sanitized)
            .collect();
        if list.is_empty() {
            return;
        }
        self.profile_index = doc["selected"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(0)
            .min(list.len() - 1);
        self.profiles = list;
    }

    /// Open the options screens from the main menu or the pause menu.
    fn open_options(&mut self, from_pause: bool) {
        let _ = self.ui_assets();
        let strings: Vec<String> = self
            .ui_assets
            .as_ref()
            .map(|a| a.strings.clone())
            .unwrap_or_default();
        self.screen = Screen::Options {
            screen: OptionsScreen::new(self.settings.clone(), &strings),
            from_pause,
        };
        self.frame = None;
    }

    /// Apply settings: volumes reach the audio output, the rest is kept (and written to
    /// `settings.json` under the artifact directory).
    fn apply_settings(&mut self, settings: Settings) {
        if let Some(audio) = self.audio.as_mut() {
            audio.set_music_volume(f32::from(settings.volumes[2]) / 10.0);
            audio.set_effects_volume(f32::from(settings.volumes[0]) / 10.0);
        }
        self.settings = settings.sanitized();
        let path = self.artifacts.join("settings.json");
        let result = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| e.to_string())
            .and_then(|text| write_atomic(&path, &text).map_err(|e| e.to_string()));
        self.note_persistence("settings", &path, result);
    }

    /// The stored volumes reach the audio output (at startup and whenever it is opened).
    fn apply_volumes(&mut self) {
        if let Some(audio) = self.audio.as_mut() {
            audio.set_music_volume(f32::from(self.settings.volumes[2]) / 10.0);
            audio.set_effects_volume(f32::from(self.settings.volumes[0]) / 10.0);
        }
    }

    /// Read `settings.json` and `profiles.json` if present (called once at startup by `main`).
    /// A malformed or oversized document is ignored (logged); values are clamped to their ranges.
    pub fn load_settings(&mut self) {
        self.load_profiles();
        let path = self.artifacts.join("settings.json");
        if let Some(text) = read_bounded(&path, PERSIST_MAX_BYTES) {
            // The version envelope first (exactly the integer 1), then the document.
            match serde_json::from_str::<Value>(&text) {
                Ok(doc) if doc["format"].as_u64() == Some(1) => {
                    match serde_json::from_value::<Settings>(doc) {
                        Ok(s) => self.settings = s.sanitized(),
                        Err(e) => {
                            eprintln!("opensherwood: settings {}: {e}; ignored", path.display());
                        }
                    }
                }
                Ok(_) => eprintln!(
                    "opensherwood: settings {}: missing or unknown format; ignored",
                    path.display()
                ),
                Err(e) => eprintln!("opensherwood: settings {}: {e}; ignored", path.display()),
            }
        }
        self.apply_volumes();
    }

    /// Leave the load / save screen the way it was entered.
    fn leave_saves(&mut self, from_pause: bool) {
        if from_pause && self.world.is_some() {
            self.open_pause();
        } else {
            self.open_menu();
        }
    }

    /// Load a save from any screen (the menu has no world; the pause menu holds a paused one).
    fn load_save_any(&mut self, name: &str) -> Result<(), RpcError> {
        if self.recording.is_some() {
            return Err(engine_err(
                "a replay is being recorded; stop it before loading",
            ));
        }
        let path = self.saves_dir().join(format!("{name}.json"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| engine_err(format!("save {}: {e}", path.display())))?;
        if text.len() > limits::MAX_SAVE_BYTES {
            return Err(engine_err("save file too large"));
        }
        let file: SaveFile = serde_json::from_str(&text)
            .map_err(|e| engine_err(format!("save {}: {e}", path.display())))?;
        if file.format != SAVE_FORMAT {
            return Err(engine_err(format!(
                "save format {} is not {SAVE_FORMAT}",
                file.format
            )));
        }
        self.notice = None;
        self.restore_snapshot(&file.snapshot)
    }

    /// Write the current world as a save file (`saves/<name>.json`): the snapshot envelope with the
    /// content identity, so a load checks the game data it was made from. Quick and auto saves are
    /// modern additions (the original saves through its menu); they never run while a screen is
    /// shown or a replay is recorded.
    fn write_save(&mut self, name: &str) -> Result<PathBuf, RpcError> {
        self.require_world_screen()?;
        self.require_no_notice()?;
        let content = self
            .world
            .as_ref()
            .map(|w| self.scenario_content(&w.scenario))
            .transpose()?
            .flatten();
        let world = self.world()?;
        let file = SaveFile {
            format: SAVE_FORMAT,
            world_tick: world.tick,
            snapshot: world.snapshot(content),
        };
        let dir = self.saves_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
        let path = dir.join(format!("{name}.json"));
        let text = serde_json::to_string(&file)
            .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
        std::fs::write(&path, text)
            .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
        Ok(path)
    }

    /// Load a save file written by `write_save` (transactional like `restore`).
    fn load_save(&mut self, name: &str) -> Result<(), RpcError> {
        self.require_world_screen()?;
        self.require_no_notice()?;
        if self.recording.is_some() {
            return Err(engine_err(
                "a replay is being recorded; stop it before loading",
            ));
        }
        let path = self.saves_dir().join(format!("{name}.json"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| engine_err(format!("save {}: {e}", path.display())))?;
        if text.len() > limits::MAX_SAVE_BYTES {
            return Err(engine_err("save file too large"));
        }
        let file: SaveFile = serde_json::from_str(&text)
            .map_err(|e| engine_err(format!("save {}: {e}", path.display())))?;
        if file.format != SAVE_FORMAT {
            return Err(engine_err(format!(
                "save format {} is not {SAVE_FORMAT}",
                file.format
            )));
        }
        self.restore_snapshot(&file.snapshot)
    }

    /// Quick save (F1) and quick load (F5) from the mission, and rolling auto saves every
    /// `AUTOSAVE_TICKS` world ticks (`auto-0` .. `auto-<AUTOSAVE_SLOTS - 1>`, oldest overwritten).
    fn handle_saves(&mut self, events: &[InputEvent]) {
        let f1 = events.iter().any(|e| {
            matches!(
                e,
                InputEvent::KeyDown {
                    key: Key::Function(1)
                }
            )
        });
        let f5 = events.iter().any(|e| {
            matches!(
                e,
                InputEvent::KeyDown {
                    key: Key::Function(5)
                }
            )
        });
        if f1 {
            match self.write_save("quick") {
                Ok(p) => eprintln!("opensherwood: quick save {}", p.display()),
                Err(e) => eprintln!("opensherwood: quick save failed: {}", e.message),
            }
        }
        if f5 {
            match self.load_save("quick") {
                Ok(()) => eprintln!("opensherwood: quick load"),
                Err(e) => eprintln!("opensherwood: quick load failed: {}", e.message),
            }
        }
        let due = self
            .world
            .as_ref()
            .is_some_and(|w| w.tick > 0 && w.tick % AUTOSAVE_TICKS == 0);
        if due && self.recording.is_none() {
            let slot = self.autosave_slot % AUTOSAVE_SLOTS;
            match self.write_save(&format!("auto-{slot}")) {
                Ok(_) => self.autosave_slot = (self.autosave_slot + 1) % AUTOSAVE_SLOTS,
                Err(e) => eprintln!("opensherwood: auto save failed: {}", e.message),
            }
        }
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
        let objective = self.current_objective();
        self.screen = Screen::Pause(PauseMenu::new(objective, strings));
        self.frame = None;
    }

    /// Advance one session tick: menus consume the events without ticking the world; the briefing
    /// pauses the world; otherwise the world steps. Every call is one unit of replay time: the
    /// events are recorded at the session tick they are applied at (screens included), the
    /// session tick then advances, and a checkpoint of the world is recorded when due. This is
    /// the one path `step`, the window and replay playback take, so a replay reproduces the
    /// screens exactly as they were played.
    pub fn advance(&mut self, events: &[InputEvent]) {
        self.record_events(events);
        self.elapsed = self.elapsed.saturating_add(1);
        self.advance_screen(events);
        self.record_checkpoint();
    }

    /// The screen / world part of [`Session::advance`].
    fn advance_screen(&mut self, events: &[InputEvent]) {
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
                    Some(MenuAction::Credits) => {
                        let _ = self.ui_assets();
                        self.screen = Screen::Credits(Credits::new(crate::window::TICK_RATE));
                    }
                    Some(MenuAction::Load) => self.open_saves(false, false),
                    Some(MenuAction::Options) => self.open_options(false),
                    Some(MenuAction::SelectPlayer) => self.open_select_player(),
                    Some(other) => eprintln!("opensherwood: menu action {other:?} not implemented"),
                    None => {}
                }
            }
            Screen::Briefing(b) => {
                let mut done = false;
                let pages = b.pages.len();
                for e in events {
                    if b.handle(*e) {
                        done = true;
                        break;
                    }
                }
                self.frame = None;
                if done {
                    // The parchment presented the script's text page (native 202 / 203): dismiss it
                    // in the VM; the next page, if any, is shown by `sync_text_screen`.
                    let _ = pages;
                    self.dismiss_text();
                }
            }
            Screen::SelectPlayer(screen) => {
                let mut outcome = None;
                for e in events {
                    if let Some(o) = screen.handle(*e) {
                        outcome = Some(o);
                        break;
                    }
                }
                self.frame = None;
                match outcome {
                    None => {}
                    Some(SelectOutcome::Leave) => self.open_menu(),
                    Some(SelectOutcome::Changed) => {
                        self.profiles = screen.profiles.clone();
                        self.profile_index = screen.selected.unwrap_or(0);
                        self.save_profiles();
                    }
                    Some(SelectOutcome::Select(i)) => {
                        self.profiles = screen.profiles.clone();
                        self.profile_index = i.min(self.profiles.len().saturating_sub(1));
                        self.save_profiles();
                        self.open_menu();
                    }
                }
            }
            Screen::Options { screen, from_pause } => {
                let from_pause = *from_pause;
                let mut outcome = None;
                for e in events {
                    if let Some(o) = screen.handle(*e) {
                        outcome = Some(o);
                        break;
                    }
                }
                self.frame = None;
                match outcome {
                    None => {}
                    Some(OptionsOutcome::Back) => self.leave_saves(from_pause),
                    Some(OptionsOutcome::Apply(settings)) => self.apply_settings(settings),
                }
            }
            Screen::Saves { screen, from_pause } => {
                let from_pause = *from_pause;
                let mut outcome = None;
                for e in events {
                    if let Some(o) = screen.handle(*e) {
                        outcome = Some(o);
                        break;
                    }
                }
                self.frame = None;
                match outcome {
                    None => {}
                    Some(SaveOutcome::Cancel) => self.leave_saves(from_pause),
                    Some(SaveOutcome::Save(name)) => {
                        // Saving happens from the pause menu: write the paused world, stay paused.
                        self.screen = Screen::World;
                        match self.write_save(&name) {
                            Ok(_) => self.open_pause(),
                            Err(e) => {
                                eprintln!("opensherwood: save failed: {}", e.message);
                                self.open_pause();
                            }
                        }
                    }
                    Some(SaveOutcome::Delete(name)) => {
                        let path = self.saves_dir().join(format!("{name}.json"));
                        if let Err(e) = std::fs::remove_file(&path) {
                            eprintln!("opensherwood: delete {}: {e}", path.display());
                        }
                        self.open_saves(false, from_pause);
                    }
                    Some(SaveOutcome::Load(name)) => {
                        // Loading replaces the world whatever screen we came from.
                        let had_world = self.world.is_some();
                        self.screen = Screen::World;
                        match self.load_save_any(&name) {
                            Ok(()) => {}
                            Err(e) => {
                                eprintln!("opensherwood: load failed: {}", e.message);
                                if had_world && from_pause {
                                    self.open_pause();
                                } else {
                                    self.open_menu();
                                }
                            }
                        }
                    }
                }
            }
            Screen::Debriefing(b) => {
                let done = events.iter().any(|e| b.handle(*e));
                self.frame = None;
                if done {
                    // After a win the next level of the campaign graph launches automatically
                    // (manual, p.9); without one (or when it cannot load), and after a loss, the
                    // main menu follows.
                    match self.next_mission.clone().filter(|_| !self.ended_lost) {
                        Some(next) => match self.reset(Scenario::Mission(next.clone()), 0) {
                            Ok(()) => {
                                // The successor rule is a hypothesis of the campaign graph
                                // reading: the new world carries the assumption from tick 0.
                                if let Some(w) = self.world.as_mut() {
                                    w.record_assumption(
                                        opensherwood_core::vm::Assumption::CampaignGraph,
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("opensherwood: cannot start {next}: {e}");
                                self.open_menu();
                            }
                        },
                        None => self.open_menu(),
                    }
                }
            }
            Screen::Credits(c) => {
                let leave = events.iter().any(|e| Credits::leaves(*e));
                c.tick();
                self.frame = None;
                if leave {
                    self.open_menu();
                }
            }
            Screen::Lost(page) => {
                let outcome = events.iter().find_map(|e| page.handle(*e));
                self.frame = None;
                match outcome {
                    Some(crate::ui::LostOutcome::Restart) => {
                        if let Some((scenario, seed, money)) = self.current.clone() {
                            self.money_override = money;
                            if let Err(e) = self.reset(scenario, seed) {
                                eprintln!("opensherwood: cannot restart: {e}");
                                self.open_menu();
                            }
                        }
                    }
                    Some(crate::ui::LostOutcome::Load) => self.open_saves(false, false),
                    Some(crate::ui::LostOutcome::Ok) => self.open_menu(),
                    None => {}
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
                    Some(MenuAction::Save) => self.open_saves(true, true),
                    Some(MenuAction::Load) => self.open_saves(false, true),
                    Some(MenuAction::Options) => self.open_options(true),
                    Some(MenuAction::Restart) => {
                        // Restart replays the same reset descriptor, starting money included, so
                        // a replay with a restart does not depend on the profile file.
                        if let Some((scenario, seed, money)) = self.current.clone() {
                            self.money_override = money;
                            if let Err(e) = self.reset(scenario, seed) {
                                eprintln!("opensherwood: cannot restart: {e}");
                            }
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
                // Escape wins the tick: the world does not advance and the tick's other events
                // are dropped, so the pause always lands on the state the player saw.
                if escape_at.is_some() {
                    self.open_pause();
                } else {
                    // Clicks on HUD widgets act on the interface, not on the map: the kneel /
                    // standing figures crouch / stand the selection like the `c` / `s` keys, other
                    // widgets consume the click. Derived from canonical pointer events only, so a
                    // replay reproduces it.
                    let events = if in_mission {
                        self.route_hud_clicks(events)
                    } else {
                        events.to_vec()
                    };
                    let events = events.as_slice();
                    if let Some(world) = self.world.as_mut() {
                        world.step(events);
                        // The cached frame shows the previous tick: a checkpoint or `capture`
                        // after this tick must render the world as it is now.
                        self.frame = None;
                    }
                    if let Some((_, left)) = self.notice.as_mut() {
                        *left = left.saturating_sub(1);
                        if *left == 0 {
                            self.notice = None;
                        }
                        self.frame = None;
                    }
                    if in_mission {
                        self.handle_saves(events);
                    }
                    self.sync_text_screen();
                    self.sync_mission_end();
                }
            }
        }
    }

    /// Replace left clicks on HUD widgets by their interface action (see `ui::hud_hit`).
    fn route_hud_clicks(&mut self, events: &[InputEvent]) -> Vec<InputEvent> {
        let mut pointer = self.world.as_ref().map_or((0, 0), |w| {
            (
                Fixed::from_raw(w.pointer.0).round(),
                Fixed::from_raw(w.pointer.1).round(),
            )
        });
        let mut out = Vec::with_capacity(events.len());
        // A swallowed press whose release arrives in a later tick is swallowed too.
        let mut swallow_up = self.hud_press_pending;
        for e in events {
            match *e {
                InputEvent::PointerMove { x256, y256 } => {
                    pointer = (Fixed::from_raw(x256).round(), Fixed::from_raw(y256).round());
                    out.push(*e);
                }
                // The `;` key toggles the mini-map (`combat-measurements.md` 5) and is an interface
                // key: neither the press nor the release reaches the world.
                InputEvent::KeyDown {
                    key: Key::Semicolon,
                } => {
                    self.minimap_open = !self.minimap_open && self.minimap.is_some();
                }
                InputEvent::KeyUp {
                    key: Key::Semicolon,
                } => {}
                InputEvent::PointerDown {
                    button: opensherwood_core::Button::Left,
                } => match crate::ui::hud_hit(pointer.0, pointer.1) {
                    // Balanced press and release in the same tick: the world's held-key set (part
                    // of the canonical hash) must not keep a key the player never pressed.
                    Some(crate::ui::HudAction::Crouch) => {
                        swallow_up = true;
                        out.push(InputEvent::KeyDown {
                            key: Key::Letter('c'),
                        });
                        out.push(InputEvent::KeyUp {
                            key: Key::Letter('c'),
                        });
                    }
                    Some(crate::ui::HudAction::Stand) => {
                        swallow_up = true;
                        out.push(InputEvent::KeyDown {
                            key: Key::Letter('s'),
                        });
                        out.push(InputEvent::KeyUp {
                            key: Key::Letter('s'),
                        });
                    }
                    Some(crate::ui::HudAction::Map) => {
                        swallow_up = true;
                        self.minimap_open = !self.minimap_open && self.minimap.is_some();
                    }
                    Some(crate::ui::HudAction::Consumed) => swallow_up = true,
                    None => out.push(*e),
                },
                InputEvent::PointerUp {
                    button: opensherwood_core::Button::Left,
                } if swallow_up => swallow_up = false,
                _ => out.push(*e),
            }
        }
        self.hud_press_pending = swallow_up;
        out
    }

    /// Screen state for `observe`.
    fn ui_state(&self) -> Option<opensherwood_protocol::UiState> {
        match &self.screen {
            Screen::World => self.minimap_open.then(crate::ui::minimap_state),
            Screen::Menu(m) => Some(m.state()),
            Screen::Briefing(b) => Some(b.state()),
            Screen::Pause(p) => Some(p.state()),
            Screen::Debriefing(b) => {
                let mut st = b.state();
                st.screen = "debriefing".into();
                Some(st)
            }
            Screen::Saves { screen, .. } => Some(screen.state()),
            Screen::Options { screen, .. } => Some(screen.state()),
            Screen::SelectPlayer(screen) => Some(screen.state()),
            Screen::Credits(c) => Some(c.state()),
            Screen::Lost(page) => Some(page.state()),
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
        self.apply_volumes();
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

    /// Content identity a snapshot or replay of `scenario` carries: `None` for synthetic
    /// scenarios (they need no game data), the game directory's fingerprint otherwise.
    fn scenario_content(&self, scenario: &Scenario) -> Result<Option<String>, RpcError> {
        match scenario {
            Scenario::Synthetic(_) => Ok(None),
            _ => self.content_fingerprint(),
        }
    }

    fn replay_header(&self, world: &World) -> Result<ReplayHeader, RpcError> {
        let fingerprint = self.scenario_content(&world.scenario)?;
        Ok(ReplayHeader {
            replay_version: 1,
            protocol: PROTOCOL_VERSION,
            ruleset: opensherwood_core::RULESET_VERSION,
            content_fingerprint: fingerprint,
            scenario: world.scenario.clone(),
            time: REPLAY_TIME_SESSION.to_string(),
            viewport: world.viewport,
            tick_rate: TICK_RATE,
            hash_schema: opensherwood_core::hash::HASH_SCHEMA_VERSION,
            seed: world.seed,
            rng_streams: std::iter::once((
                "gameplay".to_string(),
                RngStreamInit {
                    algorithm: opensherwood_core::rng::Rng::ALGORITHM.to_string(),
                    seed: world.rng.seed,
                    stream: world.rng.stream,
                },
            ))
            .chain(world.vm.as_ref().map(|vm| {
                (
                    "script".to_string(),
                    RngStreamInit {
                        algorithm: opensherwood_core::rng::Rng::ALGORITHM.to_string(),
                        seed: vm.rng.seed,
                        stream: vm.rng.stream,
                    },
                )
            }))
            .collect(),
            starting_money: world.vm.as_ref().and(self.starting_money),
        })
    }

    /// Reject a `step` that would push the active recording over the `replay_limits` quotas
    /// (event count, checkpoint count, highest tick, serialised bytes). Checked before anything
    /// is mutated so a refused step leaves both the world and the recording untouched. The byte
    /// check is conservative: every event is costed at its longest representation and every
    /// checkpoint the step can add at the recorder's reserve size.
    fn check_recording_quota(&self, ticks: u32, new_events: &[InputEvent]) -> Result<(), RpcError> {
        let Some(rec) = self.recording.as_ref() else {
            return Ok(());
        };
        let quota = |what: &str| {
            RpcError::new(
                RpcError::INVALID_PARAMS,
                format!("step would exceed the replay recording quota ({what}); call replay.stop"),
            )
        };
        if self.session_tick().saturating_add(u64::from(ticks)) > replay_limits::MAX_TICK {
            return Err(quota(&format!("tick {}", replay_limits::MAX_TICK)));
        }
        if rec.recorder.events().len() + new_events.len() > replay_limits::MAX_EVENTS {
            return Err(quota(&format!("{} events", replay_limits::MAX_EVENTS)));
        }
        let planned = u64::from(ticks)
            .checked_div(rec.checkpoint_every)
            .map_or(0, |n| n + 1);
        // `replay.stop` may append one final checkpoint: keep room for it.
        if rec.recorder.checkpoints().len() as u64 + planned + 1
            > replay_limits::MAX_CHECKPOINTS as u64
        {
            return Err(quota(&format!(
                "{} checkpoints",
                replay_limits::MAX_CHECKPOINTS
            )));
        }
        let mut bytes = usize::try_from(planned)
            .unwrap_or(usize::MAX)
            .saturating_mul(rec.recorder.reserve());
        for e in new_events {
            let line = ReplayRecorder::worst_case_event_bytes(*e)
                .map_err(|e| RpcError::new(RpcError::INTERNAL, e))?;
            bytes = bytes.saturating_add(line);
        }
        if !rec.recorder.fits(bytes) {
            return Err(quota(&format!("{} bytes", rec.recorder.max_bytes())));
        }
        Ok(())
    }

    /// Record the events of the session tick about to be advanced, if a replay is being
    /// recorded. Recording stops (and the recording is marked failed) at the `replay_limits`
    /// quotas; RPC `step` refuses such a step up front through
    /// [`Session::check_recording_quota`], this is the backstop for window mode.
    fn record_events(&mut self, events: &[InputEvent]) {
        let tick = self.elapsed;
        let Some(rec) = self.recording.as_mut() else {
            return;
        };
        if rec.failed.is_some() {
            return;
        }
        // The recorder checks every quota (tick, count, ordering, bytes) per event and
        // refuses without mutating; the first refusal ends the recording.
        for (i, e) in events.iter().enumerate() {
            if let Err(why) = rec.recorder.push_event(ReplayEvent {
                tick,
                sequence: i as u32,
                event: *e,
                intent: None,
            }) {
                rec.failed = Some(why);
                break;
            }
        }
    }

    /// After a session tick: record a checkpoint of the world when one is due.
    fn record_checkpoint(&mut self) {
        let tick = self.elapsed;
        let Some(rec) = self.recording.as_ref() else {
            return;
        };
        if rec.failed.is_some()
            || rec.checkpoint_every == 0
            || !tick.is_multiple_of(rec.checkpoint_every)
        {
            return;
        }
        let checkpoint = self.observed_checkpoint(tick);
        if let Some(rec) = self.recording.as_mut()
            && let Err(why) = rec.recorder.push_checkpoint(checkpoint)
        {
            rec.failed = Some(why);
        }
    }

    /// What a checkpoint compares, observed now: the world's tick and hashes (defaults without a
    /// world), the session digest and the framebuffer hash (rendering the frame if needed).
    fn observed_checkpoint(&mut self, tick: u64) -> ReplayCheckpoint {
        let (world_tick, hashes) = self
            .world
            .as_ref()
            .map_or((0, opensherwood_core::Hashes::default()), |w| {
                (w.tick, w.hashes())
            });
        let session = self.session_digest();
        let frame = self.frame().map(Framebuffer::hash).unwrap_or_default();
        ReplayCheckpoint {
            tick,
            world_tick,
            hashes,
            session,
            frame,
        }
    }

    /// Digest of the presentation state a replay must reproduce besides the world: the screen
    /// kind, the `ui` state as `observe` reports it (items, hover, page, scroll) and the notice
    /// text with its remaining ticks. BLAKE3 over an explicit encoding, hex.
    fn session_digest(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(b"opensherwood-session\0");
        let kind: &str = match &self.screen {
            Screen::World => "world",
            Screen::Menu(_) => "menu",
            Screen::Briefing(_) => "briefing",
            Screen::Pause(_) => "pause",
            Screen::Debriefing(_) => "debriefing",
            Screen::Credits(_) => "credits",
            Screen::Lost(_) => "lost",
            Screen::Saves { .. } => "saves",
            Screen::Options { .. } => "options",
            Screen::SelectPlayer(_) => "select_player",
        };
        h.update(kind.as_bytes());
        h.update(b"\0");
        let ui = serde_json::to_vec(&self.ui_state()).unwrap_or_default();
        h.update(&(ui.len() as u64).to_le_bytes());
        h.update(&ui);
        match &self.notice {
            Some((text, left)) => {
                h.update(&[1]);
                h.update(&(text.len() as u64).to_le_bytes());
                h.update(text.as_bytes());
                h.update(&left.to_le_bytes());
            }
            None => {
                h.update(&[0]);
            }
        }
        h.finalize().to_hex().to_string()
    }

    /// Snapshots describe the world only; a notice (native 202) is session presentation that a
    /// snapshot does not carry, so `snapshot` and `restore` wait until it has expired.
    fn require_no_notice(&self) -> Result<(), RpcError> {
        if self.notice.is_some() {
            return Err(engine_err(
                "a notice is shown: step until it expires before snapshot / restore",
            ));
        }
        Ok(())
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
    /// movement), and count the map's part of the script element table
    /// (`opensherwood_script::map_element_count`: `FLIM` plus `TUPO` entries of the `.rhp`).
    /// With `lenient` (see [`lenient_assets`]) a missing or malformed `.rhp` degrades to default
    /// geometry and no element count with a log line; otherwise it is an error.
    fn load_map(
        game: &GameDir,
        map: &str,
        ambiance: &str,
        lenient: bool,
    ) -> Result<(Background, Geometry, Option<u32>), String> {
        let logical = format!("Data/Levels/{ambiance}/{map}.map");
        let data = game.read(&logical).map_err(|e| e.to_string())?;
        let img = opensherwood_formats::image_blob::parse_file(&data)
            .map_err(|e| format!("{logical}: {e}"))?;
        // Size-checked, fallible materialisation: a refused allocation is an error, not an abort.
        let rgba = img.to_rgba8_565().map_err(|e| format!("{logical}: {e}"))?;
        let mut background = Background {
            width: u32::from(img.width),
            height: u32::from(img.height),
            rgba,
            occluders: Vec::new(),
        };
        let mut geometry = Geometry::default();
        let rhp_path = format!("Data/Levels/{map}.rhp");
        // The map's geometry is required: without it every cell of the map would be walkable and
        // nothing would occlude. Only `OPENSHERWOOD_LENIENT_ASSETS=1` accepts that, logged.
        let rhp = game
            .read(&rhp_path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| opensherwood_formats::rhp::parse(&bytes).map_err(|e| e.to_string()));
        let rhp = match rhp {
            Ok(rhp) => Some(rhp),
            Err(e) if lenient => {
                eprintln!(
                    "opensherwood: {rhp_path}: {e}; OPENSHERWOOD_LENIENT_ASSETS: default geometry (everything walkable, no occluders)"
                );
                None
            }
            Err(e) => return Err(format!("{rhp_path}: {e}")),
        };
        let map_elements = rhp.as_ref().map(opensherwood_script::map_element_count);
        if let Some(rhp) = rhp {
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
            geometry.areas = rhp
                .woaw
                .areas
                .iter()
                .map(|a| {
                    a.points
                        .iter()
                        .map(|p| (p.x.round() as i32, p.y.round() as i32))
                        .collect()
                })
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
        Ok((background, geometry, map_elements))
    }

    /// Open the sprite bank once and build a catalog for the given profiles. A sprite bank that
    /// cannot be opened or a profile that cannot be loaded fails the load, unless
    /// [`lenient_assets`] is on: then the failure is logged and the catalog is left without the
    /// affected sets (entities without a set are drawn as discs).
    fn load_catalog(&mut self, profiles: &[String]) -> Result<Catalog, String> {
        let mut catalog = Catalog::default();
        let Some(game) = self.game.as_ref() else {
            return Ok(catalog);
        };
        let lenient = lenient_assets();
        if self.sprites.is_none() {
            match SpriteBank::open(game) {
                Ok(bank) => self.sprites = Some(Sprites { bank }),
                Err(e) if lenient => eprintln!(
                    "opensherwood: sprite bank unavailable: {e}; OPENSHERWOOD_LENIENT_ASSETS: no sprites"
                ),
                Err(e) => return Err(format!("sprite bank: {e}")),
            }
        }
        if self.sprites.is_none() {
            return Ok(catalog);
        }
        for name in profiles {
            match SpriteBank::load_profile(game, name) {
                Ok(profile) => {
                    catalog
                        .sets
                        .insert(name.clone(), anim_set_from_profile(&profile));
                }
                Err(e) if lenient => eprintln!(
                    "opensherwood: profile {name}: {e}; OPENSHERWOOD_LENIENT_ASSETS: drawn without a sprite"
                ),
                Err(e) => return Err(format!("profile {name}: {e}")),
            }
        }
        Ok(catalog)
    }

    /// Load a scenario (what `reset` does). Nothing of the session changes when the load fails.
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
            self.elapsed = 0;
            self.notice = None;
            self.open_menu();
            return Ok(());
        }
        let (world, background) = self.load_scenario(scenario, seed)?;
        self.install(world, background);
        Ok(())
    }

    /// Build the world and background of a scenario without touching the session (only the
    /// sprite bank cache may be filled). Shared by `reset` and a cross-scenario `restore`, so
    /// both can fail before anything observable changes.
    fn load_scenario(
        &mut self,
        scenario: Scenario,
        seed: u64,
    ) -> Result<(World, Option<Background>), String> {
        let loaded = match &scenario {
            Scenario::MapView { map, ambiance } => {
                let game = self
                    .game
                    .as_ref()
                    .ok_or("map scenarios need a game directory")?;
                let (bg, geometry, _) = Self::load_map(game, map, ambiance, lenient_assets())?;
                let info = MapInfo {
                    width: bg.width,
                    height: bg.height,
                };
                let mut world = World::new_map_view(scenario, seed, info)?;
                world.set_geometry(geometry)?;
                let catalog =
                    self.load_catalog(&["RobinHood".to_string(), "Soldier A00".to_string()])?;
                if !catalog.sets.is_empty() {
                    world.attach_catalog(catalog, Some("RobinHood"), Some("Soldier A00"));
                }
                (world, Some(bg))
            }
            Scenario::Mission(name) => {
                let game = self.game.as_ref().ok_or("missions need a game directory")?;
                let (mission_file, map) = mission::load(game, name, lenient_assets())?;
                // The background of the mission's ambiance variant; `Day` when the retail data
                // has no picture for it (`mission::ambiance_for_variant`).
                let variant = mission_file.mission.header.variant;
                let wanted = mission::ambiance_for_variant(variant);
                let ambiance = if game
                    .resolve(&format!("Data/Levels/{wanted}/{map}.map"))
                    .is_some()
                {
                    wanted
                } else {
                    eprintln!(
                        "opensherwood: mission {name}: variant {variant} background Data/Levels/{wanted}/{map}.map unavailable; using Day"
                    );
                    "Day"
                };
                let (bg, geometry, map_elements) =
                    Self::load_map(game, &map, ambiance, lenient_assets())?;
                let info = MapInfo {
                    width: bg.width,
                    height: bg.height,
                };
                let (mut spec, profiles) = mission::build_spec_checked(
                    &mission_file,
                    info,
                    geometry,
                    map_elements,
                    lenient_assets(),
                )?;
                spec.lenient_natives = self.lenient_natives;
                spec.starting_money = self.money_override.take().unwrap_or(self.profile().money);
                let mut world = World::new_mission(scenario, seed, &spec)?;
                // Optional: a level without a mini-map picture has no overlay (logged).
                let minimap = Self::load_minimap(game, &map, ambiance);
                let catalog = self.load_catalog(&profiles)?;
                if !catalog.sets.is_empty() {
                    world.attach_catalog(catalog, None, None);
                }
                self.minimap = minimap;
                self.starting_money = Some(spec.starting_money);
                (world, Some(bg))
            }
            Scenario::Synthetic(_) | Scenario::Menu(_) => {
                self.minimap = None;
                self.money_override = None;
                self.starting_money = None;
                (World::new(scenario, seed)?, None)
            }
        };
        Ok(loaded)
    }

    /// The level's mini-map picture (`Data/Levels/<ambiance>/<map>.min`), `None` when missing or
    /// unreadable (logged): the overlay is presentation, not a load requirement.
    fn load_minimap(game: &GameDir, map: &str, ambiance: &str) -> Option<crate::ui::Minimap> {
        let logical = format!("Data/Levels/{ambiance}/{map}.min");
        let decode = || -> Result<crate::ui::Minimap, String> {
            let data = game.read(&logical).map_err(|e| e.to_string())?;
            let img =
                opensherwood_formats::image_blob::parse_file(&data).map_err(|e| e.to_string())?;
            // The picture is a parchment scroll whose corners hold the UI colour key.
            let frame = crate::ui_assets::keyed_frame(&img).ok_or("undecodable picture")?;
            Ok(crate::ui::Minimap {
                width: frame.width,
                height: frame.height,
                rgba: frame.rgba,
            })
        };
        match decode() {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("opensherwood: {logical}: {e}; no mini-map");
                None
            }
        }
    }

    /// Make a freshly built world the session's world: the screen is the world, the session tick
    /// is 0, and snapshot handles, queued input and any recording belonged to the previous world.
    fn install(&mut self, world: World, background: Option<Background>) {
        self.screen = Screen::World;
        self.current = Some((world.scenario.clone(), world.seed, self.starting_money));

        if let Scenario::Mission(name) = &world.scenario {
            let name = name.clone();
            self.load_mission_texts(&name);
        } else {
            self.mission_texts.clear();
            self.mission_objectives.clear();
        }
        self.world = Some(world);
        self.background = background;
        self.frame = None;
        self.snapshots.clear();
        self.queued_input.clear();
        self.recording = None;
        self.elapsed = 0;
        // Presentation state derived from the installed world, never from session history: no
        // notice of a previous world survives, the HUD is rebuilt.
        self.notice = None;
        // Presentation state derived from the installed world, never from session history.
        self.hud = HudState {
            money: self.profile().money,
            clover: 0,
            hero_name: self.hero_name_lines(),
            arrows: 0,
            purses: 0,
        };
        self.start_scenario_music();
        self.sync_text_screen();
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
                Screen::Credits(c) => c.render(self.ui_assets.as_ref()),
                Screen::Saves { screen, .. } => screen.render(self.ui_assets.as_ref()),
                Screen::Options { screen, .. } => screen.render(self.ui_assets.as_ref()),
                Screen::SelectPlayer(screen) => screen.render(self.ui_assets.as_ref()),
                Screen::Briefing(_)
                | Screen::Debriefing(_)
                | Screen::Pause(_)
                | Screen::Lost(_)
                | Screen::World => {
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
                        // The money counter follows the script's player money (natives 236 / 237,
                        // initialised by the mission script); the profile default only stands in
                        // before the script sets it.
                        let mut hud = self.hud.clone();
                        if let Some(vm) = world.vm.as_ref() {
                            hud.money = vm.money;
                        }
                        // The portrait's counters follow the selected player character (the
                        // first one while nobody is selected).
                        let hero = world
                            .entities
                            .iter()
                            .find(|e| world.selected == Some(e.id) && e.kind == EntityKind::Player)
                            .or_else(|| {
                                world.entities.iter().find(|e| e.kind == EntityKind::Player)
                            });
                        if let Some(h) = hero {
                            hud.arrows = h.arrows;
                            hud.purses = h.purses;
                        }
                        crate::ui::draw_hud(&mut frame, a, &hud);
                        if let Some((text, _)) = &self.notice {
                            crate::ui::draw_notice(&mut frame, a, text);
                        }
                        if self.minimap_open
                            && matches!(self.screen, Screen::World)
                            && let Some(m) = self.minimap.as_ref()
                        {
                            crate::ui::draw_minimap(
                                &mut frame,
                                m,
                                world.camera,
                                crate::ui::MENU_FRAME,
                                world.map_size,
                            );
                        }
                    }
                    match &self.screen {
                        Screen::Briefing(b) | Screen::Debriefing(b) => {
                            b.render(&mut frame, self.ui_assets.as_ref());
                        }
                        Screen::Lost(page) => page.render(&mut frame, self.ui_assets.as_ref()),
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
                    "script".into(),
                ],
                content_fingerprint: self.content_fingerprint()?,
            }),
            "reset" => {
                let p: ResetParams = params_required(p)?;
                self.money_override = p.starting_money;
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
                let planned: Vec<InputEvent> = self
                    .queued_input
                    .iter()
                    .copied()
                    .chain(events.iter().map(|e| e.event))
                    .collect();
                self.check_recording_quota(p.ticks, &planned)?;
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
                    persistence_error: self.persistence_error.clone(),
                })
            }
            "snapshot" => {
                self.require_world_screen()?;
                self.require_no_notice()?;
                let scenario = self.world()?.scenario.clone();
                let content = self.scenario_content(&scenario)?;
                let world = self.world()?;
                let snapshot = world.snapshot(content);
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
                self.require_no_notice()?;
                if self.recording.is_some() {
                    return Err(engine_err(
                        "a replay is being recorded; call replay.stop before restore",
                    ));
                }
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
                self.restore_snapshot(&snap)?;
                let world = self.world()?;
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
                let p: ReplayStartParams = params(p)?;
                // Allowed at session tick 0 only: right after `reset`, even while the mission's
                // first text page is shown (its dismissal is then part of the replay). The main
                // menu has no world to record.
                let world = self
                    .world
                    .as_ref()
                    .ok_or_else(|| engine_err("no world loaded; call reset first"))?;
                if self.session_tick() != 0 {
                    return Err(engine_err(
                        "replay recording must start at session tick 0 (call reset first)",
                    ));
                }
                let header = self.replay_header(world)?;
                let hashes = world.hashes();
                let mut recorder = ReplayRecorder::new(header, &hashes).map_err(engine_err)?;
                // The initial state (world, screen, frame): playback compares it before applying
                // anything.
                let initial = self.observed_checkpoint(0);
                recorder.push_checkpoint(initial).map_err(engine_err)?;
                self.recording = Some(Recording {
                    recorder,
                    checkpoint_every: p.checkpoint_every,
                    failed: None,
                });
                ok(json!({ "recording": true }))
            }
            "replay.stop" => {
                let p: CaptureParams = params(p)?;
                let rec = self
                    .recording
                    .take()
                    .ok_or_else(|| engine_err("no replay is being recorded"))?;
                if let Some(why) = rec.failed {
                    return Err(engine_err(format!("recording discarded: {why}")));
                }
                let tick = self.session_tick();
                let last = self.observed_checkpoint(tick);
                // The final checkpoint goes into the bytes the recorder reserved for it, through
                // the recorder's validated path; the text is then built with fallible allocation
                // and is within `replay_limits::MAX_BYTES` by construction, so the parser
                // accepts it.
                let replay = rec
                    .recorder
                    .finish(last)
                    .map_err(|e| engine_err(format!("recording discarded: {e}")))?;
                let jsonl = replay
                    .to_jsonl()
                    .map_err(|e| engine_err(format!("recording discarded: {e}")))?;
                let mut written = None;
                if let Some(rel) = p.path {
                    let path = self.artifact_path(&rel)?;
                    std::fs::write(&path, &jsonl)
                        .map_err(|e| RpcError::new(RpcError::INTERNAL, e.to_string()))?;
                    written = Some(path.to_string_lossy().to_string());
                }
                ok(ReplayStopResult {
                    events: replay.events.len(),
                    checkpoints: replay.checkpoints.len(),
                    jsonl,
                    path: written,
                })
            }
            "replay.play" => {
                // Playback resets the session to the replay's scenario, so it is allowed from
                // any screen.
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
                self.money_override = replay.header.starting_money;
                self.reset(replay.header.scenario.clone(), replay.header.seed)
                    .map_err(engine_err)?;
                // The header must be exactly what this session would record now: a replay with
                // another viewport, tick rate, RNG stream identity or content is not played
                // against a session that would produce different checkpoints.
                let world = self
                    .world
                    .as_ref()
                    .ok_or_else(|| engine_err("replay scenario has no world to play"))?;
                let expected = self.replay_header(world)?;
                if expected != replay.header {
                    return Err(engine_err(format!(
                        "replay header does not match the session after reset ({})",
                        replay.header.diff(&expected).join(", ")
                    )));
                }
                let mut events = replay.events.iter().peekable();
                let mut checkpoints = replay.checkpoints.iter().peekable();
                let mut checkpoints_ok = 0usize;
                let mut first_divergence = None;
                let mut tick_events: Vec<InputEvent> = Vec::new();
                let mut simulated = 0u64;
                // Session tick `t` (0 = the state right after reset, compared before anything
                // is applied) is reached by advancing with the events recorded at tick `t - 1`,
                // through the same `advance` the recording went through (screens included).
                for tick in 0..=last {
                    if tick > 0 {
                        tick_events.clear();
                        while let Some(e) = events.peek()
                            && e.tick == tick - 1
                        {
                            tick_events.push(e.event);
                            events.next();
                        }
                        self.advance(&tick_events);
                        simulated = tick;
                    }
                    let observed = self.observed_checkpoint(tick);
                    while let Some(c) = checkpoints.peek()
                        && c.tick <= tick
                    {
                        if c.tick == tick {
                            let diff = c.diff(&observed);
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
                let hashes = self.observed_checkpoint(simulated).hashes;
                ok(ReplayPlayResult {
                    ticks: simulated,
                    checkpoints_ok,
                    first_divergence,
                    hashes,
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
                world
                    .try_ensure_nav()
                    .map_err(|e| engine_err(format!("navigation grid: {e}")))?;
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
                    "areas": world.geometry.areas.len(),
                    "obstacles": world.geometry.obstacles.len(),
                    "path_cells": path,
                }))
            }
            "debug.vm" => {
                // Inspection only: text pages are dismissed through the briefing screen with
                // canonical input (Enter / click), never from here. `win` is the one documented
                // harness shortcut (the end-of-mission flow test; `docs/harness.md`).
                #[derive(serde::Deserialize, Default)]
                struct P {
                    /// Mark the mission won (harness shortcut for the end-of-mission flow).
                    #[serde(default)]
                    win: bool,
                    /// Mark the mission lost (the same shortcut for the loss flow).
                    #[serde(default)]
                    lose: bool,
                    /// Describe one entry of the element table (native 3's index space).
                    #[serde(default)]
                    element: Option<i32>,
                }
                let p: P = params(p)?;
                self.world()?;
                if (p.win || p.lose)
                    && let Some(vm) = self.world.as_mut().and_then(|w| w.vm.as_mut())
                {
                    vm.mission_won |= p.win;
                    vm.mission_lost |= p.lose;
                }
                let world = self.world()?;
                let Some(vm) = world.vm.as_ref() else {
                    return ok(json!({ "present": false }));
                };
                let element = p.element.map(|index| {
                    use opensherwood_core::vm::Element;
                    match usize::try_from(index)
                        .ok()
                        .and_then(|i| vm.program.elements.get(i))
                    {
                        Some(Element::Map(i)) => json!({ "kind": "map", "index": i }),
                        Some(Element::Unmodelled(i)) => json!({ "kind": "unmodelled", "index": i }),
                        Some(Element::Actor(e)) => json!({ "kind": "actor", "entity": e }),
                        Some(Element::Object { x, y }) => {
                            json!({ "kind": "object", "x": x, "y": y })
                        }
                        Some(Element::Scroll { x, y }) => {
                            json!({ "kind": "scroll", "x": x, "y": y })
                        }
                        Some(Element::Item { x, y, kind, stack }) => {
                            json!({ "kind": "item", "x": x, "y": y, "item_kind": kind, "stack": stack })
                        }
                        Some(Element::Polygon(l)) => json!({ "kind": "polygon", "location": l }),
                        None => json!(null),
                    }
                });
                ok(json!({
                    "present": true,
                    "classes": vm.program.classes.len(),
                    "elements": vm.program.elements.len(),
                    "element": element,
                    "locations": vm.program.locations.len(),
                    "objectives": vm.objectives,
                    "texts": vm.pending_texts(),
                    "scrolls": vm
                        .program
                        .elements
                        .iter()
                        .enumerate()
                        .filter_map(|(i, e)| match e {
                            opensherwood_core::vm::Element::Scroll { x, y } => Some(json!({
                                "element": i,
                                "x": x,
                                "y": y,
                                "active": !vm.inactive_elements.contains(&(i as i32)),
                            })),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    "items": vm.items(),
                    "mission_won": vm.mission_won,
                    "mission_lost": vm.mission_lost,
                    "tainted": vm.tainted(),
                    "assumptions": vm.assumptions,
                    "money": vm.money,
                    "sequence_active": !vm.sequences.is_empty(),
                    "sequences": vm.sequences.len(),
                    "faulted": vm.faulted(),
                    "fault": vm.fault,
                    "lenient": vm.lenient,
                    "unknown_calls": vm.unknown_calls,
                    "pending_messages": vm.messages.len(),
                    "camera_target": vm.camera_target,
                    "debriefing": vm.debriefing,
                    "mission_vars": vm.mission_vars,
                    "counters": vm.counters,
                    "rng_draws": vm.rng.draws,
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

/// Longest persistence document (`profiles.json`, `settings.json`) that is read.
const PERSIST_MAX_BYTES: u64 = 1 << 20;

/// Read a small document, or `None` when missing, unreadable or over `max` bytes (logged).
fn read_bounded(path: &Path, max: u64) -> Option<String> {
    let len = std::fs::metadata(path).ok()?.len();
    if len > max {
        eprintln!(
            "opensherwood: {}: {len} bytes, at most {max} are read; ignored",
            path.display()
        );
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("opensherwood: {}: {e}; ignored", path.display());
            None
        }
    }
}

/// Write a document atomically: the parent directory is created, the text goes to a temporary
/// file next to the target and is renamed over it, so an interrupted write never truncates the
/// only copy.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod session_tests {
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
        assert_eq!(
            s.recording.as_ref().unwrap().recorder.checkpoints().len(),
            1
        );
        // A step within the quota records normally.
        s.dispatch("step", Some(json!({ "ticks": 10 }))).unwrap();
        // Byte quota: re-arm the recording under a tight byte cap that leaves room for a few
        // events beyond the reserved final checkpoint, then ask for one event too many. The
        // event count quota (2^20) is unreachable in practice: the byte cap binds first.
        let rec = s.recording.as_mut().unwrap();
        let world = s.world.as_ref().unwrap();
        let filler = ReplayEvent {
            tick: 0,
            sequence: 0,
            event: InputEvent::PointerMove { x256: 0, y256: 0 },
            intent: None,
        };
        let header = rec.recorder.header().clone();
        let worst = ReplayRecorder::worst_case_event_bytes(filler.event).unwrap();
        let probe = ReplayRecorder::new(header.clone(), &world.hashes()).unwrap();
        let cap = probe.bytes() + probe.reserve() + 3 * worst;
        rec.recorder = ReplayRecorder::with_max_bytes(header, &world.hashes(), cap).unwrap();
        rec.checkpoint_every = 0;
        let events: Vec<Value> = (0..4)
            .map(|i| {
                json!({ "tick_offset": 0, "sequence": i, "kind": "pointer_move", "x256": 0, "y256": 0 })
            })
            .collect();
        let err = s
            .dispatch("step", Some(json!({ "ticks": 1, "events": events })))
            .unwrap_err();
        assert_eq!(err.code, RpcError::INVALID_PARAMS);
        assert!(err.message.contains("bytes"), "{}", err.message);
        assert_eq!(s.world.as_ref().unwrap().tick, 10);
        assert!(s.recording.as_ref().unwrap().recorder.events().is_empty());
        // Three fit exactly (their real lines are shorter than the worst case).
        let three: Vec<Value> = (0..3)
            .map(|i| {
                json!({ "tick_offset": 0, "sequence": i, "kind": "pointer_move", "x256": 0, "y256": 0 })
            })
            .collect();
        s.dispatch("step", Some(json!({ "ticks": 1, "events": three })))
            .unwrap();
        assert_eq!(s.recording.as_ref().unwrap().recorder.events().len(), 3);
        // The window-mode backstop: stepping directly past the byte quota marks the recording
        // failed and `replay.stop` reports it instead of returning a replay the parser would
        // reject.
        let big = InputEvent::PointerMove {
            x256: i32::MIN,
            y256: i32::MIN,
        };
        s.advance(&[big, big, big, big]);
        assert!(s.recording.as_ref().unwrap().failed.is_some());
        let err = s.dispatch("replay.stop", Some(json!({}))).unwrap_err();
        assert!(err.message.contains("discarded"), "{}", err.message);
        assert!(s.recording.is_none());
    }

    #[test]
    fn stop_never_exceeds_the_byte_cap() {
        // Record right up to a small cap, then stop: the final checkpoint fits in its reserve
        // and the text is within the cap and parses.
        let (mut s, _dir) = session("stop-cap");
        corridor(&mut s);
        s.dispatch("replay.start", Some(json!({ "checkpoint_every": 0 })))
            .unwrap();
        let (header, hashes) = {
            let rec = s.recording.as_ref().unwrap();
            let world = s.world.as_ref().unwrap();
            (rec.recorder.header().clone(), world.hashes())
        };
        let cap = 4096;
        let mut recorder = ReplayRecorder::with_max_bytes(header, &hashes, cap).unwrap();
        let _ = hashes;
        let initial = s.observed_checkpoint(0);
        recorder.push_checkpoint(initial).unwrap();
        s.recording.as_mut().unwrap().recorder = recorder;
        let mut recorded = 0;
        loop {
            let before = s.recording.as_ref().unwrap().recorder.events().len();
            if s
                .dispatch(
                    "step",
                    Some(json!({
                        "ticks": 1,
                        "events": [{ "tick_offset": 0, "sequence": 0, "kind": "pointer_move", "x256": i32::MIN, "y256": i32::MIN }]
                    })),
                )
                .is_err()
            {
                break;
            }
            recorded = s.recording.as_ref().unwrap().recorder.events().len();
            assert_eq!(recorded, before + 1);
        }
        assert!(recorded > 0);
        let stopped = s.dispatch("replay.stop", Some(json!({}))).unwrap();
        let jsonl = stopped["jsonl"].as_str().unwrap();
        assert!(jsonl.len() <= cap, "{} > {cap}", jsonl.len());
        let replay = Replay::from_jsonl(jsonl).unwrap();
        assert_eq!(replay.events.len(), recorded);
        // The initial checkpoint and the final one (session tick = world tick: no screen).
        assert_eq!(replay.checkpoints.len(), 2);
        assert_eq!(replay.checkpoints[0].tick, 0);
        assert_eq!(replay.checkpoints[1].tick, s.session_tick());
        assert_eq!(
            replay.checkpoints[1].world_tick,
            s.world.as_ref().unwrap().tick
        );
    }

    #[test]
    fn restore_refuses_geometry_over_the_navigation_budget() {
        let (mut s, _dir) = session("nav-budget");
        corridor(&mut s);
        s.dispatch("step", Some(json!({ "ticks": 2 }))).unwrap();
        let taken = s.dispatch("snapshot", None).unwrap();
        let before = s.world.as_ref().unwrap().hashes();
        // Within the vertex budget, but the scan conversion would need far more edge tests than
        // `MAX_EDGE_TESTS`: many tall polygons with many edges on a large map.
        let mut v = taken["snapshot"].clone();
        v["world"]["map_size"] = json!([8, 32768]);
        v["world"]["viewport"] = json!([8, 8]);
        let poly: Vec<[i32; 2]> = (0..20_000).map(|i| [i % 7, (i * 13) % 32768]).collect();
        v["world"]["geometry"]["boundary"] = json!(poly);
        v["world"]["geometry"]["obstacles"] = json!([poly, poly]);
        let err = s
            .dispatch("restore", Some(json!({ "snapshot": v })))
            .unwrap_err();
        assert_eq!(err.code, RpcError::ENGINE);
        assert!(err.message.contains("navigation"), "{}", err.message);
        assert!(err.message.contains("edge tests"), "{}", err.message);
        let world = s.world.as_ref().unwrap();
        assert_eq!(world.tick, 2);
        assert_eq!(world.hashes(), before);
        // Too many polygons is refused the same way.
        let mut v = taken["snapshot"].clone();
        let tri = json!([[0, 0], [1, 0], [0, 1]]);
        v["world"]["geometry"]["obstacles"] =
            json!(vec![tri; opensherwood_core::nav::MAX_POLYGONS + 1]);
        let err = s
            .dispatch("restore", Some(json!({ "snapshot": v })))
            .unwrap_err();
        assert!(err.message.contains("polygons"), "{}", err.message);
        assert_eq!(s.world.as_ref().unwrap().hashes(), before);
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
                if v["type"] == "checkpoint" && v["tick"] != json!(0) {
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

    #[test]
    fn restore_is_transactional() {
        let (mut s, _dir) = session("restore");
        corridor(&mut s);
        s.dispatch("step", Some(json!({ "ticks": 5 }))).unwrap();
        let taken = s.dispatch("snapshot", None).unwrap();
        let handle = taken["id"].as_str().unwrap().to_string();
        assert!(
            taken["snapshot"]["content"].is_null(),
            "synthetic snapshots carry no content identity"
        );
        assert_eq!(
            taken["snapshot"]["version"],
            json!(opensherwood_core::world::SNAPSHOT_VERSION)
        );
        s.dispatch("step", Some(json!({ "ticks": 3 }))).unwrap();
        s.queue_input(vec![InputEvent::PointerMove { x256: 1, y256: 1 }]);
        let before = s.world.as_ref().unwrap().hashes();
        let handles: Vec<String> = s.snapshots.keys().cloned().collect();
        let good = taken["snapshot"].clone();
        let mut attempts: Vec<(&str, Value)> = Vec::new();
        // A scenario the session cannot build (no game directory): nothing may change.
        let mut v = good.clone();
        v["world"]["scenario"] = json!({ "mission": "H01_Lin_VL" });
        attempts.push(("mission", v));
        let mut v = good.clone();
        v["world"]["scenario"] = json!({ "map_view": { "map": "sherwood", "ambiance": "Day" } });
        attempts.push(("map", v));
        // Same scenario, invalid world.
        let mut v = good.clone();
        v["world"]["camera"] = json!([5, 0]);
        attempts.push(("camera", v));
        let mut v = good.clone();
        v["world"]["geometry"]["boundary"] = json!([[0, 0], [i32::MAX, 0], [0, i32::MAX]]);
        attempts.push(("geometry", v));
        // Envelope: versions and content identity.
        let mut v = good.clone();
        v["hash_schema"] = json!(99);
        attempts.push(("hash schema", v));
        let mut v = good.clone();
        v["version"] = json!(1);
        attempts.push(("version", v));
        let mut v = good.clone();
        v["content"] = json!("0000");
        attempts.push(("no game content", v));
        for (needle, snapshot) in attempts {
            let err = s
                .dispatch("restore", Some(json!({ "snapshot": snapshot })))
                .unwrap_err();
            assert_eq!(err.code, RpcError::ENGINE, "{needle}: {}", err.message);
            assert!(err.message.contains(needle), "{needle}: {}", err.message);
            let world = s.world.as_ref().unwrap();
            assert_eq!(world.tick, 8, "{needle} changed the world");
            assert_eq!(world.hashes(), before, "{needle} changed the world");
            assert!(matches!(world.scenario, Scenario::Synthetic(_)));
            assert!(s.background.is_none() && matches!(s.screen, Screen::World));
            let now: Vec<String> = s.snapshots.keys().cloned().collect();
            assert_eq!(now, handles, "{needle} touched the snapshot handles");
            assert_eq!(s.queued_input.len(), 1, "{needle} touched the queued input");
        }
        let err = s
            .dispatch("restore", Some(json!({ "id": "snap-nope" })))
            .unwrap_err();
        assert_eq!(err.code, RpcError::INVALID_PARAMS);
        assert_eq!(s.world.as_ref().unwrap().hashes(), before);
        // The valid snapshot restores by handle and inline.
        let r = s
            .dispatch("restore", Some(json!({ "id": handle })))
            .unwrap();
        assert_eq!(r["tick"], json!(5));
        assert_eq!(r["hashes"], taken["hashes"]);
        s.dispatch("step", Some(json!({ "ticks": 3 }))).unwrap();
        let r = s
            .dispatch("restore", Some(json!({ "snapshot": good })))
            .unwrap();
        assert_eq!(r["hashes"], taken["hashes"]);
    }

    #[test]
    fn notice_blocks_snapshot_and_restore_and_never_survives_install() {
        let (mut s, _dir) = session("notice");
        corridor(&mut s);
        s.dispatch("step", Some(json!({ "ticks": 2 }))).unwrap();
        let taken = s.dispatch("snapshot", None).unwrap();
        let handle = taken["id"].as_str().unwrap().to_string();
        let digest_without = s.session_digest();
        s.notice = Some(("hello".to_string(), 30));
        assert_ne!(
            s.session_digest(),
            digest_without,
            "the notice is in the session digest"
        );
        for method in ["snapshot", "restore"] {
            let err = s
                .dispatch(method, Some(json!({ "id": handle })))
                .unwrap_err();
            assert_eq!(err.code, RpcError::ENGINE, "{method}");
            assert!(err.message.contains("notice"), "{method}: {}", err.message);
        }
        // Every reset path installs a world without the previous world's notice.
        corridor(&mut s);
        assert!(s.notice.is_none());
        assert_eq!(s.session_digest(), digest_without);
        s.notice = Some(("hello".to_string(), 30));
        s.dispatch("reset", Some(json!({ "scenario": { "menu": "main" } })))
            .unwrap();
        assert!(s.notice.is_none(), "the menu path clears it too");
        corridor(&mut s);
        s.dispatch("step", Some(json!({ "ticks": 2 }))).unwrap();
        s.notice = Some(("hello".to_string(), 30));
        // The notice counts down with the world and is gone after its ticks; then both work again.
        s.dispatch("step", Some(json!({ "ticks": 30 }))).unwrap();
        assert!(s.notice.is_none());
        let taken = s.dispatch("snapshot", None).unwrap();
        s.dispatch("restore", Some(json!({ "id": taken["id"] })))
            .unwrap();
    }

    #[test]
    fn failed_reset_leaves_the_session_untouched() {
        let (mut s, _dir) = session("reset");
        corridor(&mut s);
        s.dispatch("step", Some(json!({ "ticks": 4 }))).unwrap();
        s.dispatch("snapshot", None).unwrap();
        let err = s
            .dispatch(
                "reset",
                Some(json!({ "scenario": { "mission": "H01_Lin_VL" }, "seed": 1 })),
            )
            .unwrap_err();
        assert_eq!(err.code, RpcError::ENGINE);
        assert_eq!(s.world.as_ref().unwrap().tick, 4);
        assert_eq!(s.snapshots.len(), 1);
        let err = s
            .dispatch(
                "reset",
                Some(json!({ "scenario": { "synthetic": "nope" }, "seed": 1 })),
            )
            .unwrap_err();
        assert_eq!(err.code, RpcError::ENGINE);
        assert_eq!(s.world.as_ref().unwrap().tick, 4);
    }
}
