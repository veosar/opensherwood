//! `MEUH` map files (`.rhp`): the static geometry of a location. Spec: `docs/formats/rhp.md`.
//!
//! Coordinates are pixels of the prerendered background (`Levels/<Variant>/<map>.map`), origin top-left.
//! `STAT`, `FARM` and ` AZ ` have typed navigation records, specified in
//! `docs/original/spec-navigation.md` section 2. `TUPO` and `LOUD` remain raw.

use crate::chunk;
use crate::reader::{FormatError, Reader, tag_string};

/// Root container version seen in retail data.
pub const VERSION: u32 = 2;

/// Bond area id meaning "no area" (map edge).
pub const NO_AREA: u16 = 0xffff;

/// `FACE` kind bit: the record carries a list of `u16` references in its trailer.
pub const FACE_HAS_REFS: u8 = 0x10;

/// A point in map pixels. Signed: the files store `i16` and polygons may extend past the map
/// edge (`docs/original/spec-navigation.md` section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    /// Column.
    pub x: i16,
    /// Row.
    pub y: i16,
}

/// A point in map pixels with a height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point3 {
    /// Column.
    pub x: u16,
    /// Row.
    pub y: u16,
    /// Height above the ground plane.
    pub z: u16,
}

/// `u8 id, u16 count, count x Point, u8 id2`: the common point-list framing. The two id bytes are the
/// same in every map for the same list index (editor-assigned pseudo-random ids); see spec.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Polyline {
    /// Leading id byte.
    pub id: u8,
    /// Points.
    pub points: Vec<Point>,
    /// Trailing id byte.
    pub id2: u8,
}

/// `SPOK`: 9-byte header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spok {
    /// 100 / 86 / 101 for Croisement01..03, 92 for sherwood, 194..916 for towns; matches the first
    /// `u32` of the `FOOT` chunk of the missions on that map.
    pub unknown_0x00: u32,
    /// 1 for the forest maps and sherwood, 0 for the towns.
    pub unknown_0x04: u32,
    /// Always 0.
    pub unknown_0x08: u8,
}

/// A line segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Segment {
    /// First end.
    pub a: Point,
    /// Second end.
    pub b: Point,
}

/// An obstacle polygon of a `STAT` sector: a hole in the sector's floor
/// (`spec-navigation.md` NAV-012).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obstacle {
    /// Availability mask (NAV-056); 0 for most obstacles.
    pub mask: u32,
    /// Outline (`id` / `id2` are the framing bytes).
    pub polygon: Polyline,
}

/// One sector of a `STAT` layer: the walkable floor, its search barriers and its holes
/// (`spec-navigation.md` 2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sector {
    /// 0 = plain sector, 1 = lift sector (` AZ ` attaches its ends).
    pub kind: u8,
    /// Stored; no routine read consumes it (NAV-012 note); 0 on every retail sector.
    pub sector_mask: u32,
    /// The walkable floor outline, in screen coordinates.
    pub outline: Polyline,
    /// Search barrier segments (NAV-141); empty on the forest maps.
    pub segments: Vec<Segment>,
    /// 0 on every retail sector; consulted when a player character passes a door (NAV-171).
    pub unknown_flag: u32,
    /// Holes in the floor.
    pub obstacles: Vec<Obstacle>,
}

/// One layer of `STAT`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatLayer {
    /// The layer's sectors, in file order.
    pub sectors: Vec<Sector>,
}

/// Where a graph edge's endpoint sits in the graph hierarchy (`spec-navigation.md` 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NodeRef {
    /// Layer index.
    pub layer: u16,
    /// Sector *position within the layer* (not the polygon number).
    pub sector: u16,
    /// Region within the sector.
    pub region: u16,
    /// Node within the region.
    pub node: u16,
}

/// A corner of the path graph (`spec-navigation.md` 2.3, NAV-021).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    /// One byte per size class: which of the four candidate waypoints are usable (NAV-021).
    pub offset_bits: Vec<u8>,
    /// Corner, column.
    pub x: i16,
    /// Corner, row.
    pub y: i16,
    /// Stored geometric input, not consumed by any routine read (NAV-025).
    pub v1x: i16,
    /// Idem.
    pub v1y: i16,
    /// Idem.
    pub v2x: i16,
    /// Idem.
    pub v2y: i16,
    /// Availability mask (NAV-056).
    pub node_mask: u32,
    /// Indices of the edges that **enter** this node (NAV-022).
    pub entering_edges: Vec<u16>,
}

/// An edge of the path graph (`spec-navigation.md` 2.3, NAV-024).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    /// Node A.
    pub a: NodeRef,
    /// Node B.
    pub b: NodeRef,
    /// The edge's length as the search uses it, as raw `f32` bits (the struct stays `Eq`).
    pub cost_bits: u32,
    /// Availability mask (NAV-056); nonzero on five maps.
    pub edge_mask: u32,
    /// One record index per size class.
    pub records: Vec<u16>,
}

impl GraphEdge {
    /// The edge cost (NAV-024).
    #[must_use]
    pub fn cost(&self) -> f32 {
        f32::from_bits(self.cost_bits)
    }
}

/// A per-size-class record of an edge: the candidate pairs joined by a straight walkable
/// segment (`spec-navigation.md` NAV-023).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GraphRecord {
    /// -1 = impassable for this size class (no further fields); else the OR of `bits_b`.
    pub kind: i8,
    /// The OR of `bits_a`.
    pub mask: u8,
    /// Candidate bit at node B of pair `i`.
    pub bits_b: Vec<u8>,
    /// Candidate bit at node A of pair `i`.
    pub bits_a: Vec<u8>,
}

impl GraphRecord {
    /// Whether the edge is passable for this size class.
    #[must_use]
    pub fn passable(&self) -> bool {
        self.kind >= 0
    }
}

/// The path graph stored in `STAT` after the sectors (`spec-navigation.md` 2.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Graph {
    /// `(half_width, half_height)` per size class, as raw `f32` bits; one class `(6, 4)` on
    /// every retail map.
    pub size_classes: Vec<(u32, u32)>,
    /// `regions[layer][sector][region]` -> the region's nodes, in file order.
    pub regions: Vec<Vec<Vec<Vec<GraphNode>>>>,
    /// The edge table.
    pub edges: Vec<GraphEdge>,
    /// The record table (one entry per edge on every retail map).
    pub records: Vec<GraphRecord>,
}

/// `STAT` (version 2): the layers with their sectors, and the path graph
/// (`docs/original/spec-navigation.md` 2.2 and 2.3, which supersede the earlier reading).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stat {
    /// Layers in file order.
    pub layers: Vec<StatLayer>,
    /// The path graph.
    pub graph: Graph,
}

impl Stat {
    /// The polygon number of sector `s` of layer `l` (NAV-011: sectors and their obstacles are
    /// numbered together in reading order). `None` when the indices name no sector.
    #[must_use]
    pub fn sector_number(&self, layer: usize, sector: usize) -> Option<u32> {
        let mut n = 0u32;
        for (li, la) in self.layers.iter().enumerate() {
            for (si, se) in la.sectors.iter().enumerate() {
                if li == layer && si == sector {
                    return Some(n);
                }
                n += 1 + se.obstacles.len() as u32;
            }
        }
        None
    }

    /// Total number of polygon numbers the map uses (sectors plus obstacles); the level's
    /// default sector follows this number (NAV-011).
    #[must_use]
    pub fn polygon_count(&self) -> u32 {
        self.layers
            .iter()
            .flat_map(|l| l.sectors.iter())
            .map(|s| 1 + s.obstacles.len() as u32)
            .sum()
    }
}

/// A `TEXT` zone: a polygon with a small kind number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextZone {
    /// 0..8.
    pub kind: u8,
    /// Outline.
    pub polygon: Polyline,
}

/// A vertex of a projection area.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AreaPoint {
    /// Column.
    pub x: f32,
    /// Row.
    pub y: f32,
    /// 0.001 for most vertices; a second height for some.
    pub unknown_0x08: f32,
    /// Height.
    pub z: f32,
}

/// A reference stored with an area. The `(unknown_ref, unknown_layer)` pairs of the `PPPP` zones of a map
/// are a subset of the pairs found here (all of them in Croisement01); the target is not established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AreaLink {
    /// Id, 0..0x7f.
    pub unknown_ref: u16,
    /// 0..12; below the layer count.
    pub unknown_layer: u16,
}

/// A `WOAW` projection area.
#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    /// Polygon (3 or more vertices).
    pub points: Vec<AreaPoint>,
    /// Minimum of `x`, `y` and `unknown_0x08` over the vertices.
    pub min: [f32; 3],
    /// Maximum of `x`, `y` and `z` over the vertices.
    pub max: [f32; 3],
    /// Zone references.
    pub links: Vec<AreaLink>,
    /// Mostly `[1, 1, 1, 1]`.
    pub unknown_flags: [u8; 4],
    /// 0..7.
    pub unknown_a: u8,
    /// Layer ids (empty for nearly every area).
    pub layers: Vec<u16>,
}

/// `WOAW`: layers and projection areas.
#[derive(Debug, Clone, PartialEq)]
pub struct Woaw {
    /// Layer ids.
    pub layers: Vec<u16>,
    /// Areas.
    pub areas: Vec<Area>,
}

/// `007 `: a bond between two projection areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bond {
    /// First end, column.
    pub x1: i16,
    /// First end, row.
    pub y1: i16,
    /// Second end, column.
    pub x2: i16,
    /// Second end, row.
    pub y2: i16,
    /// Index into `Woaw::areas`.
    pub area_a: u16,
    /// Index into `Woaw::areas` or [`NO_AREA`].
    pub area_b: u16,
    /// 0..10.
    pub unknown_0x0c: u16,
}

/// `FACE`: a foreground occluder mask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    /// 0..12; same value range as `DarkZone::unknown_0x00` (layer?).
    pub unknown_0x00: u16,
    /// Bits 0 and 1: number of polylines; bit 4: reference list present.
    pub kind: u8,
    /// Depth-sorting polylines (0, 1 or 2).
    pub lines: Vec<Polyline>,
    /// Left edge of the mask on the map.
    pub x: u16,
    /// Top edge of the mask on the map.
    pub y: u16,
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// 1-bit mask, rows of `stride()` bytes, most significant bit first.
    pub mask: Vec<u8>,
    /// Reference list (only when `kind & FACE_HAS_REFS`; values are below the `WOAW` area count).
    pub refs: Vec<u16>,
}

impl Face {
    /// Bytes per mask row.
    #[must_use]
    pub fn stride(&self) -> usize {
        usize::from(self.width).div_ceil(8)
    }

    /// Whether the mask covers the pixel at (`x`, `y`) relative to the mask origin.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        if x >= usize::from(self.width) || y >= usize::from(self.height) {
            return false;
        }
        let byte = self.mask[y * self.stride() + x / 8];
        byte & (0x80 >> (x % 8)) != 0
    }
}

/// `FLIM`: an animated background element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flim {
    /// Sprite profile name (e.g. `Treecr01`).
    pub sprite: String,
    /// Instance name.
    pub name: String,
    /// Column.
    pub x: u16,
    /// Row.
    pub y: u16,
    /// 0..1200; purpose unknown.
    pub unknown_0x04: u16,
    /// Three flag bytes, each 0 or 1.
    pub unknown_flags: [u8; 3],
    /// Sorting line (0 or 2 points).
    pub line: Polyline,
}

/// `DARK`: a dark zone polygon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DarkZone {
    /// 0..12.
    pub unknown_0x00: u16,
    /// Outline.
    pub polygon: Polyline,
    /// 2, 4 or 6.
    pub unknown_value: u32,
}

/// `PPPP`: a zone polygon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    /// Outline.
    pub polygon: Polyline,
    /// Id, 0 or 0x19..0x81; nonzero values also occur as `AreaLink::unknown_ref`.
    pub unknown_ref: u16,
    /// 0..8; also occurs as `AreaLink::unknown_layer`.
    pub unknown_layer: u16,
    /// 0 or 1.
    pub unknown_flag: u8,
}

/// `PPPP`: a jump line from one 3D segment to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JumpLine {
    /// Start segment.
    pub from: [Point3; 2],
    /// Small integer.
    pub unknown_a: u16,
    /// End segment.
    pub to: [Point3; 2],
    /// Small integer.
    pub unknown_b: u16,
    /// 0 or 1.
    pub unknown_c: u8,
}

/// `PPPP` chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pppp {
    /// Zones.
    pub zones: Vec<Zone>,
    /// Jump lines.
    pub jump_lines: Vec<JumpLine>,
}

/// A door record of `FARM` or ` AZ ` (`spec-navigation.md` 2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Door {
    /// 0 default, 3 gate, 7 gate variant, 1 / 2 building door, 4 upper end of stairs,
    /// 5 lower end, 6 upper end of a climb (NAV-040, NAV-170).
    pub door_type: u8,
    /// Initial open state (1 = open).
    pub open: u8,
    /// The active lock set, in file order: player, lock-pick, civilian, soldier.
    pub lock: [u8; 4],
    /// The alternate lock set a patch swaps in (NAV-172).
    pub alt: [u8; 4],
    /// The door leaf outline on the background; an empty polygon means "no leaf".
    pub leaf: Polyline,
    /// Side-A point, inside sector A.
    pub a: Point,
    /// Polygon number of sector A.
    pub sector_a: u16,
    /// Layer of sector A.
    pub layer_a: u16,
    /// The threshold, between the sides.
    pub threshold: Point,
    /// Side-B point, inside sector B.
    pub b: Point,
    /// Polygon number of sector B (ignored for building and lift doors).
    pub sector_b: u16,
    /// Layer of sector B (ignored for building and lift doors).
    pub layer_b: u16,
}

/// A `FARM` record: a passage (kind 0) or a building (any other kind), which is only its doors
/// (`spec-navigation.md` NAV-043).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FarmRecord {
    /// 0 = passage record, else a building.
    pub kind: u8,
    /// The record's doors.
    pub doors: Vec<Door>,
}

/// An ` AZ ` record: a lift (stairs, ladder or climb) attached to a kind-1 sector
/// (`spec-navigation.md` NAV-042).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lift {
    /// Polygon number of the lift's sector.
    pub sector: u16,
    /// Layer (equals the sector's layer on retail data).
    pub layer: u16,
    /// 1 = stairs, 2 = ladder, 3 = climb; 0 is corrected to 1.
    pub lift_type: u8,
    /// Shape polygon (0 points on retail data).
    pub shape: Polyline,
    /// The lift's end doors.
    pub doors: Vec<Door>,
    /// Trailer word; meaning unread (`spec-navigation.md` section 9 item 8).
    pub unknown_trailer: u16,
}

/// A parsed map.
#[derive(Debug, Clone, PartialEq)]
pub struct Rhp {
    /// Root version (2).
    pub version: u32,
    /// `SPOK`.
    pub spok: Spok,
    /// `STAT`.
    pub stat: Stat,
    /// `TEXT`.
    pub text: Vec<TextZone>,
    /// `WOAW`.
    pub woaw: Woaw,
    /// `007 `.
    pub bonds: Vec<Bond>,
    /// `FACE`.
    pub faces: Vec<Face>,
    /// `FLIM`.
    pub flims: Vec<Flim>,
    /// `FARM`: passage records and buildings, in file order.
    pub farm: Vec<FarmRecord>,
    /// ` AZ `: lifts, in file order.
    pub az: Vec<Lift>,
    /// `DARK`.
    pub dark: Vec<DarkZone>,
    /// `TUPO` body after the version word (layout not established beyond the leading count).
    pub tupo: Vec<u8>,
    /// `LOUD` body after the version word (layout not established).
    pub loud: Vec<u8>,
    /// `PPPP`.
    pub pppp: Pppp,
}

impl Rhp {
    /// Number of `TUPO` records (map patches): the leading `u16` of the raw body, 0 when the
    /// body is shorter than that. With the `FLIM` count it sizes the map's part of the script
    /// element table (`docs/formats/scb.md`, "Index spaces").
    #[must_use]
    pub fn tupo_count(&self) -> u16 {
        tupo_count(&self.tupo)
    }
}

/// The `u16 n` at the head of a raw `TUPO` body (`docs/formats/rhp.md`, "Raw chunks"); 0 for a
/// body too short to hold it.
#[must_use]
pub fn tupo_count(tupo: &[u8]) -> u16 {
    match tupo {
        [lo, hi, ..] => u16::from_le_bytes([*lo, *hi]),
        _ => 0,
    }
}

fn room(r: &Reader<'_>, n: usize, size: usize, what: &'static str) -> Result<(), FormatError> {
    if n.saturating_mul(size) > r.remaining() {
        return Err(FormatError::Invalid {
            offset: r.pos(),
            what,
            value: n.to_string(),
        });
    }
    Ok(())
}

fn read_point(r: &mut Reader<'_>, what: &'static str) -> Result<Point, FormatError> {
    Ok(Point {
        x: r.i16(what)?,
        y: r.i16(what)?,
    })
}

fn read_point3(r: &mut Reader<'_>, what: &'static str) -> Result<Point3, FormatError> {
    Ok(Point3 {
        x: r.u16(what)?,
        y: r.u16(what)?,
        z: r.u16(what)?,
    })
}

fn read_point_list(r: &mut Reader<'_>, what: &'static str) -> Result<Vec<Point>, FormatError> {
    let n = usize::from(r.u16(what)?);
    room(r, n, 4, what)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(read_point(r, what)?);
    }
    Ok(v)
}

fn read_polyline(r: &mut Reader<'_>, what: &'static str) -> Result<Polyline, FormatError> {
    let id = r.u8(what)?;
    let points = read_point_list(r, what)?;
    let id2 = r.u8(what)?;
    Ok(Polyline { id, points, id2 })
}

/// Parse the `SPOK` body (after the version word).
pub fn parse_spok(body: &[u8]) -> Result<Spok, FormatError> {
    let mut r = Reader::new(body);
    let s = Spok {
        unknown_0x00: r.u32("SPOK unknown_0x00")?,
        unknown_0x04: r.u32("SPOK unknown_0x04")?,
        unknown_0x08: r.u8("SPOK unknown_0x08")?,
    };
    r.expect_end("SPOK")?;
    Ok(s)
}

/// Parse the `STAT` body (after the version word): the layers with their sectors
/// (`spec-navigation.md` 2.2) followed by the path graph (2.3). The body is consumed exactly.
pub fn parse_stat(body: &[u8]) -> Result<Stat, FormatError> {
    let mut r = Reader::new(body);
    let nlayers = usize::from(r.u16("STAT layer count")?);
    room(&r, nlayers, 2, "STAT layer count")?;
    let mut layers = Vec::with_capacity(nlayers);
    for _ in 0..nlayers {
        let nsectors = usize::from(r.u16("STAT sector count")?);
        room(&r, nsectors, 14, "STAT sector count")?;
        let mut sectors = Vec::with_capacity(nsectors);
        for _ in 0..nsectors {
            let kind = r.u8("STAT sector kind")?;
            let sector_mask = r.u32("STAT sector mask")?;
            let outline = read_polyline(&mut r, "STAT sector outline")?;
            let nseg = usize::from(r.u16("STAT barrier count")?);
            room(&r, nseg, 8, "STAT barrier count")?;
            let mut segments = Vec::with_capacity(nseg);
            for _ in 0..nseg {
                segments.push(Segment {
                    a: read_point(&mut r, "STAT barrier")?,
                    b: read_point(&mut r, "STAT barrier")?,
                });
            }
            let unknown_flag = r.u32("STAT sector unknown_flag")?;
            let nobst = usize::from(r.u16("STAT obstacle count")?);
            room(&r, nobst, 8, "STAT obstacle count")?;
            let mut obstacles = Vec::with_capacity(nobst);
            for _ in 0..nobst {
                let mask = r.u32("STAT obstacle mask")?;
                let polygon = read_polyline(&mut r, "STAT obstacle")?;
                obstacles.push(Obstacle { mask, polygon });
            }
            sectors.push(Sector {
                kind,
                sector_mask,
                outline,
                segments,
                unknown_flag,
                obstacles,
            });
        }
        layers.push(StatLayer { sectors });
    }
    let graph = parse_graph(&mut r, &layers)?;
    r.expect_end("STAT")?;
    Ok(Stat { layers, graph })
}

/// The path graph that follows the sectors inside `STAT` (`spec-navigation.md` 2.3). The
/// hierarchy is encoded independently of the polygon lists and kept as read (NAV-020); the
/// layer and sector counts must agree with the sector lists.
fn parse_graph(r: &mut Reader<'_>, layers: &[StatLayer]) -> Result<Graph, FormatError> {
    let nclasses = usize::from(r.u16("STAT size class count")?);
    room(r, nclasses, 8, "STAT size class count")?;
    let mut size_classes = Vec::with_capacity(nclasses);
    for _ in 0..nclasses {
        size_classes.push((
            r.f32("STAT size class")?.to_bits(),
            r.f32("STAT size class")?.to_bits(),
        ));
    }
    let nlayers = usize::from(r.u16("STAT graph layer count")?);
    if nlayers != layers.len() {
        return Err(FormatError::Invalid {
            offset: r.pos() - 2,
            what: "STAT graph layer count",
            value: format!("{nlayers} (sectors gave {})", layers.len()),
        });
    }
    let mut regions = Vec::with_capacity(nlayers);
    for layer in layers {
        let nsectors = usize::from(r.u16("STAT graph sector count")?);
        if nsectors != layer.sectors.len() {
            return Err(FormatError::Invalid {
                offset: r.pos() - 2,
                what: "STAT graph sector count",
                value: format!("{nsectors} (sectors gave {})", layer.sectors.len()),
            });
        }
        let mut per_sector = Vec::with_capacity(nsectors);
        for _ in 0..nsectors {
            let nregions = usize::from(r.u16("STAT region count")?);
            room(r, nregions, 2, "STAT region count")?;
            let mut per_region = Vec::with_capacity(nregions);
            for _ in 0..nregions {
                let nnodes = usize::from(r.u16("STAT node count")?);
                room(r, nnodes, 20, "STAT node count")?;
                let mut nodes = Vec::with_capacity(nnodes);
                for _ in 0..nnodes {
                    let nbits = usize::from(r.u16("STAT node class count")?);
                    room(r, nbits, 1, "STAT node class count")?;
                    let offset_bits = r.bytes(nbits, "STAT node offset bits")?.to_vec();
                    let x = r.i16("STAT node x")?;
                    let y = r.i16("STAT node y")?;
                    let v1x = r.i16("STAT node v1")?;
                    let v1y = r.i16("STAT node v1")?;
                    let v2x = r.i16("STAT node v2")?;
                    let v2y = r.i16("STAT node v2")?;
                    let node_mask = r.u32("STAT node mask")?;
                    let nedges = usize::from(r.u16("STAT node edge count")?);
                    room(r, nedges, 2, "STAT node edge count")?;
                    let mut entering_edges = Vec::with_capacity(nedges);
                    for _ in 0..nedges {
                        entering_edges.push(r.u16("STAT node edge")?);
                    }
                    nodes.push(GraphNode {
                        offset_bits,
                        x,
                        y,
                        v1x,
                        v1y,
                        v2x,
                        v2y,
                        node_mask,
                        entering_edges,
                    });
                }
                per_region.push(nodes);
            }
            per_sector.push(per_region);
        }
        regions.push(per_sector);
    }
    let nedges = usize::from(r.u16("STAT edge count")?);
    room(r, nedges, 26, "STAT edge count")?;
    let node_ref = |r: &mut Reader<'_>| -> Result<NodeRef, FormatError> {
        Ok(NodeRef {
            layer: r.u16("STAT edge endpoint")?,
            sector: r.u16("STAT edge endpoint")?,
            region: r.u16("STAT edge endpoint")?,
            node: r.u16("STAT edge endpoint")?,
        })
    };
    let mut edges = Vec::with_capacity(nedges);
    for _ in 0..nedges {
        let a = node_ref(r)?;
        let b = node_ref(r)?;
        let cost_bits = r.f32("STAT edge cost")?.to_bits();
        let edge_mask = r.u32("STAT edge mask")?;
        let nrec = usize::from(r.u16("STAT edge record count")?);
        room(r, nrec, 2, "STAT edge record count")?;
        let mut records = Vec::with_capacity(nrec);
        for _ in 0..nrec {
            records.push(r.u16("STAT edge record")?);
        }
        edges.push(GraphEdge {
            a,
            b,
            cost_bits,
            edge_mask,
            records,
        });
    }
    let nrecords = usize::from(r.u16("STAT record count")?);
    room(r, nrecords, 1, "STAT record count")?;
    let mut records = Vec::with_capacity(nrecords);
    for _ in 0..nrecords {
        let kind = r.i8("STAT record kind")?;
        if kind < 0 {
            records.push(GraphRecord {
                kind,
                ..GraphRecord::default()
            });
            continue;
        }
        let mask = r.u8("STAT record mask")?;
        let nb = usize::from(r.u16("STAT record pair count")?);
        room(r, nb, 1, "STAT record pair count")?;
        let bits_b = r.bytes(nb, "STAT record bits_b")?.to_vec();
        let na = usize::from(r.u16("STAT record pair count")?);
        if na != nb {
            return Err(FormatError::Invalid {
                offset: r.pos() - 2,
                what: "STAT record pair count",
                value: format!("{na} (first list had {nb})"),
            });
        }
        let bits_a = r.bytes(na, "STAT record bits_a")?.to_vec();
        records.push(GraphRecord {
            kind,
            mask,
            bits_b,
            bits_a,
        });
    }
    Ok(Graph {
        size_classes,
        regions,
        edges,
        records,
    })
}

fn read_door(r: &mut Reader<'_>) -> Result<Door, FormatError> {
    let door_type = r.u8("door type")?;
    let open = r.u8("door open")?;
    let lock = r.array::<4>("door lock set")?;
    let alt = r.array::<4>("door alternate lock set")?;
    let leaf = read_polyline(r, "door leaf")?;
    let a = read_point(r, "door side A")?;
    let sector_a = r.u16("door sector A")?;
    let layer_a = r.u16("door layer A")?;
    let threshold = read_point(r, "door threshold")?;
    let b = read_point(r, "door side B")?;
    let sector_b = r.u16("door sector B")?;
    let layer_b = r.u16("door layer B")?;
    Ok(Door {
        door_type,
        open,
        lock,
        alt,
        leaf,
        a,
        sector_a,
        layer_a,
        threshold,
        b,
        sector_b,
        layer_b,
    })
}

/// Parse the `FARM` body (after the version word): passage records and buildings
/// (`spec-navigation.md` 2.6).
pub fn parse_farm(body: &[u8]) -> Result<Vec<FarmRecord>, FormatError> {
    let mut r = Reader::new(body);
    let n = usize::from(r.u16("FARM count")?);
    room(&r, n, 3, "FARM count")?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let kind = r.u8("FARM record kind")?;
        let nd = usize::from(r.u16("FARM door count")?);
        room(&r, nd, 30, "FARM door count")?;
        let mut doors = Vec::with_capacity(nd);
        for _ in 0..nd {
            doors.push(read_door(&mut r)?);
        }
        out.push(FarmRecord { kind, doors });
    }
    r.expect_end("FARM")?;
    Ok(out)
}

/// Parse the ` AZ ` body (after the version word): the lifts (`spec-navigation.md` NAV-042).
pub fn parse_az(body: &[u8]) -> Result<Vec<Lift>, FormatError> {
    let mut r = Reader::new(body);
    let n = usize::from(r.u16("AZ count")?);
    room(&r, n, 10, "AZ count")?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let sector = r.u16("AZ sector")?;
        let layer = r.u16("AZ layer")?;
        let lift_type = r.u8("AZ lift type")?;
        let shape = read_polyline(&mut r, "AZ shape")?;
        let nd = usize::from(r.u16("AZ door count")?);
        room(&r, nd, 30, "AZ door count")?;
        let mut doors = Vec::with_capacity(nd);
        for _ in 0..nd {
            doors.push(read_door(&mut r)?);
        }
        let unknown_trailer = r.u16("AZ trailer")?;
        out.push(Lift {
            sector,
            layer,
            lift_type,
            shape,
            doors,
            unknown_trailer,
        });
    }
    r.expect_end("AZ")?;
    Ok(out)
}

/// Parse the `TEXT` body (after the version word).
pub fn parse_text(body: &[u8]) -> Result<Vec<TextZone>, FormatError> {
    let mut r = Reader::new(body);
    let n = usize::from(r.u16("TEXT count")?);
    room(&r, n, 5, "TEXT count")?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let kind = r.u8("TEXT kind")?;
        let polygon = read_polyline(&mut r, "TEXT zone")?;
        v.push(TextZone { kind, polygon });
    }
    r.expect_end("TEXT")?;
    Ok(v)
}

/// Parse the `WOAW` body (after the version word).
pub fn parse_woaw(body: &[u8]) -> Result<Woaw, FormatError> {
    let mut r = Reader::new(body);
    let nl = usize::from(r.u16("WOAW layer count")?);
    room(&r, nl, 2, "WOAW layer count")?;
    let mut layers = Vec::with_capacity(nl);
    for _ in 0..nl {
        layers.push(r.u16("WOAW layer id")?);
    }
    let na = usize::from(r.u16("WOAW area count")?);
    room(&r, na, 2 + 24 + 8, "WOAW area count")?;
    let mut areas = Vec::with_capacity(na);
    for _ in 0..na {
        let npts = usize::from(r.u16("WOAW area point count")?);
        room(&r, npts, 16, "WOAW area point count")?;
        let mut points = Vec::with_capacity(npts);
        for _ in 0..npts {
            points.push(AreaPoint {
                x: r.f32("WOAW area point")?,
                y: r.f32("WOAW area point")?,
                unknown_0x08: r.f32("WOAW area point")?,
                z: r.f32("WOAW area point")?,
            });
        }
        let mut min = [0f32; 3];
        let mut max = [0f32; 3];
        for m in &mut min {
            *m = r.f32("WOAW area min")?;
        }
        for m in &mut max {
            *m = r.f32("WOAW area max")?;
        }
        let nlinks = usize::from(r.u8("WOAW area link count")?);
        room(&r, nlinks, 4, "WOAW area link count")?;
        let mut links = Vec::with_capacity(nlinks);
        for _ in 0..nlinks {
            links.push(AreaLink {
                unknown_ref: r.u16("WOAW area link")?,
                unknown_layer: r.u16("WOAW area link")?,
            });
        }
        let unknown_flags = r.array::<4>("WOAW area flags")?;
        let unknown_a = r.u8("WOAW area unknown_a")?;
        let nl2 = usize::from(r.u16("WOAW area layer count")?);
        room(&r, nl2, 2, "WOAW area layer count")?;
        let mut area_layers = Vec::with_capacity(nl2);
        for _ in 0..nl2 {
            area_layers.push(r.u16("WOAW area layer")?);
        }
        areas.push(Area {
            points,
            min,
            max,
            links,
            unknown_flags,
            unknown_a,
            layers: area_layers,
        });
    }
    r.expect_end("WOAW")?;
    Ok(Woaw { layers, areas })
}

/// Parse the `007 ` body (after the version word).
pub fn parse_bonds(body: &[u8]) -> Result<Vec<Bond>, FormatError> {
    let mut r = Reader::new(body);
    let n = usize::from(r.u16("007 count")?);
    room(&r, n, 14, "007 count")?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(Bond {
            x1: r.i16("007 bond")?,
            y1: r.i16("007 bond")?,
            x2: r.i16("007 bond")?,
            y2: r.i16("007 bond")?,
            area_a: r.u16("007 bond area")?,
            area_b: r.u16("007 bond area")?,
            unknown_0x0c: r.u16("007 bond unknown_0x0c")?,
        });
    }
    r.expect_end("007 ")?;
    Ok(v)
}

/// Decode `height` run-length packed mask rows of `width` pixels into `stride * height` bytes.
///
/// Each row is `u8 packed_len` followed by control bytes: `0x80 | n` repeats the next byte `n` times,
/// `n < 0x80` copies the next `n` bytes.
fn read_mask(r: &mut Reader<'_>, width: u16, height: u16) -> Result<Vec<u8>, FormatError> {
    let stride = usize::from(width).div_ceil(8);
    room(r, usize::from(height), 1, "FACE mask height")?;
    let mut mask = Vec::with_capacity(stride * usize::from(height));
    for _ in 0..height {
        let packed = usize::from(r.u8("FACE row length")?);
        let row_start = r.pos();
        let end = row_start + packed;
        let packed_bytes = r.bytes(packed, "FACE packed row")?;
        let mut p = 0usize;
        let mut produced = 0usize;
        while p < packed_bytes.len() {
            let c = packed_bytes[p];
            p += 1;
            if c & 0x80 != 0 {
                let n = usize::from(c & 0x7f);
                let Some(&v) = packed_bytes.get(p) else {
                    return Err(FormatError::Eof {
                        offset: row_start + p,
                        needed: 1,
                        what: "FACE run value",
                    });
                };
                p += 1;
                mask.extend(std::iter::repeat_n(v, n));
                produced += n;
            } else {
                let n = usize::from(c);
                let Some(lit) = packed_bytes.get(p..p + n) else {
                    return Err(FormatError::Eof {
                        offset: row_start + p,
                        needed: n,
                        what: "FACE literal run",
                    });
                };
                p += n;
                mask.extend_from_slice(lit);
                produced += n;
            }
            if produced > stride {
                break;
            }
        }
        if produced != stride {
            return Err(FormatError::Invalid {
                offset: row_start,
                what: "FACE row width",
                value: format!("{produced} bytes, expected {stride}"),
            });
        }
        debug_assert_eq!(r.pos(), end);
    }
    Ok(mask)
}

/// Parse the `FACE` body (after the version word).
pub fn parse_faces(body: &[u8]) -> Result<Vec<Face>, FormatError> {
    let mut r = Reader::new(body);
    let count = usize::from(r.u16("FACE count")?);
    room(&r, count, 13, "FACE count")?;
    let mut faces = Vec::with_capacity(count);
    for _ in 0..count {
        let unknown_0x00 = r.u16("FACE unknown_0x00")?;
        let kind = r.u8("FACE kind")?;
        let nlines = usize::from((kind & 3).count_ones() as u8);
        let mut lines = Vec::with_capacity(nlines);
        for _ in 0..nlines {
            lines.push(read_polyline(&mut r, "FACE line")?);
        }
        let x = r.u16("FACE x")?;
        let y = r.u16("FACE y")?;
        let width = r.u16("FACE width")?;
        let height = r.u16("FACE height")?;
        let packed = usize::from(r.u16("FACE packed size")?);
        let mask_start = r.pos();
        let mask = read_mask(&mut r, width, height)?;
        if r.pos() - mask_start != packed {
            return Err(FormatError::Invalid {
                offset: mask_start - 2,
                what: "FACE packed size",
                value: format!("{packed}, rows used {}", r.pos() - mask_start),
            });
        }
        let mut refs = Vec::new();
        if kind & FACE_HAS_REFS != 0 {
            let n = usize::from(r.u16("FACE ref count")?);
            room(&r, n, 2, "FACE ref count")?;
            for _ in 0..n {
                refs.push(r.u16("FACE ref")?);
            }
        }
        faces.push(Face {
            unknown_0x00,
            kind,
            lines,
            x,
            y,
            width,
            height,
            mask,
            refs,
        });
    }
    r.expect_end("FACE")?;
    Ok(faces)
}

/// Parse the `FLIM` body (after the version word).
pub fn parse_flims(body: &[u8]) -> Result<Vec<Flim>, FormatError> {
    let mut r = Reader::new(body);
    let n = usize::from(r.u16("FLIM count")?);
    room(&r, n, 16, "FLIM count")?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let sprite = r.pstring16("FLIM sprite")?;
        let name = r.pstring16("FLIM name")?;
        let x = r.u16("FLIM x")?;
        let y = r.u16("FLIM y")?;
        let unknown_0x04 = r.u16("FLIM unknown_0x04")?;
        let unknown_flags = r.array::<3>("FLIM flags")?;
        let line = read_polyline(&mut r, "FLIM line")?;
        v.push(Flim {
            sprite,
            name,
            x,
            y,
            unknown_0x04,
            unknown_flags,
            line,
        });
    }
    r.expect_end("FLIM")?;
    Ok(v)
}

/// Parse the `DARK` body (after the version word).
pub fn parse_dark(body: &[u8]) -> Result<Vec<DarkZone>, FormatError> {
    let mut r = Reader::new(body);
    let n = usize::from(r.u16("DARK count")?);
    room(&r, n, 10, "DARK count")?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let unknown_0x00 = r.u16("DARK unknown_0x00")?;
        let polygon = read_polyline(&mut r, "DARK zone")?;
        let unknown_value = r.u32("DARK value")?;
        v.push(DarkZone {
            unknown_0x00,
            polygon,
            unknown_value,
        });
    }
    r.expect_end("DARK")?;
    Ok(v)
}

/// Parse the `PPPP` body (after the version word).
pub fn parse_pppp(body: &[u8]) -> Result<Pppp, FormatError> {
    let mut r = Reader::new(body);
    let n = usize::from(r.u16("PPPP zone count")?);
    room(&r, n, 9, "PPPP zone count")?;
    let mut zones = Vec::with_capacity(n);
    for _ in 0..n {
        let polygon = read_polyline(&mut r, "PPPP zone")?;
        let unknown_ref = r.u16("PPPP zone unknown_ref")?;
        let unknown_layer = r.u16("PPPP zone unknown_layer")?;
        let unknown_flag = r.u8("PPPP zone flag")?;
        zones.push(Zone {
            polygon,
            unknown_ref,
            unknown_layer,
            unknown_flag,
        });
    }
    let n = usize::from(r.u16("PPPP jump line count")?);
    room(&r, n, 29, "PPPP jump line count")?;
    let mut jump_lines = Vec::with_capacity(n);
    for _ in 0..n {
        let from = [
            read_point3(&mut r, "PPPP jump line")?,
            read_point3(&mut r, "PPPP jump line")?,
        ];
        let unknown_a = r.u16("PPPP jump line unknown_a")?;
        let to = [
            read_point3(&mut r, "PPPP jump line")?,
            read_point3(&mut r, "PPPP jump line")?,
        ];
        let unknown_b = r.u16("PPPP jump line unknown_b")?;
        let unknown_c = r.u8("PPPP jump line unknown_c")?;
        jump_lines.push(JumpLine {
            from,
            unknown_a,
            to,
            unknown_b,
            unknown_c,
        });
    }
    r.expect_end("PPPP")?;
    Ok(Pppp { zones, jump_lines })
}

fn child<'a>(
    c: &'a chunk::Container<'a>,
    tag: [u8; 4],
    version: u32,
) -> Result<&'a [u8], FormatError> {
    let ch = c.child(&tag).ok_or_else(|| FormatError::BadMagic {
        offset: 12,
        expected: tag_string(tag),
        found: "missing chunk".into(),
    })?;
    if ch.version != version {
        return Err(FormatError::Invalid {
            offset: ch.offset + 8,
            what: "RHP chunk version",
            value: format!("{} (expected {version} for {})", ch.version, ch.tag_str()),
        });
    }
    Ok(ch.body)
}

/// Parse a whole `.rhp` file.
pub fn parse(data: &[u8]) -> Result<Rhp, FormatError> {
    let c = chunk::parse_container(data, b"MEUH")?;
    if c.version != VERSION {
        return Err(FormatError::Invalid {
            offset: 8,
            what: "MEUH version",
            value: c.version.to_string(),
        });
    }
    Ok(Rhp {
        version: c.version,
        spok: parse_spok(child(&c, *b"SPOK", 3)?)?,
        stat: parse_stat(child(&c, *b"STAT", 2)?)?,
        text: parse_text(child(&c, *b"TEXT", 2)?)?,
        woaw: parse_woaw(child(&c, *b"WOAW", 3)?)?,
        bonds: parse_bonds(child(&c, *b"007 ", 2)?)?,
        faces: parse_faces(child(&c, *b"FACE", 2)?)?,
        flims: parse_flims(child(&c, *b"FLIM", 2)?)?,
        farm: parse_farm(child(&c, *b"FARM", 4)?)?,
        az: parse_az(child(&c, *b" AZ ", 2)?)?,
        dark: parse_dark(child(&c, *b"DARK", 2)?)?,
        tupo: child(&c, *b"TUPO", 3)?.to_vec(),
        loud: child(&c, *b"LOUD", 2)?.to_vec(),
        pppp: parse_pppp(child(&c, *b"PPPP", 4)?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le16(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    #[test]
    fn bonds_and_zones_round_trip() {
        let mut b = Vec::new();
        le16(&mut b, 1);
        for x in [10i16, 20, -3, 40] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        le16(&mut b, 5);
        le16(&mut b, NO_AREA);
        le16(&mut b, 2);
        let v = parse_bonds(&b).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].x2, v[0].y2, v[0].area_b), (-3, 40, NO_AREA));

        let mut p = Vec::new();
        le16(&mut p, 1); // one zone
        p.push(0x4c);
        le16(&mut p, 3);
        for (x, y) in [(1u16, 2u16), (3, 4), (5, 6)] {
            le16(&mut p, x);
            le16(&mut p, y);
        }
        p.push(0xf2);
        le16(&mut p, 0x30);
        le16(&mut p, 1);
        p.push(0);
        le16(&mut p, 1); // one jump line
        for v in [1u16, 2, 50, 3, 4, 50, 1, 5, 6, 0, 7, 8, 0, 0] {
            le16(&mut p, v);
        }
        p.push(1);
        let z = parse_pppp(&p).unwrap();
        assert_eq!(z.zones[0].polygon.points.len(), 3);
        assert_eq!(
            (z.zones[0].unknown_ref, z.zones[0].unknown_layer),
            (0x30, 1)
        );
        assert_eq!(z.jump_lines[0].from[0].z, 50);
        assert_eq!(z.jump_lines[0].to[1], Point3 { x: 7, y: 8, z: 0 });
        assert_eq!(z.jump_lines[0].unknown_c, 1);
    }

    #[test]
    fn face_mask_rows_decode() {
        // count 1; record: prefix 2, kind 0x14 (no lines, refs), x 3, y 4, w 12 (stride 2), h 2,
        // packed 7, rows [82 ff] and [02 f0 0f], then 1 ref = 9
        let mut b = Vec::new();
        le16(&mut b, 1);
        le16(&mut b, 2);
        b.push(0x14);
        for v in [3u16, 4, 12, 2, 7] {
            le16(&mut b, v);
        }
        b.extend_from_slice(&[2, 0x82, 0xff, 3, 0x02, 0xf0, 0x0f]);
        le16(&mut b, 1);
        le16(&mut b, 9);
        let faces = parse_faces(&b).unwrap();
        let f = &faces[0];
        assert_eq!(
            (f.unknown_0x00, f.x, f.y, f.width, f.height),
            (2, 3, 4, 12, 2)
        );
        assert_eq!(f.mask, vec![0xff, 0xff, 0xf0, 0x0f]);
        assert_eq!(f.refs, vec![9]);
        assert!(f.pixel(0, 0) && f.pixel(11, 0) && f.pixel(3, 1) && !f.pixel(4, 1));
        assert!(!f.pixel(12, 0));
        // a wrong packed size is rejected
        b[13] = 8;
        assert!(parse_faces(&b).is_err());
    }

    #[test]
    fn tupo_count_is_the_leading_word() {
        assert_eq!(tupo_count(&[]), 0);
        assert_eq!(tupo_count(&[7]), 0);
        assert_eq!(tupo_count(&[6, 0]), 6);
        assert_eq!(tupo_count(&[0x0b, 0, 1, 2, 3]), 11);
    }

    #[test]
    fn garbage_does_not_panic() {
        for n in 0..200usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 131 % 251) as u8).collect();
            let _ = parse(&data);
            let _ = parse_stat(&data);
            let _ = parse_farm(&data);
            let _ = parse_az(&data);
            let _ = parse_woaw(&data);
            let _ = parse_faces(&data);
            let _ = parse_flims(&data);
            let _ = parse_pppp(&data);
            let _ = parse_text(&data);
            let _ = parse_dark(&data);
        }
        let mut huge = 0xffff_ffffu32.to_le_bytes().to_vec();
        huge.extend_from_slice(&[7u8; 16]);
        assert!(parse_faces(&huge).is_err());
    }
}
