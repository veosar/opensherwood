//! Engine session: world + assets + RPC method dispatch, shared by headless and window modes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use opensherwood_assets::GameDir;
use opensherwood_core::{InputEvent, MapInfo, Scenario, Snapshot, World};
use opensherwood_protocol::{
    CaptureParams, CaptureResult, HelloResult, ObserveResult, PROTOCOL_VERSION, ResetParams,
    RestoreParams, RpcError, SnapshotResult, StepParams, StepResult,
};
use opensherwood_render::{Background, Framebuffer, render};
use serde_json::{Value, json};

/// Result of an RPC method.
pub type RpcResult = Result<Value, RpcError>;

/// One engine instance.
pub struct Session {
    game: Option<GameDir>,
    artifacts: PathBuf,
    /// The world, if a scenario is loaded.
    pub world: Option<World>,
    background: Option<Background>,
    snapshots: BTreeMap<String, Snapshot>,
    next_snapshot: u64,
    frame: Option<Framebuffer>,
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
            snapshots: BTreeMap::new(),
            next_snapshot: 0,
            frame: None,
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
                (World::new_map_view(scenario, seed, info)?, Some(bg))
            }
            _ => (World::new(scenario, seed)?, None),
        };
        self.world = Some(world);
        self.background = background;
        self.frame = None;
        Ok(())
    }

    /// Advance one tick with the given events (window mode).
    pub fn tick(&mut self, events: &[InputEvent]) {
        if let Some(w) = self.world.as_mut() {
            w.step(events);
            self.frame = None;
        }
    }

    /// The current frame, rendering it if needed.
    pub fn frame(&mut self) -> Option<&Framebuffer> {
        let world = self.world.as_ref()?;
        if self.frame.is_none() {
            self.frame = Some(render(world, self.background.as_ref()));
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
                if p.ticks == 0 || p.ticks > 1_000_000 {
                    return Err(RpcError::new(
                        RpcError::INVALID_PARAMS,
                        "ticks must be in 1..=1000000",
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
                let world = self.world()?;
                let mut per_tick = Vec::new();
                let mut cursor = 0usize;
                let mut tick_events: Vec<InputEvent> = Vec::new();
                for offset in 0..p.ticks {
                    tick_events.clear();
                    while cursor < events.len() && events[cursor].tick_offset == offset {
                        tick_events.push(events[cursor].event);
                        cursor += 1;
                    }
                    world.step(&tick_events);
                    if p.hash_every_tick {
                        per_tick.push(world.hashes());
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
                let world = self.world()?;
                ok(ObserveResult {
                    observation: world.observe(),
                    hashes: world.hashes(),
                })
            }
            "snapshot" => {
                let world = self.world()?;
                let snapshot = world.snapshot();
                let hashes = world.hashes();
                self.next_snapshot += 1;
                let id = format!("snap-{}", self.next_snapshot);
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
                let world = self.world.get_or_insert_with(|| snap.world.clone());
                world.restore(&snap).map_err(engine_err)?;
                self.frame = None;
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
            "shutdown" => ok(json!({ "ok": true })),
            _ => Err(RpcError::new(
                RpcError::METHOD_NOT_FOUND,
                format!("unknown method '{method}'"),
            )),
        }
    }
}
