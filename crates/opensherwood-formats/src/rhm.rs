//! Mission files (`.rhm`, root chunk `DUTY`). Spec: `docs/formats/rhm.md`.
//!
//! A mission places actors, objects, waypoints, patrol paths, script polygons and scrolls on a map
//! described by an `.rhp` file. Every record layout below was established by observation over the
//! 39 retail missions (each chunk is consumed exactly by these readers); fields whose meaning is not
//! established are named `unknown_*` and their observed value sets are listed in the spec.

use crate::chunk::{self, RawChunk};
use crate::reader::{FormatError, Reader, latin1, tag_string};

/// Root chunk version seen in retail data.
pub const VERSION: u32 = 2;

/// `FOOT`: header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Chunk version (4 in retail data).
    pub version: u32,
    /// Map id; equals the first word of the `SPOK` chunk of the matching `.rhp` (100 = Croisement01, ...).
    pub map_id: u32,
    /// Ambiance variant: 1, 2, 4 or 16 (Day / Night / Fog / Custom is the working hypothesis).
    pub variant: u32,
    /// Map name, e.g. "Croisement03", "lincoln".
    pub map: String,
    /// Mission id (0..=0x33 in retail data; 0 for several ambushes).
    pub mission_id: u32,
}

/// Position and placement qualifier shared by most records (18 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Placement {
    /// Map x in background pixels.
    pub x: u16,
    /// Map y in background pixels.
    pub y: u16,
    /// Facing direction, 0..=15 (16 directions, 0 = east and counter-clockwise is the working hypothesis).
    pub direction: u32,
    /// Kind / flag word. Actors: 3 (normal), 0x88 (hidden PC), 0x86, 0x0e, 0x9f, 0x10e, 0x2f, 0xa2, 0x2d, 0.
    /// Scrolls: always 190. `ZORG`: 189 + `unknown_b`.
    pub unknown_0x08: u32,
    /// Placement qualifier a: -1 or a small id (projection area or sector index is the hypothesis).
    pub unknown_0x0c: i16,
    /// Placement qualifier b: 0 or a value up to ~150 (height or layer of the projection area?).
    pub unknown_0x0e: u16,
    /// Placement qualifier c: 0..=11.
    pub unknown_0x10: u16,
}

/// A polygon prefixed and suffixed by one byte each (both look random: editor colour or hash).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Polygon {
    /// Byte before the point count.
    pub unknown_a: u8,
    /// Points in map pixels.
    pub points: Vec<(u16, u16)>,
    /// Byte after the points.
    pub unknown_b: u8,
}

/// `POUF` entry: an animated map element used by the mission (copied from the editor's library).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenant {
    /// Sprite bank name (`Animations/<Variant>/<sprite>.rhs`), e.g. "Trapcr03".
    pub sprite: String,
    /// Editor label, e.g. "Croisement03 - piege02h".
    pub label: String,
    /// Undecoded body (see spec: position, flag bytes, empty polygons, an id list).
    pub body: Vec<u8>,
}

/// `SCOT` record: a player character (Robin and the merry men; hidden ones are activated by script).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerCharacter {
    /// Position.
    pub placement: Placement,
    /// 0..=4 (3 or 0 mostly).
    pub unknown_0x12: u32,
    /// Ten bytes, single `1`s at various offsets in a few files; zero otherwise.
    pub unknown_0x16: [u8; 10],
    /// Script class name (`hidden_pc01_80000048`); `None` for the ordinary PCs.
    pub name: Option<String>,
    /// Byte after the name: 0, or 4 (one record per story mission), 2, 5.
    pub unknown_trailer: u8,
}

/// `BORG` record: a non-player human (soldiers, guards, merry men NPCs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Npc {
    /// Position.
    pub placement: Placement,
    /// 0..=4.
    pub unknown_0x12: u32,
    /// Character profile index (1..=62; 42/43 = merry men with bow/staff, 30 = lancer, 18 = officer in
    /// the tutorial; the table itself is not yet located).
    pub profile: u32,
    /// 0 or 1 (1 for 127 of 2463 records; "patrol chief" is the hypothesis).
    pub unknown_0x1a: u8,
    /// 0, 1..=20, 50, 99, 100.
    pub unknown_0x1b: u32,
    /// Always 0.
    pub unknown_0x1f: u32,
    /// 0 or a percentage (10..=100).
    pub unknown_0x23: u32,
    /// Indices of other `BORG` records (company members of a chief).
    pub members: Vec<u16>,
    /// Index into `RAIL` (patrol path) or -1.
    pub rail: i16,
    /// -1 or a small index (7..=22); sync target is the hypothesis.
    pub unknown_i16: i16,
    /// Script class name.
    pub name: Option<String>,
}

/// `OILE` record: a civilian.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Civilian {
    /// Position.
    pub placement: Placement,
    /// 0..=3.
    pub unknown_0x12: u32,
    /// Character profile index (0..=21).
    pub profile: u32,
    /// -1 or a small index.
    pub unknown_i16_a: i16,
    /// 0, 25, or a large value (1500..=4500).
    pub unknown_i16_b: i16,
    /// Always 0.
    pub unknown_u16: u16,
    /// Present only when `profile == 1`: ten lists of `u16` ids.
    pub lists: Option<Vec<Vec<u16>>>,
    /// Script class name.
    pub name: Option<String>,
}

/// `TOTO` record: a named non-player character (wedding guests, prisoners, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vip {
    /// Position.
    pub placement: Placement,
    /// 0..=3.
    pub unknown_0x12: u32,
    /// Character profile index (1..=9).
    pub profile: u32,
    /// 0 or 1.
    pub unknown_i16_a: i16,
    /// Always 0.
    pub unknown_i16_b: i16,
    /// Script class name.
    pub name: Option<String>,
}

/// `BOOM` record: an object (trap, target, cart part, mechanism, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// Map x.
    pub x: u16,
    /// Map y.
    pub y: u16,
    /// -1 or 75..=300 (targets: a range?).
    pub unknown_0x04: i16,
    /// Always 0.
    pub unknown_0x06: u16,
    /// Always 0.
    pub unknown_0x08: u16,
    /// 0 or 148..=195.
    pub unknown_0x0a: u16,
    /// Always 0.
    pub unknown_0x0c: u16,
    /// Placement qualifier a (see [`Placement::unknown_0x0c`]).
    pub unknown_0x0e: i16,
    /// Placement qualifier b.
    pub unknown_0x10: u16,
    /// Placement qualifier c.
    pub unknown_0x12: u16,
    /// Sprite bank name ("TG_BowTarget", "Trapcr03", "chariot05").
    pub sprite: String,
    /// Label; for animated elements it equals a `POUF` entry label.
    pub label: String,
    /// Bit flags: 0, 1, 2, 4, 16, 68, 97, 128.
    pub unknown_flags: u32,
    /// Second position (anchor of the animation; shared by objects of one element).
    pub x2: u16,
    /// Second position y.
    pub y2: u16,
    /// Placement qualifier b of the second position.
    pub unknown_q2: u16,
    /// Placement qualifier c of the second position.
    pub unknown_r2: u16,
    /// Polygon (0..=8 points).
    pub polygon: Polygon,
    /// Always 1.
    pub unknown_u8: u8,
    /// Script class name.
    pub name: Option<String>,
}

/// One class group of the `BOYZ` chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorGroup {
    /// `MEOW`: always empty in retail data.
    Meow {
        /// Group version (2).
        version: u32,
        /// Declared count (0).
        count: u16,
    },
    /// `SCOT` (version 4).
    PlayerCharacters {
        /// Group version.
        version: u32,
        /// Records.
        records: Vec<PlayerCharacter>,
    },
    /// `OILE` (version 3).
    Civilians {
        /// Group version.
        version: u32,
        /// Records.
        records: Vec<Civilian>,
    },
    /// `TOTO` (version 2).
    Vips {
        /// Group version.
        version: u32,
        /// Records.
        records: Vec<Vip>,
    },
    /// `BORG` (version 4).
    Npcs {
        /// Group version.
        version: u32,
        /// Records.
        records: Vec<Npc>,
    },
    /// `BOOM` (version 5).
    Objects {
        /// Group version.
        version: u32,
        /// Records.
        records: Vec<Object>,
    },
    /// A class tag this reader does not know.
    Unknown {
        /// Tag.
        tag: [u8; 4],
        /// Version.
        version: u32,
        /// Body after the version word.
        body: Vec<u8>,
    },
}

/// `ZORG` record: probably a bonus / pick-up (see spec).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZorgEntry {
    /// 0..=18.
    pub unknown_a: u16,
    /// 1..=5; `placement.unknown_0x08 == 189 + unknown_b` in every record.
    pub unknown_b: u16,
    /// Position (direction always 0).
    pub placement: Placement,
}

/// `HOLE` record: a waypoint with a facing direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waypoint {
    /// Map x.
    pub x: u16,
    /// Map y.
    pub y: u16,
    /// Placement qualifier b.
    pub unknown_0x04: u16,
    /// Placement qualifier c.
    pub unknown_0x06: u16,
    /// Facing direction 0..=15.
    pub direction: u16,
}

/// `BUSH` / `GULP` point: a position with placement qualifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// Map x.
    pub x: u16,
    /// Map y.
    pub y: u16,
    /// Placement qualifier b.
    pub unknown_0x04: u16,
    /// Placement qualifier c.
    pub unknown_0x06: u16,
}

/// `NLIP` point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NlipPoint {
    /// Position.
    pub point: Point,
    /// 0 or 1.
    pub unknown_flag: u8,
    /// 0 or 4..=8 (only when the flag is 1).
    pub unknown_value: u16,
}

/// `NLIP` record: a polygon with a set of points inside (only Emb04/Tac18 use it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nlip {
    /// Always 0.
    pub unknown_0x00: u32,
    /// Polygon.
    pub polygon: Polygon,
    /// Points.
    pub points: Vec<NlipPoint>,
}

/// `HIRN`: AI data ("Hirn" = brain).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Brains {
    /// `HOLE`: waypoints.
    pub waypoints: Vec<Waypoint>,
    /// `BUSH`: positions (hiding places is the hypothesis).
    pub bushes: Vec<Point>,
    /// `POW `: beam-me points at the map edges (direction points inwards).
    pub beam_points: Vec<Placement>,
    /// `NLIP`: tactical zones.
    pub nlips: Vec<Nlip>,
    /// Versions of the four sub-chunks in file order, with their tags.
    pub versions: Vec<([u8; 4], u32)>,
}

/// One waypoint command: opcode and raw operand bytes (operand sizes are established, meanings are not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// Opcode.
    pub opcode: u8,
    /// Operand bytes (0, 2, 4 or 6 bytes depending on the opcode).
    pub args: Vec<u8>,
}

/// Operand size of a waypoint command opcode, if known.
#[must_use]
pub fn command_arg_size(opcode: u8) -> Option<usize> {
    match opcode {
        0x00 | 0x01 | 0x07 | 0x09 | 0x0a | 0x0b | 0x0c | 0x0e | 0x10 => Some(0),
        0x02 | 0x03 | 0x04 | 0x08 | 0x0d | 0x0f => Some(2),
        0x05 | 0x81 => Some(4),
        0x06 | 0x82 => Some(6),
        _ => None,
    }
}

/// A command block chosen with a probability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBlock {
    /// Percentage (blocks of one table sum to 100 in nearly every case).
    pub percent: u8,
    /// Commands.
    pub commands: Vec<Command>,
}

/// A table of command blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTable {
    /// 0, 1 or 2; a waypoint has one table (id 0 mostly) or two (ids 1 and 2).
    pub id: u8,
    /// Blocks.
    pub blocks: Vec<CommandBlock>,
}

/// A `RAIL` point: a patrol waypoint with either a name or a command program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailPoint {
    /// Position.
    pub point: Point,
    /// 0 = program, 1 = name.
    pub kind: u8,
    /// Name (`kind == 1`), e.g. "Point1__0___8000039f" (referenced by scripts).
    pub name: Option<String>,
    /// Command tables (`kind == 0`; empty payload gives an empty list).
    pub tables: Vec<CommandTable>,
}

/// `SKRO` record: a scroll (parchment message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scroll {
    /// Position (`unknown_0x08` is always 190).
    pub placement: Placement,
    /// Five flag bytes (`01 01 01 00 00` most often).
    pub unknown_flags: [u8; 5],
    /// Script class name.
    pub name: Option<String>,
}

/// `FLIM` item inside a `TING` entry: the animation of a mobile element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobileAnimation {
    /// Sprite bank name ("chariot05").
    pub sprite: String,
    /// Sequence name ("chariot05_cart8").
    pub animation: String,
    /// Offset x (negative).
    pub dx: i16,
    /// Offset y (negative).
    pub dy: i16,
    /// Always 0.
    pub unknown_0x04: u16,
    /// Always `01 01 01`.
    pub unknown_0x06: [u8; 3],
    /// Polygon (0 or 6 points).
    pub polygon: Polygon,
}

/// `TING` entry: a mobile element (cart).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mobile {
    /// `FLIM` sub-chunk version (2).
    pub flim_version: u32,
    /// Animations.
    pub animations: Vec<MobileAnimation>,
    /// `WOAW` sub-chunk version (3).
    pub woaw_version: u32,
    /// `WOAW` count (0 in retail data) and any bytes after it.
    pub woaw_count: u16,
    /// Rest of the `WOAW` body.
    pub woaw_rest: Vec<u8>,
    /// Footprint polygon (3 points).
    pub polygon: Polygon,
    /// Position x.
    pub x: u16,
    /// Position y.
    pub y: u16,
    /// Always 0.
    pub unknown_a: u16,
    /// 0 or 1.
    pub unknown_b: u32,
    /// Always 0.
    pub unknown_c: u16,
    /// Always 3.
    pub unknown_d: u32,
    /// Always -1.
    pub unknown_e: i16,
}

/// `GULP` polygon: a script sector (`EnterZone` / `ExitZone` in scripts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPolygon {
    /// Polygon.
    pub polygon: Polygon,
    /// Placement qualifier b.
    pub unknown_0x00: u16,
    /// Placement qualifier c.
    pub unknown_0x02: u16,
    /// Script class name.
    pub name: Option<String>,
}

/// `GULP`: script sectors.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptAreas {
    /// Points (near actor positions; purpose unknown).
    pub points: Vec<Point>,
    /// Polygons.
    pub polygons: Vec<ScriptPolygon>,
}

/// `CAVE` entry: a list of ids and a flag; the count is constant per map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaveEntry {
    /// Ids.
    pub ids: Vec<u16>,
    /// 0 or 1.
    pub unknown_flag: u8,
}

/// A parsed mission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mission {
    /// Root version (2).
    pub version: u32,
    /// `FOOT`.
    pub header: Header,
    /// `POUF` (version 3).
    pub tenants: Vec<Tenant>,
    /// `BOYZ` (version 3): actor groups in file order.
    pub actor_groups: Vec<ActorGroup>,
    /// `ZORG` (version 2).
    pub zorg: Vec<ZorgEntry>,
    /// `HIRN` (version 2).
    pub brains: Brains,
    /// `RAIL` (version 3): patrol paths.
    pub rails: Vec<Vec<RailPoint>>,
    /// `SKRO` (version 4).
    pub scrolls: Vec<Scroll>,
    /// `TING` (version 3).
    pub mobiles: Vec<Mobile>,
    /// `GULP` (version 2).
    pub script_areas: ScriptAreas,
    /// `CAVE` (version 3).
    pub cave: Vec<CaveEntry>,
    /// Tags and versions of every child chunk in file order.
    pub chunk_versions: Vec<([u8; 4], u32)>,
    /// Child chunks this reader does not know (the original loader skips unknown tags too).
    pub unknown_chunks: Vec<([u8; 4], u32)>,
}

impl Mission {
    /// Player characters.
    #[must_use]
    pub fn player_characters(&self) -> &[PlayerCharacter] {
        self.actor_groups
            .iter()
            .find_map(|g| match g {
                ActorGroup::PlayerCharacters { records, .. } => Some(records.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    /// Non-player humans.
    #[must_use]
    pub fn npcs(&self) -> &[Npc] {
        self.actor_groups
            .iter()
            .find_map(|g| match g {
                ActorGroup::Npcs { records, .. } => Some(records.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    /// Civilians.
    #[must_use]
    pub fn civilians(&self) -> &[Civilian] {
        self.actor_groups
            .iter()
            .find_map(|g| match g {
                ActorGroup::Civilians { records, .. } => Some(records.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    /// Named non-player characters.
    #[must_use]
    pub fn vips(&self) -> &[Vip] {
        self.actor_groups
            .iter()
            .find_map(|g| match g {
                ActorGroup::Vips { records, .. } => Some(records.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    /// Objects.
    #[must_use]
    pub fn objects(&self) -> &[Object] {
        self.actor_groups
            .iter()
            .find_map(|g| match g {
                ActorGroup::Objects { records, .. } => Some(records.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    /// Every script class name referenced by the mission (actors, objects, scrolls, script polygons,
    /// named rail points). Scripts define one class per name.
    #[must_use]
    pub fn script_names(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for g in &self.actor_groups {
            match g {
                ActorGroup::PlayerCharacters { records, .. } => {
                    out.extend(records.iter().filter_map(|r| r.name.as_deref()));
                }
                ActorGroup::Civilians { records, .. } => {
                    out.extend(records.iter().filter_map(|r| r.name.as_deref()));
                }
                ActorGroup::Vips { records, .. } => {
                    out.extend(records.iter().filter_map(|r| r.name.as_deref()));
                }
                ActorGroup::Npcs { records, .. } => {
                    out.extend(records.iter().filter_map(|r| r.name.as_deref()));
                }
                ActorGroup::Objects { records, .. } => {
                    out.extend(records.iter().filter_map(|r| r.name.as_deref()));
                }
                ActorGroup::Meow { .. } | ActorGroup::Unknown { .. } => {}
            }
        }
        out.extend(self.scrolls.iter().filter_map(|s| s.name.as_deref()));
        out.extend(
            self.script_areas
                .polygons
                .iter()
                .filter_map(|p| p.name.as_deref()),
        );
        out.extend(
            self.rails
                .iter()
                .flatten()
                .filter_map(|p| p.name.as_deref()),
        );
        out
    }
}

fn count(r: &mut Reader<'_>, what: &'static str) -> Result<usize, FormatError> {
    Ok(usize::from(r.u16(what)?))
}

fn placement(r: &mut Reader<'_>) -> Result<Placement, FormatError> {
    Ok(Placement {
        x: r.u16("x")?,
        y: r.u16("y")?,
        direction: r.u32("direction")?,
        unknown_0x08: r.u32("placement unknown_0x08")?,
        unknown_0x0c: r.i16("placement unknown_0x0c")?,
        unknown_0x0e: r.u16("placement unknown_0x0e")?,
        unknown_0x10: r.u16("placement unknown_0x10")?,
    })
}

fn point(r: &mut Reader<'_>) -> Result<Point, FormatError> {
    Ok(Point {
        x: r.u16("x")?,
        y: r.u16("y")?,
        unknown_0x04: r.u16("point unknown_0x04")?,
        unknown_0x06: r.u16("point unknown_0x06")?,
    })
}

fn polygon(r: &mut Reader<'_>) -> Result<Polygon, FormatError> {
    let unknown_a = r.u8("polygon unknown_a")?;
    let n = count(r, "polygon point count")?;
    let mut points = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        points.push((r.u16("polygon x")?, r.u16("polygon y")?));
    }
    Ok(Polygon {
        unknown_a,
        points,
        unknown_b: r.u8("polygon unknown_b")?,
    })
}

fn optional_name(r: &mut Reader<'_>) -> Result<Option<String>, FormatError> {
    let flag = r.u8("has_name")?;
    match flag {
        0 => Ok(None),
        1 => Ok(Some(r.pstring16("name")?)),
        other => Err(FormatError::Invalid {
            offset: r.pos() - 1,
            what: "has_name",
            value: other.to_string(),
        }),
    }
}

/// Read `u16 count` sub-chunks (`tag`, `u32 size`, `u32 version`, body) from a chunk body.
fn sub_chunks<'a>(r: &mut Reader<'a>) -> Result<Vec<RawChunk<'a>>, FormatError> {
    let n = count(r, "sub-chunk count")?;
    let mut out = Vec::with_capacity(n.min(64));
    for _ in 0..n {
        let offset = r.pos();
        let tag = r.tag("sub-chunk tag")?;
        let size = r.u32("sub-chunk size")? as usize;
        if size < 4 {
            return Err(FormatError::Invalid {
                offset: offset + 4,
                what: "sub-chunk size",
                value: size.to_string(),
            });
        }
        let version = r.u32("sub-chunk version")?;
        let body = r.bytes(size - 4, "sub-chunk body")?;
        out.push(RawChunk {
            tag,
            offset,
            version,
            body,
        });
    }
    Ok(out)
}

fn parse_header(c: &RawChunk<'_>) -> Result<Header, FormatError> {
    let mut r = Reader::new(c.body);
    let h = Header {
        version: c.version,
        map_id: r.u32("FOOT map id")?,
        variant: r.u32("FOOT variant")?,
        map: r.pstring16("FOOT map name")?,
        mission_id: r.u32("FOOT mission id")?,
    };
    r.expect_end("FOOT")?;
    Ok(h)
}

/// True when two printable `pstring16`s (1..=64 bytes each) start at `p`.
fn tenant_entry_start(b: &[u8], p: usize) -> bool {
    fn printable(s: &[u8]) -> bool {
        s.iter().all(|&c| (0x20..0x7f).contains(&c))
    }
    let Some(l1) = b.get(p..p + 2) else {
        return false;
    };
    let n1 = usize::from(u16::from_le_bytes([l1[0], l1[1]]));
    if !(1..=64).contains(&n1) {
        return false;
    }
    let Some(s1) = b.get(p + 2..p + 2 + n1) else {
        return false;
    };
    if !printable(s1) {
        return false;
    }
    let q = p + 2 + n1;
    let Some(l2) = b.get(q..q + 2) else {
        return false;
    };
    let n2 = usize::from(u16::from_le_bytes([l2[0], l2[1]]));
    if !(1..=64).contains(&n2) {
        return false;
    }
    b.get(q + 2..q + 2 + n2).is_some_and(printable)
}

/// `POUF` entries: the body length of an entry is not decoded; the reader finds the next entry by its
/// name pair (see spec). Every retail file splits cleanly this way.
fn parse_tenants(c: &RawChunk<'_>) -> Result<Vec<Tenant>, FormatError> {
    let b = c.body;
    let mut r = Reader::new(b);
    let n = count(&mut r, "POUF count")?;
    let mut out = Vec::with_capacity(n.min(256));
    for i in 0..n {
        if !tenant_entry_start(b, r.pos()) {
            return Err(FormatError::Invalid {
                offset: r.pos(),
                what: "POUF entry start",
                value: format!("entry {i}"),
            });
        }
        let sprite = r.pstring16("POUF sprite")?;
        let label = r.pstring16("POUF label")?;
        let start = r.pos();
        let mut end = b.len();
        if i + 1 < n {
            let mut p = start;
            while p < b.len() && !tenant_entry_start(b, p) {
                p += 1;
            }
            end = p;
        }
        out.push(Tenant {
            sprite,
            label,
            body: b[start..end].to_vec(),
        });
        r.seek(end)?;
    }
    r.expect_end("POUF")?;
    Ok(out)
}

fn parse_player_character(r: &mut Reader<'_>) -> Result<PlayerCharacter, FormatError> {
    let placement = placement(r)?;
    let unknown_0x12 = r.u32("SCOT unknown_0x12")?;
    let unknown_0x16 = r.array::<10>("SCOT unknown_0x16")?;
    let name = optional_name(r)?;
    let unknown_trailer = r.u8("SCOT trailer")?;
    Ok(PlayerCharacter {
        placement,
        unknown_0x12,
        unknown_0x16,
        name,
        unknown_trailer,
    })
}

fn parse_npc(r: &mut Reader<'_>) -> Result<Npc, FormatError> {
    let placement = placement(r)?;
    let unknown_0x12 = r.u32("BORG unknown_0x12")?;
    let profile = r.u32("BORG profile")?;
    let unknown_0x1a = r.u8("BORG unknown_0x1a")?;
    let unknown_0x1b = r.u32("BORG unknown_0x1b")?;
    let unknown_0x1f = r.u32("BORG unknown_0x1f")?;
    let unknown_0x23 = r.u32("BORG unknown_0x23")?;
    let n = count(r, "BORG member count")?;
    let mut members = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        members.push(r.u16("BORG member")?);
    }
    let rail = r.i16("BORG rail")?;
    let unknown_i16 = r.i16("BORG unknown_i16")?;
    let name = optional_name(r)?;
    Ok(Npc {
        placement,
        unknown_0x12,
        profile,
        unknown_0x1a,
        unknown_0x1b,
        unknown_0x1f,
        unknown_0x23,
        members,
        rail,
        unknown_i16,
        name,
    })
}

fn parse_civilian(r: &mut Reader<'_>) -> Result<Civilian, FormatError> {
    let placement = placement(r)?;
    let unknown_0x12 = r.u32("OILE unknown_0x12")?;
    let profile = r.u32("OILE profile")?;
    let unknown_i16_a = r.i16("OILE unknown_i16_a")?;
    let unknown_i16_b = r.i16("OILE unknown_i16_b")?;
    let unknown_u16 = r.u16("OILE unknown_u16")?;
    let lists = if profile == 1 {
        let mut lists = Vec::with_capacity(10);
        for _ in 0..10 {
            let m = count(r, "OILE list length")?;
            let mut l = Vec::with_capacity(m.min(256));
            for _ in 0..m {
                l.push(r.u16("OILE list id")?);
            }
            lists.push(l);
        }
        Some(lists)
    } else {
        None
    };
    let name = optional_name(r)?;
    Ok(Civilian {
        placement,
        unknown_0x12,
        profile,
        unknown_i16_a,
        unknown_i16_b,
        unknown_u16,
        lists,
        name,
    })
}

fn parse_vip(r: &mut Reader<'_>) -> Result<Vip, FormatError> {
    Ok(Vip {
        placement: placement(r)?,
        unknown_0x12: r.u32("TOTO unknown_0x12")?,
        profile: r.u32("TOTO profile")?,
        unknown_i16_a: r.i16("TOTO unknown_i16_a")?,
        unknown_i16_b: r.i16("TOTO unknown_i16_b")?,
        name: optional_name(r)?,
    })
}

fn parse_object(r: &mut Reader<'_>) -> Result<Object, FormatError> {
    Ok(Object {
        x: r.u16("BOOM x")?,
        y: r.u16("BOOM y")?,
        unknown_0x04: r.i16("BOOM unknown_0x04")?,
        unknown_0x06: r.u16("BOOM unknown_0x06")?,
        unknown_0x08: r.u16("BOOM unknown_0x08")?,
        unknown_0x0a: r.u16("BOOM unknown_0x0a")?,
        unknown_0x0c: r.u16("BOOM unknown_0x0c")?,
        unknown_0x0e: r.i16("BOOM unknown_0x0e")?,
        unknown_0x10: r.u16("BOOM unknown_0x10")?,
        unknown_0x12: r.u16("BOOM unknown_0x12")?,
        sprite: r.pstring16("BOOM sprite")?,
        label: r.pstring16("BOOM label")?,
        unknown_flags: r.u32("BOOM unknown_flags")?,
        x2: r.u16("BOOM x2")?,
        y2: r.u16("BOOM y2")?,
        unknown_q2: r.u16("BOOM unknown_q2")?,
        unknown_r2: r.u16("BOOM unknown_r2")?,
        polygon: polygon(r)?,
        unknown_u8: r.u8("BOOM unknown_u8")?,
        name: optional_name(r)?,
    })
}

fn records<T>(
    body: &[u8],
    what: &'static str,
    mut f: impl FnMut(&mut Reader<'_>) -> Result<T, FormatError>,
) -> Result<Vec<T>, FormatError> {
    let mut r = Reader::new(body);
    let n = count(&mut r, what)?;
    let mut out = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        out.push(f(&mut r)?);
    }
    r.expect_end(what)?;
    Ok(out)
}

fn parse_actor_groups(c: &RawChunk<'_>) -> Result<Vec<ActorGroup>, FormatError> {
    let mut r = Reader::new(c.body);
    let groups = sub_chunks(&mut r)?;
    r.expect_end("BOYZ")?;
    let mut out = Vec::with_capacity(groups.len());
    for g in groups {
        let version = g.version;
        out.push(match &g.tag {
            b"MEOW" => {
                let mut gr = Reader::new(g.body);
                let n = gr.u16("MEOW count")?;
                if n != 0 {
                    return Err(FormatError::Invalid {
                        offset: g.offset + 12,
                        what: "MEOW count",
                        value: n.to_string(),
                    });
                }
                gr.expect_end("MEOW")?;
                ActorGroup::Meow { version, count: n }
            }
            b"SCOT" => ActorGroup::PlayerCharacters {
                version,
                records: records(g.body, "SCOT", parse_player_character)?,
            },
            b"OILE" => ActorGroup::Civilians {
                version,
                records: records(g.body, "OILE", parse_civilian)?,
            },
            b"TOTO" => ActorGroup::Vips {
                version,
                records: records(g.body, "TOTO", parse_vip)?,
            },
            b"BORG" => ActorGroup::Npcs {
                version,
                records: records(g.body, "BORG", parse_npc)?,
            },
            b"BOOM" => ActorGroup::Objects {
                version,
                records: records(g.body, "BOOM", parse_object)?,
            },
            _ => ActorGroup::Unknown {
                tag: g.tag,
                version,
                body: g.body.to_vec(),
            },
        });
    }
    Ok(out)
}

fn parse_zorg(c: &RawChunk<'_>) -> Result<Vec<ZorgEntry>, FormatError> {
    records(c.body, "ZORG", |r| {
        Ok(ZorgEntry {
            unknown_a: r.u16("ZORG unknown_a")?,
            unknown_b: r.u16("ZORG unknown_b")?,
            placement: placement(r)?,
        })
    })
}

fn parse_brains(c: &RawChunk<'_>) -> Result<Brains, FormatError> {
    let mut r = Reader::new(c.body);
    let subs = sub_chunks(&mut r)?;
    r.expect_end("HIRN")?;
    let mut b = Brains::default();
    for s in subs {
        b.versions.push((s.tag, s.version));
        match &s.tag {
            b"HOLE" => {
                b.waypoints = records(s.body, "HOLE", |r| {
                    Ok(Waypoint {
                        x: r.u16("HOLE x")?,
                        y: r.u16("HOLE y")?,
                        unknown_0x04: r.u16("HOLE unknown_0x04")?,
                        unknown_0x06: r.u16("HOLE unknown_0x06")?,
                        direction: r.u16("HOLE direction")?,
                    })
                })?;
            }
            b"BUSH" => b.bushes = records(s.body, "BUSH", point)?,
            b"POW " => b.beam_points = records(s.body, "POW", placement)?,
            b"NLIP" => {
                b.nlips = records(s.body, "NLIP", |r| {
                    let unknown_0x00 = r.u32("NLIP unknown_0x00")?;
                    let polygon = polygon(r)?;
                    let m = count(r, "NLIP point count")?;
                    let mut points = Vec::with_capacity(m.min(256));
                    for _ in 0..m {
                        points.push(NlipPoint {
                            point: point(r)?,
                            unknown_flag: r.u8("NLIP flag")?,
                            unknown_value: r.u16("NLIP value")?,
                        });
                    }
                    Ok(Nlip {
                        unknown_0x00,
                        polygon,
                        points,
                    })
                })?;
            }
            _ => {
                return Err(FormatError::BadMagic {
                    offset: s.offset,
                    expected: String::from("HOLE/BUSH/POW /NLIP"),
                    found: tag_string(s.tag),
                });
            }
        }
    }
    Ok(b)
}

fn parse_commands(block: &[u8], base: usize) -> Result<Vec<Command>, FormatError> {
    let mut r = Reader::new(block);
    let mut out = Vec::new();
    while r.remaining() > 0 {
        let opcode = r.u8("command opcode")?;
        let Some(n) = command_arg_size(opcode) else {
            return Err(FormatError::Invalid {
                offset: base + r.pos() - 1,
                what: "waypoint command opcode",
                value: format!("{opcode:#04x}"),
            });
        };
        out.push(Command {
            opcode,
            args: r.bytes(n, "command operands")?.to_vec(),
        });
    }
    Ok(out)
}

/// Program: `u16 table count`, tables `(u8 id, u16 offset)`; each table at its offset: `u16 block count`,
/// blocks `(u8 percent, u16 offset)`; each block at its offset: `u16 length`, commands. Offsets are
/// relative to the program start and always point at the next byte, so the layout is sequential.
fn parse_program(payload: &[u8], base: usize) -> Result<Vec<CommandTable>, FormatError> {
    if payload.is_empty() {
        return Ok(Vec::new());
    }
    let mut r = Reader::new(payload);
    let ntab = count(&mut r, "program table count")?;
    let mut heads = Vec::with_capacity(ntab.min(16));
    for _ in 0..ntab {
        heads.push((r.u8("table id")?, r.u16("table offset")?));
    }
    let mut tables = Vec::with_capacity(ntab.min(16));
    for (id, off) in heads {
        if usize::from(off) != r.pos() {
            return Err(FormatError::Invalid {
                offset: base + r.pos(),
                what: "table offset",
                value: off.to_string(),
            });
        }
        let nseg = count(&mut r, "block count")?;
        let mut segs = Vec::with_capacity(nseg.min(16));
        for _ in 0..nseg {
            segs.push((r.u8("block percent")?, r.u16("block offset")?));
        }
        let mut blocks = Vec::with_capacity(nseg.min(16));
        for (percent, off) in segs {
            if usize::from(off) != r.pos() {
                return Err(FormatError::Invalid {
                    offset: base + r.pos(),
                    what: "block offset",
                    value: off.to_string(),
                });
            }
            let len = count(&mut r, "block length")?;
            let start = r.pos();
            let bytes = r.bytes(len, "block")?;
            blocks.push(CommandBlock {
                percent,
                commands: parse_commands(bytes, base + start)?,
            });
        }
        tables.push(CommandTable { id, blocks });
    }
    r.expect_end("program")?;
    Ok(tables)
}

fn parse_rails(c: &RawChunk<'_>) -> Result<Vec<Vec<RailPoint>>, FormatError> {
    let mut r = Reader::new(c.body);
    let n = count(&mut r, "RAIL count")?;
    let mut rails = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        let m = count(&mut r, "RAIL point count")?;
        let mut pts = Vec::with_capacity(m.min(1024));
        for _ in 0..m {
            let point = point(&mut r)?;
            let kind = r.u8("RAIL point kind")?;
            let len = count(&mut r, "RAIL payload length")?;
            let start = r.pos();
            let payload = r.bytes(len, "RAIL payload")?;
            let (name, tables) = match kind {
                0 => (None, parse_program(payload, c.offset + 12 + start)?),
                1 => (Some(latin1(payload)), Vec::new()),
                other => {
                    return Err(FormatError::Invalid {
                        offset: c.offset + 12 + start - 3,
                        what: "RAIL point kind",
                        value: other.to_string(),
                    });
                }
            };
            pts.push(RailPoint {
                point,
                kind,
                name,
                tables,
            });
        }
        rails.push(pts);
    }
    r.expect_end("RAIL")?;
    Ok(rails)
}

fn parse_scrolls(c: &RawChunk<'_>) -> Result<Vec<Scroll>, FormatError> {
    records(c.body, "SKRO", |r| {
        Ok(Scroll {
            placement: placement(r)?,
            unknown_flags: r.array::<5>("SKRO flags")?,
            name: optional_name(r)?,
        })
    })
}

fn parse_mobiles(c: &RawChunk<'_>) -> Result<Vec<Mobile>, FormatError> {
    let mut r = Reader::new(c.body);
    let n = count(&mut r, "TING count")?;
    let mut out = Vec::with_capacity(n.min(64));
    for _ in 0..n {
        r.expect(b"FLIM", "TING FLIM tag")?;
        let size = r.u32("FLIM size")? as usize;
        if size < 4 {
            return Err(FormatError::Invalid {
                offset: r.pos() - 4,
                what: "FLIM size",
                value: size.to_string(),
            });
        }
        let flim_version = r.u32("FLIM version")?;
        let mut f = Reader::new(r.bytes(size - 4, "FLIM body")?);
        let m = count(&mut f, "FLIM count")?;
        let mut animations = Vec::with_capacity(m.min(16));
        for _ in 0..m {
            animations.push(MobileAnimation {
                sprite: f.pstring16("FLIM sprite")?,
                animation: f.pstring16("FLIM animation")?,
                dx: f.i16("FLIM dx")?,
                dy: f.i16("FLIM dy")?,
                unknown_0x04: f.u16("FLIM unknown_0x04")?,
                unknown_0x06: f.array::<3>("FLIM unknown_0x06")?,
                polygon: polygon(&mut f)?,
            });
        }
        f.expect_end("FLIM")?;
        r.expect(b"WOAW", "TING WOAW tag")?;
        let size = r.u32("WOAW size")? as usize;
        if size < 6 {
            return Err(FormatError::Invalid {
                offset: r.pos() - 4,
                what: "WOAW size",
                value: size.to_string(),
            });
        }
        let woaw_version = r.u32("WOAW version")?;
        let woaw_count = r.u16("WOAW count")?;
        let woaw_rest = r.bytes(size - 6, "WOAW body")?.to_vec();
        out.push(Mobile {
            flim_version,
            animations,
            woaw_version,
            woaw_count,
            woaw_rest,
            polygon: polygon(&mut r)?,
            x: r.u16("TING x")?,
            y: r.u16("TING y")?,
            unknown_a: r.u16("TING unknown_a")?,
            unknown_b: r.u32("TING unknown_b")?,
            unknown_c: r.u16("TING unknown_c")?,
            unknown_d: r.u32("TING unknown_d")?,
            unknown_e: r.i16("TING unknown_e")?,
        });
    }
    r.expect_end("TING")?;
    Ok(out)
}

fn parse_script_areas(c: &RawChunk<'_>) -> Result<ScriptAreas, FormatError> {
    let mut r = Reader::new(c.body);
    let n = count(&mut r, "GULP point count")?;
    let mut points = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        points.push(point(&mut r)?);
    }
    let m = count(&mut r, "GULP polygon count")?;
    let mut polygons = Vec::with_capacity(m.min(256));
    for _ in 0..m {
        let unknown_a = r.u8("GULP polygon unknown_a")?;
        let k = count(&mut r, "GULP polygon point count")?;
        let mut pts = Vec::with_capacity(k.min(256));
        for _ in 0..k {
            pts.push((r.u16("GULP x")?, r.u16("GULP y")?));
        }
        let unknown_b = r.u8("GULP polygon unknown_b")?;
        polygons.push(ScriptPolygon {
            polygon: Polygon {
                unknown_a,
                points: pts,
                unknown_b,
            },
            unknown_0x00: r.u16("GULP polygon unknown_0x00")?,
            unknown_0x02: r.u16("GULP polygon unknown_0x02")?,
            name: optional_name(&mut r)?,
        });
    }
    r.expect_end("GULP")?;
    Ok(ScriptAreas { points, polygons })
}

fn parse_cave(c: &RawChunk<'_>) -> Result<Vec<CaveEntry>, FormatError> {
    records(c.body, "CAVE", |r| {
        let m = count(r, "CAVE id count")?;
        let mut ids = Vec::with_capacity(m.min(256));
        for _ in 0..m {
            ids.push(r.u16("CAVE id")?);
        }
        Ok(CaveEntry {
            ids,
            unknown_flag: r.u8("CAVE flag")?,
        })
    })
}

/// Parse a mission file.
pub fn parse(data: &[u8]) -> Result<Mission, FormatError> {
    let c = chunk::parse_container(data, b"DUTY")?;
    let mut header = None;
    let mut m = Mission {
        version: c.version,
        header: Header {
            version: 0,
            map_id: 0,
            variant: 0,
            map: String::new(),
            mission_id: 0,
        },
        tenants: Vec::new(),
        actor_groups: Vec::new(),
        zorg: Vec::new(),
        brains: Brains::default(),
        rails: Vec::new(),
        scrolls: Vec::new(),
        mobiles: Vec::new(),
        script_areas: ScriptAreas::default(),
        cave: Vec::new(),
        chunk_versions: Vec::new(),
        unknown_chunks: Vec::new(),
    };
    for ch in &c.children {
        m.chunk_versions.push((ch.tag, ch.version));
        match &ch.tag {
            b"FOOT" => header = Some(parse_header(ch)?),
            b"POUF" => m.tenants = parse_tenants(ch)?,
            b"BOYZ" => m.actor_groups = parse_actor_groups(ch)?,
            b"ZORG" => m.zorg = parse_zorg(ch)?,
            b"HIRN" => m.brains = parse_brains(ch)?,
            b"RAIL" => m.rails = parse_rails(ch)?,
            b"SKRO" => m.scrolls = parse_scrolls(ch)?,
            b"TING" => m.mobiles = parse_mobiles(ch)?,
            b"GULP" => m.script_areas = parse_script_areas(ch)?,
            b"CAVE" => m.cave = parse_cave(ch)?,
            _ => m.unknown_chunks.push((ch.tag, ch.version)),
        }
    }
    let Some(header) = header else {
        return Err(FormatError::BadMagic {
            offset: 12,
            expected: String::from("FOOT"),
            found: String::from("(missing)"),
        });
    };
    m.header = header;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: [u8; 4], version: u32, body: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        v.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        v.extend_from_slice(&version.to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn pstr(s: &str) -> Vec<u8> {
        let mut v = (s.len() as u16).to_le_bytes().to_vec();
        v.extend_from_slice(s.as_bytes());
        v
    }

    fn placement_bytes(x: u16, y: u16, dir: u32, flags: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&x.to_le_bytes());
        v.extend_from_slice(&y.to_le_bytes());
        v.extend_from_slice(&dir.to_le_bytes());
        v.extend_from_slice(&flags.to_le_bytes());
        v.extend_from_slice(&(-1i16).to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v
    }

    fn synthetic() -> Vec<u8> {
        let mut foot = 101u32.to_le_bytes().to_vec();
        foot.extend_from_slice(&1u32.to_le_bytes());
        foot.extend(pstr("Croisement03"));
        foot.extend_from_slice(&20u32.to_le_bytes());

        // BOYZ: one SCOT record with a name, one BORG record with a member list.
        let mut scot = 1u16.to_le_bytes().to_vec();
        scot.extend(placement_bytes(10, 20, 3, 3));
        scot.extend_from_slice(&3u32.to_le_bytes());
        scot.extend_from_slice(&[0u8; 10]);
        scot.push(1);
        scot.extend(pstr("hidden_pc01_80000048"));
        scot.push(4);
        let mut borg = 1u16.to_le_bytes().to_vec();
        borg.extend(placement_bytes(30, 40, 8, 3));
        borg.extend_from_slice(&3u32.to_le_bytes());
        borg.extend_from_slice(&30u32.to_le_bytes());
        borg.push(1);
        borg.extend_from_slice(&[0u8; 12]);
        borg.extend_from_slice(&2u16.to_le_bytes());
        borg.extend_from_slice(&5u16.to_le_bytes());
        borg.extend_from_slice(&6u16.to_le_bytes());
        borg.extend_from_slice(&0i16.to_le_bytes());
        borg.extend_from_slice(&(-1i16).to_le_bytes());
        borg.push(0);
        let mut boyz = 2u16.to_le_bytes().to_vec();
        boyz.extend(chunk(*b"SCOT", 4, &scot));
        boyz.extend(chunk(*b"BORG", 4, &borg));

        // RAIL: one rail with a named point and a point with a program.
        let program: Vec<u8> = vec![
            1, 0, 0, 5, 0, 1, 0, 100, 10, 0, 6, 0, 0x03, 6, 0, 0x04, 0xf4, 0x01,
        ];
        let mut rail = 1u16.to_le_bytes().to_vec();
        rail.extend_from_slice(&2u16.to_le_bytes());
        rail.extend_from_slice(&[1, 0, 2, 0, 0, 0, 0, 0, 1]);
        rail.extend(pstr("Point1__0___8000039f"));
        rail.extend_from_slice(&[3, 0, 4, 0, 0, 0, 0, 0, 0]);
        rail.extend_from_slice(&(program.len() as u16).to_le_bytes());
        rail.extend(program);

        let mut gulp = 0u16.to_le_bytes().to_vec();
        gulp.extend_from_slice(&1u16.to_le_bytes());
        gulp.extend_from_slice(&[
            7, 3, 0, 1, 0, 1, 0, 2, 0, 1, 0, 1, 0, 2, 0, 9, 0, 0, 0, 0, 1,
        ]);
        gulp.extend(pstr("trou01_v2_8000003e"));

        let mut hirn = 2u16.to_le_bytes().to_vec();
        hirn.extend(chunk(*b"HOLE", 2, &[1, 0, 5, 0, 6, 0, 0, 0, 0, 0, 9, 0]));
        hirn.extend(chunk(*b"POW ", 2, &[0, 0]));

        let mut body = chunk(*b"FOOT", 4, &foot);
        body.extend(chunk(*b"POUF", 3, &[0, 0]));
        body.extend(chunk(*b"BOYZ", 3, &boyz));
        body.extend(chunk(*b"ZORG", 2, &[0, 0]));
        body.extend(chunk(*b"HIRN", 2, &hirn));
        body.extend(chunk(*b"RAIL", 3, &rail));
        body.extend(chunk(*b"SKRO", 4, &[0, 0]));
        body.extend(chunk(*b"TING", 3, &[0, 0]));
        body.extend(chunk(*b"GULP", 2, &gulp));
        body.extend(chunk(*b"CAVE", 3, &[1, 0, 1, 0, 7, 0, 1]));
        chunk(*b"DUTY", 2, &body)
    }

    #[test]
    fn parses_synthetic_mission() {
        let m = parse(&synthetic()).unwrap();
        assert_eq!(m.header.map, "Croisement03");
        assert_eq!(m.header.mission_id, 20);
        let pcs = m.player_characters();
        assert_eq!(pcs.len(), 1);
        assert_eq!(pcs[0].name.as_deref(), Some("hidden_pc01_80000048"));
        assert_eq!(pcs[0].unknown_trailer, 4);
        let npcs = m.npcs();
        assert_eq!(npcs[0].profile, 30);
        assert_eq!(npcs[0].members, vec![5, 6]);
        assert_eq!(npcs[0].rail, 0);
        assert_eq!(m.rails.len(), 1);
        assert_eq!(m.rails[0][0].name.as_deref(), Some("Point1__0___8000039f"));
        let tables = &m.rails[0][1].tables;
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].blocks[0].percent, 100);
        assert_eq!(
            tables[0].blocks[0].commands,
            vec![
                Command {
                    opcode: 3,
                    args: vec![6, 0]
                },
                Command {
                    opcode: 4,
                    args: vec![0xf4, 1]
                }
            ]
        );
        assert_eq!(m.script_areas.polygons[0].polygon.points.len(), 3);
        assert_eq!(
            m.script_areas.polygons[0].name.as_deref(),
            Some("trou01_v2_8000003e")
        );
        assert_eq!(m.brains.waypoints[0].direction, 9);
        assert_eq!(m.cave[0].ids, vec![7]);
        assert_eq!(m.script_names().len(), 3);
    }

    #[test]
    fn garbage_does_not_panic() {
        let good = synthetic();
        for n in 0..good.len() {
            let _ = parse(&good[..n]);
        }
        for i in 0..good.len() {
            let mut bad = good.clone();
            bad[i] ^= 0xff;
            let _ = parse(&bad);
        }
    }
}
