//! Wire types for the harness protocol (ADR-0004, `docs/harness.md`) and the `ReplayV1` format.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use opensherwood_core::{Hashes, InputEvent, Observation, Scenario, Snapshot};

/// Protocol version reported by `hello`.
pub const PROTOCOL_VERSION: u32 = 3;

/// Limits of a replay file (format-wide; checked while parsing and recording).
pub mod replay_limits {
    /// Most bytes of JSON Lines accepted.
    pub const MAX_BYTES: usize = 64 * 1024 * 1024;
    /// Most events.
    pub const MAX_EVENTS: usize = 1 << 20;
    /// Most checkpoints.
    pub const MAX_CHECKPOINTS: usize = 1 << 16;
    /// Highest tick a replay may reach.
    pub const MAX_TICK: u64 = 1 << 24;
}

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Always "2.0".
    pub jsonrpc: String,
    /// Id (absent for notifications).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// Method name.
    pub method: String,
    /// Parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Code.
    pub code: i32,
    /// Message.
    pub message: String,
    /// Extra data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// Standard codes.
    pub const PARSE: i32 = -32700;
    /// Invalid request.
    pub const INVALID_REQUEST: i32 = -32600;
    /// Unknown method.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Bad params.
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal.
    pub const INTERNAL: i32 = -32603;
    /// Engine-level failure (no world loaded, bad scenario, ...).
    pub const ENGINE: i32 = -32000;

    /// Construct.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Always "2.0".
    pub jsonrpc: String,
    /// Id echoed from the request.
    pub id: Value,
    /// Result on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    /// Success.
    #[must_use]
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Failure.
    #[must_use]
    pub fn err(id: Value, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// `hello` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloResult {
    /// Protocol version.
    pub protocol: u32,
    /// Engine build version.
    pub build: String,
    /// Ruleset version.
    pub ruleset: u32,
    /// Capabilities.
    pub capabilities: Vec<String>,
    /// Content fingerprint of the game directory, if one is attached.
    pub content_fingerprint: Option<String>,
}

/// `reset` params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetParams {
    /// Scenario.
    pub scenario: Scenario,
    /// Seed.
    #[serde(default)]
    pub seed: u64,
}

/// One event scheduled inside a `step`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimedEvent {
    /// Tick offset from the first tick of the step.
    pub tick_offset: u32,
    /// Order inside the tick.
    pub sequence: u32,
    /// The event.
    #[serde(flatten)]
    pub event: InputEvent,
}

/// `step` params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepParams {
    /// Ticks to advance (>= 1).
    pub ticks: u32,
    /// Events to inject.
    #[serde(default)]
    pub events: Vec<TimedEvent>,
    /// Return the hashes after every tick (for triage).
    #[serde(default)]
    pub hash_every_tick: bool,
}

/// `step` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Tick after stepping.
    pub tick: u64,
    /// Hashes after the last tick.
    pub hashes: Hashes,
    /// Per-tick hashes if requested.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub per_tick: Vec<Hashes>,
}

/// `observe` params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserveParams {
    /// Include entities (default true).
    #[serde(default = "default_true")]
    pub entities: bool,
}

impl Default for ObserveParams {
    fn default() -> Self {
        Self { entities: true }
    }
}

fn default_true() -> bool {
    true
}

/// `observe` result. The world fields are flattened and absent while no world is loaded (main menu).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserveResult {
    /// Observation of the world, if one is loaded.
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<Observation>,
    /// Hashes of the world (all empty strings without a world).
    pub hashes: Hashes,
    /// Active app screen, `None` while the world is played directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiState>,
}

/// A clickable element of a screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiItem {
    /// Action identifier (`play`, `load`, `continue`, `quit`, `yes`, `no`, ...).
    pub action: String,
    /// Label as displayed (from the player's files).
    pub label: String,
    /// Rectangle in logical pixels: x, y, width, height.
    pub rect: [i32; 4],
    /// Whether the element reacts to clicks.
    pub enabled: bool,
}

/// State of the screen shown over (or instead of) the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiState {
    /// `main_menu`, `pause_menu`, `briefing` or `dialog`.
    pub screen: String,
    /// Elements.
    pub items: Vec<UiItem>,
    /// Hovered element index.
    #[serde(default)]
    pub hovered: Option<usize>,
    /// Briefing page (1-based) and page count.
    #[serde(default)]
    pub page: Option<[usize; 2]>,
}

/// `snapshot` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotResult {
    /// Server-side handle.
    pub id: String,
    /// The snapshot itself (so clients can store it and pass it back).
    pub snapshot: Snapshot,
    /// Hashes at snapshot time.
    pub hashes: Hashes,
}

/// `restore` params: either a handle or a snapshot value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreParams {
    /// Handle from a previous `snapshot`.
    #[serde(default)]
    pub id: Option<String>,
    /// Inline snapshot.
    #[serde(default)]
    pub snapshot: Option<Snapshot>,
}

/// `capture` params.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaptureParams {
    /// File name (relative to the artifact directory) to write a PNG to.
    #[serde(default)]
    pub path: Option<String>,
}

/// `capture` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    /// BLAKE3 of the RGBA framebuffer.
    pub hash: String,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Absolute path written, if any.
    pub path: Option<String>,
}

/// `replay.start` params.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplayStartParams {
    /// Record a checkpoint (all hashes) every N ticks (0 = only at stop).
    #[serde(default)]
    pub checkpoint_every: u64,
}

/// `replay.stop` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayStopResult {
    /// The replay as JSON Lines.
    pub jsonl: String,
    /// Events recorded.
    pub events: usize,
    /// Checkpoints recorded.
    pub checkpoints: usize,
    /// Path written, if `path` was given.
    pub path: Option<String>,
}

/// `replay.play` params: the replay text (or a file under the artifact directory).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplayPlayParams {
    /// Replay JSON Lines.
    #[serde(default)]
    pub jsonl: Option<String>,
    /// Relative path under the artifact directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Stop at the first checkpoint mismatch (default true).
    #[serde(default = "default_true")]
    pub stop_on_divergence: bool,
}

/// `replay.play` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayPlayResult {
    /// Ticks simulated.
    pub ticks: u64,
    /// Checkpoints that matched.
    pub checkpoints_ok: usize,
    /// First checkpoint that did not match: (tick, differing parts).
    pub first_divergence: Option<(u64, Vec<String>)>,
    /// Hashes at the end.
    pub hashes: Hashes,
}

/// `ReplayV1` header line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayHeader {
    /// Always 1.
    pub replay_version: u32,
    /// Protocol version.
    pub protocol: u32,
    /// Ruleset version.
    pub ruleset: u32,
    /// Content fingerprint (None for synthetic scenarios).
    pub content_fingerprint: Option<String>,
    /// Scenario.
    pub scenario: Scenario,
    /// Logical viewport.
    pub viewport: (u32, u32),
    /// Tick rate as a rational (numerator, denominator) in Hz.
    pub tick_rate: (u32, u32),
    /// Hash schema the checkpoints were produced with.
    pub hash_schema: u32,
    /// Seed.
    pub seed: u64,
    /// Named RNG streams and their initial (seed, stream id).
    pub rng_streams: std::collections::BTreeMap<String, RngStreamInit>,
}

/// Initial state of one named RNG stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RngStreamInit {
    /// Algorithm name (`pcg32`).
    pub algorithm: String,
    /// Seed.
    pub seed: u64,
    /// Stream id.
    pub stream: u64,
}

impl ReplayHeader {
    /// Check the header's versions and values.
    pub fn validate(&self) -> Result<(), String> {
        if self.replay_version != 1 {
            return Err(format!(
                "unsupported replay version {}",
                self.replay_version
            ));
        }
        if self.protocol != PROTOCOL_VERSION {
            return Err(format!(
                "replay protocol {} != {PROTOCOL_VERSION}",
                self.protocol
            ));
        }
        if self.ruleset != opensherwood_core::RULESET_VERSION {
            return Err(format!(
                "replay ruleset {} != {}",
                self.ruleset,
                opensherwood_core::RULESET_VERSION
            ));
        }
        if self.hash_schema != opensherwood_core::hash::HASH_SCHEMA_VERSION {
            return Err(format!(
                "replay hash schema {} is not current",
                self.hash_schema
            ));
        }
        if self.tick_rate.0 == 0 || self.tick_rate.1 == 0 {
            return Err("tick rate must be a positive rational".into());
        }
        if self.viewport.0 == 0 || self.viewport.1 == 0 {
            return Err("viewport must be non-zero".into());
        }
        if !matches!(self.scenario, Scenario::Synthetic(_)) && self.content_fingerprint.is_none() {
            return Err("replays of game-data scenarios need a content fingerprint".into());
        }
        if self.rng_streams.is_empty() {
            return Err("at least one named RNG stream is required".into());
        }
        Ok(())
    }
}

/// `ReplayV1` event line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayEvent {
    /// Absolute tick.
    pub tick: u64,
    /// Order inside the tick.
    pub sequence: u32,
    /// Event.
    #[serde(flatten)]
    pub event: InputEvent,
    /// Free-text annotation (never authoritative).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

/// `ReplayV1` checkpoint expectation line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayCheckpoint {
    /// Tick after which the hashes must match.
    pub tick: u64,
    /// Expected hashes.
    pub hashes: Hashes,
}

/// A line of a replay file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReplayLine {
    /// Header (first line).
    Header(ReplayHeader),
    /// Event.
    Event(ReplayEvent),
    /// Checkpoint.
    Checkpoint(ReplayCheckpoint),
}

/// A replay in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    /// Header.
    pub header: ReplayHeader,
    /// Events sorted by (tick, sequence).
    pub events: Vec<ReplayEvent>,
    /// Checkpoints sorted by tick.
    pub checkpoints: Vec<ReplayCheckpoint>,
}

impl Replay {
    /// Parse JSON Lines: the header must come first, events must be strictly ordered by
    /// `(tick, sequence)`, checkpoints strictly by tick, and all versions must be current.
    pub fn from_jsonl(text: &str) -> Result<Self, String> {
        if text.len() > replay_limits::MAX_BYTES {
            return Err("replay exceeds the size limit".into());
        }
        let mut header: Option<ReplayHeader> = None;
        let mut events: Vec<ReplayEvent> = Vec::new();
        let mut checkpoints: Vec<ReplayCheckpoint> = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: ReplayLine =
                serde_json::from_str(line).map_err(|e| format!("line {}: {e}", n + 1))?;
            match parsed {
                ReplayLine::Header(h) => {
                    if header.is_some() || !events.is_empty() || !checkpoints.is_empty() {
                        return Err(format!("line {}: header must be the first line", n + 1));
                    }
                    h.validate().map_err(|e| format!("line {}: {e}", n + 1))?;
                    header = Some(h);
                }
                ReplayLine::Event(e) => {
                    if header.is_none() {
                        return Err(format!("line {}: event before header", n + 1));
                    }
                    if e.tick >= replay_limits::MAX_TICK
                        || events.len() >= replay_limits::MAX_EVENTS
                    {
                        return Err(format!("line {}: replay too long", n + 1));
                    }
                    if let Some(prev) = events.last()
                        && (e.tick, e.sequence) <= (prev.tick, prev.sequence)
                    {
                        return Err(format!(
                            "line {}: events must be strictly ordered by (tick, sequence)",
                            n + 1
                        ));
                    }
                    events.push(e);
                }
                ReplayLine::Checkpoint(c) => {
                    if header.is_none() {
                        return Err(format!("line {}: checkpoint before header", n + 1));
                    }
                    if c.tick > replay_limits::MAX_TICK
                        || checkpoints.len() >= replay_limits::MAX_CHECKPOINTS
                    {
                        return Err(format!("line {}: replay too long", n + 1));
                    }
                    if let Some(prev) = checkpoints.last()
                        && c.tick <= prev.tick
                    {
                        return Err(format!(
                            "line {}: checkpoints must be strictly ordered",
                            n + 1
                        ));
                    }
                    checkpoints.push(c);
                }
            }
        }
        let header = header.ok_or("missing header line")?;
        Ok(Self {
            header,
            events,
            checkpoints,
        })
    }

    /// Serialise to JSON Lines.
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        let mut push = |l: &ReplayLine| {
            out.push_str(&serde_json::to_string(l).expect("replay lines are serialisable"));
            out.push('\n');
        };
        push(&ReplayLine::Header(self.header.clone()));
        for e in &self.events {
            push(&ReplayLine::Event(e.clone()));
        }
        for c in &self.checkpoints {
            push(&ReplayLine::Checkpoint(c.clone()));
        }
        out
    }

    /// Number of ticks to simulate so that every event is applied and every checkpoint reached:
    /// `max(event.tick + 1, checkpoint.tick)`.
    #[must_use]
    pub fn last_tick(&self) -> u64 {
        self.events
            .iter()
            .map(|e| e.tick.saturating_add(1))
            .chain(self.checkpoints.iter().map(|c| c.tick))
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensherwood_core::Button;

    #[test]
    fn replay_round_trip() {
        let r = Replay {
            header: ReplayHeader {
                replay_version: 1,
                protocol: PROTOCOL_VERSION,
                ruleset: opensherwood_core::RULESET_VERSION,
                content_fingerprint: None,
                scenario: Scenario::Synthetic("corridor".into()),
                viewport: (640, 480),
                tick_rate: (30, 1),
                hash_schema: opensherwood_core::hash::HASH_SCHEMA_VERSION,
                seed: 5,
                rng_streams: [(
                    "gameplay".to_string(),
                    RngStreamInit {
                        algorithm: "pcg32".into(),
                        seed: 5,
                        stream: 1,
                    },
                )]
                .into_iter()
                .collect(),
            },
            events: vec![ReplayEvent {
                tick: 3,
                sequence: 0,
                event: InputEvent::PointerDown {
                    button: Button::Left,
                },
                intent: Some("select".into()),
            }],
            checkpoints: vec![],
        };
        let text = r.to_jsonl();
        assert!(text.contains("\"kind\":\"pointer_down\""));
        assert_eq!(Replay::from_jsonl(&text).unwrap(), r);
        assert!(Replay::from_jsonl("").is_err());
    }

    #[test]
    fn timed_event_flattens_kind() {
        let t: TimedEvent = serde_json::from_str(
            r#"{"tick_offset":0,"sequence":1,"kind":"pointer_move","x256":10,"y256":20}"#,
        )
        .unwrap();
        assert_eq!(t.event, InputEvent::PointerMove { x256: 10, y256: 20 });
    }
}
