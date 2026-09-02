//! Wire types for the harness protocol (ADR-0004, `docs/harness.md`) and the `ReplayV1` format.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use opensherwood_core::{Hashes, InputEvent, Observation, Scenario, Snapshot};

/// Protocol version reported by `hello`.
pub const PROTOCOL_VERSION: u32 = 1;

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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObserveParams {
    /// Include entities (default true).
    #[serde(default = "default_true")]
    pub entities: bool,
}

fn default_true() -> bool {
    true
}

/// `observe` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserveResult {
    /// Observation.
    #[serde(flatten)]
    pub observation: Observation,
    /// Hashes.
    pub hashes: Hashes,
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
    /// Seed.
    pub seed: u64,
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
    /// Parse JSON Lines.
    pub fn from_jsonl(text: &str) -> Result<Self, String> {
        let mut header = None;
        let mut events = Vec::new();
        let mut checkpoints = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: ReplayLine =
                serde_json::from_str(line).map_err(|e| format!("line {}: {e}", n + 1))?;
            match parsed {
                ReplayLine::Header(h) => {
                    if header.is_some() {
                        return Err(format!("line {}: duplicate header", n + 1));
                    }
                    if h.replay_version != 1 {
                        return Err(format!("unsupported replay version {}", h.replay_version));
                    }
                    header = Some(h);
                }
                ReplayLine::Event(e) => events.push(e),
                ReplayLine::Checkpoint(c) => checkpoints.push(c),
            }
        }
        let header = header.ok_or("missing header line")?;
        events.sort_by_key(|e| (e.tick, e.sequence));
        checkpoints.sort_by_key(|c| c.tick);
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

    /// Last tick that has an event or checkpoint.
    #[must_use]
    pub fn last_tick(&self) -> u64 {
        self.events
            .iter()
            .map(|e| e.tick)
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
                ruleset: 1,
                content_fingerprint: None,
                scenario: Scenario::Synthetic("corridor".into()),
                viewport: (640, 480),
                tick_rate: (30, 1),
                seed: 5,
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
