//! Engine session: world + assets + RPC method dispatch, shared by headless and window modes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use std::sync::Arc;

use opensherwood_assets::{GameDir, SpriteBank};
use opensherwood_core::{
    AnimSet, Catalog, FrameSpec, InputEvent, MapInfo, Scenario, Snapshot, World,
};
use opensherwood_protocol::{
    CaptureParams, CaptureResult, HelloResult, ObserveParams, ObserveResult, PROTOCOL_VERSION,
    Replay, ReplayCheckpoint, ReplayEvent, ReplayHeader, ReplayPlayParams, ReplayPlayResult,
    ReplayStartParams, ReplayStopResult, ResetParams, RestoreParams, RngStreamInit, RpcError,
    SnapshotResult, StepParams, StepResult,
};
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
}

/// Tick rate used by replays and the window (ticks per second).
pub const TICK_RATE: (u32, u32) = (60, 1);

struct Recording {
    replay: Replay,
    checkpoint_every: u64,
}

/// Limits that keep a hostile client from exhausting memory or time.
pub mod limits {
    /// Most ticks in one `step`.
    pub const MAX_TICKS: u32 = 100_000;
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
        }
    }

    fn replay_header(&self, world: &World) -> ReplayHeader {
        let fingerprint = match world.scenario {
            Scenario::Synthetic(_) => None,
            _ => self.game.as_ref().map(GameDir::fingerprint),
        };
        ReplayHeader {
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
        }
    }

    /// Run one tick, recording it if a replay is being recorded.
    fn step_recorded(&mut self, events: &[InputEvent]) {
        let Some(world) = self.world.as_mut() else {
            return;
        };
        let tick = world.tick;
        if let Some(rec) = self.recording.as_mut() {
            for (i, e) in events.iter().enumerate() {
                rec.replay.events.push(ReplayEvent {
                    tick,
                    sequence: i as u32,
                    event: *e,
                    intent: None,
                });
            }
        }
        world.step(events);
        if let Some(rec) = self.recording.as_mut()
            && rec.checkpoint_every > 0
            && world.tick.is_multiple_of(rec.checkpoint_every)
        {
            rec.replay.checkpoints.push(ReplayCheckpoint {
                tick: world.tick,
                hashes: world.hashes(),
            });
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
            Some(name) => Ok(Scenario::Synthetic(name.to_string())),
            None => Err("empty scenario".into()),
        }
    }

    /// Load a scenario (what `reset` does).
    pub fn reset(&mut self, scenario: Scenario, seed: u64) -> Result<(), String> {
        let (world, background) = match &scenario {
            Scenario::MapView { map, ambiance } => {
                let game = self
                    .game
                    .as_ref()
                    .ok_or("map scenarios need a game directory")?;
                let logical = format!("Data/Levels/{ambiance}/{map}.map");
                let data = game.read(&logical).map_err(|e| e.to_string())?;
                let img = opensherwood_formats::image_blob::parse_file(&data)
                    .map_err(|e| format!("{logical}: {e}"))?;
                let bg = Background {
                    width: u32::from(img.width),
                    height: u32::from(img.height),
                    rgba: img.to_rgba8_565(),
                };
                let info = MapInfo {
                    width: bg.width,
                    height: bg.height,
                };
                let mut world = World::new_map_view(scenario, seed, info)?;
                if self.sprites.is_none() {
                    match SpriteBank::open(game) {
                        Ok(bank) => self.sprites = Some(Sprites { bank }),
                        Err(e) => eprintln!("opensherwood: sprite bank unavailable: {e}"),
                    }
                }
                if self.sprites.is_some() {
                    let mut catalog = Catalog::default();
                    for name in ["RobinHood", "Soldier A00"] {
                        match SpriteBank::load_profile(game, name) {
                            Ok(profile) => {
                                catalog
                                    .sets
                                    .insert(name.to_string(), anim_set_from_profile(&profile));
                            }
                            Err(e) => eprintln!("opensherwood: profile {name}: {e}"),
                        }
                    }
                    world.attach_catalog(catalog, Some("RobinHood"), Some("Soldier A00"));
                }
                (world, Some(bg))
            }
            _ => (World::new(scenario, seed)?, None),
        };
        self.world = Some(world);
        self.background = background;
        self.frame = None;
        self.snapshots.clear();
        self.queued_input.clear();
        self.recording = None;
        Ok(())
    }

    /// Advance one tick with the given events (window mode).
    pub fn tick(&mut self, events: &[InputEvent]) {
        if let Some(w) = self.world.as_mut() {
            w.step(events);
            self.frame = None;
        }
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
        let world = self.world.as_ref()?;
        if self.frame.is_none() {
            let frame = match self.sprites.as_mut() {
                Some(s) => render(world, self.background.as_ref(), s),
                None => render(world, self.background.as_ref(), &mut NoSprites),
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
                ],
                content_fingerprint: self.game.as_ref().map(GameDir::fingerprint),
            }),
            "reset" => {
                let p: ResetParams = params_required(p)?;
                self.reset(p.scenario, p.seed).map_err(engine_err)?;
                let world = self.world()?;
                ok(json!({ "tick": world.tick, "hashes": world.hashes() }))
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
                let queued = std::mem::take(&mut self.queued_input);
                self.world()?;
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
                    self.step_recorded(&tick_events);
                    if p.hash_every_tick {
                        per_tick.push(self.world()?.hashes());
                    }
                }
                self.frame = None;
                let world = self.world()?;
                ok(StepResult {
                    tick: world.tick,
                    hashes: world.hashes(),
                    per_tick,
                })
            }
            "observe" => {
                let p: ObserveParams = params(p)?;
                let world = self.world()?;
                ok(ObserveResult {
                    observation: world.observe(p.entities),
                    hashes: world.hashes(),
                })
            }
            "snapshot" => {
                let world = self.world()?;
                let snapshot = world.snapshot();
                let hashes = world.hashes();
                self.next_snapshot += 1;
                let id = format!("snap-{}", self.next_snapshot);
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
                    .ok_or_else(|| engine_err("no world loaded; call reset first"))?;
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
                let world = self
                    .world
                    .as_ref()
                    .ok_or_else(|| engine_err("no world loaded; call reset first"))?;
                if world.tick != 0 {
                    return Err(engine_err(
                        "replay recording must start at tick 0 (call reset first)",
                    ));
                }
                let header = self.replay_header(world);
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
                });
                ok(json!({ "recording": true }))
            }
            "replay.stop" => {
                let p: CaptureParams = params(p)?;
                let mut rec = self
                    .recording
                    .take()
                    .ok_or_else(|| engine_err("no replay is being recorded"))?;
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
                let p: ReplayPlayParams = params(p)?;
                let text = match (p.jsonl, p.path) {
                    (Some(t), _) => t,
                    (None, Some(rel)) => {
                        let path = self.artifact_path(&rel)?;
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
                if let Some(fp) = &replay.header.content_fingerprint
                    && self.game.as_ref().map(GameDir::fingerprint).as_ref() != Some(fp)
                {
                    return Err(engine_err(
                        "replay was recorded with different game content",
                    ));
                }
                self.reset(replay.header.scenario.clone(), replay.header.seed)
                    .map_err(engine_err)?;
                let last = replay.last_tick();
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
            "shutdown" => ok(json!({ "ok": true })),
            _ => Err(RpcError::new(
                RpcError::METHOD_NOT_FOUND,
                format!("unknown method '{method}'"),
            )),
        }
    }
}
