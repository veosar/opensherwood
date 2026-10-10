//! The geometric tests, the corner-graph search and the door route of
//! `docs/original/spec-navigation.md`: NAV-110 (clearance), NAV-111 (unsticking), NAV-112 (the
//! corridor), NAV-141 to NAV-145 (the search, the quadrant rule, the corner walk, the
//! waypoints, the smoothing) and NAV-121 to NAV-123 (the route across sectors).
//!
//! Everything here is a pure function of the map, the availability words and the request: it
//! holds no state, so a search can be run at any point of the tick and repeated after a patch
//! (`spec-navigation.md` 8.1).
//!
//! **Precision:** stored scores and improvements use `f32`; the cross-product below uses a
//! wider intermediate. This working policy is pending reconciliation with section 8.5 and
//! review 48, not an approved claim of original numeric equivalence.

use super::grid::NavError;
use super::map::{DoorSide, NavMap, WallOwner};
use super::state::Availability;

/// Search window margin around the bounding box of start and goal (NAV-141 step 2).
pub const SEARCH_WINDOW: f32 = 400.0;
/// Rounds of unsticking before it fails (NAV-111).
pub const UNSTICK_ROUNDS: u32 = 50;
/// Where a corner ends after a separation, and the depth below which a corner is moved
/// (NAV-111).
pub const CORNER_TARGET: f32 = 1.0;
/// Corners shallower than this are not moved (NAV-111).
pub const CORNER_EPSILON: f32 = 0.1;
/// How far the smoothing pulls the two ends of a leg in before testing it (NAV-141 step 6).
pub const CORRIDOR_SHRINK: f32 = 0.000_05;
/// Longest route the door search accepts (NAV-121).
pub const MAX_ROUTE_DOORS: usize = 102;

/// Whether an object with this mask is available in a sector with this state word
/// (`spec-navigation.md` NAV-056).
#[must_use]
pub fn available(mask: u32, state: u32) -> bool {
    mask & state == mask
}

/// An axis-aligned box in map pixels, `(min x, min y, max x, max y)`.
pub type Box2 = (f32, f32, f32, f32);

fn charge(budget: &mut u64, n: u64) -> bool {
    if *budget < n {
        *budget = 0;
        return false;
    }
    *budget -= n;
    true
}

/// Whether a segment meets an axis-aligned box (touching counts).
fn segment_meets_box(a: (f32, f32), b: (f32, f32), bx: Box2) -> bool {
    // Trivial rejection on the segment's own bounding box.
    if a.0.min(b.0) > bx.2 || a.0.max(b.0) < bx.0 || a.1.min(b.1) > bx.3 || a.1.max(b.1) < bx.1 {
        return false;
    }
    // Either end inside.
    let inside = |p: (f32, f32)| p.0 >= bx.0 && p.0 <= bx.2 && p.1 >= bx.1 && p.1 <= bx.3;
    if inside(a) || inside(b) {
        return true;
    }
    // Or a crossing of one of the four sides.
    let corners = [(bx.0, bx.1), (bx.2, bx.1), (bx.2, bx.3), (bx.0, bx.3)];
    (0..4).any(|i| segments_cross(a, b, corners[i], corners[(i + 1) % 4]))
}

/// Working cross-product policy: round differences to f32, evaluate products in f64, round
/// the result to f32. Equivalence to the original remains a numeric-policy review gate.
fn cross(o: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (ax, ay) = (f64::from(a.0 - o.0), f64::from(a.1 - o.1));
    let (bx, by) = (f64::from(b.0 - o.0), f64::from(b.1 - o.1));
    (ax * by - ay * bx) as f32
}

fn on_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> bool {
    p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0) && p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1)
}

/// Whether two closed segments intersect (touching counts).
#[must_use]
pub fn segments_cross(p1: (f32, f32), p2: (f32, f32), q1: (f32, f32), q2: (f32, f32)) -> bool {
    let d1 = cross(q1, q2, p1);
    let d2 = cross(q1, q2, p2);
    let d3 = cross(p1, p2, q1);
    let d4 = cross(p1, p2, q2);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && on_segment(p1, q1, q2))
        || (d2 == 0.0 && on_segment(p2, q1, q2))
        || (d3 == 0.0 && on_segment(q1, p1, p2))
        || (d4 == 0.0 && on_segment(q2, p1, p2))
}

/// Whether a wall is enabled: sector outlines always, an obstacle exactly while it is
/// available (`spec-navigation.md` NAV-012, NAV-056).
#[must_use]
pub fn wall_enabled(_map: &NavMap, avail: &Availability, owner: WallOwner) -> bool {
    match owner {
        WallOwner::Sector(_) => true,
        // The enabled obstacles of a sector are its available sequence (NAV-056, NAV-058).
        WallOwner::Obstacle(s, o) => avail.sector_obstacles(s).contains(&o),
    }
}

/// **Clearance of a box** on a layer (`spec-navigation.md` NAV-110): it holds iff the box
/// overlaps the level rectangle and no enabled wall segment of the layer meets it. Clearance
/// says nothing about sector membership.
#[must_use]
pub fn clearance(
    map: &NavMap,
    avail: &Availability,
    layer: u32,
    bx: Box2,
    budget: &mut u64,
) -> bool {
    let (w, h) = (map.map_size.0 as f32, map.map_size.1 as f32);
    if bx.2 < 0.0 || bx.0 > w || bx.3 < 0.0 || bx.1 > h {
        return false;
    }
    let walls = map.walls(layer);
    let Some(candidates) = map.walls_in(layer, (bx.0, bx.1), (bx.2, bx.3), budget) else {
        return false;
    };
    for i in candidates {
        if !charge(budget, 1) {
            return false;
        }
        let wl = &walls[i as usize];
        if wall_enabled(map, avail, wl.owner) && segment_meets_box(wl.a, wl.b, bx) {
            return false;
        }
    }
    true
}

/// The separation of one wall for one box (`spec-navigation.md` NAV-111, revision 6): the
/// distance the box is translated along the wall's oriented normal, so that the wall's
/// **reference corner** ends exactly 1 px on the free side and no corner ends below 0.1 px.
/// The reference corner is the last of the chain that starts at the first corner of the fixed
/// order lying below 0.1 px on the free side and continues to the next corner more than 0.9 px
/// deeper; with no such corner the separation is zero, as it is when the box's centre is not on
/// the free side. The running form below is that chain: `T` starts at 0 and, at each corner in
/// the fixed order whose distance *after the translation so far* is still below 0.1 px, becomes
/// `1 - d`, which is exactly "more than 0.9 px deeper than the current reference".
#[must_use]
pub fn separation(normal: (f32, f32), p1: (f32, f32), bx: Box2) -> f32 {
    let c = (f32::midpoint(bx.0, bx.2), f32::midpoint(bx.1, bx.3));
    if normal.0 * (c.0 - p1.0) + normal.1 * (c.1 - p1.1) <= 0.0 {
        return 0.0;
    }
    // The fixed corner order of NAV-111.
    let corners = [(bx.0, bx.1), (bx.2, bx.3), (bx.2, bx.1), (bx.0, bx.3)];
    let mut t = 0.0f32;
    for c in corners {
        let d = normal.0 * (c.0 - p1.0) + normal.1 * (c.1 - p1.1);
        if d + t < CORNER_EPSILON {
            t = CORNER_TARGET - d;
        }
    }
    t
}

/// **Unsticking a box** (`spec-navigation.md` NAV-111). Returns the box and whether it came out
/// clear; a box that is not clear after [`UNSTICK_ROUNDS`] rounds has failed.
#[must_use]
pub fn unstick(
    map: &NavMap,
    avail: &Availability,
    layer: u32,
    mut bx: Box2,
    budget: &mut u64,
) -> (Box2, bool) {
    let (mw, mh) = (map.map_size.0 as f32, map.map_size.1 as f32);
    // A box that does not overlap the map is first translated onto it.
    if bx.2 < 0.0 {
        let d = -bx.0;
        bx = (bx.0 + d, bx.1, bx.2 + d, bx.3);
    } else if bx.0 > mw {
        let d = mw - bx.2;
        bx = (bx.0 + d, bx.1, bx.2 + d, bx.3);
    }
    if bx.3 < 0.0 {
        let d = -bx.1;
        bx = (bx.0, bx.1 + d, bx.2, bx.3 + d);
    } else if bx.1 > mh {
        let d = mh - bx.3;
        bx = (bx.0, bx.1 + d, bx.2, bx.3 + d);
    }
    let walls = map.walls(layer);
    for _ in 0..UNSTICK_ROUNDS {
        if clearance(map, avail, layer, bx, budget) {
            return (bx, true);
        }
        let Some(candidates) = map.walls_in(layer, (bx.0, bx.1), (bx.2, bx.3), budget) else {
            return (bx, false);
        };
        for i in candidates {
            if !charge(budget, 1) {
                return (bx, false);
            }
            let wl = &walls[i as usize];
            if !wall_enabled(map, avail, wl.owner) || !segment_meets_box(wl.a, wl.b, bx) {
                continue;
            }
            let t = separation(wl.normal, wl.a, bx);
            if t != 0.0 {
                bx = (
                    bx.0 + wl.normal.0 * t,
                    bx.1 + wl.normal.1 * t,
                    bx.2 + wl.normal.0 * t,
                    bx.3 + wl.normal.1 * t,
                );
            }
        }
    }
    let clear = clearance(map, avail, layer, bx, budget);
    (bx, clear)
}

/// The four sides of the corridor from `p` to `q` for half-size `(w, h)`
/// (`spec-navigation.md` NAV-112): the rectangle spanned by the two boxes of half-size
/// `(w - 1, h - 1)`, its sides taken from the boxes' corners by the signs of `q - p`. A purely
/// horizontal or vertical leg gives an axis-aligned box.
#[must_use]
pub fn corridor_quad(p: (f32, f32), q: (f32, f32), half: (f32, f32)) -> [(f32, f32); 4] {
    let (w, h) = (half.0 - 1.0, half.1 - 1.0);
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    if dx == 0.0 || dy == 0.0 {
        let (x0, x1) = (p.0.min(q.0) - w, p.0.max(q.0) + w);
        let (y0, y1) = (p.1.min(q.1) - h, p.1.max(q.1) + h);
        return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    }
    let sx = if dx > 0.0 { 1.0 } else { -1.0 };
    let sy = if dy > 0.0 { 1.0 } else { -1.0 };
    let c1 = (sx * w, -sy * h);
    let c2 = (-sx * w, sy * h);
    [
        (p.0 + c1.0, p.1 + c1.1),
        (q.0 + c1.0, q.1 + c1.1),
        (q.0 + c2.0, q.1 + c2.1),
        (p.0 + c2.0, p.1 + c2.1),
    ]
}

fn point_in_quad(p: (f32, f32), quad: &[(f32, f32); 4]) -> bool {
    let mut pos = false;
    let mut neg = false;
    for i in 0..4 {
        let c = cross(quad[i], quad[(i + 1) % 4], p);
        if c > 0.0 {
            pos = true;
        }
        if c < 0.0 {
            neg = true;
        }
    }
    !(pos && neg)
}

/// **Corridor test** from `p` to `q` (`spec-navigation.md` NAV-112): free iff no enabled wall
/// segment of the layer crosses a side of the corridor or has an end inside it. Costs one work
/// unit per evaluation (8.1).
#[must_use]
pub fn corridor_free(
    map: &NavMap,
    avail: &Availability,
    layer: u32,
    p: (f32, f32),
    q: (f32, f32),
    half: (f32, f32),
    budget: &mut u64,
) -> bool {
    if !charge(budget, 1) {
        return false;
    }
    let quad = corridor_quad(p, q, half);
    let min = (
        quad.iter().fold(f32::MAX, |m, c| m.min(c.0)),
        quad.iter().fold(f32::MAX, |m, c| m.min(c.1)),
    );
    let max = (
        quad.iter().fold(f32::MIN, |m, c| m.max(c.0)),
        quad.iter().fold(f32::MIN, |m, c| m.max(c.1)),
    );
    let walls = map.walls(layer);
    let Some(candidates) = map.walls_in(layer, min, max, budget) else {
        return false;
    };
    for i in candidates {
        if !charge(budget, 1) {
            return false;
        }
        let wl = &walls[i as usize];
        if !wall_enabled(map, avail, wl.owner) {
            continue;
        }
        if point_in_quad(wl.a, &quad) || point_in_quad(wl.b, &quad) {
            return false;
        }
        for k in 0..4 {
            if segments_cross(wl.a, wl.b, quad[k], quad[(k + 1) % 4]) {
                return false;
            }
        }
    }
    true
}

/// The offset of a candidate bit (`spec-navigation.md` NAV-021): bit 1 `(-w, -h)`, bit 2
/// `(+w, -h)`, bit 4 `(+w, +h)`, bit 8 `(-w, +h)`.
#[must_use]
pub fn candidate_offset(bit: u8, half: (f32, f32)) -> Option<(f32, f32)> {
    let (sx, sy) = match bit {
        1 => (-1.0, -1.0),
        2 => (1.0, -1.0),
        4 => (1.0, 1.0),
        8 => (-1.0, 1.0),
        _ => return None,
    };
    Some((sx * half.0, sy * half.1))
}

/// The position of a candidate of a corner.
#[must_use]
pub fn candidate_point(corner: (f32, f32), bit: u8, half: (f32, f32)) -> Option<(f32, f32)> {
    candidate_offset(bit, half).map(|(ox, oy)| (corner.0 + ox, corner.1 + oy))
}

/// **Quadrant rule** (`spec-navigation.md` NAV-143): the candidate passes iff `Q` lies on the
/// candidate's outer side (or exactly on its line) in one axis and strictly on its inner side
/// in the other.
#[must_use]
pub fn quadrant_passes(bit: u8, corner: (f32, f32), half: (f32, f32), q: (f32, f32)) -> bool {
    let Some((ox, oy)) = candidate_offset(bit, half) else {
        return false;
    };
    let (sx, sy) = (ox.signum(), oy.signum());
    let v = (q.0 - (corner.0 + ox), q.1 - (corner.1 + oy));
    (sx * v.0 >= 0.0 && sy * v.1 < 0.0) || (sy * v.1 >= 0.0 && sx * v.0 < 0.0)
}

/// The next candidate bit clockwise on the screen (1 -> 2 -> 4 -> 8 -> 1).
fn next_cw(bit: u8) -> u8 {
    match bit {
        1 => 2,
        2 => 4,
        4 => 8,
        _ => 1,
    }
}

/// The next candidate bit counter-clockwise.
fn next_ccw(bit: u8) -> u8 {
    match bit {
        1 => 8,
        8 => 4,
        4 => 2,
        _ => 1,
    }
}

/// One run from `g` to the first member of `s` reached, stepping through members of `u`.
/// Returns the run listed **from that member of `s` to `g`** (the travel order) and the number
/// of candidates passed strictly between. A run that meets a bit outside `u` is void.
fn run(u: u8, g: u8, s: u8, clockwise: bool) -> Option<(Vec<u8>, usize)> {
    let mut path = vec![g];
    let mut cur = g;
    for _ in 0..4 {
        cur = if clockwise {
            next_cw(cur)
        } else {
            next_ccw(cur)
        };
        if u & cur == 0 {
            return None;
        }
        path.push(cur);
        if s & cur != 0 {
            let passed = path.len() - 2;
            path.reverse();
            return Some((path, passed));
        }
    }
    None
}

/// **Corner walk** (`spec-navigation.md` NAV-144): the waypoints at a graph node in travel
/// order, given the usable candidates `u`, the candidates `g` from which the leg towards the
/// goal departs and the candidates `s` at which the leg from the start side arrives. The first
/// element is the member of `s` the walk reached, which fixes `g` at the previous node.
#[must_use]
pub fn corner_walk(u: u8, g: u8, s: u8) -> Vec<u8> {
    let both = g & s;
    if both.is_power_of_two() {
        return vec![both];
    }
    // Candidate runs, each with its own g, listed travel order and the candidates passed.
    let mut best: Option<(usize, u8, bool, Vec<u8>)> = None;
    let single = g.is_power_of_two();
    for bit in [1u8, 2, 4, 8] {
        if g & bit == 0 {
            continue;
        }
        for clockwise in [false, true] {
            let Some((list, passed)) = run(u, bit, s, clockwise) else {
                continue;
            };
            // For one `g` the shorter run wins; over several `g` the run passing the fewest
            // candidates wins, ties to the smaller `g`. Both tie-breaks take the run whose
            // listed order follows 1 -> 2 -> 4 -> 8, that is the run stepped counter-clockwise.
            let key = if single { list.len() - 1 } else { passed };
            let better = match &best {
                None => true,
                Some((bk, bg, bcw, _)) => {
                    key < *bk || (key == *bk && (bit < *bg || (bit == *bg && *bcw && !clockwise)))
                }
            };
            if better {
                best = Some((key, bit, clockwise, list));
            }
        }
    }
    match best {
        Some((_, _, _, list)) => list,
        None => Vec::new(),
    }
}

// --------------------------------------------------------------------------------------------
// The search inside one sector
// --------------------------------------------------------------------------------------------

/// A path request as the search takes it (`spec-navigation.md` NAV-140, NAV-141).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchInput {
    /// The layer the search works on.
    pub layer: u32,
    /// The flat sector index.
    pub sector: u32,
    /// Where the walk starts.
    pub start: (f32, f32),
    /// Where it must arrive.
    pub goal: (f32, f32),
    /// Size class (0 on retail maps).
    pub class: usize,
    /// The request's unstuck flag (NAV-140 step 3): 1 enables the direct leg of NAV-141 step 1
    /// and keeps `start` in the delivered waypoints (NAV-145).
    pub unstuck: bool,
}

/// What a search produced.
#[derive(Debug, Clone, PartialEq)]
pub enum PathOutcome {
    /// The waypoints, `start` first and `goal` last (NAV-145).
    Path(Vec<(f32, f32)>),
    /// No path (an inadmissible goal, nothing left to examine, an abandoned request).
    NoPath,
    /// The work budget ran out: an empty result delivered as failed-pending (8.1).
    Exhausted,
}

/// Inputs of the smoothing pass, kept separate so a caller can smooth an unsmoothed path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothInput {
    /// The layer.
    pub layer: u32,
    /// Half size of the size class.
    pub half: (f32, f32),
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    node: u32,
    f: f32,
    seq: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct NodeState {
    opened: bool,
    g: f32,
    h: f32,
    /// The edge the current `g` arrived by, and the predecessor node.
    from: Option<(u32, u32)>,
    /// The candidates that passed the opening test (first node only).
    open_bits: u8,
}

/// Deterministic single-precision Euclidean length; no platform libm hypot.
pub(crate) fn length(dx: f32, dy: f32) -> f32 {
    (dx * dx + dy * dy).sqrt()
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    length(a.0 - b.0, a.1 - b.1)
}

/// Whether a segment crosses one of the sector's barrier segments (NAV-141 steps 1 and 2).
fn barrier_between(map: &NavMap, sector: u32, a: (f32, f32), b: (f32, f32)) -> bool {
    let Some(s) = map.sectors.get(sector as usize) else {
        return false;
    };
    s.barriers
        .iter()
        .any(|&(p, q)| segments_cross(a, b, (p.0 as f32, p.1 as f32), (q.0 as f32, q.1 as f32)))
}

/// **Search** inside one sector (`spec-navigation.md` NAV-141). `states` holds the availability
/// word of every sector; `budget` is charged one unit per corridor test, per node examination
/// and per reaching (8.1).
// Queue ties compare the stored f32 scores exactly, as required by NAV-141.
#[allow(clippy::too_many_lines, clippy::float_cmp)]
#[must_use]
pub fn search_sector(
    map: &NavMap,
    avail: &Availability,
    input: SearchInput,
    budget: &mut u64,
) -> PathOutcome {
    let half = map.half_size(input.class);
    if map.sectors.get(input.sector as usize).is_none() {
        return PathOutcome::NoPath;
    }
    let state = avail.state(input.sector);

    // Step 0: the goal check. `NavGoalCheckTail`: the goal is inadmissible iff any enabled wall
    // segment meets the goal box of half-size (w - 1, h - 1).
    let gb = (
        input.goal.0 - (half.0 - 1.0),
        input.goal.1 - (half.1 - 1.0),
        input.goal.0 + (half.0 - 1.0),
        input.goal.1 + (half.1 - 1.0),
    );
    {
        let walls = map.walls(input.layer);
        let Some(candidates) = map.walls_in(input.layer, (gb.0, gb.1), (gb.2, gb.3), budget) else {
            return PathOutcome::Exhausted;
        };
        for i in candidates {
            if !charge(budget, 1) {
                return PathOutcome::Exhausted;
            }
            let wl = &walls[i as usize];
            if wall_enabled(map, avail, wl.owner) && segment_meets_box(wl.a, wl.b, gb) {
                return PathOutcome::NoPath;
            }
        }
    }

    // Step 1: the direct leg, only for a request that was unstuck.
    if input.unstuck
        && !barrier_between(map, input.sector, input.start, input.goal)
        && corridor_free(
            map,
            avail,
            input.layer,
            input.start,
            input.goal,
            half,
            budget,
        )
    {
        return PathOutcome::Path(vec![input.start, input.goal]);
    }
    if *budget == 0 {
        return PathOutcome::Exhausted;
    }

    // Step 2: the opening.
    let window = (
        input.start.0.min(input.goal.0) - SEARCH_WINDOW,
        input.start.1.min(input.goal.1) - SEARCH_WINDOW,
        input.start.0.max(input.goal.0) + SEARCH_WINDOW,
        input.start.1.max(input.goal.1) + SEARCH_WINDOW,
    );
    let mut nodes: std::collections::BTreeMap<u32, NodeState> = std::collections::BTreeMap::new();
    let mut queue: Vec<Entry> = Vec::new();
    let mut seq = 0u64;
    let mut first_opened: Option<u32> = None;
    for region in 0..avail.region_count(input.sector) as u32 {
        // NAV-058: the search sees the region's **available sequence**, in its order.
        for &ni in avail.region_nodes(input.sector, region) {
            let node = &map.nodes[ni as usize];
            let corner = (node.corner.0 as f32, node.corner.1 as f32);
            if corner.0 < window.0
                || corner.0 > window.2
                || corner.1 < window.1
                || corner.1 > window.3
            {
                continue;
            }
            // The node's walker box must not contain the start.
            if (input.start.0 - corner.0).abs() <= half.0
                && (input.start.1 - corner.1).abs() <= half.1
            {
                continue;
            }
            if barrier_between(map, input.sector, input.start, corner) {
                continue;
            }
            let bits = node.bits(input.class);
            let mut passing = 0u8;
            for bit in [1u8, 2, 4, 8] {
                if bits & bit == 0 {
                    continue;
                }
                if !quadrant_passes(bit, corner, half, input.start) {
                    continue;
                }
                let Some(p) = candidate_point(corner, bit, half) else {
                    continue;
                };
                if corridor_free(map, avail, input.layer, input.start, p, half, budget) {
                    passing |= bit;
                }
                if *budget == 0 {
                    return PathOutcome::Exhausted;
                }
            }
            if passing == 0 {
                continue;
            }
            let g = dist(input.start, corner);
            let h = dist(corner, input.goal);
            nodes.insert(
                ni,
                NodeState {
                    opened: true,
                    g,
                    h,
                    from: None,
                    open_bits: passing,
                },
            );
            seq += 1;
            queue.push(Entry {
                node: ni,
                f: g + h,
                seq,
            });
            if first_opened.is_none() {
                first_opened = Some(ni);
            }
        }
    }

    // Step 3 and 4: examination and reaching.
    let mut answer: Option<(u32, u8)> = None;
    while !queue.is_empty() {
        if !charge(budget, queue.len() as u64) {
            return PathOutcome::Exhausted;
        }
        // Least f, most recently opened first among equal f.
        let mut best = 0usize;
        for i in 1..queue.len() {
            let (a, b) = (&queue[i], &queue[best]);
            if a.f < b.f || (a.f == b.f && a.seq > b.seq) {
                best = i;
            }
        }
        let entry = queue.swap_remove(best);
        if !charge(budget, 1) {
            return PathOutcome::Exhausted;
        }
        let node = &map.nodes[entry.node as usize];
        let bits = node.bits(input.class);
        let corner = (node.corner.0 as f32, node.corner.1 as f32);
        let dead = bits == 5 || bits == 10;
        if !dead && !barrier_between(map, input.sector, corner, input.goal) {
            let mut passing = 0u8;
            for bit in [1u8, 2, 4, 8] {
                if bits & bit == 0 || !quadrant_passes(bit, corner, half, input.goal) {
                    continue;
                }
                let Some(p) = candidate_point(corner, bit, half) else {
                    continue;
                };
                if corridor_free(map, avail, input.layer, p, input.goal, half, budget) {
                    passing |= bit;
                }
                if *budget == 0 {
                    return PathOutcome::Exhausted;
                }
            }
            if passing != 0 {
                answer = Some((entry.node, passing));
                break;
            }
        }
        if dead {
            continue;
        }
        let g_here = nodes.get(&entry.node).map_or(entry.f, |n| n.g);
        // NAV-058: the edges entering the node in its available sequence's order.
        for &ei in avail.node_edges(entry.node) {
            if !charge(budget, 1) {
                return PathOutcome::Exhausted;
            }
            let Some(edge) = map.edges.get(ei as usize) else {
                continue;
            };
            let Some(&ri) = edge.records.get(input.class) else {
                continue;
            };
            let Some(record) = map.records.get(ri as usize) else {
                continue;
            };
            if !record.passable {
                continue;
            }
            let far = edge.a;
            let Some(fnode) = map.nodes.get(far as usize) else {
                continue;
            };
            if !available(fnode.mask, state) {
                continue;
            }
            let tentative = g_here + edge.cost;
            let fcorner = (fnode.corner.0 as f32, fnode.corner.1 as f32);
            let slot = nodes.entry(far).or_insert(NodeState {
                opened: false,
                g: f32::INFINITY,
                h: dist(fcorner, input.goal),
                from: None,
                open_bits: 0,
            });
            if slot.opened && tentative >= slot.g {
                continue;
            }
            if !slot.opened {
                slot.opened = true;
                slot.h = dist(fcorner, input.goal);
            } else if tentative >= slot.g {
                continue;
            }
            slot.g = tentative;
            slot.from = Some((ei, entry.node));
            let f = slot.g + slot.h;
            seq += 1;
            queue.push(Entry { node: far, f, seq });
        }
    }
    let Some((answer, goal_bits)) = answer else {
        return PathOutcome::NoPath;
    };

    // Step 5: the waypoints. The reconstruction runs from the answer back to the first opened
    // node; the candidate reached at each node fixes `G` at its predecessor (NAV-144).
    let mut legs: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut cur = answer;
    let mut g_bits = goal_bits;
    loop {
        if !charge(budget, 1) {
            return PathOutcome::Exhausted;
        }
        let node = &map.nodes[cur as usize];
        let u = node.bits(input.class);
        let corner = (node.corner.0 as f32, node.corner.1 as f32);
        let st = nodes.get(&cur).copied().unwrap_or_default();
        let (s_bits, prev) = match st.from {
            None => (st.open_bits, None),
            Some((ei, prev)) => {
                let record = map
                    .edges
                    .get(ei as usize)
                    .and_then(|e| e.records.get(input.class))
                    .and_then(|&ri| map.records.get(ri as usize));
                // The edge enters `prev` and leads to `cur`: pair i joins `bits_b[i]` of the
                // edge's B (the previous node) with `bits_a[i]` of its A (this node).
                let bits = record.map_or(0u8, |r| r.bits_a.iter().fold(0, |m, &b| m | b));
                (bits, Some((ei, prev)))
            }
        };
        let walk = corner_walk(u, g_bits, s_bits);
        if walk.is_empty() {
            return PathOutcome::NoPath;
        }
        let mut leg: Vec<(f32, f32)> = Vec::with_capacity(walk.len());
        for &bit in &walk {
            if let Some(p) = candidate_point(corner, bit, half) {
                leg.push(p);
            }
        }
        legs.push(leg);
        let reached = walk.first().copied().unwrap_or(0);
        match prev {
            None => break,
            Some((ei, prev)) => {
                let record = map
                    .edges
                    .get(ei as usize)
                    .and_then(|e| e.records.get(input.class))
                    .and_then(|&ri| map.records.get(ri as usize));
                g_bits = record.map_or(0u8, |r| {
                    r.bits_a
                        .iter()
                        .zip(r.bits_b.iter())
                        .filter(|(a, _)| **a == reached)
                        .fold(0u8, |m, (_, b)| m | *b)
                });
                cur = prev;
            }
        }
    }
    legs.reverse();
    let mut path = vec![input.start];
    for leg in legs {
        // Each leg is listed from the arriving candidate to the departing one, which is the
        // travel order.
        path.extend(leg);
    }
    path.push(input.goal);

    // Step 6: the smoothing, only when the path has more than three points.
    let path = smooth_path(
        map,
        avail,
        SmoothInput {
            layer: input.layer,
            half,
        },
        &path,
        budget,
    );
    if *budget == 0 {
        return PathOutcome::Exhausted;
    }
    PathOutcome::Path(path)
}

/// **Smoothing** (`spec-navigation.md` NAV-141 step 6): only when the path has more than three
/// points; `start` and `goal` are kept and each interior point is dropped when the corridor
/// from the last kept point to the point after it, both pulled in by
/// [`CORRIDOR_SHRINK`] of the leg, is free.
#[must_use]
pub fn smooth_path(
    map: &NavMap,
    avail: &Availability,
    input: SmoothInput,
    path: &[(f32, f32)],
    budget: &mut u64,
) -> Vec<(f32, f32)> {
    if path.len() <= 3 {
        return path.to_vec();
    }
    let mut out = vec![path[0]];
    let mut k = path[0];
    for i in 1..path.len() - 1 {
        let q = path[i];
        let r = path[i + 1];
        let d = (r.0 - k.0, r.1 - k.1);
        let a = (k.0 + CORRIDOR_SHRINK * d.0, k.1 + CORRIDOR_SHRINK * d.1);
        let b = (r.0 - CORRIDOR_SHRINK * d.0, r.1 - CORRIDOR_SHRINK * d.1);
        if corridor_free(map, avail, input.layer, a, b, input.half, budget) {
            continue;
        }
        out.push(q);
        k = q;
    }
    out.push(path[path.len() - 1]);
    out
}

// --------------------------------------------------------------------------------------------
// The route across sectors
// --------------------------------------------------------------------------------------------

/// One crossing of a route: the door and the side the character enters it from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crossing {
    /// Index into [`NavMap::doors`].
    pub door: u32,
    /// The side the character arrives at (the door is crossed into the other side).
    pub from: DoorSide,
}

#[derive(Debug, Clone, Copy)]
struct DoorEntry {
    door: u32,
    from: DoorSide,
    g: f32,
    h: f32,
    f: f32,
    seq: u64,
    prev: Option<usize>,
    link_from: Option<u32>,
}

/// **Route search** across sectors (`spec-navigation.md` NAV-121 to NAV-123): the chain of door
/// crossings from the character's sector to the target sector, or `None`. `admits` answers the
/// planning form of NAV-170 for a door entered from a side.
///
/// The initial set is queued in door-list order and ties are examined newest-queued first, so
/// among equal scores the last-listed door is examined first (NAV-121, revision 6).
#[must_use]
#[allow(clippy::too_many_arguments, clippy::float_cmp)] // NAV-121 compares stored scores exactly.
pub fn route(
    map: &NavMap,
    start: (f32, f32),
    start_sector: u32,
    start_layer: u32,
    target_sector: u32,
    target: (f32, f32),
    admits: &dyn Fn(u32, DoorSide) -> bool,
    budget: &mut u64,
) -> Option<Vec<Crossing>> {
    let mut entries: Vec<DoorEntry> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let mut best_g: std::collections::BTreeMap<(u32, u8), (f32, usize)> =
        std::collections::BTreeMap::new();
    let mut seq = 0u64;
    let sector_number = map
        .sectors
        .get(start_sector as usize)
        .map_or(u32::MAX, |s| s.number);
    // The doors of the start sector the character may pass from it.
    for (di, d) in map.doors.iter().enumerate() {
        if !charge(budget, open.len() as u64) {
            return None;
        }
        for side in [DoorSide::A, DoorSide::B] {
            if d.far_side_is_object() && side == DoorSide::B {
                continue;
            }
            if d.sector(side) != sector_number || d.layer(side) != start_layer {
                continue;
            }
            if !admits(di as u32, side) {
                continue;
            }
            let p = d.point(side);
            let g = dist(start, p);
            let h = dist(d.point(side.other()), target);
            seq += 1;
            let idx = entries.len();
            entries.push(DoorEntry {
                door: di as u32,
                from: side,
                g,
                h,
                // NAV-121 with 8.5 (iv): the three-term score is summed `door_cost + h`,
                // then `+ g`, in that order.
                f: (d.cost + h) + g,
                seq,
                prev: None,
                link_from: None,
            });
            best_g.insert((di as u32, side.tag()), (g, idx));
            open.push(idx);
        }
    }
    while !open.is_empty() {
        if !charge(budget, 1) {
            return None;
        }
        let mut best = 0usize;
        for i in 1..open.len() {
            let (a, b) = (&entries[open[i]], &entries[open[best]]);
            if a.f < b.f || (a.f == b.f && a.seq > b.seq) {
                best = i;
            }
        }
        let cur = open.swap_remove(best);
        let (door, from) = (entries[cur].door, entries[cur].from);
        let d = &map.doors[door as usize];
        let into = from.other();
        if d.sector(into) == target_sector {
            // Unwind the chain; a chain longer than the limit is a failure.
            let mut chain = Vec::new();
            let mut at = Some(cur);
            while let Some(i) = at {
                chain.push(Crossing {
                    door: entries[i].door,
                    from: entries[i].from,
                });
                at = entries[i].prev;
                if chain.len() > MAX_ROUTE_DOORS {
                    return None;
                }
            }
            chain.reverse();
            return Some(chain);
        }
        // Follow the links of this door on the entered side.
        let links = &map.links[door as usize][into as usize];
        for &(other, other_side, len) in links {
            if !charge(budget, 1) {
                return None;
            }
            if Some(other) == entries[cur].link_from {
                continue;
            }
            // NAV-123: the link is excluded when the two doors' recorded sides on the same
            // letter are equal - side A when the crossing enters side B, side B otherwise.
            let o = &map.doors[other as usize];
            let excluded = match into {
                DoorSide::B => d.sector_a == o.sector_a && d.layer_a == o.layer_a,
                DoorSide::A => d.sector_b == o.sector_b && d.layer_b == o.layer_b,
            };
            if excluded {
                continue;
            }
            if !admits(other, other_side) {
                continue;
            }
            let g = entries[cur].g + d.cost + len;
            let key = (other, other_side.tag());
            if let Some(&(old, _)) = best_g.get(&key)
                && g >= old
            {
                continue;
            }
            let h = if let Some(&(_, idx)) = best_g.get(&key) {
                entries[idx].h
            } else {
                dist(o.point(other_side.other()), target)
            };
            seq += 1;
            let idx = entries.len();
            entries.push(DoorEntry {
                door: other,
                from: other_side,
                g,
                h,
                f: (o.cost + h) + g,
                seq,
                prev: Some(cur),
                link_from: Some(door),
            });
            best_g.insert(key, (g, idx));
            open.push(idx);
        }
    }
    None
}

/// The error a caller reports when a request names a polygon that is not a sector (NAV-014 and
/// 8.4: OpenSherwood rejects such a request instead of searching the layer's first sector).
#[must_use]
pub fn not_a_sector() -> NavError {
    NavError::MapContent {
        what: "the request names a polygon that is not a sector",
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::type_complexity)] // Exact synthetic acceptance values and door rows.
mod tests {
    use super::super::map::NavMap;
    use super::*;

    fn f1() -> NavMap {
        NavMap::build(&crate::nav::fixtures::obstacle_course()).unwrap()
    }

    /// `spec-navigation.md` 7.2 test 4 and test 16: the quadrant rule, all four bits.
    #[test]
    fn quadrant_rule_matches_the_specification() {
        let corner = (100.0, 100.0);
        let half = (6.0, 4.0);
        let p = |bit, q| quadrant_passes(bit, corner, half, q);
        assert!(p(1, (94.0, 120.0)) && p(1, (80.0, 120.0)));
        assert!(p(1, (120.0, 96.0)) && p(1, (120.0, 80.0)));
        for q in [(80.0, 80.0), (120.0, 120.0), (50.0, 96.0), (94.0, 80.0)] {
            assert!(!p(1, q), "bit 1 at {q:?}");
        }
        assert!(p(2, (106.0, 120.0)) && p(2, (120.0, 120.0)));
        assert!(p(2, (80.0, 96.0)) && p(2, (80.0, 80.0)));
        for q in [(120.0, 80.0), (80.0, 120.0), (140.0, 96.0), (106.0, 80.0)] {
            assert!(!p(2, q), "bit 2 at {q:?}");
        }
        assert!(p(4, (106.0, 80.0)) && p(4, (120.0, 80.0)));
        assert!(p(4, (80.0, 104.0)) && p(4, (80.0, 120.0)));
        for q in [(120.0, 120.0), (80.0, 80.0), (140.0, 104.0), (106.0, 120.0)] {
            assert!(!p(4, q), "bit 4 at {q:?}");
        }
        assert!(p(8, (94.0, 80.0)) && p(8, (80.0, 80.0)));
        assert!(p(8, (120.0, 104.0)) && p(8, (120.0, 120.0)));
        for q in [(80.0, 120.0), (120.0, 80.0), (50.0, 104.0), (94.0, 120.0)] {
            assert!(!p(8, q), "bit 8 at {q:?}");
        }
    }

    /// `spec-navigation.md` 7.2 tests 5 and 17: the corner walk.
    #[test]
    fn corner_walks_match_the_specification() {
        assert_eq!(corner_walk(15, 1, 4), vec![4, 8, 1]);
        assert_eq!(corner_walk(15, 1, 2), vec![2, 1]);
        assert_eq!(corner_walk(15, 3, 4), vec![4, 2]);
        assert_eq!(corner_walk(15, 1, 1), vec![1]);
        assert_eq!(corner_walk(7, 1, 4), vec![4, 2, 1]);
        // Test 17: overlapping sets.
        assert_eq!(corner_walk(15, 3, 6), vec![2]);
        assert_eq!(corner_walk(15, 3, 3), vec![2, 1]);
        assert_eq!(corner_walk(15, 12, 1), vec![1, 8]);
    }

    /// `spec-navigation.md` 7.2 test 2: clearance is not polygon membership.
    #[test]
    fn clearance_follows_the_specification() {
        let m = f1();
        let states = super::super::state::Availability::new(&m);
        let mut b = 1_000_000u64;
        let bx = |cx: f32, cy: f32| (cx - 6.0, cy - 4.0, cx + 6.0, cy + 4.0);
        assert!(!clearance(&m, &states, 0, bx(180.0, 100.0), &mut b));
        assert!(clearance(&m, &states, 0, bx(200.0, 100.0), &mut b));
        assert!(clearance(&m, &states, 0, bx(100.0, 100.0), &mut b));
        assert!(!clearance(
            &m,
            &states,
            0,
            (-40.0, -40.0, -28.0, -32.0),
            &mut b
        ));
        // A layer with no sector at all: every box that overlaps the map is clear, including
        // one straddling the map's left edge (the F1 layer has the sector's own left wall
        // there, so the specification's "when no segment meets it" is tested here).
        assert!(clearance(&m, &states, 1, bx(100.0, 100.0), &mut b));
        assert!(clearance(&m, &states, 1, (-6.0, 100.0, 6.0, 108.0), &mut b));
    }

    /// `spec-navigation.md` 7.2 test 15: unsticking.
    #[test]
    fn unsticking_matches_the_specification() {
        // (a) a vertical wall with normal (-1, 0).
        let bx = (91.0, 96.0, 103.0, 104.0);
        let t = separation((-1.0, 0.0), (100.0, 50.0), bx);
        assert!((t - 4.0).abs() < 1e-5, "{t}");
        // (b) a diagonal wall.
        let bx = (42.0, 48.0, 54.0, 56.0);
        let n = (-1.0 / 2f32.sqrt(), 1.0 / 2f32.sqrt());
        let t = separation(n, (0.0, 0.0), bx);
        assert!((t - (1.0 + 3.0 * 2f32.sqrt())).abs() < 1e-4, "{t}");
        // (d) the box on the wall's blocked side is not moved.
        let bx = (97.0, 96.0, 109.0, 104.0);
        assert_eq!(separation((-1.0, 0.0), (100.0, 50.0), bx), 0.0);
        // (e) the order-sensitive case: the deepest corner ends 0.302 px on the free side.
        let a = 5f32.to_radians();
        let n = (-a.cos(), a.sin());
        let bx = (91.0, 96.0, 103.0, 104.0);
        let t = separation(n, (100.0, 100.0), bx);
        assert!((t - 3.640).abs() < 0.01, "{t}");
    }

    /// `spec-navigation.md` 7.2 test 3: the corridor.
    #[test]
    fn corridor_sides_follow_the_specification() {
        let q = corridor_quad((100.0, 100.0), (200.0, 100.0), (6.0, 4.0));
        let ys: Vec<f32> = q.iter().map(|p| p.1).collect();
        assert!(ys.contains(&97.0) && ys.contains(&103.0));
        let blocks = |a: (f32, f32), b: (f32, f32)| {
            point_in_quad(a, &q)
                || point_in_quad(b, &q)
                || (0..4).any(|k| segments_cross(a, b, q[k], q[(k + 1) % 4]))
        };
        assert!(blocks((150.0, 96.0), (150.0, 104.0)));
        assert!(!blocks((150.0, 104.0), (150.0, 120.0)));
        assert!(blocks((150.0, 90.0), (150.0, 103.0)));
    }

    /// `spec-navigation.md` 7.2 test 6: the search on fixture F1 finds the upper path.
    #[test]
    fn search_on_fixture_f1_finds_the_upper_path() {
        let m = f1();
        let states = super::super::state::Availability::new(&m);
        let mut b = 1_000_000u64;
        let out = search_sector(
            &m,
            &states,
            SearchInput {
                layer: 0,
                sector: 0,
                start: (100.0, 100.0),
                goal: (300.0, 100.0),
                class: 0,
                unstuck: false,
            },
            &mut b,
        );
        assert_eq!(
            out,
            PathOutcome::Path(vec![
                (100.0, 100.0),
                (174.0, 76.0),
                (226.0, 76.0),
                (300.0, 100.0),
            ])
        );
    }

    /// `spec-navigation.md` 7.2 test 7: with the obstacle unavailable the path collapses.
    #[test]
    fn smoothing_collapses_a_straight_path() {
        let m = f1();
        let states = super::super::state::Availability::new(&m);
        let mut b = 1_000_000u64;
        let path = [
            (100.0, 100.0),
            (150.0, 100.0),
            (200.0, 100.0),
            (300.0, 100.0),
        ];
        // The obstacle is in the way here; with it removed the path collapses to the ends.
        let mut empty = crate::nav::fixtures::obstacle_course();
        empty.layers[0][0].obstacles.clear();
        empty.regions[0][0].truncate(1);
        empty.edges.clear();
        empty.records.clear();
        let m2 = NavMap::build(&empty).unwrap();
        let states2 = super::super::state::Availability::new(&m2);
        let out = smooth_path(
            &m2,
            &states2,
            SmoothInput {
                layer: 0,
                half: (6.0, 4.0),
            },
            &path,
            &mut b,
        );
        assert_eq!(out, vec![(100.0, 100.0), (300.0, 100.0)]);
        // With the obstacle the smoothing removes nothing.
        let kept = smooth_path(
            &m,
            &states,
            SmoothInput {
                layer: 0,
                half: (6.0, 4.0),
            },
            &[(100.0, 100.0), (174.0, 76.0), (226.0, 76.0), (300.0, 100.0)],
            &mut b,
        );
        assert_eq!(kept.len(), 4);
    }

    /// A map of plain sectors on one layer with the given doors, for the route fixtures.
    fn route_map(
        sectors: &[[(i32, i32); 4]],
        doors: &[(u8, (i32, i32), u32, (i32, i32), (i32, i32), u32)],
    ) -> NavMap {
        use super::super::map::{DoorSpec, FarmSpec, NavContent, SectorSpec};
        let c = NavContent {
            map_size: (1000, 600),
            layers: vec![
                sectors
                    .iter()
                    .map(|s| SectorSpec {
                        kind: 0,
                        outline: s.to_vec(),
                        ..SectorSpec::default()
                    })
                    .collect(),
            ],
            size_classes: vec![(6.0, 4.0)],
            regions: vec![vec![vec![Vec::new()]; sectors.len()]],
            farm: vec![FarmSpec {
                kind: 0,
                doors: doors
                    .iter()
                    .map(|&(t, a, sa, th, b, sb)| DoorSpec {
                        door_type: t,
                        open: true,
                        a,
                        sector_a: sa,
                        layer_a: 0,
                        threshold: th,
                        b,
                        sector_b: sb,
                        layer_b: 0,
                        ..DoorSpec::default()
                    })
                    .collect(),
            }],
            ..NavContent::default()
        };
        NavMap::build(&c).unwrap()
    }

    /// `spec-navigation.md` 7.2 test 8: the route across sectors, its link exclusion (NAV-123)
    /// and the closed door.
    #[test]
    fn route_across_sectors_follows_the_specification() {
        // Three sectors side by side; their polygon numbers are 0 (X), 1 (Y) and 2 (Z).
        let sectors = [
            [(0, 0), (210, 0), (210, 400), (0, 400)],
            [(210, 0), (410, 0), (410, 400), (210, 400)],
            [(410, 0), (600, 0), (600, 400), (410, 400)],
        ];
        let doors = [
            (0u8, (200, 100), 0u32, (210, 100), (220, 100), 1u32),
            (0, (400, 100), 1, (410, 100), (420, 100), 2),
            (0, (220, 300), 1, (210, 300), (200, 300), 0),
        ];
        let m = route_map(&sectors, &doors);
        assert_eq!(m.doors[0].cost, 70.0);
        let always = |_: u32, _: DoorSide| true;
        let mut b = 1_000_000u64;
        let r = route(&m, (100.0, 100.0), 0, 0, 2, (500.0, 100.0), &always, &mut b)
            .expect("a route exists");
        assert_eq!(
            r,
            vec![
                Crossing {
                    door: 0,
                    from: DoorSide::A
                },
                Crossing {
                    door: 1,
                    from: DoorSide::A
                }
            ]
        );
        // Variant: D3 recorded as (A = X at (200,300), B = Y at (220,300)) excludes the link
        // D1-D3 (both side-A sectors are X) and gives the same route.
        let doors2 = [
            doors[0],
            doors[1],
            (0, (200, 300), 0, (210, 300), (220, 300), 1),
        ];
        let m2 = route_map(&sectors, &doors2);
        let mut b = 1_000_000u64;
        let r2 = route(
            &m2,
            (100.0, 100.0),
            0,
            0,
            2,
            (500.0, 100.0),
            &always,
            &mut b,
        )
        .expect("a route exists");
        assert_eq!(r2, r);
        // With D2 shut there is no route.
        let shut = |d: u32, _: DoorSide| d != 1;
        let mut b = 1_000_000u64;
        assert!(route(&m, (100.0, 100.0), 0, 0, 2, (500.0, 100.0), &shut, &mut b).is_none());
    }

    /// `spec-navigation.md` 8.5 fixture (5): the rounding-sensitive door order. The exact score
    /// separates the two doors by 5 * 2^-19; the policy's two roundings make them tie, and the
    /// door queued last is examined first, so the route goes through D1 (the original documents
    /// D2).
    #[test]
    fn the_door_score_is_summed_cost_plus_h_then_plus_g() {
        let sectors = [
            [(0, 0), (40, 0), (40, 400), (0, 400)],
            [(40, 0), (200, 0), (200, 400), (40, 400)],
        ];
        // D2 is listed first, D1 second (NAV-121: the initial set is queued in door-list order).
        let doors = [
            (0u8, (30, 200), 0u32, (30, 165), (30, 130), 1u32),
            (0, (135, 100), 0, (95, 100), (55, 100), 1),
        ];
        let mut m = route_map(&sectors, &doors);
        // A search-kernel fixture: both costs are exactly 70.
        m.doors[0].cost = 70.0;
        m.doors[1].cost = 70.0;
        let t = (30.0 - 5.0 * f32::powi(2.0, -19), 100.0);
        let always = |_: u32, _: DoorSide| true;
        let mut b = 1_000_000u64;
        let r = route(&m, (30.0, 100.0), 0, 0, 1, t, &always, &mut b).expect("a route exists");
        assert_eq!(
            r,
            vec![Crossing {
                door: 1,
                from: DoorSide::A
            }]
        );
    }
}
