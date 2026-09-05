//! Wire types for the harness protocol (ADR-0004, `docs/harness.md`) and the `ReplayV1` format.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use opensherwood_core::{Hashes, InputEvent, Observation, Scenario, Snapshot};

/// Protocol version reported by `hello`. 4: replay time is the session tick (`ReplayHeader::time`
/// = [`REPLAY_TIME_SESSION`]), checkpoints carry `world_tick`, the tick-0 checkpoint is kept.
/// 5: checkpoints carry the `session` digest (screen, UI state, notice) and the `frame` hash; a
/// replay needs a checkpoint at tick 0 and a terminal one at its last tick.
pub const PROTOCOL_VERSION: u32 = 6;

/// The only replay time model: one unit per `advance` of the session (every `step` tick, whether
/// a screen consumed the events or the world stepped). Screens are part of the timeline.
pub const REPLAY_TIME_SESSION: &str = "session";

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
    /// Starting money of a mission (the selected profile's money when absent). Replays record
    /// the value used, so playback does not depend on the profile file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starting_money: Option<i32>,
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
    /// The last failed profile / settings write (cleared by a successful one): the player's
    /// data did not reach the disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence_error: Option<String>,
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
    /// Whether the element is the current selection (list rows).
    #[serde(default)]
    pub selected: bool,
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
    /// Record a checkpoint (all hashes) every N session ticks (0 = only at stop).
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
    /// Session ticks simulated.
    pub ticks: u64,
    /// Checkpoints that matched.
    pub checkpoints_ok: usize,
    /// First checkpoint that did not match: (session tick, differing hash parts, `world_tick`).
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
    /// Time model of `tick` in events and checkpoints: always [`REPLAY_TIME_SESSION`].
    pub time: String,
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
    /// Starting money of a mission scenario (`None` for the others): a canonical input of the
    /// mission, recorded so playback does not read the profile file.
    #[serde(default)]
    pub starting_money: Option<i32>,
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
    /// Names of the fields that differ from `other` (for the "header does not match" report).
    #[must_use]
    pub fn diff(&self, other: &ReplayHeader) -> Vec<String> {
        let mut out = Vec::new();
        let mut check = |name: &str, same: bool| {
            if !same {
                out.push(name.to_string());
            }
        };
        check(
            "replay_version",
            self.replay_version == other.replay_version,
        );
        check("protocol", self.protocol == other.protocol);
        check("ruleset", self.ruleset == other.ruleset);
        check(
            "content_fingerprint",
            self.content_fingerprint == other.content_fingerprint,
        );
        check("scenario", self.scenario == other.scenario);
        check("time", self.time == other.time);
        check("viewport", self.viewport == other.viewport);
        check("tick_rate", self.tick_rate == other.tick_rate);
        check("hash_schema", self.hash_schema == other.hash_schema);
        check("seed", self.seed == other.seed);
        check("rng_streams", self.rng_streams == other.rng_streams);
        check(
            "starting_money",
            self.starting_money == other.starting_money,
        );
        out
    }

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
        if self.time != REPLAY_TIME_SESSION {
            return Err(format!(
                "replay time model '{}' is not '{REPLAY_TIME_SESSION}'",
                self.time
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
    /// Session tick the event is applied at (see [`REPLAY_TIME_SESSION`]).
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
    /// Session tick after which the hashes must match (0: the state right after `reset`, before
    /// anything is applied).
    pub tick: u64,
    /// World tick at that session tick (the world lags while a screen is shown).
    pub world_tick: u64,
    /// Expected world hashes.
    pub hashes: Hashes,
    /// Digest of the session's presentation state at that tick: screen kind, the `ui` state as
    /// `observe` reports it, and the notice text with its remaining ticks (BLAKE3, hex).
    pub session: String,
    /// BLAKE3 of the RGBA framebuffer at that tick (`capture.hash`; empty without a frame).
    pub frame: String,
}

impl ReplayCheckpoint {
    /// Names of what differs from `observed` (a checkpoint built from the session at the same
    /// tick): the differing hash parts, `world_tick`, `session` and `frame`.
    #[must_use]
    pub fn diff(&self, observed: &ReplayCheckpoint) -> Vec<String> {
        let mut parts = self.hashes.diff(&observed.hashes);
        if self.world_tick != observed.world_tick {
            parts.push("world_tick".to_string());
        }
        if self.session != observed.session {
            parts.push("session".to_string());
        }
        if self.frame != observed.frame {
            parts.push("frame".to_string());
        }
        parts
    }
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
        let replay = Self {
            header,
            events,
            checkpoints,
        };
        replay.check_checkpoints()?;
        Ok(replay)
    }

    /// A replay compares something: its first checkpoint is at tick 0 (the state right after
    /// `reset`) and its last one is terminal, at [`Replay::last_tick`] (after the last event).
    /// Enforced by the parser and by [`ReplayRecorder::finish`], so a replay with its initial or
    /// final expectation deleted is refused before any session is reset.
    pub fn check_checkpoints(&self) -> Result<(), String> {
        let first = self
            .checkpoints
            .first()
            .ok_or("replay has no checkpoint (one at tick 0 and a terminal one are required)")?;
        if first.tick != 0 {
            return Err(format!(
                "replay's first checkpoint is at tick {}, not 0",
                first.tick
            ));
        }
        let last = self.checkpoints.last().map_or(0, |c| c.tick);
        let end = self.last_tick();
        if last != end {
            return Err(format!(
                "replay's last checkpoint is at tick {last}, its last tick is {end}: the terminal checkpoint is missing"
            ));
        }
        Ok(())
    }

    /// Serialise to JSON Lines. The output is built line by line into a buffer reserved with
    /// `try_reserve`, and a replay whose text would exceed [`replay_limits::MAX_BYTES`] (one the
    /// parser would refuse) is an error rather than a string.
    pub fn to_jsonl(&self) -> Result<String, String> {
        let header = encode_line(&ReplayLine::Header(self.header.clone()))?;
        let mut total = header.len();
        let mut lines: Vec<Vec<u8>> = Vec::new();
        lines
            .try_reserve_exact(self.events.len() + self.checkpoints.len())
            .map_err(|_| "replay: cannot allocate the line table".to_string())?;
        for e in &self.events {
            let line = encode_line(&ReplayLine::Event(e.clone()))?;
            total = add_bytes(total, line.len())?;
            lines.push(line);
        }
        for c in &self.checkpoints {
            let line = encode_line(&ReplayLine::Checkpoint(c.clone()))?;
            total = add_bytes(total, line.len())?;
            lines.push(line);
        }
        let mut out = String::new();
        out.try_reserve_exact(total)
            .map_err(|_| format!("replay: cannot allocate {total} bytes"))?;
        out.push_str(std::str::from_utf8(&header).map_err(|e| e.to_string())?);
        for line in &lines {
            out.push_str(std::str::from_utf8(line).map_err(|e| e.to_string())?);
        }
        Ok(out)
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

/// Serialised JSON Lines form of one replay line, newline included.
pub fn encode_line(line: &ReplayLine) -> Result<Vec<u8>, String> {
    let mut v = serde_json::to_vec(line).map_err(|e| format!("replay line: {e}"))?;
    v.push(b'\n');
    Ok(v)
}

/// Bytes one replay line takes in the JSON Lines text (newline included).
pub fn line_bytes(line: &ReplayLine) -> Result<usize, String> {
    encode_line(line).map(|v| v.len())
}

/// `total + extra`, refused beyond [`replay_limits::MAX_BYTES`].
fn add_bytes(total: usize, extra: usize) -> Result<usize, String> {
    let sum = total.saturating_add(extra);
    if sum > replay_limits::MAX_BYTES {
        return Err(format!(
            "replay exceeds the size limit ({} bytes)",
            replay_limits::MAX_BYTES
        ));
    }
    Ok(sum)
}

/// Builds a replay incrementally under the [`replay_limits`], accounting the serialised bytes of
/// every line before it is accepted: a recorder never holds a replay its own parser
/// ([`Replay::from_jsonl`]) would reject, whatever the events look like. Room for the checkpoint
/// [`ReplayRecorder::finish`] appends is reserved from the start, so stopping a recording that
/// stayed within the quotas cannot fail on size. A refused push leaves the recorder unchanged.
#[derive(Debug, Clone)]
pub struct ReplayRecorder {
    header: ReplayHeader,
    events: Vec<ReplayEvent>,
    checkpoints: Vec<ReplayCheckpoint>,
    /// Bytes of the header line and of every accepted line.
    bytes: usize,
    /// Most bytes the finished replay may have.
    max_bytes: usize,
    /// Bytes held back for the final checkpoint.
    reserve: usize,
}

impl ReplayRecorder {
    /// Start a recording under [`replay_limits::MAX_BYTES`]. `sample_hashes` must have the shape
    /// every later checkpoint's hashes have (the same part names, digests of the same length): the
    /// final checkpoint's reserve is sized from it, at the highest tick a replay can reach.
    pub fn new(header: ReplayHeader, sample_hashes: &Hashes) -> Result<Self, String> {
        Self::with_max_bytes(header, sample_hashes, replay_limits::MAX_BYTES)
    }

    /// [`ReplayRecorder::new`] under a tighter byte limit (never above the format's).
    pub fn with_max_bytes(
        header: ReplayHeader,
        sample_hashes: &Hashes,
        max_bytes: usize,
    ) -> Result<Self, String> {
        header.validate()?;
        let max_bytes = max_bytes.min(replay_limits::MAX_BYTES);
        let bytes = line_bytes(&ReplayLine::Header(header.clone()))?;
        let reserve = line_bytes(&ReplayLine::Checkpoint(ReplayCheckpoint {
            tick: replay_limits::MAX_TICK,
            world_tick: replay_limits::MAX_TICK,
            hashes: sample_hashes.clone(),
            session: "0".repeat(64),
            frame: "0".repeat(64),
        }))?;
        if bytes.checked_add(reserve).is_none_or(|n| n > max_bytes) {
            return Err(format!(
                "replay header and final checkpoint need {} bytes, the limit is {max_bytes}",
                bytes.saturating_add(reserve)
            ));
        }
        Ok(Self {
            header,
            events: Vec::new(),
            checkpoints: Vec::new(),
            bytes,
            max_bytes,
            reserve,
        })
    }

    /// Header.
    #[must_use]
    pub fn header(&self) -> &ReplayHeader {
        &self.header
    }

    /// Events accepted so far.
    #[must_use]
    pub fn events(&self) -> &[ReplayEvent] {
        &self.events
    }

    /// Checkpoints accepted so far.
    #[must_use]
    pub fn checkpoints(&self) -> &[ReplayCheckpoint] {
        &self.checkpoints
    }

    /// Bytes the replay text has so far (header included).
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Byte limit of the finished replay.
    #[must_use]
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Bytes reserved for the final checkpoint.
    #[must_use]
    pub fn reserve(&self) -> usize {
        self.reserve
    }

    /// Bytes still available to events and intermediate checkpoints.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.max_bytes
            .saturating_sub(self.reserve)
            .saturating_sub(self.bytes)
    }

    /// Whether `extra` more bytes of events or checkpoints would still fit.
    #[must_use]
    pub fn fits(&self, extra: usize) -> bool {
        extra <= self.remaining()
    }

    /// Upper bound of the bytes an event takes wherever it is recorded (highest tick and sequence
    /// number), for callers that check a batch before recording it.
    pub fn worst_case_event_bytes(event: InputEvent) -> Result<usize, String> {
        line_bytes(&ReplayLine::Event(ReplayEvent {
            tick: replay_limits::MAX_TICK.saturating_sub(1),
            sequence: u32::MAX,
            event,
            intent: None,
        }))
    }

    /// Append an event: it must be strictly after the last one by `(tick, sequence)`, below
    /// the tick and event quotas, and fit in the bytes left. Nothing changes on error.
    pub fn push_event(&mut self, event: ReplayEvent) -> Result<(), String> {
        if event.tick >= replay_limits::MAX_TICK {
            return Err(format!(
                "tick quota {} reached at tick {}",
                replay_limits::MAX_TICK,
                event.tick
            ));
        }
        if self.events.len() >= replay_limits::MAX_EVENTS {
            return Err(format!(
                "event quota {} exceeded at tick {}",
                replay_limits::MAX_EVENTS,
                event.tick
            ));
        }
        if let Some(prev) = self.events.last()
            && (event.tick, event.sequence) <= (prev.tick, prev.sequence)
        {
            return Err(format!(
                "event ({}, {}) is not after the last recorded event ({}, {})",
                event.tick, event.sequence, prev.tick, prev.sequence
            ));
        }
        let line = line_bytes(&ReplayLine::Event(event.clone()))?;
        if !self.fits(line) {
            return Err(format!(
                "byte quota {} exceeded at tick {}",
                self.max_bytes, event.tick
            ));
        }
        self.events
            .try_reserve(1)
            .map_err(|_| "cannot allocate the event".to_string())?;
        self.events.push(event);
        self.bytes += line;
        Ok(())
    }

    /// Append an intermediate checkpoint: strictly after the last one, leaving room in the count
    /// for the final one, and within the bytes left. Nothing changes on error.
    pub fn push_checkpoint(&mut self, checkpoint: ReplayCheckpoint) -> Result<(), String> {
        if self.checkpoints.len() + 1 >= replay_limits::MAX_CHECKPOINTS {
            return Err(format!(
                "checkpoint quota {} exceeded at tick {}",
                replay_limits::MAX_CHECKPOINTS,
                checkpoint.tick
            ));
        }
        let budget = self.remaining();
        self.accept_checkpoint(checkpoint, budget)
    }

    /// The one path every checkpoint (intermediate or final) takes: tick within the format's
    /// quota, strictly after the last checkpoint, its line within `budget` bytes, capacity
    /// reserved fallibly. Nothing changes on error.
    fn accept_checkpoint(
        &mut self,
        checkpoint: ReplayCheckpoint,
        budget: usize,
    ) -> Result<(), String> {
        if checkpoint.tick > replay_limits::MAX_TICK {
            return Err(format!(
                "tick quota {} reached at tick {}",
                replay_limits::MAX_TICK,
                checkpoint.tick
            ));
        }
        if let Some(prev) = self.checkpoints.last()
            && checkpoint.tick <= prev.tick
        {
            return Err(format!(
                "checkpoint at tick {} is not after the last one at tick {}",
                checkpoint.tick, prev.tick
            ));
        }
        let line = line_bytes(&ReplayLine::Checkpoint(checkpoint.clone()))?;
        if line > budget {
            return Err(format!(
                "byte quota {} exceeded at tick {} ({line} bytes needed, {budget} left)",
                self.max_bytes, checkpoint.tick
            ));
        }
        self.checkpoints
            .try_reserve(1)
            .map_err(|_| "cannot allocate the checkpoint".to_string())?;
        self.checkpoints.push(checkpoint);
        self.bytes += line;
        Ok(())
    }

    /// Close the recording with the hashes at the current tick. The final checkpoint goes through
    /// the same validation and allocation as every other one (tick quota, ordering, fallible
    /// capacity), into the bytes reserved for it; a checkpoint already recorded at that tick is
    /// replaced by the final one. Nothing is dropped: the tick-0 checkpoint fixes the state right
    /// after `reset` and playback compares it before applying anything. The result is within
    /// every limit of [`replay_limits`], so [`Replay::from_jsonl`] accepts its text.
    pub fn finish(mut self, last: ReplayCheckpoint) -> Result<Replay, String> {
        if self.checkpoints.last().is_some_and(|c| c.tick == last.tick) {
            let same = self.checkpoints.pop().ok_or("no checkpoint to replace")?;
            self.bytes -= line_bytes(&ReplayLine::Checkpoint(same))?;
        }
        if self.checkpoints.len() >= replay_limits::MAX_CHECKPOINTS {
            return Err(format!(
                "checkpoint quota {} exceeded",
                replay_limits::MAX_CHECKPOINTS
            ));
        }
        let budget = self.reserve.saturating_add(self.remaining());
        self.accept_checkpoint(last, budget)
            .map_err(|e| format!("final checkpoint: {e}"))?;
        debug_assert!(self.bytes <= self.max_bytes);
        let replay = Replay {
            header: self.header,
            events: self.events,
            checkpoints: self.checkpoints,
        };
        replay.check_checkpoints()?;
        Ok(replay)
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
                time: REPLAY_TIME_SESSION.into(),
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
                starting_money: None,
            },
            events: vec![ReplayEvent {
                tick: 3,
                sequence: 0,
                event: InputEvent::PointerDown {
                    button: Button::Left,
                },
                intent: Some("select".into()),
            }],
            checkpoints: vec![checkpoint(0), checkpoint(4)],
        };
        let text = r.to_jsonl().unwrap();
        assert!(text.contains("\"kind\":\"pointer_down\""));
        assert_eq!(Replay::from_jsonl(&text).unwrap(), r);
        assert!(Replay::from_jsonl("").is_err());
    }

    #[test]
    fn parser_requires_the_initial_and_terminal_checkpoints() {
        let base = Replay {
            header: header(),
            events: vec![worst_event(2, 0)],
            checkpoints: vec![checkpoint(0), checkpoint(2), checkpoint(3)],
        };
        assert_eq!(Replay::from_jsonl(&base.to_jsonl().unwrap()).unwrap(), base);
        let mut no_initial = base.clone();
        no_initial.checkpoints.remove(0);
        let err = Replay::from_jsonl(&no_initial.to_jsonl().unwrap()).unwrap_err();
        assert!(err.contains("not 0"), "{err}");
        let mut none = base.clone();
        none.checkpoints.clear();
        let err = Replay::from_jsonl(&none.to_jsonl().unwrap()).unwrap_err();
        assert!(err.contains("no checkpoint"), "{err}");
        let mut no_terminal = base.clone();
        no_terminal.checkpoints.pop();
        let err = Replay::from_jsonl(&no_terminal.to_jsonl().unwrap()).unwrap_err();
        assert!(err.contains("terminal"), "{err}");
        // A checkpoint-only replay is fine when its single checkpoint is at tick 0.
        let only_initial = Replay {
            header: header(),
            events: vec![],
            checkpoints: vec![checkpoint(0)],
        };
        assert!(Replay::from_jsonl(&only_initial.to_jsonl().unwrap()).is_ok());
        // The recorder cannot finish without the initial checkpoint either.
        let r = ReplayRecorder::with_max_bytes(header(), &hashes(), 4096).unwrap();
        assert!(r.finish(checkpoint(5)).unwrap_err().contains("not 0"));
    }

    fn header() -> ReplayHeader {
        ReplayHeader {
            replay_version: 1,
            protocol: PROTOCOL_VERSION,
            ruleset: opensherwood_core::RULESET_VERSION,
            content_fingerprint: None,
            scenario: Scenario::Synthetic("corridor".into()),
            time: REPLAY_TIME_SESSION.into(),
            viewport: (640, 480),
            tick_rate: (60, 1),
            hash_schema: opensherwood_core::hash::HASH_SCHEMA_VERSION,
            seed: 1,
            rng_streams: [(
                "gameplay".to_string(),
                RngStreamInit {
                    algorithm: "pcg32".into(),
                    seed: 1,
                    stream: 1,
                },
            )]
            .into_iter()
            .collect(),
            starting_money: None,
        }
    }

    fn hashes() -> Hashes {
        let mut h = Hashes::default();
        for name in ["world", "actors", "orders", "rng", "total"] {
            h.parts.insert(name.into(), "ab".repeat(32));
        }
        h
    }

    fn checkpoint(tick: u64) -> ReplayCheckpoint {
        ReplayCheckpoint {
            tick,
            world_tick: tick,
            hashes: hashes(),
            session: "cd".repeat(32),
            frame: "ef".repeat(32),
        }
    }

    /// The longest JSON any input event can take.
    fn worst_event(tick: u64, sequence: u32) -> ReplayEvent {
        ReplayEvent {
            tick,
            sequence,
            event: InputEvent::PointerMove {
                x256: i32::MIN,
                y256: i32::MIN,
            },
            intent: None,
        }
    }

    #[test]
    fn recorder_accounts_every_byte_and_refuses_without_mutating() {
        let cap = 2048;
        let mut r = ReplayRecorder::with_max_bytes(header(), &hashes(), cap).unwrap();
        let header_bytes = line_bytes(&ReplayLine::Header(header())).unwrap();
        assert_eq!(r.bytes(), header_bytes);
        assert_eq!(
            r.reserve(),
            line_bytes(&ReplayLine::Checkpoint(checkpoint(replay_limits::MAX_TICK))).unwrap()
        );
        r.push_checkpoint(checkpoint(0)).unwrap();
        let mut expected =
            header_bytes + line_bytes(&ReplayLine::Checkpoint(checkpoint(0))).unwrap();
        let mut n = 0u32;
        loop {
            let e = worst_event(5, n);
            let line = line_bytes(&ReplayLine::Event(e.clone())).unwrap();
            let before = (r.bytes(), r.events().len());
            match r.push_event(e) {
                Ok(()) => {
                    expected += line;
                    assert_eq!(r.bytes(), expected);
                    n += 1;
                }
                Err(msg) => {
                    assert!(msg.contains("byte quota"), "{msg}");
                    assert_eq!((r.bytes(), r.events().len()), before, "refusal mutated");
                    assert!(before.0 + line > cap - r.reserve());
                    break;
                }
            }
        }
        assert!(n > 0);
        assert!(r.bytes() + r.reserve() <= cap);
        // Ordering and quota checks refuse without touching the recorder either.
        let snapshot = r.clone();
        assert!(r.push_event(worst_event(5, 0)).is_err());
        assert!(
            r.push_event(worst_event(replay_limits::MAX_TICK, 0))
                .is_err()
        );
        assert!(r.push_checkpoint(checkpoint(0)).is_err());
        assert_eq!(r.bytes(), snapshot.bytes());
        assert_eq!(r.events().len(), snapshot.events().len());
        // Stopping fits thanks to the reserve, keeps the tick-0 checkpoint and stays within the cap.
        let replay = r.finish(checkpoint(6)).unwrap();
        assert_eq!(replay.checkpoints.len(), 2);
        assert_eq!(replay.checkpoints[0].tick, 0);
        assert_eq!(replay.checkpoints[1].tick, 6);
        let text = replay.to_jsonl().unwrap();
        assert!(text.len() <= cap, "{} > {cap}", text.len());
        assert_eq!(Replay::from_jsonl(&text).unwrap(), replay);
        // A cap that cannot hold the header and the final checkpoint is refused up front.
        assert!(ReplayRecorder::with_max_bytes(header(), &hashes(), header_bytes).is_err());
        // A lone tick-0 checkpoint finished at tick 0 is one checkpoint.
        let mut r = ReplayRecorder::with_max_bytes(header(), &hashes(), cap).unwrap();
        r.push_checkpoint(checkpoint(0)).unwrap();
        assert_eq!(r.finish(checkpoint(0)).unwrap().checkpoints.len(), 1);
    }

    #[test]
    fn finish_takes_the_validated_path() {
        // The final checkpoint replaces one already recorded at the same tick (the final hashes
        // win), the byte accounting stays exact, and the text parses.
        let mut r = ReplayRecorder::with_max_bytes(header(), &hashes(), 4096).unwrap();
        r.push_checkpoint(checkpoint(0)).unwrap();
        r.push_checkpoint(checkpoint(3)).unwrap();
        let bytes_before = r.bytes();
        let mut last = checkpoint(3);
        last.hashes.parts.insert("total".into(), "cd".repeat(32));
        let replaced_line = line_bytes(&ReplayLine::Checkpoint(last.clone())).unwrap();
        let replay = r.finish(last.clone()).unwrap();
        assert_eq!(replay.checkpoints.len(), 2);
        assert_eq!(replay.checkpoints[1], last);
        let text = replay.to_jsonl().unwrap();
        assert_eq!(text.len(), bytes_before, "same tick, same line length");
        assert_eq!(
            replaced_line,
            line_bytes(&ReplayLine::Checkpoint(checkpoint(3))).unwrap()
        );
        assert_eq!(Replay::from_jsonl(&text).unwrap(), replay);
        // A final tick beyond the format's quota is refused: what `finish` returns always parses.
        let mut r = ReplayRecorder::with_max_bytes(header(), &hashes(), 4096).unwrap();
        r.push_checkpoint(checkpoint(0)).unwrap();
        let err = r
            .clone()
            .finish(checkpoint(replay_limits::MAX_TICK + 1))
            .unwrap_err();
        assert!(err.contains("tick quota"), "{err}");
        // ... and one at the quota itself is accepted and parses.
        let replay = r.finish(checkpoint(replay_limits::MAX_TICK)).unwrap();
        assert_eq!(
            Replay::from_jsonl(&replay.to_jsonl().unwrap()).unwrap(),
            replay
        );
        // A final checkpoint before the last recorded one is refused.
        let mut r = ReplayRecorder::with_max_bytes(header(), &hashes(), 4096).unwrap();
        r.push_checkpoint(checkpoint(0)).unwrap();
        r.push_checkpoint(checkpoint(5)).unwrap();
        assert!(r.finish(checkpoint(4)).is_err());
    }

    #[test]
    fn recorder_at_the_format_cap_with_worst_case_events_stays_parseable() {
        let mut r = ReplayRecorder::new(header(), &hashes()).unwrap();
        assert_eq!(r.max_bytes(), replay_limits::MAX_BYTES);
        r.push_checkpoint(checkpoint(0)).unwrap();
        let worst = ReplayRecorder::worst_case_event_bytes(InputEvent::PointerMove {
            x256: i32::MIN,
            y256: i32::MIN,
        })
        .unwrap();
        // Every event at the highest tick with a ten-digit sequence: the longest line an event
        // can produce, so the byte cap binds long before the event count does.
        let tick = replay_limits::MAX_TICK - 1;
        let mut sequence = 1_000_000_000u32;
        loop {
            match r.push_event(worst_event(tick, sequence)) {
                Ok(()) => sequence += 1,
                Err(msg) => {
                    assert!(msg.contains("byte quota"), "{msg}");
                    break;
                }
            }
        }
        assert!(r.events().len() < replay_limits::MAX_EVENTS);
        assert!(r.remaining() < worst);
        assert!(r.bytes() + r.reserve() <= replay_limits::MAX_BYTES);
        let replay = r.finish(checkpoint(replay_limits::MAX_TICK)).unwrap();
        let text = replay.to_jsonl().unwrap();
        assert!(text.len() <= replay_limits::MAX_BYTES);
        // Only the unusable tail (less than one worst-case event) is left below the cap.
        assert!(
            text.len() + worst > replay_limits::MAX_BYTES,
            "not at the cap"
        );
        let parsed = Replay::from_jsonl(&text).expect("the recorder's output parses");
        assert_eq!(parsed.events.len(), replay.events.len());
        assert_eq!(parsed.checkpoints.len(), 2);
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
