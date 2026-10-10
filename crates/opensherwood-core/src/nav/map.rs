//! The navigation content of a map: layers, sectors, obstacles, the corner graph, doors,
//! buildings and lifts, with the cell grid and the wall segments the geometric queries use.
//!
//! [`NavContent`] is the plain data the app decodes from the map file (`STAT`, `FARM`, ` AZ `,
//! `007 `, `PPPP` and the mission's tenants chunk); [`NavMap::build`] turns it into the indexed
//! form the search and the click resolution consume. Both are content, not state: nothing here
//! changes while a mission runs, so a [`NavMap`] is shared (`Arc`) and left out of the snapshot
//! exactly like the animation catalog. What does change - the availability words, the door and
//! building state, the request queue - is [`super::state::NavState`].
//!
//! Specification: `docs/original/spec-navigation.md` 2.2, 2.3, 2.6, 2.8, NAV-010 to NAV-043,
//! NAV-050, NAV-100 to NAV-102, NAV-120.

use std::collections::BTreeMap;

use super::grid::NavError;

/// Grid cell size in map pixels (`spec-navigation.md` NAV-050).
pub const CELL_SIZE: i32 = 64;

/// Largest number of layers a map may declare (the retail maps have at most 14; the run-time
/// indices of NAV-010 add two).
pub const MAX_LAYERS: usize = 64;

/// Largest number of sectors, obstacles, nodes, edges or doors accepted from one map. The
/// retail maps stay far below it (York: 109 sectors, 893 nodes, 4240 edges, 182 doors); the
/// bound keeps the derived indices and the work budgets finite.
pub const MAX_OBJECTS: usize = 1 << 18;
/// Aggregate structural and spatial-index limits, checked before building derived data.
pub const MAX_INDEX_ENTRIES: u64 = 1 << 22;
pub const MAX_DOORS: usize = 1024;

fn check_content(c: &NavContent) -> Result<(), NavError> {
    let error = |what| NavError::MapContent { what };
    if c.layers.is_empty()
        || c.layers.len() > MAX_LAYERS
        || c.map_size.0 <= 0
        || c.map_size.1 <= 0
        || c.map_size.0 > 32768
        || c.map_size.1 > 32768
    {
        return Err(error("navigation dimensions exceed the limit"));
    }
    if c.size_classes.is_empty()
        || c.size_classes.len() > 16
        || c.size_classes.iter().any(|&(w, h)| {
            !w.is_finite()
                || !h.is_finite()
                || !(1.0..=256.0).contains(&w)
                || !(1.0..=256.0).contains(&h)
        })
    {
        return Err(error("invalid navigation size class"));
    }
    let cells = (c.map_size.0 as u64).div_ceil(64) * (c.map_size.1 as u64).div_ceil(64);
    let mut work = cells * (c.layers.len() as u64 + 2);
    let mut objects = c.layers.len() + c.edges.len() + c.records.len() + c.bonds.len();
    let mut poly = |points: &[(i32, i32)]| -> Result<(), NavError> {
        objects = objects.saturating_add(1 + points.len());
        if points
            .iter()
            .any(|&(x, y)| !(-32768..=32767).contains(&x) || !(-32768..=32767).contains(&y))
        {
            return Err(error("navigation coordinate outside signed 16-bit range"));
        }
        // This upper bound includes the polygon's cell entries and every wall's entries.
        let (min, max) = bounds(points);
        let width = ((max.0 as i64 - min.0 as i64).max(0) as u64 / 64) + 2;
        let height = ((max.1 as i64 - min.1 as i64).max(0) as u64 / 64) + 2;
        work = work.saturating_add(width * height);
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            work = work.saturating_add(
                (u64::from(a.0.abs_diff(b.0)) / 64 + 2) * (u64::from(a.1.abs_diff(b.1)) / 64 + 2),
            );
        }
        if work > MAX_INDEX_ENTRIES || objects > MAX_OBJECTS {
            return Err(error("navigation index exceeds the work limit"));
        }
        Ok(())
    };
    for s in c.layers.iter().flatten() {
        poly(&s.outline)?;
        for (_, p) in &s.obstacles {
            poly(p)?;
        }
        for &(a, b) in &s.barriers {
            poly(&[a, b])?;
        }
    }
    let doors: Vec<_> = c
        .farm
        .iter()
        .flat_map(|f| &f.doors)
        .chain(c.lifts.iter().flat_map(|l| &l.doors))
        .collect();
    if doors.len() > MAX_DOORS {
        return Err(error("too many navigation doors"));
    }
    for d in doors {
        if d.layer_a as usize >= c.layers.len() || d.layer_b as usize >= c.layers.len() {
            return Err(error("door layer outside map"));
        }
        poly(&d.leaf)?;
        poly(&[d.a, d.b, d.threshold])?;
    }
    for l in &c.lifts {
        poly(&l.shape)?;
    }
    for z in &c.jump_zones {
        poly(&z.polygon)?;
    }
    for b in &c.bonds {
        poly(&[b.a, b.b])?;
    }
    for layer in &c.regions {
        objects = objects.saturating_add(layer.len());
        for sector in layer {
            objects = objects.saturating_add(sector.len());
            for region in sector {
                objects = objects.saturating_add(region.len());
                for n in region {
                    objects = objects.saturating_add(n.entering_edges.len() + n.offset_bits.len());
                    if n.offset_bits.len() != c.size_classes.len() {
                        return Err(error("node size classes differ from graph"));
                    }
                }
            }
        }
    }
    for e in &c.edges {
        if !e.cost.is_finite()
            || e.cost < 0.0
            || e.a.0 != e.b.0
            || e.a.1 != e.b.1
            || e.records.len() != c.size_classes.len()
            || e.records.iter().any(|&r| r as usize >= c.records.len())
        {
            return Err(error("invalid navigation edge"));
        }
        objects = objects.saturating_add(e.records.len());
    }
    for r in &c.records {
        if r.bits_a.len() != r.bits_b.len()
            || r.bits_a
                .iter()
                .chain(&r.bits_b)
                .any(|&b| !matches!(b, 1 | 2 | 4 | 8))
        {
            return Err(error("invalid navigation edge candidates"));
        }
        objects = objects.saturating_add(r.bits_a.len() + r.bits_b.len());
    }
    if objects > MAX_OBJECTS || work > MAX_INDEX_ENTRIES {
        return Err(error("navigation content exceeds the limit"));
    }
    Ok(())
}

/// Door crossing cost added to the distance between the door's two points: 50 px for the
/// passage and lift types, 100 px for the building types (`spec-navigation.md` NAV-041).
pub const DOOR_COST_PASSAGE: f32 = 50.0;
/// Door crossing cost of the building types (NAV-041).
pub const DOOR_COST_BUILDING: f32 = 100.0;
/// How far the threshold of a type-2 door, and of a type-4 door of a climb, is pushed towards
/// the far side (NAV-041).
pub const THRESHOLD_PUSH: f32 = 60.0;
/// The same for a type-6 door (NAV-041).
pub const THRESHOLD_PUSH_CLIMB: f32 = 65.0;
/// A building's capacity and its admission kind (`spec-navigation.md` NAV-043).
pub const BUILDING_CAPACITY: u32 = 65535;

/// A polygon in map pixels.
pub type Poly = Vec<(i32, i32)>;

/// Which side of a door a character is on (`spec-navigation.md` NAV-170).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum DoorSide {
    /// Side A: the door's `sector_a` on `layer_a`.
    #[default]
    A,
    /// Side B: `sector_b` on `layer_b`, or the building / lift itself.
    B,
}

impl DoorSide {
    /// The other side.
    #[must_use]
    pub fn other(self) -> Self {
        match self {
            DoorSide::A => DoorSide::B,
            DoorSide::B => DoorSide::A,
        }
    }

    /// A stable tag for the canonical hash.
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            DoorSide::A => 0,
            DoorSide::B => 1,
        }
    }
}

// --------------------------------------------------------------------------------------------
// Input data (what the app decodes from the map)
// --------------------------------------------------------------------------------------------

/// One sector of a layer as the map file gives it (`spec-navigation.md` 2.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectorSpec {
    /// 0 = plain sector, 1 = lift sector.
    pub kind: u8,
    /// The walkable floor outline.
    pub outline: Poly,
    /// Search barrier segments (NAV-141 steps 1 to 3).
    pub barriers: Vec<((i32, i32), (i32, i32))>,
    /// Consulted when a player character passes a door (NAV-171); 0 on retail data.
    pub unknown_flag: u32,
    /// Holes in the floor: `(availability mask, outline)`.
    pub obstacles: Vec<(u32, Poly)>,
}

/// One corner of the path graph (`spec-navigation.md` 2.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeSpec {
    /// Usable candidate bits, one byte per size class (NAV-021).
    pub offset_bits: Vec<u8>,
    /// The corner.
    pub corner: (i32, i32),
    /// Availability mask (NAV-056).
    pub mask: u32,
    /// Indices of the edges that enter this node (NAV-022).
    pub entering_edges: Vec<u32>,
}

/// One edge of the path graph (`spec-navigation.md` 2.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgeSpec {
    /// Node A as `(layer, sector position, region, node)`.
    pub a: (u32, u32, u32, u32),
    /// Node B likewise.
    pub b: (u32, u32, u32, u32),
    /// The edge's length as the search uses it.
    pub cost: f32,
    /// Availability mask (NAV-056).
    pub mask: u32,
    /// One record index per size class.
    pub records: Vec<u32>,
}

/// A per-size-class record of an edge (`spec-navigation.md` NAV-023).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordSpec {
    /// `false` = impassable for this size class.
    pub passable: bool,
    /// Candidate bit at node B of pair `i`.
    pub bits_b: Vec<u8>,
    /// Candidate bit at node A of pair `i`.
    pub bits_a: Vec<u8>,
}

/// A door of `FARM` or ` AZ ` (`spec-navigation.md` 2.6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DoorSpec {
    /// 0 default, 3 gate, 7 gate variant, 1 / 2 building, 4 / 5 / 6 lift end.
    pub door_type: u8,
    /// Initially open.
    pub open: bool,
    /// The active lock set: player, lock-pick, civilian, soldier.
    pub lock: [u8; 4],
    /// The alternate lock set a patch swaps in (NAV-172).
    pub alt: [u8; 4],
    /// The leaf outline; empty means "no leaf" and therefore no click target.
    pub leaf: Poly,
    /// Side-A point.
    pub a: (i32, i32),
    /// Polygon number of sector A.
    pub sector_a: u32,
    /// Layer of sector A.
    pub layer_a: u32,
    /// The threshold, between the sides.
    pub threshold: (i32, i32),
    /// Side-B point.
    pub b: (i32, i32),
    /// Polygon number of sector B (ignored for building and lift doors).
    pub sector_b: u32,
    /// Layer of sector B (ignored for building and lift doors).
    pub layer_b: u32,
}

/// A `FARM` record (`spec-navigation.md` NAV-043).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FarmSpec {
    /// 0 = passage record, else a building.
    pub kind: u8,
    /// The record's doors.
    pub doors: Vec<DoorSpec>,
}

/// An ` AZ ` record (`spec-navigation.md` NAV-042).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiftSpec {
    /// Polygon number of the lift's sector.
    pub sector: u32,
    /// The lift's layer.
    pub layer: u32,
    /// 1 stairs, 2 ladder, 3 climb (0 is corrected to 1).
    pub lift_type: u8,
    /// The lift's click shape (empty on every retail map).
    pub shape: Poly,
    /// The lift's end doors.
    pub doors: Vec<DoorSpec>,
}

/// A `PPPP` jump zone: a clickable polygon on a layer (`spec-navigation.md` 8.7
/// `NavJumpZones`; the jump itself is unread and does not exist).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JumpZoneSpec {
    /// The zone's layer.
    pub layer: u32,
    /// The outline.
    pub polygon: Poly,
}

/// A `007 ` bond: crossing it swaps a character's projection area (`spec-navigation.md` 2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BondSpec {
    /// First end.
    pub a: (i32, i32),
    /// Second end.
    pub b: (i32, i32),
    /// The `WOAW` area on one side (`0xffff` = none).
    pub area_a: u32,
    /// The area on the other side.
    pub area_b: u32,
    /// The layer the bond lives on.
    pub layer: u32,
}

/// The navigation content of one map plus the mission's building tenants: the plain data the
/// app decodes, before [`NavMap::build`] indexes it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NavContent {
    /// The level rectangle in map pixels.
    pub map_size: (i32, i32),
    /// Sectors per layer, in file order.
    pub layers: Vec<Vec<SectorSpec>>,
    /// `(half width, half height)` per size class.
    pub size_classes: Vec<(f32, f32)>,
    /// `regions[layer][sector][region]` -> the region's nodes.
    pub regions: Vec<Vec<Vec<Vec<NodeSpec>>>>,
    /// The edge table.
    pub edges: Vec<EdgeSpec>,
    /// The record table.
    pub records: Vec<RecordSpec>,
    /// `FARM` records in file order.
    pub farm: Vec<FarmSpec>,
    /// ` AZ ` records in file order.
    pub lifts: Vec<LiftSpec>,
    /// `PPPP` jump zones.
    pub jump_zones: Vec<JumpZoneSpec>,
    /// `007 ` bonds.
    pub bonds: Vec<BondSpec>,
    /// Per building, in building-list order: the element indices of its initial occupants and
    /// the record's trailing byte (`spec-navigation.md` NAV-193).
    pub tenants: Vec<(Vec<u32>, u8)>,
}

// --------------------------------------------------------------------------------------------
// Indexed form
// --------------------------------------------------------------------------------------------

/// What a wall segment belongs to (`spec-navigation.md` NAV-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallOwner {
    /// The outline of the sector at this flat index (always enabled).
    Sector(u32),
    /// Obstacle `1` of the sector at flat index `0`: enabled exactly while the obstacle is
    /// available (NAV-056).
    Obstacle(u32, u32),
}

/// One blocking segment of a layer, with the oriented unit normal that points to its free side
/// (`spec-navigation.md` NAV-012, used by NAV-110 and NAV-111).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wall {
    /// First end.
    pub a: (f32, f32),
    /// Second end.
    pub b: (f32, f32),
    /// Unit normal pointing to the free side.
    pub normal: (f32, f32),
    /// The polygon the segment belongs to.
    pub owner: WallOwner,
}

/// One obstacle of a sector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavObstacle {
    /// The polygon number this obstacle owns (NAV-011).
    pub number: u32,
    /// Availability mask (NAV-056).
    pub mask: u32,
    /// The outline.
    pub polygon: Poly,
}

/// One sector of a layer, indexed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavSector {
    /// 0 = plain, 1 = lift sector.
    pub kind: u8,
    /// The polygon number that names this sector everywhere else (NAV-011).
    pub number: u32,
    /// The layer it lives on.
    pub layer: u32,
    /// Its position within the layer (what a path request converts the number to, NAV-014).
    pub position: u32,
    /// The walkable floor outline.
    pub outline: Poly,
    /// Search barrier segments.
    pub barriers: Vec<((i32, i32), (i32, i32))>,
    /// Consulted when a player character passes a door (NAV-171).
    pub unknown_flag: u32,
    /// Holes in the floor.
    pub obstacles: Vec<NavObstacle>,
}

/// One corner of the path graph, indexed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavNode {
    /// Usable candidate bits, one byte per size class.
    pub offset_bits: Vec<u8>,
    /// The corner.
    pub corner: (i32, i32),
    /// Availability mask.
    pub mask: u32,
    /// Indices of the edges that enter this node.
    pub entering_edges: Vec<u32>,
}

impl NavNode {
    /// The usable candidate bits for a size class (NAV-021); 0 when the class is absent.
    #[must_use]
    pub fn bits(&self, class: usize) -> u8 {
        self.offset_bits.get(class).copied().unwrap_or(0)
    }
}

/// One edge of the path graph, indexed.
#[derive(Debug, Clone, PartialEq)]
pub struct NavEdge {
    /// Node A, as a flat node index.
    pub a: u32,
    /// Node B, as a flat node index.
    pub b: u32,
    /// The edge's length.
    pub cost: f32,
    /// Availability mask.
    pub mask: u32,
    /// One record index per size class.
    pub records: Vec<u32>,
}

/// A per-size-class record of an edge, indexed.
pub type NavRecord = RecordSpec;

/// A door of the flat door list (`FARM` doors in file order, then the lift doors, NAV-042).
#[derive(Debug, Clone, PartialEq)]
pub struct NavDoor {
    /// The door's type byte.
    pub door_type: u8,
    /// Initially open (the live flag is [`super::state::NavState`]).
    pub initially_open: bool,
    /// The lock set as the file gives it.
    pub lock: [u8; 4],
    /// The alternate lock set.
    pub alt: [u8; 4],
    /// The leaf outline (empty = no leaf, therefore no click target).
    pub leaf: Poly,
    /// Side-A point.
    pub a: (i32, i32),
    /// Side-A sector (polygon number) and layer.
    pub sector_a: u32,
    /// Side-A layer.
    pub layer_a: u32,
    /// The stored threshold.
    pub threshold: (i32, i32),
    /// The threshold after the push of NAV-041 (the door's point for native 64).
    pub adjusted_threshold: (f32, f32),
    /// Side-B point.
    pub b: (i32, i32),
    /// Side-B sector (polygon number); for a building or lift door this is the record's own
    /// sector: the building's index space or the lift's sector.
    pub sector_b: u32,
    /// Side-B layer; `layer_count - 1` for a building or lift door (NAV-010).
    pub layer_b: u32,
    /// The crossing cost of NAV-041.
    pub cost: f32,
    /// The building this door belongs to, if any (index into [`NavMap::buildings`]).
    pub building: Option<u32>,
    /// The lift this door belongs to, if any (index into [`NavMap::lifts`]).
    pub lift: Option<u32>,
}

impl NavDoor {
    /// The door's point on the given side.
    #[must_use]
    pub fn point(&self, side: DoorSide) -> (f32, f32) {
        let p = match side {
            DoorSide::A => self.a,
            DoorSide::B => self.b,
        };
        (p.0 as f32, p.1 as f32)
    }

    /// The sector the given side leads into.
    #[must_use]
    pub fn sector(&self, side: DoorSide) -> u32 {
        match side {
            DoorSide::A => self.sector_a,
            DoorSide::B => self.sector_b,
        }
    }

    /// The layer of the given side.
    #[must_use]
    pub fn layer(&self, side: DoorSide) -> u32 {
        match side {
            DoorSide::A => self.layer_a,
            DoorSide::B => self.layer_b,
        }
    }

    /// Whether side B is a building or a lift rather than an ordinary sector (NAV-040).
    #[must_use]
    pub fn far_side_is_object(&self) -> bool {
        self.building.is_some() || self.lift.is_some()
    }
}

/// A building: nothing but its doors and its tenants (`spec-navigation.md` NAV-043, NAV-193).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NavBuilding {
    /// Indices into [`NavMap::doors`], in file order.
    pub doors: Vec<u32>,
    /// The element indices of the initial occupants (NAV-193).
    pub tenants: Vec<u32>,
    /// The tenants record's trailing byte; kept, meaning unread.
    pub tenant_trailer: u8,
}

/// A lift: stairs, a ladder or a climb (`spec-navigation.md` NAV-042, NAV-192).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NavLift {
    /// The lift's sector (polygon number).
    pub sector: u32,
    /// The lift's layer.
    pub layer: u32,
    /// 1 stairs, 2 ladder, 3 climb.
    pub lift_type: u8,
    /// Indices into [`NavMap::doors`].
    pub doors: Vec<u32>,
    /// The door with the largest side-A row: the lift's lower end (NAV-042).
    pub lower_end: Option<u32>,
    /// The door with the smallest side-A row: the upper end.
    pub upper_end: Option<u32>,
}

/// What a click on one layer found (`spec-navigation.md` NAV-100).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickAnswer {
    /// The layer holds nothing at the point.
    Nothing,
    /// An obstacle of the layer covers the point: the resolution stops and yields no target.
    Blocked,
    /// A target was found.
    Hit(ClickTarget),
}

/// The target a click resolved to (`spec-navigation.md` NAV-100, NAV-102).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickTarget {
    /// A walkable sector: its flat index.
    Sector(u32),
    /// A door leaf: the door index (the click routes to the door, NAV-122).
    DoorLeaf(u32),
    /// A lift shape: it becomes the lift's sector on the lift's layer.
    LiftShape(u32),
    /// A jump zone on a sector: the walk goes to the clicked point on that sector
    /// (`NavJumpZones`).
    JumpZone(u32, u32),
}

/// One cell of a layer's grid (`spec-navigation.md` NAV-050).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Cell {
    /// Wall segment indices on this layer.
    walls: Vec<u32>,
    /// Click polygons in registration order (= file order).
    polys: Vec<u32>,
    /// Bond indices on this layer.
    bonds: Vec<u32>,
}

/// One registered click polygon of a layer, in registration order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClickPoly {
    kind: ClickKind,
    polygon: Poly,
    min: (i32, i32),
    max: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClickKind {
    SectorOutline(u32),
    Obstacle(u32, u32),
    DoorLeaf(u32),
    LiftShape(u32),
    JumpZone(u32),
}

/// One layer's grid, walls and click polygons.
#[derive(Debug, Clone, PartialEq)]
struct Layer {
    cells_w: i32,
    cells_h: i32,
    cells: Vec<Cell>,
    walls: Vec<Wall>,
    polys: Vec<ClickPoly>,
}

impl Layer {
    fn cell_index(&self, cx: i32, cy: i32) -> Option<usize> {
        (cx >= 0 && cy >= 0 && cx < self.cells_w && cy < self.cells_h)
            .then(|| (cy * self.cells_w + cx) as usize)
    }
}

/// The indexed navigation content of a map.
#[derive(Debug, Clone, PartialEq)]
pub struct NavMap {
    /// The level rectangle in map pixels.
    pub map_size: (i32, i32),
    /// Number of layers the map file declares (NAV-010: two further run-time indices exist).
    pub layer_count: u32,
    /// Sectors, flat, in file order across the layers.
    pub sectors: Vec<NavSector>,
    /// `layer_sectors[layer][position]` -> flat sector index.
    pub layer_sectors: Vec<Vec<u32>>,
    /// Polygon number -> the sector it names, when it names one (NAV-011).
    number_to_sector: BTreeMap<u32, u32>,
    /// `(half width, half height)` per size class.
    pub size_classes: Vec<(f32, f32)>,
    /// Graph nodes, flat, in file order.
    pub nodes: Vec<NavNode>,
    /// `node_index[layer][sector][region]` -> the flat indices of the region's nodes.
    pub node_index: Vec<Vec<Vec<Vec<u32>>>>,
    /// Graph edges.
    pub edges: Vec<NavEdge>,
    /// Edge records.
    pub records: Vec<NavRecord>,
    /// The flat door list (NAV-042).
    pub doors: Vec<NavDoor>,
    /// Buildings in `FARM` order (the index space of native 8).
    pub buildings: Vec<NavBuilding>,
    /// Lifts in ` AZ ` order.
    pub lifts: Vec<NavLift>,
    /// Bonds (NAV-035).
    pub bonds: Vec<BondSpec>,
    /// Door links (NAV-120): for each door and side, `(other door, other side, length)` for
    /// every door sharing that sector.
    pub links: Vec<[Vec<(u32, DoorSide, f32)>; 2]>,
    /// Per layer: the grid, the walls and the click polygons.
    layers: Vec<Layer>,
}

/// Signed area of a polygon, doubled (`i128` so no polygon can overflow it).
fn double_area(poly: &[(i32, i32)]) -> i128 {
    let n = poly.len();
    let mut a: i128 = 0;
    for i in 0..n {
        let (x1, y1) = (i128::from(poly[i].0), i128::from(poly[i].1));
        let (x2, y2) = (
            i128::from(poly[(i + 1) % n].0),
            i128::from(poly[(i + 1) % n].1),
        );
        a += x1 * y2 - x2 * y1;
    }
    a
}

/// The wall segments of one polygon with the normal that points to its free side (NAV-012):
/// the interior for a floor, the exterior for an obstacle. The polygon's own winding decides
/// which of the two perpendiculars that is, so the rule needs no assumption about the authoring.
fn walls_of(poly: &[(i32, i32)], floor: bool, owner: WallOwner, out: &mut Vec<Wall>) {
    if poly.len() < 2 {
        return;
    }
    let interior_is_left = double_area(poly) > 0;
    let n = poly.len();
    for i in 0..n {
        let a = (poly[i].0 as f32, poly[i].1 as f32);
        let b = (poly[(i + 1) % n].0 as f32, poly[(i + 1) % n].1 as f32);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = super::search::length(dx, dy);
        if len == 0.0 {
            continue;
        }
        // (-dy, dx) is the interior side when the polygon's signed area is positive.
        let mut n0 = (-dy / len, dx / len);
        if !interior_is_left {
            n0 = (-n0.0, -n0.1);
        }
        if !floor {
            n0 = (-n0.0, -n0.1);
        }
        out.push(Wall {
            a,
            b,
            normal: n0,
            owner,
        });
    }
}

fn bounds(poly: &[(i32, i32)]) -> ((i32, i32), (i32, i32)) {
    let mut min = (i32::MAX, i32::MAX);
    let mut max = (i32::MIN, i32::MIN);
    for &(x, y) in poly {
        min = (min.0.min(x), min.1.min(y));
        max = (max.0.max(x), max.1.max(y));
    }
    (min, max)
}

/// The cell a coordinate falls in (NAV-050: `trunc(coordinate) >> 6`, clamped to the grid).
#[must_use]
pub fn cell_of(v: f32) -> i32 {
    let t = if v.is_finite() {
        v.clamp(-(1 << 24) as f32, (1 << 24) as f32).trunc() as i32
    } else {
        0
    };
    t >> 6
}

impl NavMap {
    /// Index a map's navigation content. Fails when the content is inconsistent (a graph that
    /// does not match the sector lists, an index out of range) or larger than [`MAX_OBJECTS`].
    #[allow(clippy::too_many_lines)]
    pub fn build(c: &NavContent) -> Result<Self, NavError> {
        check_content(c)?;
        let too_many = |n: usize| n > MAX_OBJECTS;
        if c.layers.len() > MAX_LAYERS
            || c.regions.len() != c.layers.len()
            || too_many(c.edges.len())
            || too_many(c.records.len())
        {
            return Err(NavError::MapContent {
                what: "too many layers, edges or records",
            });
        }
        if c.map_size.0 <= 0 || c.map_size.1 <= 0 {
            return Err(NavError::MapContent {
                what: "empty level rectangle",
            });
        }

        // --- sectors and polygon numbers (NAV-011) ---
        let mut sectors = Vec::new();
        let mut layer_sectors = Vec::new();
        let mut number_to_sector = BTreeMap::new();
        let mut number = 0u32;
        for (li, layer) in c.layers.iter().enumerate() {
            let mut here = Vec::new();
            for (si, s) in layer.iter().enumerate() {
                let flat = sectors.len() as u32;
                number_to_sector.insert(number, flat);
                let mut obstacles = Vec::with_capacity(s.obstacles.len());
                for (n, (mask, poly)) in (number + 1..).zip(&s.obstacles) {
                    obstacles.push(NavObstacle {
                        number: n,
                        mask: *mask,
                        polygon: poly.clone(),
                    });
                }
                sectors.push(NavSector {
                    kind: s.kind,
                    number,
                    layer: li as u32,
                    position: si as u32,
                    outline: s.outline.clone(),
                    barriers: s.barriers.clone(),
                    unknown_flag: s.unknown_flag,
                    obstacles,
                });
                here.push(flat);
                number += 1 + s.obstacles.len() as u32;
            }
            layer_sectors.push(here);
        }
        if too_many(sectors.len()) {
            return Err(NavError::MapContent {
                what: "too many sectors",
            });
        }

        // --- graph nodes, flattened in file order ---
        let mut nodes: Vec<NavNode> = Vec::new();
        let mut node_index: Vec<Vec<Vec<Vec<u32>>>> = Vec::with_capacity(c.regions.len());
        let mut node_of: BTreeMap<(u32, u32, u32, u32), u32> = BTreeMap::new();
        for (li, layer) in c.regions.iter().enumerate() {
            if layer.len() != c.layers[li].len() {
                return Err(NavError::MapContent {
                    what: "the graph's sector count differs from the sector list",
                });
            }
            let mut per_sector = Vec::with_capacity(layer.len());
            for (si, sector) in layer.iter().enumerate() {
                let mut per_region = Vec::with_capacity(sector.len());
                for (ri, region) in sector.iter().enumerate() {
                    let mut flat = Vec::with_capacity(region.len());
                    for (ni, n) in region.iter().enumerate() {
                        let idx = nodes.len() as u32;
                        if too_many(nodes.len()) {
                            return Err(NavError::MapContent {
                                what: "too many graph nodes",
                            });
                        }
                        node_of.insert((li as u32, si as u32, ri as u32, ni as u32), idx);
                        nodes.push(NavNode {
                            offset_bits: n.offset_bits.clone(),
                            corner: n.corner,
                            mask: n.mask,
                            entering_edges: n.entering_edges.clone(),
                        });
                        flat.push(idx);
                    }
                    per_region.push(flat);
                }
                per_sector.push(per_region);
            }
            node_index.push(per_sector);
        }
        let mut edges = Vec::with_capacity(c.edges.len());
        for e in &c.edges {
            let (Some(&a), Some(&b)) = (node_of.get(&e.a), node_of.get(&e.b)) else {
                return Err(NavError::MapContent {
                    what: "an edge names a node that does not exist",
                });
            };
            edges.push(NavEdge {
                a,
                b,
                cost: e.cost,
                mask: e.mask,
                records: e.records.clone(),
            });
        }
        for (ni, n) in nodes.iter().enumerate() {
            if n.entering_edges
                .iter()
                .any(|&i| edges.get(i as usize).is_none_or(|e| e.b as usize != ni))
            {
                return Err(NavError::MapContent {
                    what: "a node names an edge that does not exist",
                });
            }
        }

        // --- doors: the FARM doors in file order, then the lift doors (NAV-042) ---
        let mut doors: Vec<NavDoor> = Vec::new();
        let mut buildings: Vec<NavBuilding> = Vec::new();
        let layer_count = c.layers.len() as u32;
        // The side-B layer of building and lift doors (NAV-010).
        let object_layer = layer_count.saturating_sub(1);
        for rec in &c.farm {
            let building = (rec.kind != 0).then_some(buildings.len() as u32);
            if building.is_some() {
                buildings.push(NavBuilding::default());
            }
            for d in &rec.doors {
                let idx = doors.len() as u32;
                let mut door = make_door(d, building, None, object_layer);
                if let Some(bi) = building {
                    // Side B of a building door is the building itself.
                    door.sector_b = sector_number_of_building(bi, &sectors);
                    buildings[bi as usize].doors.push(idx);
                }
                doors.push(door);
            }
        }
        let mut lifts: Vec<NavLift> = Vec::new();
        for l in &c.lifts {
            let li = lifts.len() as u32;
            let mut lift = NavLift {
                sector: l.sector,
                layer: l.layer,
                lift_type: if l.lift_type == 0 { 1 } else { l.lift_type },
                ..NavLift::default()
            };
            for d in &l.doors {
                let idx = doors.len() as u32;
                let mut door = make_door(d, None, Some(li), object_layer);
                door.sector_b = l.sector;
                door.layer_b = l.layer;
                if door.door_type == 4 && lift.lift_type == 3 {
                    door.adjusted_threshold = push_threshold(d, THRESHOLD_PUSH);
                }
                lift.doors.push(idx);
                doors.push(door);
            }
            lift.lower_end = lift
                .doors
                .iter()
                .copied()
                .max_by_key(|&i| doors[i as usize].a.1);
            lift.upper_end = lift
                .doors
                .iter()
                .copied()
                .min_by_key(|&i| doors[i as usize].a.1);
            lifts.push(lift);
        }
        if too_many(doors.len()) {
            return Err(NavError::MapContent {
                what: "too many doors",
            });
        }
        for (bi, b) in buildings.iter_mut().enumerate() {
            if let Some((tenants, trailer)) = c.tenants.get(bi) {
                b.tenants.clone_from(tenants);
                b.tenant_trailer = *trailer;
            }
        }

        // --- door links (NAV-120) ---
        let mut links: Vec<[Vec<(u32, DoorSide, f32)>; 2]> =
            vec![[Vec::new(), Vec::new()]; doors.len()];
        for i in 0..doors.len() {
            for j in (i + 1)..doors.len() {
                for si in [DoorSide::A, DoorSide::B] {
                    for sj in [DoorSide::A, DoorSide::B] {
                        let (a, b) = (&doors[i], &doors[j]);
                        if a.far_side_is_object() && si == DoorSide::B {
                            continue;
                        }
                        if b.far_side_is_object() && sj == DoorSide::B {
                            continue;
                        }
                        if a.sector(si) != b.sector(sj) || a.layer(si) != b.layer(sj) {
                            continue;
                        }
                        let (pa, pb) = (a.point(si), b.point(sj));
                        let len = super::search::length(pa.0 - pb.0, pa.1 - pb.1);
                        links[i][si as usize].push((j as u32, sj, len));
                        links[j][sj as usize].push((i as u32, si, len));
                    }
                }
            }
        }

        // --- per layer: walls, click polygons and the cell grid ---
        let grid_layers = (layer_count + 2) as usize;
        let cells_w = (c.map_size.0 - 1).max(0) / CELL_SIZE + 1;
        let cells_h = (c.map_size.1 - 1).max(0) / CELL_SIZE + 1;
        let mut layers: Vec<Layer> = (0..grid_layers)
            .map(|_| Layer {
                cells_w,
                cells_h,
                cells: vec![Cell::default(); (cells_w * cells_h) as usize],
                walls: Vec::new(),
                polys: Vec::new(),
            })
            .collect();
        for (flat, s) in sectors.iter().enumerate() {
            let l = &mut layers[s.layer as usize];
            walls_of(
                &s.outline,
                true,
                WallOwner::Sector(flat as u32),
                &mut l.walls,
            );
            for (oi, o) in s.obstacles.iter().enumerate() {
                walls_of(
                    &o.polygon,
                    false,
                    WallOwner::Obstacle(flat as u32, oi as u32),
                    &mut l.walls,
                );
            }
        }
        // Click polygons, in registration order: the STAT polygons first (chunk order), then
        // the door leaves of `FARM` and ` AZ `, then the `PPPP` jump zones (NAV-100).
        for (flat, s) in sectors.iter().enumerate() {
            let layer = s.layer as usize;
            register_poly(
                &mut layers[layer],
                ClickKind::SectorOutline(flat as u32),
                &s.outline,
            );
            for (oi, o) in s.obstacles.iter().enumerate() {
                register_poly(
                    &mut layers[layer],
                    ClickKind::Obstacle(flat as u32, oi as u32),
                    &o.polygon,
                );
            }
        }
        for (di, d) in doors.iter().enumerate() {
            if d.leaf.len() < 3 {
                continue;
            }
            // The leaf is registered on the higher of the two layers, unless that is the
            // object layer (NAV-040).
            let layer = d.layer_a.max(d.layer_b);
            if layer == object_layer && d.far_side_is_object() {
                register_poly(
                    &mut layers[d.layer_a as usize],
                    ClickKind::DoorLeaf(di as u32),
                    &d.leaf,
                );
            } else if (layer as usize) < layers.len() {
                register_poly(
                    &mut layers[layer as usize],
                    ClickKind::DoorLeaf(di as u32),
                    &d.leaf,
                );
            }
        }
        for (li, l) in c.lifts.iter().enumerate() {
            if let Some(layer) = layers.get_mut(l.layer as usize) {
                register_poly(layer, ClickKind::LiftShape(li as u32), &l.shape);
            }
        }
        for (zi, z) in c.jump_zones.iter().enumerate() {
            if let Some(l) = layers.get_mut(z.layer as usize) {
                register_poly(l, ClickKind::JumpZone(zi as u32), &z.polygon);
            }
        }
        for (bi, b) in c.bonds.iter().enumerate() {
            if let Some(l) = layers.get_mut(b.layer as usize) {
                let (min, max) = bounds(&[b.a, b.b]);
                for_each_cell(l, min, max, |cell| cell.bonds.push(bi as u32));
            }
        }
        // Fill the cells.
        for l in &mut layers {
            for (wi, w) in l.walls.iter().enumerate() {
                let min = (cell_of(w.a.0.min(w.b.0)), cell_of(w.a.1.min(w.b.1)));
                let max = (cell_of(w.a.0.max(w.b.0)), cell_of(w.a.1.max(w.b.1)));
                let idx = wi as u32;
                for cy in min.1.max(0)..=max.1.min(l.cells_h - 1) {
                    for cx in min.0.max(0)..=max.0.min(l.cells_w - 1) {
                        if let Some(i) = l.cell_index(cx, cy) {
                            l.cells[i].walls.push(idx);
                        }
                    }
                }
            }
            for (pi, p) in l.polys.iter().enumerate() {
                let (min, max) = (p.min, p.max);
                let idx = pi as u32;
                let (c0, c1) = (
                    (cell_of(min.0 as f32), cell_of(min.1 as f32)),
                    (cell_of(max.0 as f32), cell_of(max.1 as f32)),
                );
                for cy in c0.1.max(0)..=c1.1.min(l.cells_h - 1) {
                    for cx in c0.0.max(0)..=c1.0.min(l.cells_w - 1) {
                        if let Some(i) = l.cell_index(cx, cy) {
                            l.cells[i].polys.push(idx);
                        }
                    }
                }
            }
        }

        Ok(NavMap {
            map_size: c.map_size,
            layer_count,
            sectors,
            layer_sectors,
            number_to_sector,
            size_classes: c
                .size_classes
                .iter()
                .map(|&(w, h)| {
                    (
                        if w.is_finite() { w } else { 6.0 },
                        if h.is_finite() { h } else { 4.0 },
                    )
                })
                .collect(),
            nodes,
            node_index,
            edges,
            records: c.records.clone(),
            doors,
            buildings,
            lifts,
            bonds: c.bonds.clone(),
            links,
            layers,
        })
    }

    /// The flat sector index a polygon number names (NAV-011); `None` for an obstacle number or
    /// an unknown one, which NAV-014 / 8.4 reject.
    #[must_use]
    pub fn sector_of_number(&self, number: u32) -> Option<u32> {
        self.number_to_sector.get(&number).copied()
    }

    /// The sector at `(layer, position)`.
    #[must_use]
    pub fn sector_at(&self, layer: u32, position: u32) -> Option<u32> {
        self.layer_sectors
            .get(layer as usize)?
            .get(position as usize)
            .copied()
    }

    /// The `(half width, half height)` of a size class; `(6, 4)` is class 0 on retail maps.
    #[must_use]
    pub fn half_size(&self, class: usize) -> (f32, f32) {
        self.size_classes.get(class).copied().unwrap_or((6.0, 4.0))
    }

    /// The walls of a layer.
    #[must_use]
    pub fn walls(&self, layer: u32) -> &[Wall] {
        self.layers
            .get(layer as usize)
            .map_or(&[][..], |l| &l.walls[..])
    }

    /// The bonds of a layer that the cells of a box hold, in cell order without repetition.
    #[must_use]
    pub fn bonds_in(&self, layer: u32, min: (f32, f32), max: (f32, f32)) -> Vec<u32> {
        let Some(l) = self.layers.get(layer as usize) else {
            return Vec::new();
        };
        let mut seen = vec![false; self.bonds.len()];
        let mut out = Vec::new();
        Self::for_cells(l, min, max, |cell| {
            for &b in &cell.bonds {
                if !seen[b as usize] {
                    seen[b as usize] = true;
                    out.push(b);
                }
            }
        });
        out
    }

    /// The wall indices of a layer that the cells covering a box hold, in cell order and
    /// without repetition (a wall registered in several cells is applied once: NAV-111's
    /// "in cell order" over a grid that visits each segment once per query).
    #[must_use]
    pub fn walls_in(
        &self,
        layer: u32,
        min: (f32, f32),
        max: (f32, f32),
        budget: &mut u64,
    ) -> Option<Vec<u32>> {
        let Some(l) = self.layers.get(layer as usize) else {
            return Some(Vec::new());
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        let c0 = (cell_of(min.0).max(0), cell_of(min.1).max(0));
        let c1 = (
            cell_of(max.0).min(l.cells_w - 1),
            cell_of(max.1).min(l.cells_h - 1),
        );
        for cy in c0.1..=c1.1 {
            for cx in c0.0..=c1.0 {
                if !crate::vm::charge_budget(budget, 1) {
                    return None;
                }
                let cell = &l.cells[l.cell_index(cx, cy)?];
                for &w in &cell.walls {
                    // Includes bounded ordered-set lookup; never allocate a whole-layer bitmap.
                    if !crate::vm::charge_budget(budget, 24) {
                        return None;
                    }
                    if seen.insert(w) {
                        out.push(w);
                    }
                }
            }
        }
        Some(out)
    }

    fn for_cells(l: &Layer, min: (f32, f32), max: (f32, f32), mut f: impl FnMut(&Cell)) {
        let c0 = (cell_of(min.0).max(0), cell_of(min.1).max(0));
        let c1 = (
            cell_of(max.0).min(l.cells_w - 1),
            cell_of(max.1).min(l.cells_h - 1),
        );
        for cy in c0.1..=c1.1 {
            for cx in c0.0..=c1.0 {
                if let Some(i) = l.cell_index(cx, cy) {
                    f(&l.cells[i]);
                }
            }
        }
    }

    /// Resolve a point on one layer (`spec-navigation.md` NAV-100): the cell that contains it,
    /// its polygons in registration order, three passes. `enabled` answers whether an obstacle
    /// is available and `leaf_enabled` whether a door leaf's click target is on (native 191);
    /// `reference` is the selected character's position, which pass 3 measures against.
    #[must_use]
    pub fn resolve_on_layer(
        &self,
        layer: u32,
        point: (f32, f32),
        enabled: &dyn Fn(u32, u32) -> bool,
        leaf_enabled: &dyn Fn(u32) -> bool,
        reference: (f32, f32),
    ) -> ClickAnswer {
        let Some(l) = self.layers.get(layer as usize) else {
            return ClickAnswer::Nothing;
        };
        let (cx, cy) = (cell_of(point.0), cell_of(point.1));
        let Some(ci) = l.cell_index(cx, cy) else {
            return ClickAnswer::Nothing;
        };
        let list = &l.cells[ci].polys;
        let px = point.0.floor() as i32;
        let py = point.1.floor() as i32;
        let hits = |p: &ClickPoly| -> bool {
            px >= p.min.0
                && px <= p.max.0
                && py >= p.min.1
                && py <= p.max.1
                && crate::geom::point_in_polygon(px, py, &p.polygon)
        };
        // Pass 1: patch targets, lift shapes and door leaves share one traversal.
        for &pi in list {
            let p = &l.polys[pi as usize];
            match p.kind {
                ClickKind::DoorLeaf(d) if leaf_enabled(d) && hits(p) => {
                    return ClickAnswer::Hit(ClickTarget::DoorLeaf(d));
                }
                ClickKind::LiftShape(k) if hits(p) => {
                    return ClickAnswer::Hit(ClickTarget::LiftShape(k));
                }
                _ => {}
            }
        }
        // Pass 2: the wall polygons. The last sector outline containing the point wins; any
        // obstacle containing it blocks the layer.
        let mut candidate = None;
        for &pi in list {
            let p = &l.polys[pi as usize];
            match p.kind {
                ClickKind::SectorOutline(s) if hits(p) => candidate = Some(s),
                ClickKind::Obstacle(s, o) if enabled(s, o) && hits(p) => {
                    return ClickAnswer::Blocked;
                }
                _ => {}
            }
        }
        let Some(sector) = candidate else {
            return ClickAnswer::Nothing;
        };
        // Pass 3: the jump zones containing the point; the one whose first edge's midpoint is
        // nearest the reference point replaces the candidate.
        let mut best: Option<(f32, u32)> = None;
        for &pi in list {
            let p = &l.polys[pi as usize];
            if let ClickKind::JumpZone(z) = p.kind
                && hits(p)
                && p.polygon.len() >= 2
            {
                let m = (
                    (p.polygon[0].0 + p.polygon[1].0) as f32 / 2.0,
                    (p.polygon[0].1 + p.polygon[1].1) as f32 / 2.0,
                );
                let d = super::search::length(m.0 - reference.0, m.1 - reference.1);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, z));
                }
            }
        }
        if let Some((_, z)) = best {
            return ClickAnswer::Hit(ClickTarget::JumpZone(z, sector));
        }
        ClickAnswer::Hit(ClickTarget::Sector(sector))
    }

    /// Ordinary click resolution (`spec-navigation.md` NAV-100): the layers from the top down,
    /// stopping at the first that does not answer "nothing". The alternate resolution of
    /// NAV-101 is never selected (`Assumption::NavAltResolution`).
    #[must_use]
    pub fn resolve(
        &self,
        point: (f32, f32),
        enabled: &dyn Fn(u32, u32) -> bool,
        leaf_enabled: &dyn Fn(u32) -> bool,
        reference: (f32, f32),
    ) -> (Option<u32>, ClickAnswer) {
        for layer in (0..self.layer_count).rev() {
            let a = self.resolve_on_layer(layer, point, enabled, leaf_enabled, reference);
            if a != ClickAnswer::Nothing {
                return (Some(layer), a);
            }
        }
        (None, ClickAnswer::Nothing)
    }
}

// `Eq` by hand rather than by derive: both types carry `f32` distances, which `derive(Eq)`
// refuses, and both are reached by the world's own derives. Every `f32` they hold is finite -
// [`NavMap::build`] replaces a non-finite cost or size class with a finite value and skips
// zero-length walls - so equality is reflexive and the marker is honest.
impl Eq for NavContent {}
impl Eq for NavMap {}

fn register_poly(l: &mut Layer, kind: ClickKind, poly: &[(i32, i32)]) {
    if poly.len() < 3 {
        return;
    }
    let (min, max) = bounds(poly);
    l.polys.push(ClickPoly {
        kind,
        polygon: poly.to_vec(),
        min,
        max,
    });
}

fn for_each_cell(l: &mut Layer, min: (i32, i32), max: (i32, i32), mut f: impl FnMut(&mut Cell)) {
    let c0 = (cell_of(min.0 as f32).max(0), cell_of(min.1 as f32).max(0));
    let c1 = (
        cell_of(max.0 as f32).min(l.cells_w - 1),
        cell_of(max.1 as f32).min(l.cells_h - 1),
    );
    for cy in c0.1..=c1.1 {
        for cx in c0.0..=c1.0 {
            if let Some(i) = l.cell_index(cx, cy) {
                f(&mut l.cells[i]);
            }
        }
    }
}

/// The threshold pushed `d` px further along the direction from the threshold to the far side
/// (NAV-041).
fn push_threshold(d: &DoorSpec, by: f32) -> (f32, f32) {
    let t = (d.threshold.0 as f32, d.threshold.1 as f32);
    let far = (d.b.0 as f32, d.b.1 as f32);
    let (dx, dy) = (far.0 - t.0, far.1 - t.1);
    let len = super::search::length(dx, dy);
    if len == 0.0 {
        return t;
    }
    (t.0 + dx / len * by, t.1 + dy / len * by)
}

fn make_door(d: &DoorSpec, building: Option<u32>, lift: Option<u32>, object_layer: u32) -> NavDoor {
    let adjusted_threshold = match d.door_type {
        2 => push_threshold(d, THRESHOLD_PUSH),
        6 => push_threshold(d, THRESHOLD_PUSH_CLIMB),
        _ => (d.threshold.0 as f32, d.threshold.1 as f32),
    };
    let dist = super::search::length((d.b.0 - d.a.0) as f32, (d.b.1 - d.a.1) as f32);
    let cost = dist
        + if matches!(d.door_type, 1 | 2) {
            DOOR_COST_BUILDING
        } else {
            DOOR_COST_PASSAGE
        };
    let object = building.is_some() || lift.is_some();
    NavDoor {
        door_type: d.door_type,
        initially_open: d.open,
        lock: d.lock,
        alt: d.alt,
        leaf: d.leaf.clone(),
        a: d.a,
        sector_a: d.sector_a,
        layer_a: d.layer_a,
        threshold: d.threshold,
        adjusted_threshold,
        b: d.b,
        sector_b: d.sector_b,
        layer_b: if object { object_layer } else { d.layer_b },
        cost,
        building,
        lift,
    }
}

/// Side B of a building door is the building itself, which occupies no polygon number: the
/// engine names it by a number above every polygon so that "the actor's sector is a building"
/// (native 98) is decidable from the sector alone.
fn sector_number_of_building(building: u32, sectors: &[NavSector]) -> u32 {
    let top = sectors
        .last()
        .map_or(0, |s| s.number + s.obstacles.len() as u32 + 1);
    // The level's default sector follows the last polygon number (NAV-011); the buildings
    // follow it.
    top + 1 + building
}

/// The first sector number a building occupies, for [`sector_number_of_building`]'s inverse.
#[must_use]
pub fn building_of_sector_number(
    number: u32,
    sectors: &[NavSector],
    buildings: usize,
) -> Option<u32> {
    let top = sectors
        .last()
        .map_or(0, |s| s.number + s.obstacles.len() as u32 + 1);
    let first = top + 1;
    (number >= first && (number - first) < buildings as u32).then(|| number - first)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use crate::nav::fixtures::obstacle_course as fixture_f1;

    #[test]
    fn polygon_numbers_and_wall_normals_follow_the_specification() {
        let m = NavMap::build(&fixture_f1()).unwrap();
        assert_eq!(m.sectors.len(), 1);
        assert_eq!(m.sectors[0].number, 0);
        assert_eq!(m.sectors[0].obstacles[0].number, 1);
        assert_eq!(m.sector_of_number(0), Some(0));
        assert_eq!(m.sector_of_number(1), None);
        // The outline's normals point inwards, the obstacle's outwards (NAV-012).
        let walls = m.walls(0);
        assert_eq!(walls.len(), 8);
        let top = walls
            .iter()
            .find(|w| w.a == (0.0, 0.0) && w.b == (400.0, 0.0))
            .unwrap();
        assert_eq!(top.normal, (0.0, 1.0));
        let left = walls
            .iter()
            .find(|w| w.a == (180.0, 80.0) && w.b == (220.0, 80.0))
            .unwrap();
        assert_eq!(left.normal, (0.0, -1.0));
    }

    #[test]
    fn a_click_resolves_to_the_sector_and_an_obstacle_blocks() {
        let m = NavMap::build(&fixture_f1()).unwrap();
        let on = |_: u32, _: u32| true;
        let off = |_: u32, _: u32| false;
        let leaf = |_: u32| true;
        assert_eq!(
            m.resolve_on_layer(0, (100.0, 100.0), &on, &leaf, (0.0, 0.0)),
            ClickAnswer::Hit(ClickTarget::Sector(0))
        );
        assert_eq!(
            m.resolve_on_layer(0, (200.0, 100.0), &on, &leaf, (0.0, 0.0)),
            ClickAnswer::Blocked
        );
        // An unavailable obstacle does not block (NAV-056).
        assert_eq!(
            m.resolve_on_layer(0, (200.0, 100.0), &off, &leaf, (0.0, 0.0)),
            ClickAnswer::Hit(ClickTarget::Sector(0))
        );
        assert_eq!(
            m.resolve_on_layer(0, (900.0, 100.0), &on, &leaf, (0.0, 0.0)),
            ClickAnswer::Nothing
        );
    }
}
