//! Navigation grid and A* pathfinding over the walkable geometry.
//!
//! The original engine has its own path graph (`docs/formats/rhp.md`, `STAT` rest, console EULER)
//! that is not decoded yet; until it is, paths are computed on a cell grid rasterised from the
//! boundary and obstacle polygons. Everything is integer and deterministic: ties in the open set are
//! broken by cell index.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::geom::Geometry;

/// Grid cell size in map pixels.
pub const CELL: i32 = 8;
/// Upper bound on cells (a 32768x32768 map at 8 px would be 16M cells; keep it sane).
pub const MAX_CELLS: usize = 4 << 20;

/// Neighbour offsets and step costs (10 straight, 14 diagonal).
const DIRS: [(i32, i32, u32); 8] = [
    (1, 0, 10),
    (-1, 0, 10),
    (0, 1, 10),
    (0, -1, 10),
    (1, 1, 14),
    (-1, 1, 14),
    (1, -1, 14),
    (-1, -1, 14),
];

/// Sort key for nearest-cell searches: squared distance, then y, then x.
type RankedCell = (i64, i32, i32);

/// Rasterised walkability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavGrid {
    /// Cells per row.
    pub width: i32,
    /// Rows.
    pub height: i32,
    walkable: Vec<bool>,
}

impl NavGrid {
    /// Rasterise `geometry` for a map of `map_w x map_h` pixels. Cells whose centre is inside the
    /// boundary (if any) and outside every obstacle are walkable.
    #[must_use]
    pub fn build(geometry: &Geometry, map_w: u32, map_h: u32) -> Self {
        let width = ((map_w as i32 + CELL - 1) / CELL).max(1);
        let height = ((map_h as i32 + CELL - 1) / CELL).max(1);
        let n = (width as usize) * (height as usize);
        let mut walkable = vec![geometry.boundary.len() < 3; n.min(MAX_CELLS)];
        if n > MAX_CELLS {
            return Self {
                width,
                height,
                walkable: vec![false; 1],
            };
        }
        if geometry.boundary.len() >= 3 {
            fill_polygon(&geometry.boundary, width, height, &mut walkable, true);
        }
        for o in &geometry.obstacles {
            if o.len() >= 3 {
                fill_polygon(o, width, height, &mut walkable, false);
            }
        }
        // Keep paths one cell away from every edge so straight moves between cell centres never
        // cross a polygon boundary (erode the walkable set by one cell).
        let eroded: Vec<bool> = (0..walkable.len())
            .map(|i| {
                let (cx, cy) = ((i as i32) % width, (i as i32) / width);
                (-1..=1).all(|dy| {
                    (-1..=1).all(|dx| {
                        let (x, y) = (cx + dx, cy + dy);
                        x >= 0
                            && y >= 0
                            && x < width
                            && y < height
                            && walkable[(y * width + x) as usize]
                    })
                })
            })
            .collect();
        Self {
            width,
            height,
            walkable: eroded,
        }
    }

    /// Cell containing a map pixel.
    #[must_use]
    pub fn cell_of(&self, x: i32, y: i32) -> (i32, i32) {
        (
            (x.div_euclid(CELL)).clamp(0, self.width - 1),
            (y.div_euclid(CELL)).clamp(0, self.height - 1),
        )
    }

    /// Centre of a cell in map pixels.
    #[must_use]
    pub fn centre(&self, (cx, cy): (i32, i32)) -> (i32, i32) {
        (cx * CELL + CELL / 2, cy * CELL + CELL / 2)
    }

    /// Whether a cell is walkable.
    #[must_use]
    pub fn walkable(&self, (cx, cy): (i32, i32)) -> bool {
        if cx < 0 || cy < 0 || cx >= self.width || cy >= self.height {
            return false;
        }
        self.walkable
            .get((cy * self.width + cx) as usize)
            .copied()
            .unwrap_or(false)
    }

    /// Nearest walkable cell to `c` within `radius` cells (spiral search, deterministic).
    #[must_use]
    pub fn nearest_walkable(&self, c: (i32, i32), radius: i32) -> Option<(i32, i32)> {
        if self.walkable(c) {
            return Some(c);
        }
        for r in 1..=radius {
            let mut best: Option<(RankedCell, (i32, i32))> = None;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let p = (c.0 + dx, c.1 + dy);
                    if self.walkable(p) {
                        let d = (
                            i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy),
                            p.1,
                            p.0,
                        );
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, p));
                        }
                    }
                }
            }
            if let Some((_, p)) = best {
                return Some(p);
            }
        }
        None
    }

    /// A* from cell `from` to cell `to` (8-connected, no corner cutting). Returns the cell path
    /// including `to` and excluding `from`, or `None` when unreachable.
    #[must_use]
    pub fn find_path(&self, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        if !self.walkable(to) {
            return None;
        }
        self.search(from, to, false)
    }

    /// A* towards `to`; when `to` is unreachable (or not walkable) the path ends at the reachable
    /// cell closest to it (what an order on water or behind a wall does in the original).
    /// Returns `None` only when `from` is not walkable or no move is possible at all.
    #[must_use]
    pub fn find_path_near(&self, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        self.search(from, to, true)
    }

    fn search(&self, from: (i32, i32), to: (i32, i32), nearest: bool) -> Option<Vec<(i32, i32)>> {
        if !self.walkable(from) {
            return None;
        }
        if from == to {
            return Some(Vec::new());
        }
        let idx = |(x, y): (i32, i32)| (y * self.width + x) as usize;
        let n = (self.width * self.height) as usize;
        let mut g = vec![u32::MAX; n];
        let mut parent = vec![u32::MAX; n];
        let mut closed = vec![false; n];
        let h = |c: (i32, i32)| -> u32 {
            let dx = (c.0 - to.0).unsigned_abs();
            let dy = (c.1 - to.1).unsigned_abs();
            let (lo, hi) = (dx.min(dy), dx.max(dy));
            14 * lo + 10 * (hi - lo)
        };
        let mut open: BinaryHeap<Reverse<(u32, u32, u32)>> = BinaryHeap::new();
        g[idx(from)] = 0;
        open.push(Reverse((h(from), 0, idx(from) as u32)));
        let mut best = (h(from), idx(from));
        let unwind = |end: usize, parent: &[u32]| {
            let mut path = Vec::new();
            let mut cur = end;
            while cur != idx(from) {
                path.push(((cur as i32) % self.width, (cur as i32) / self.width));
                cur = parent[cur] as usize;
            }
            path.reverse();
            path
        };
        while let Some(Reverse((_, gc, ci))) = open.pop() {
            let ci = ci as usize;
            if closed[ci] || gc != g[ci] {
                continue;
            }
            closed[ci] = true;
            let c = ((ci as i32) % self.width, (ci as i32) / self.width);
            if c == to {
                return Some(unwind(ci, &parent));
            }
            let hc = h(c);
            if hc < best.0 {
                best = (hc, ci);
            }
            for &(dx, dy, cost) in &DIRS {
                let nb = (c.0 + dx, c.1 + dy);
                if !self.walkable(nb) {
                    continue;
                }
                // No diagonal squeezing between two blocked orthogonal neighbours.
                if dx != 0
                    && dy != 0
                    && (!self.walkable((c.0 + dx, c.1)) || !self.walkable((c.0, c.1 + dy)))
                {
                    continue;
                }
                let ni = idx(nb);
                let ng = gc + cost;
                if ng < g[ni] {
                    g[ni] = ng;
                    parent[ni] = ci as u32;
                    open.push(Reverse((ng + h(nb), ng, ni as u32)));
                }
            }
        }
        if nearest && best.1 != idx(from) {
            return Some(unwind(best.1, &parent));
        }
        None
    }

    /// Whether every cell on the segment between two cells is walkable (supercover line).
    #[must_use]
    pub fn line_clear(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let (mut x, mut y) = a;
        let dx = (b.0 - a.0).abs();
        let dy = -(b.1 - a.1).abs();
        let sx = if a.0 < b.0 { 1 } else { -1 };
        let sy = if a.1 < b.1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if !self.walkable((x, y)) {
                return false;
            }
            if (x, y) == b {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                // Moving horizontally: also require the vertical neighbour to avoid corner clipping.
                if e2 <= dx && !self.walkable((x + sx, y)) {
                    return false;
                }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if e2 >= dy && !self.walkable((x, y + sy)) {
                    return false;
                }
                err += dx;
                y += sy;
            }
        }
    }

    /// Drop intermediate cells that can be skipped with a clear line (greedy string pulling).
    #[must_use]
    pub fn smooth(&self, from: (i32, i32), path: &[(i32, i32)]) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        let mut anchor = from;
        let mut i = 0;
        while i < path.len() {
            let mut j = path.len() - 1;
            while j > i && !self.line_clear(anchor, path[j]) {
                j -= 1;
            }
            out.push(path[j]);
            anchor = path[j];
            i = j + 1;
        }
        out
    }
}

/// Scanline fill of a polygon at cell resolution (cell centres), even-odd rule.
fn fill_polygon(poly: &[(i32, i32)], width: i32, height: i32, cells: &mut [bool], value: bool) {
    let n = poly.len();
    for cy in 0..height {
        let y = i64::from(cy * CELL + CELL / 2);
        let mut xs: Vec<i64> = Vec::new();
        for i in 0..n {
            let (x1, y1) = (i64::from(poly[i].0), i64::from(poly[i].1));
            let (x2, y2) = (
                i64::from(poly[(i + 1) % n].0),
                i64::from(poly[(i + 1) % n].1),
            );
            if (y1 > y) != (y2 > y) {
                // Intersection x in 1/256 units to keep integer math exact enough.
                let x = x1 * 256 + (y - y1) * (x2 - x1) * 256 / (y2 - y1);
                xs.push(x);
            }
        }
        xs.sort_unstable();
        for pair in xs.chunks_exact(2) {
            let (xa, xb) = (pair[0], pair[1]);
            // Cells whose centre lies within [xa, xb].
            let c0 = ((xa - 256 * (CELL as i64) / 2) / (256 * CELL as i64) + 1).max(0);
            let c1 =
                ((xb - 256 * (CELL as i64) / 2) / (256 * CELL as i64)).min(i64::from(width) - 1);
            for cx in c0..=c1 {
                cells[(cy as usize) * (width as usize) + cx as usize] = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> NavGrid {
        let g = Geometry {
            boundary: vec![(0, 0), (200, 0), (200, 200), (0, 200)],
            obstacles: vec![vec![(90, 0), (110, 0), (110, 150), (90, 150)]],
        };
        NavGrid::build(&g, 200, 200)
    }

    #[test]
    fn rasterises_boundary_and_obstacle() {
        let g = grid();
        assert_eq!((g.width, g.height), (25, 25));
        assert!(g.walkable(g.cell_of(20, 20)));
        assert!(!g.walkable(g.cell_of(100, 50)));
        assert!(g.walkable(g.cell_of(100, 180)));
        assert!(!g.walkable((30, 0)));
        assert!(
            !g.walkable((0, 5)),
            "cells touching the boundary are eroded"
        );
    }

    #[test]
    fn finds_a_path_around_the_wall() {
        let g = grid();
        let from = g.cell_of(20, 20);
        let to = g.cell_of(180, 20);
        let path = g.find_path(from, to).expect("reachable");
        assert!(!path.is_empty());
        assert_eq!(*path.last().unwrap(), to);
        assert!(path.iter().all(|&c| g.walkable(c)));
        assert!(
            path.iter().any(|&(_, cy)| cy >= 19),
            "must go below the wall"
        );
        let smooth = g.smooth(from, &path);
        assert!(smooth.len() <= path.len());
        assert_eq!(*smooth.last().unwrap(), to);
        assert!(g.find_path(from, g.cell_of(100, 50)).is_none());
        let near = g
            .find_path_near(from, g.cell_of(100, 50))
            .expect("walks as close as it can");
        assert!(!near.is_empty());
        let last = *near.last().unwrap();
        assert!(!g.walkable(g.cell_of(100, 50)) && g.walkable(last));
        assert_eq!(g.nearest_walkable(g.cell_of(100, 50), 4), Some((9, 6)));
    }

    #[test]
    fn deterministic_and_bounded() {
        let g = grid();
        let a = g.find_path((2, 2), (22, 2));
        let b = g.find_path((2, 2), (22, 2));
        assert_eq!(a, b);
        assert_eq!(g.find_path((2, 2), (2, 2)), Some(Vec::new()));
        assert!(g.find_path((-1, 0), (2, 2)).is_none());
    }
}
