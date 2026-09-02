//! Navigation grid and A* pathfinding over the walkable geometry.
//!
//! The original engine has its own path graph (`docs/formats/rhp.md`, `STAT` rest, console EULER)
//! that is not decoded yet; until it is, paths are computed on a cell grid rasterised from the
//! boundary and obstacle polygons. Everything is integer and deterministic: ties in the open set are
//! broken by cell index.
//!
//! Every public function accepts any `i32` (cell or pixel) input without overflowing: cell
//! arithmetic widens to `i64`, the scan conversion to `i128`, so debug and release builds behave
//! identically on hostile input (ADR-0004). Coordinates far outside the grid are simply not walkable.
//!
//! Building a grid is bounded in time and memory as well as in arithmetic: the polygon count, the
//! cell count and the total scan-conversion work (edge tests, each polygon scanned over its own
//! rows only) are checked against [`MAX_POLYGONS`], [`MAX_CELLS`] and [`MAX_EDGE_TESTS`] before
//! anything is allocated, allocations are fallible, and [`NavGrid::try_build`] reports the
//! violation while [`NavGrid::build`] degrades to a grid with no walkable cell.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt;

use crate::geom::Geometry;

/// Grid cell size in map pixels.
pub const CELL: i32 = 8;
/// Upper bound on cells (a 32768x32768 map at 8 px would be 16M cells; keep it sane).
pub const MAX_CELLS: usize = 4 << 20;
/// Most polygons (boundary, areas and obstacles together) a grid is built from. Retail maps have
/// at most about a thousand.
pub const MAX_POLYGONS: usize = 1 << 16;
/// Most edge tests one build may perform: the sum over polygons of `rows the polygon spans x its
/// edges`. Retail maps need under 150,000; the budget is about 500 times that and bounds the
/// work a hostile snapshot can request to a fraction of a second.
pub const MAX_EDGE_TESTS: u64 = 1 << 26;

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

/// Why a grid could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavError {
    /// The map needs more cells than [`MAX_CELLS`].
    TooManyCells {
        /// Cells requested.
        cells: u64,
    },
    /// More polygons than [`MAX_POLYGONS`].
    TooManyPolygons {
        /// Polygons in the geometry.
        polygons: usize,
    },
    /// Scan conversion would need more than [`MAX_EDGE_TESTS`] edge tests.
    TooMuchWork {
        /// Edge tests needed.
        edge_tests: u64,
    },
    /// The allocator refused the grid.
    Allocation {
        /// Cells requested.
        cells: usize,
    },
}

impl fmt::Display for NavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCells { cells } => {
                write!(f, "navigation grid needs {cells} cells (limit {MAX_CELLS})")
            }
            Self::TooManyPolygons { polygons } => {
                write!(f, "geometry has {polygons} polygons (limit {MAX_POLYGONS})")
            }
            Self::TooMuchWork { edge_tests } => write!(
                f,
                "rasterising the geometry needs {edge_tests} edge tests (limit {MAX_EDGE_TESTS})"
            ),
            Self::Allocation { cells } => {
                write!(f, "cannot allocate a navigation grid of {cells} cells")
            }
        }
    }
}

impl std::error::Error for NavError {}

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
    /// Grid dimensions in cells for a map of `map_w x map_h` pixels (`u32 / 8` always fits `i32`).
    fn dimensions(map_w: u32, map_h: u32) -> (i32, i32) {
        (
            map_w.div_ceil(CELL as u32).max(1) as i32,
            map_h.div_ceil(CELL as u32).max(1) as i32,
        )
    }

    /// Check the cost of building a grid for `geometry` on a `map_w x map_h` map without building
    /// it: polygon count, cell count and the exact number of edge tests the scan conversion will
    /// perform (each polygon over the rows it spans). Returns that number. Total over any input.
    pub fn check_budget(geometry: &Geometry, map_w: u32, map_h: u32) -> Result<u64, NavError> {
        let (width, height) = Self::dimensions(map_w, map_h);
        let cells = u64::from(width as u32) * u64::from(height as u32);
        if cells > MAX_CELLS as u64 {
            return Err(NavError::TooManyCells { cells });
        }
        let polygons = usize::from(!geometry.boundary.is_empty())
            .saturating_add(geometry.areas.len())
            .saturating_add(geometry.obstacles.len());
        if polygons > MAX_POLYGONS {
            return Err(NavError::TooManyPolygons { polygons });
        }
        let mut edge_tests = 0u64;
        for (poly, _) in Self::polygons(geometry) {
            edge_tests = edge_tests.saturating_add(polygon_work(poly, height));
            if edge_tests > MAX_EDGE_TESTS {
                return Err(NavError::TooMuchWork { edge_tests });
            }
        }
        Ok(edge_tests)
    }

    /// Polygons in raster order with the value they paint: boundary and areas walkable, then
    /// obstacles blocked. Polygons with fewer than three vertices are skipped.
    fn polygons(geometry: &Geometry) -> impl Iterator<Item = (&[(i32, i32)], bool)> {
        std::iter::once(geometry.boundary.as_slice())
            .chain(geometry.areas.iter().map(Vec::as_slice))
            .map(|p| (p, true))
            .chain(geometry.obstacles.iter().map(|p| (p.as_slice(), false)))
            .filter(|(p, _)| p.len() >= 3)
    }

    /// Rasterise `geometry` for a map of `map_w x map_h` pixels. Cells whose centre is inside the
    /// boundary (if any) and outside every obstacle are walkable. Geometry over the budgets
    /// ([`NavGrid::check_budget`]) or a refused allocation is an error; nothing is allocated
    /// before the budgets pass.
    pub fn try_build(geometry: &Geometry, map_w: u32, map_h: u32) -> Result<Self, NavError> {
        let mut budget = Self::check_budget(geometry, map_w, map_h)?;
        let (width, height) = Self::dimensions(map_w, map_h);
        let n = width as usize * height as usize;
        let bounded = geometry.boundary.len() >= 3 || !geometry.areas.is_empty();
        let mut walkable = alloc_cells(n)?;
        walkable.resize(n, !bounded);
        for (poly, value) in Self::polygons(geometry) {
            fill_polygon(poly, width, height, &mut walkable, value, &mut budget)?;
        }
        // Keep paths one cell away from every edge so straight moves between cell centres never
        // cross a polygon boundary (erode the walkable set by one cell).
        let mut eroded = alloc_cells(n)?;
        eroded.extend((0..n).map(|i| {
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
        }));
        Ok(Self {
            width,
            height,
            walkable: eroded,
        })
    }

    /// [`NavGrid::try_build`] that never fails: when the geometry or the map is over budget the
    /// grid has the map's dimensions and no walkable cell (movement is refused rather than
    /// unbounded). Callers that can report an error should use `try_build`.
    #[must_use]
    pub fn build(geometry: &Geometry, map_w: u32, map_h: u32) -> Self {
        Self::try_build(geometry, map_w, map_h).unwrap_or_else(|_| {
            let (width, height) = Self::dimensions(map_w, map_h);
            Self {
                width,
                height,
                walkable: Vec::new(),
            }
        })
    }

    /// Cell containing a map pixel.
    #[must_use]
    pub fn cell_of(&self, x: i32, y: i32) -> (i32, i32) {
        (
            (x.div_euclid(CELL)).clamp(0, self.width - 1),
            (y.div_euclid(CELL)).clamp(0, self.height - 1),
        )
    }

    /// Centre of a cell in map pixels (saturating for cells beyond any real grid).
    #[must_use]
    pub fn centre(&self, (cx, cy): (i32, i32)) -> (i32, i32) {
        (
            cx.saturating_mul(CELL).saturating_add(CELL / 2),
            cy.saturating_mul(CELL).saturating_add(CELL / 2),
        )
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
                    let p = (c.0.saturating_add(dx), c.1.saturating_add(dy));
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
        // Octile distance; `to` may be any cell, so the differences are formed in `i64` and the
        // result saturates (a target that far away is unreachable anyway).
        let h = |c: (i32, i32)| -> u32 {
            let dx = (i64::from(c.0) - i64::from(to.0)).unsigned_abs();
            let dy = (i64::from(c.1) - i64::from(to.1)).unsigned_abs();
            let (lo, hi) = (dx.min(dy), dx.max(dy));
            u32::try_from(14 * lo + 10 * (hi - lo)).unwrap_or(u32::MAX)
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
                let ng = gc.saturating_add(cost);
                if ng < g[ni] {
                    g[ni] = ng;
                    parent[ni] = ci as u32;
                    open.push(Reverse((ng.saturating_add(h(nb)), ng, ni as u32)));
                }
            }
        }
        if nearest && best.1 != idx(from) {
            return Some(unwind(best.1, &parent));
        }
        None
    }

    /// Whether every cell on the segment between two cells is walkable (supercover line). The
    /// walk is done in `i64` so any pair of `i32` cells is accepted; the first cell outside the
    /// grid ends it with `false`.
    #[must_use]
    pub fn line_clear(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let walkable = |x: i64, y: i64| match (i32::try_from(x), i32::try_from(y)) {
            (Ok(x), Ok(y)) => self.walkable((x, y)),
            _ => false,
        };
        let (bx, by) = (i64::from(b.0), i64::from(b.1));
        let (mut x, mut y) = (i64::from(a.0), i64::from(a.1));
        let dx = (bx - x).abs();
        let dy = -(by - y).abs();
        let sx = if x < bx { 1 } else { -1 };
        let sy = if y < by { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if !walkable(x, y) {
                return false;
            }
            if (x, y) == (bx, by) {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                // Moving horizontally: also require the vertical neighbour to avoid corner clipping.
                if e2 <= dx && !walkable(x + sx, y) {
                    return false;
                }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if e2 >= dy && !walkable(x, y + sy) {
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

/// An empty cell buffer with room for `n` cells, or [`NavError::Allocation`].
fn alloc_cells(n: usize) -> Result<Vec<bool>, NavError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| NavError::Allocation { cells: n })?;
    Ok(v)
}

/// Rows of a `height`-row grid whose cell centres (`cy * CELL + CELL / 2`) lie in the polygon's
/// vertical extent `[min_y, max_y)`, the only rows an edge of the polygon can cross. `None` when
/// the polygon misses the grid entirely. Any `i32` polygon is accepted (`i64` arithmetic).
fn row_range(poly: &[(i32, i32)], height: i32) -> Option<(i32, i32)> {
    let min_y = i64::from(poly.iter().map(|p| p.1).min()?);
    let max_y = i64::from(poly.iter().map(|p| p.1).max()?);
    let half = i64::from(CELL / 2);
    let cell = i64::from(CELL);
    // Smallest cy with cy * CELL + half >= min_y, largest with cy * CELL + half < max_y.
    let lo = (min_y - half + cell - 1).div_euclid(cell).max(0);
    let hi = (max_y - half - 1)
        .div_euclid(cell)
        .min(i64::from(height) - 1);
    (lo <= hi).then_some((lo as i32, hi as i32))
}

/// Edge tests scanning one polygon costs: rows it spans times its edges.
fn polygon_work(poly: &[(i32, i32)], height: i32) -> u64 {
    match row_range(poly, height) {
        Some((lo, hi)) => (u64::from((hi - lo) as u32) + 1) * poly.len() as u64,
        None => 0,
    }
}

/// Scanline fill of a polygon at cell resolution (cell centres), even-odd rule, over the rows the
/// polygon spans only. Intersections are computed in `i128` (a product of two `i32` differences
/// times 256 needs 73 bits), so the rasteriser is total over `i32` polygons whether or not they
/// passed `Geometry::check_bounds`. Every edge test is charged to `budget`; running out is an
/// error (the caller's [`NavGrid::check_budget`] makes that unreachable for accepted input).
fn fill_polygon(
    poly: &[(i32, i32)],
    width: i32,
    height: i32,
    cells: &mut [bool],
    value: bool,
    budget: &mut u64,
) -> Result<(), NavError> {
    let n = poly.len();
    let sub = 256i128;
    let cell = i128::from(CELL);
    let Some((lo, hi)) = row_range(poly, height) else {
        return Ok(());
    };
    let mut xs: Vec<i128> = Vec::new();
    for cy in lo..=hi {
        let y = i128::from(cy) * cell + cell / 2;
        xs.clear();
        let needed = n as u64;
        if *budget < needed {
            return Err(NavError::TooMuchWork {
                edge_tests: MAX_EDGE_TESTS.saturating_add(needed - *budget),
            });
        }
        *budget -= needed;
        for i in 0..n {
            let (x1, y1) = (i128::from(poly[i].0), i128::from(poly[i].1));
            let (x2, y2) = (
                i128::from(poly[(i + 1) % n].0),
                i128::from(poly[(i + 1) % n].1),
            );
            if (y1 > y) != (y2 > y) {
                // Intersection x in 1/256 units to keep integer math exact enough.
                let x = x1 * sub + (y - y1) * (x2 - x1) * sub / (y2 - y1);
                xs.push(x);
            }
        }
        xs.sort_unstable();
        for pair in xs.chunks_exact(2) {
            let (xa, xb) = (pair[0], pair[1]);
            // Cells whose centre lies within [xa, xb], clamped to the row.
            let c0 = ((xa - sub * cell / 2) / (sub * cell) + 1).clamp(0, i128::from(width));
            let c1 = ((xb - sub * cell / 2) / (sub * cell)).clamp(-1, i128::from(width) - 1);
            for cx in c0..=c1 {
                cells[(cy as usize) * (width as usize) + cx as usize] = value;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> NavGrid {
        let g = Geometry {
            boundary: vec![(0, 0), (200, 0), (200, 200), (0, 200)],
            obstacles: vec![vec![(90, 0), (110, 0), (110, 150), (90, 150)]],
            areas: Vec::new(),
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

    #[test]
    fn scans_only_the_rows_a_polygon_spans() {
        // Rows: centres at 4, 12, 20, ...; a polygon over y in [10, 30) touches rows 1..=3.
        let poly = [(0, 10), (50, 10), (50, 30), (0, 30)];
        assert_eq!(row_range(&poly, 100), Some((1, 3)));
        assert_eq!(polygon_work(&poly, 100), 3 * 4);
        // Entirely above or below the grid: no rows, no work.
        assert_eq!(row_range(&[(0, -100), (5, -100), (5, -50)], 100), None);
        assert_eq!(row_range(&[(0, 900), (5, 900), (5, 950)], 100), None);
        // Extreme polygons still get a clamped row range.
        assert_eq!(
            row_range(&[(i32::MIN, i32::MIN), (i32::MAX, i32::MAX), (0, 0)], 25),
            Some((0, 24))
        );
        // The estimate is what the build charges: an exact budget succeeds, one short fails.
        let g = Geometry {
            boundary: vec![(0, 0), (200, 0), (200, 200), (0, 200)],
            obstacles: vec![poly.to_vec()],
            areas: Vec::new(),
        };
        let cost = NavGrid::check_budget(&g, 200, 200).unwrap();
        assert_eq!(cost, 25 * 4 + 3 * 4);
        let mut cells = vec![true; 25 * 25];
        let mut exact = 3 * 4;
        fill_polygon(&poly, 25, 25, &mut cells, false, &mut exact).unwrap();
        assert_eq!(exact, 0);
        let mut short = 3 * 4 - 1;
        assert!(matches!(
            fill_polygon(&poly, 25, 25, &mut cells, false, &mut short),
            Err(NavError::TooMuchWork { .. })
        ));
    }

    #[test]
    fn budgets_bound_polygons_cells_and_work() {
        let square = |y: i32| vec![(0, y), (100, y), (100, y + 100), (0, y + 100)];
        // Polygon count.
        let many = Geometry {
            boundary: Vec::new(),
            obstacles: (0..=MAX_POLYGONS).map(|_| square(0)).collect(),
            areas: Vec::new(),
        };
        assert!(matches!(
            NavGrid::check_budget(&many, 8, 8),
            Err(NavError::TooManyPolygons { .. })
        ));
        assert!(matches!(
            NavGrid::try_build(&many, 8, 8),
            Err(NavError::TooManyPolygons { .. })
        ));
        // Work: a few polygons with very many edges over the whole height of a large map.
        let rows = 4096u32;
        let big: Vec<(i32, i32)> = (0..20_000)
            .map(|i| (i % 7, (i * 13) % (rows as i32 * CELL)))
            .collect();
        let heavy = Geometry {
            boundary: big.clone(),
            obstacles: vec![big.clone(), big],
            areas: Vec::new(),
        };
        let err = NavGrid::check_budget(&heavy, 8, rows * CELL as u32).unwrap_err();
        assert!(matches!(err, NavError::TooMuchWork { .. }), "{err}");
        assert!(NavGrid::try_build(&heavy, 8, rows * CELL as u32).is_err());
        // The infallible constructor degrades to a grid with no walkable cell.
        let degraded = NavGrid::build(&heavy, 8, rows * CELL as u32);
        assert_eq!((degraded.width, degraded.height), (1, rows as i32));
        assert!(!degraded.walkable((0, 0)));
        assert!(degraded.find_path((0, 0), (0, 1)).is_none());
        // Cells.
        assert!(matches!(
            NavGrid::check_budget(&Geometry::default(), u32::MAX, u32::MAX),
            Err(NavError::TooManyCells { .. })
        ));
        // Retail-like input is far inside every budget.
        let retail = Geometry {
            boundary: (0..3000)
                .map(|i| (i, if i % 2 == 0 { 0 } else { 3520 }))
                .collect(),
            obstacles: (0..1000).map(|i| square(i * 3)).collect(),
            areas: Vec::new(),
        };
        let cost = NavGrid::check_budget(&retail, 2304, 3520).unwrap();
        assert!(cost < MAX_EDGE_TESTS / 16, "{cost}");
        assert!(NavGrid::try_build(&retail, 2304, 3520).is_ok());
    }

    #[test]
    fn extreme_inputs_never_overflow() {
        let ext = [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX];
        // Hostile polygons through the scan conversion (bounds are enforced one level up, in the
        // world; the rasteriser itself must still be total).
        let g = Geometry {
            boundary: vec![
                (i32::MIN, i32::MIN),
                (i32::MAX, i32::MIN),
                (i32::MAX, i32::MAX),
                (i32::MIN, i32::MAX),
            ],
            obstacles: vec![vec![(i32::MAX, i32::MIN), (i32::MIN, i32::MAX), (7, 7)]],
            areas: Vec::new(),
        };
        let hostile = NavGrid::try_build(&g, 200, 200).unwrap();
        assert_eq!((hostile.width, hostile.height), (25, 25));
        // Oversized maps degrade to a grid without walkable cells instead of allocating.
        let huge = NavGrid::build(&Geometry::default(), u32::MAX, u32::MAX);
        assert!(!huge.walkable((0, 0)));
        let grid = grid();
        for &x in &ext {
            for &y in &ext {
                let c = (x, y);
                assert!(!grid.walkable(c) || (x < 25 && y < 25));
                let _ = grid.cell_of(x, y);
                let _ = grid.centre(c);
                let _ = grid.nearest_walkable(c, 3);
                assert!(!grid.line_clear(c, (2, 2)) || c == (2, 2));
                assert!(!grid.line_clear((2, 2), c) || c == (2, 2));
                assert!(grid.find_path((2, 2), c).is_none() || c == (2, 2));
                // Nearest-cell search towards an absurd target still terminates.
                let _ = grid.find_path_near((2, 2), c);
                let _ = grid.smooth((2, 2), &[c, (3, 3)]);
            }
        }
        assert!(grid.line_clear((2, 2), (2, 2)));
    }
}
