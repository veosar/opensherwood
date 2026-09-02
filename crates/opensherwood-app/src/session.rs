//! JSON-RPC session over stdio.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use opensherwood_assets::GameDir;
use opensherwood_core::{InputEvent, Snapshot, World};
use opensherwood_protocol::{
    CaptureParams, CaptureResult, HelloResult, ObserveResult, PROTOCOL_VERSION, Request,
    ResetParams, Response, RestoreParams, RpcError, SnapshotResult, StepParams, StepResult,
};
use opensherwood_render::{Framebuffer, render};
use serde_json::{Value, json};

struct Session {
    game: Option<GameDir>,
    artifacts: PathBuf,
    world: Option<World>,
    snapshots: BTreeMap<String, Snapshot>,
    next_snapshot: u64,
    frame: Option<Framebuffer>,
}

type RpcResult = Result<Value, RpcError>;

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

impl Session {
    fn world(&mut self) -> Result<&mut World, RpcError> {
        self.world
            .as_mut()
            .ok_or_else(|| RpcError::new(RpcError::ENGINE, "no world loaded; call reset first"))
    }

    fn dispatch(&mut self, method: &str, p: Option<Value>) -> RpcResult {
        match method {
            "hello" => ok(HelloResult {
                protocol: PROTOCOL_VERSION,
                build: env!("CARGO_PKG_VERSION").to_string(),
                ruleset: opensherwood_core::RULESET_VERSION,
                capabilities: vec!["synthetic".into(), "capture".into(), "snapshot".into()],
                content_fingerprint: self.game.as_ref().map(GameDir::fingerprint),
            }),
            "reset" => {
                let p: ResetParams = params_required(p)?;
                let world = World::new(p.scenario, p.seed)
                    .map_err(|e| RpcError::new(RpcError::ENGINE, e))?;
                self.frame = None;
                let hashes = world.hashes();
                self.world = Some(world);
                ok(json!({ "tick": 0, "hashes": hashes }))
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
                world
                    .restore(&snap)
                    .map_err(|e| RpcError::new(RpcError::ENGINE, e))?;
                self.frame = None;
                ok(json!({ "tick": world.tick, "hashes": world.hashes() }))
            }
            "capture" => {
                let p: CaptureParams = params(p)?;
                if self.frame.is_none() {
                    let world = self.world.as_ref().ok_or_else(|| {
                        RpcError::new(RpcError::ENGINE, "no world loaded; call reset first")
                    })?;
                    self.frame = Some(render(world));
                }
                let frame = self.frame.as_ref().expect("frame rendered above");
                let mut written = None;
                if let Some(rel) = p.path {
                    if rel.contains("..") || PathBuf::from(&rel).is_absolute() {
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

/// Serve requests from stdin until EOF or `shutdown`.
pub fn serve_stdio(game: Option<GameDir>, artifacts: PathBuf) -> anyhow::Result<()> {
    let mut session = Session {
        game,
        artifacts,
        world: None,
        snapshots: BTreeMap::new(),
        next_snapshot: 0,
        frame: None,
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Err(e) => Response::err(Value::Null, RpcError::new(RpcError::PARSE, e.to_string())),
            Ok(req) => {
                let id = req.id.clone().unwrap_or(Value::Null);
                let is_shutdown = req.method == "shutdown";
                let resp = match session.dispatch(&req.method, req.params) {
                    Ok(v) => Response::ok(id, v),
                    Err(e) => Response::err(id, e),
                };
                if is_shutdown {
                    serde_json::to_writer(&mut out, &resp)?;
                    out.write_all(b"\n")?;
                    out.flush()?;
                    return Ok(());
                }
                resp
            }
        };
        serde_json::to_writer(&mut out, &response)?;
        out.write_all(b"\n")?;
        out.flush()?;
    }
    Ok(())
}
